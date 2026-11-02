//! # asterisk-app
//!
//! The app around the `asterisk` engine: a gallery of language families,
//! each rebuilt at boot by replaying its command log, each kept moving by a
//! durable Harvest workflow, live over SSE.
//!
//!   Engine       -> `asterisk`: phonology, the sound-change language, the
//!                   tree, the command log's replay, the export.
//!   Log          -> [`store::Store`]: families and their commands in `SQLite`
//!                   (`asterisk-data/log.db`). The only durable state.
//!   Clock        -> [`chronicle`]: one `chronicle` workflow per family on
//!                   autumn-harvest-sqlite (`asterisk-data/harvest.db`). Every
//!                   century and every typed rule is a turn of it.
//!   Live events  -> one topic, `family`; browsers refetch fragments.
//!   Interface    -> Maud pages, a server-rendered SVG tree, one script.
//!
//! See docs/adr/0002-durable-replay-on-harvest.md.

pub mod atlas;
pub mod chronicle;
pub mod routes;
pub mod store;
pub mod view;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use asterisk::world::Command;
use autumn_web::AppState;

use crate::atlas::{Atlas, AtlasError, Broadcast, NewFamily, TOPIC};
use crate::chronicle::{ChronicleInput, Driver};
use crate::store::Store;

/// The first family, planted when the gallery is empty.
pub const FIRST_SEED: &str = "asterisk";
/// Default seconds per century.
pub const CENTURY_SECS: u64 = 6;
/// Default span, in centuries.
pub const SPAN: u32 = 300;

/// Publish one event. A publish with nobody listening is not an error worth
/// reporting.
pub fn publish(state: &AppState, b: &Broadcast) {
    match serde_json::to_string(b) {
        Ok(json) => {
            if let Err(e) = state.channels().publish(TOPIC, json) {
                tracing::debug!(error = %e, "family event not delivered");
            }
        }
        Err(e) => tracing::warn!(error = %e, "family event did not serialise"),
    }
}

/// Unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The first family's request.
pub fn first_family(century_secs: u64) -> NewFamily {
    NewFamily {
        title: "The Asterisk family".into(),
        seed: FIRST_SEED.into(),
        vocab: None,
        span: Some(SPAN),
        century_secs,
    }
}

/// Install a gallery without a clock: commands run as they arrive. For
/// tests, and for `ASTERISK_CLOCK=off`.
pub fn install_without_clock(state: &AppState, atlas: Atlas) {
    state.insert_extension(atlas);
}

/// The installed gallery.
pub fn atlas(state: &AppState) -> Option<Arc<Atlas>> {
    state.extension::<Atlas>()
}

/// The installed clock, if there is one.
pub fn driver(state: &AppState) -> Option<Arc<Driver>> {
    state.extension::<Driver>()
}

static KEYS: AtomicU64 = AtomicU64::new(0);

/// Hand a command to a family: through its chronicle if the clock runs,
/// otherwise at once.
pub async fn submit(state: &AppState, slug: &str, cmd: Command) -> Result<(), AtlasError> {
    let atlas = atlas(state).ok_or_else(|| AtlasError::NoFamily(slug.to_owned()))?;
    if let Some(d) = driver(state) {
        let key = format!(
            "{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
            KEYS.fetch_add(1, Ordering::Relaxed)
        );
        d.send(slug, cmd, key).await?;
        return Ok(());
    }
    let (_, b) = atlas.apply_now(slug, &cmd)?;
    if let Some(b) = b {
        publish(state, &b);
    }
    Ok(())
}

/// Start a family's chronicle, if there is a clock and it is not complete.
pub async fn start_chronicle(state: &AppState, slug: &str) {
    let (Some(d), Some(a)) = (driver(state), atlas(state)) else {
        return;
    };
    let Ok(f) = a.get(slug) else { return };
    let complete = f.world().complete();
    if complete {
        return;
    }
    let input = {
        ChronicleInput {
            slug: slug.to_owned(),
            century_secs: f.row.century_secs,
            next: f.applied(),
        }
    };
    if let Err(e) = d.start(input).await {
        tracing::warn!(error = %e, slug, "a chronicle did not start");
    }
}

fn data_dir() -> PathBuf {
    std::env::var("ASTERISK_DATA").map_or_else(|_| PathBuf::from("asterisk-data"), PathBuf::from)
}

fn unavailable(e: &dyn std::fmt::Display) -> autumn_web::error::AutumnError {
    autumn_web::error::AutumnError::service_unavailable_msg(e.to_string())
}

/// Boot: open the log, rebuild every family by replay, plant the first
/// family if there are none, and start the clock.
///
/// # Errors
///
/// If the log cannot be opened or a family cannot be rebuilt.
pub async fn boot(state: AppState) -> autumn_web::AutumnResult<()> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| unavailable(&e))?;
    let started = std::time::Instant::now();
    let store = Store::open(&dir.join("log.db")).map_err(|e| unavailable(&e))?;
    let atlas = Atlas::open(store).map_err(|e| unavailable(&e))?;
    let century_secs = std::env::var("ASTERISK_CENTURY_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CENTURY_SECS);
    if atlas.all().is_empty() {
        atlas
            .create(&first_family(century_secs), now())
            .map_err(|e| unavailable(&e))?;
    }
    let commands: u64 = atlas.all().iter().map(|f| f.applied()).sum();
    tracing::info!(
        families = atlas.all().len(),
        commands,
        ms = started.elapsed().as_millis(),
        "every family rebuilt from its log"
    );
    state.insert_extension(atlas);
    let atlas = crate::atlas(&state).ok_or_else(|| unavailable(&"the gallery did not install"))?;
    if std::env::var("ASTERISK_CLOCK").is_ok_and(|v| v == "off") {
        tracing::info!("the clock is off: commands run as they arrive");
        return Ok(());
    }
    let driver = Driver::spawn(
        Some(&dir.join("harvest.db")),
        Arc::clone(&atlas),
        Some(state.clone()),
    )
    .map_err(|e| unavailable(&e))?;
    state.insert_extension(driver);
    for f in atlas.all() {
        start_chronicle(&state, &f.row.slug).await;
    }
    Ok(())
}
