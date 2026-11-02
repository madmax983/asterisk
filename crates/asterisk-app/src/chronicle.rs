//! The clock: one durable Harvest workflow per family, on the embedded
//! `SQLite` backend.
//!
//! ```text
//! loop {
//!     race: a `cmd` signal  vs  one century's durable timer
//!     signal → that command;  timer → a scheduled Advance
//!     activity apply_command(slug, next, cmd) → Outcome
//!     next = outcome.next;  Complete → the workflow ends
//!     every 200 turns → continue_as_new(next)
//! }
//! ```
//!
//! Everything that changes a world goes through here, so Harvest is the
//! single writer that orders time and hands. The activity runs the command
//! through [`Atlas::apply_at`], which logs it in the command log keyed by
//! `(family, seq)`; a retried activity finds its slot taken and runs
//! nothing. If the process dies between a century and its record, Harvest
//! re-runs the activity on reopen and the log turns it into a no-op.
//!
//! The timer is durable, but catch-up is not the point: a server that sleeps
//! for a night wakes to the next century, not to a hundred of them.

use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use asterisk::world::Command;
use autumn_harvest::prelude::*;
use autumn_harvest_sqlite::SqliteRuntime;
use autumn_web::AppState;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::atlas::{Atlas, Outcome};

/// The workflow's registered name.
pub const WORKFLOW: &str = "chronicle";
/// The signal that carries a command.
pub const CMD: &str = "cmd";
/// Turns before the workflow hands over to a fresh run.
pub const TURNS_PER_RUN: u32 = 200;
/// How often the driver wakes when nothing else does.
pub const DRIVE_EVERY: Duration = Duration::from_millis(500);

/// What the activity needs: the gallery, and somewhere to publish.
#[derive(Clone)]
struct Deps {
    atlas: Arc<Atlas>,
    state: Option<AppState>,
}

/// Activities are free functions the runtime calls by name, so what they
/// need is installed here when a driver starts. Replaceable, so a test can
/// stop one driver and open another on the same file, as a restart would.
static DEPS: RwLock<Option<Deps>> = RwLock::new(None);

fn deps() -> Result<Deps, String> {
    DEPS.read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .ok_or_else(|| "the chronicle driver is not installed".to_owned())
}

/// The workflow's input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleInput {
    /// Which family.
    pub slug: String,
    /// Seconds per century.
    pub century_secs: u64,
    /// The log position the next command takes.
    pub next: u64,
}

/// The activity's input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// Which family.
    pub slug: String,
    /// The position offered.
    pub seq: u64,
    /// The command.
    pub cmd: Command,
    /// Did the clock send it (rather than a person)?
    pub scheduled: bool,
}

/// The workflow id for a family.
pub fn workflow_id(slug: &str) -> String {
    format!("family:{slug}")
}

/// A family's chronicle.
#[workflow]
pub async fn chronicle(ctx: &WorkflowContext, input: ChronicleInput) -> Result<String, String> {
    let mut next = input.next;
    for _ in 0..TURNS_PER_RUN {
        let signal = ctx
            .wait_for_signal_timeout(CMD, Duration::from_secs(input.century_secs.max(1)))
            .await
            .map_err(|e| e.to_string())?;
        let (cmd, scheduled) = match signal {
            Some(v) => match serde_json::from_value::<Command>(v) {
                Ok(c) => (c, false),
                Err(_) => continue,
            },
            None => (Command::Advance, true),
        };
        let turn = Turn {
            slug: input.slug.clone(),
            seq: next,
            cmd,
            scheduled,
        };
        let out: Outcome = ctx
            .execute_activity(&apply_command_info(), turn)
            .await
            .map_err(|e| e.to_string())?;
        next = match out {
            Outcome::Applied { next }
            | Outcome::Duplicate { next }
            | Outcome::Skipped { next, .. } => next,
            Outcome::Complete => return Ok("complete".into()),
        };
    }
    let more = ChronicleInput { next, ..input };
    ctx.continue_as_new(serde_json::to_value(&more).map_err(|e| e.to_string())?)
        .await
        .map_err(|e| e.to_string())?;
    Ok("continued".into())
}

/// Run one command against the gallery and publish what happened.
#[allow(clippy::unused_async)] // `#[activity]` takes an async fn; this one has nothing to await
#[activity]
pub async fn apply_command(_ctx: &ActivityContext, turn: Turn) -> Result<Outcome, String> {
    let d = deps()?;
    let (out, b) = d
        .atlas
        .apply_at(&turn.slug, turn.seq, &turn.cmd, turn.scheduled)
        .map_err(|e| e.to_string())?;
    if let (Some(b), Some(state)) = (b, &d.state) {
        crate::publish(state, &b);
    }
    Ok(out)
}

/// Something the driver could not do.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// The task is gone.
    #[error("the chronicle driver is not running")]
    Gone,
    /// The runtime refused.
    #[error("{0}")]
    Runtime(String),
}

enum Msg {
    Start {
        input: ChronicleInput,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Send {
        slug: String,
        cmd: Command,
        key: String,
        reply: oneshot::Sender<Result<bool, String>>,
    },
}

/// A handle on the task that owns the Harvest runtime.
#[derive(Debug, Clone)]
pub struct Driver {
    tx: mpsc::Sender<Msg>,
}

fn registered(mut rt: SqliteRuntime) -> SqliteRuntime {
    rt.register_workflow(&chronicle_info());
    rt.register_activity(&apply_command_info());
    rt
}

impl Driver {
    /// Open (or create) the Harvest file, install the activity's
    /// dependencies, and start the task that owns the runtime. The task ends
    /// when the last handle is dropped.
    pub fn spawn(
        path: Option<&Path>,
        atlas: Arc<Atlas>,
        state: Option<AppState>,
    ) -> Result<Self, DriverError> {
        *DEPS.write().unwrap_or_else(PoisonError::into_inner) = Some(Deps { atlas, state });
        let rt = path
            .map_or_else(SqliteRuntime::open_in_memory, SqliteRuntime::open)
            .map_err(|e| DriverError::Runtime(e.to_string()))?;
        let mut rt = registered(rt);
        let (tx, mut rx) = mpsc::channel::<Msg>(256);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(DRIVE_EVERY);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    msg = rx.recv() => match msg {
                        Some(m) => handle(&mut rt, m),
                        None => break,
                    },
                    _ = tick.tick() => {}
                }
                if let Err(e) = rt.run_until_idle().await {
                    tracing::warn!(error = %e, "chronicle driver pass failed");
                }
            }
        });
        Ok(Self { tx })
    }

    /// Start a family's chronicle — or attach to the one already running
    /// after a restart (same workflow id).
    pub async fn start(&self, input: ChronicleInput) -> Result<(), DriverError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Msg::Start { input, reply })
            .await
            .map_err(|_| DriverError::Gone)?;
        rx.await
            .map_err(|_| DriverError::Gone)?
            .map_err(DriverError::Runtime)
    }

    /// Hand a command to a family's chronicle. `key` deduplicates a
    /// resubmitted form.
    pub async fn send(&self, slug: &str, cmd: Command, key: String) -> Result<bool, DriverError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Msg::Send {
                slug: slug.to_owned(),
                cmd,
                key,
                reply,
            })
            .await
            .map_err(|_| DriverError::Gone)?;
        rx.await
            .map_err(|_| DriverError::Gone)?
            .map_err(DriverError::Runtime)
    }
}

fn handle(rt: &mut SqliteRuntime, msg: Msg) {
    match msg {
        Msg::Start { input, reply } => {
            let id = workflow_id(&input.slug);
            let r = serde_json::to_value(&input)
                .map_err(|e| e.to_string())
                .and_then(|v| {
                    rt.start_workflow_with_id(WORKFLOW, &id, v)
                        .map_err(|e| e.to_string())
                })
                .map(|_| ());
            let _ = reply.send(r);
        }
        Msg::Send {
            slug,
            cmd,
            key,
            reply,
        } => {
            let r = serde_json::to_value(&cmd)
                .map_err(|e| e.to_string())
                .and_then(|v| {
                    rt.send_signal_by_workflow_id(WORKFLOW, &workflow_id(&slug), CMD, v, Some(&key))
                        .map_err(|e| e.to_string())
                })
                .map(|d| d.delivered);
            let _ = reply.send(r);
        }
    }
}

impl std::fmt::Debug for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start { input, .. } => write!(f, "Start({})", input.slug),
            Self::Send { slug, .. } => write!(f, "Send({slug})"),
        }
    }
}
