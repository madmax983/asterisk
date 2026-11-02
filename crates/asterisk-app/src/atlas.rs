//! The gallery of families, each a world rebuilt from its command log.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use asterisk::lexicon::Vocabulary;
use asterisk::world::{Command, FeedLine, World};
use asterisk::{VocabError, WorldError};
use serde::Serialize;

use crate::store::{FamilyRow, Store, StoreError};

/// The live-events topic.
pub const TOPIC: &str = "family";

/// What every browser hears.
#[derive(Debug, Clone, Serialize)]
pub struct Broadcast {
    /// Strictly increasing per process.
    pub seq: u64,
    /// Which family.
    pub family: String,
    /// The century now.
    pub century: u32,
    /// `century`, `hand`, `drift`, `complete` or `born`.
    pub kind: &'static str,
    /// What happened.
    pub lines: Vec<FeedLine>,
    /// Is time running?
    pub drifting: bool,
}

/// Something the atlas refused.
#[derive(Debug, thiserror::Error)]
pub enum AtlasError {
    /// No such family.
    #[error("there is no family called {0}")]
    NoFamily(String),
    /// The log failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The world refused.
    #[error(transparent)]
    World(#[from] WorldError),
    /// A word list that did not parse.
    #[error(transparent)]
    Vocab(#[from] VocabError),
    /// The clock refused.
    #[error(transparent)]
    Driver(#[from] crate::chronicle::DriverError),
    /// A rebuild did not reproduce the logged world.
    #[error(
        "{slug}: command {seq} replayed to a different world (logged {logged:016x}, got {got:016x})"
    )]
    Diverged {
        /// Family.
        slug: String,
        /// Where.
        seq: u64,
        /// The logged digest.
        logged: u64,
        /// The rebuilt one.
        got: u64,
    },
}

/// What happened to a command offered at a position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum Outcome {
    /// Run and logged. `next` is the position the following command takes.
    Applied {
        /// Next position.
        next: u64,
    },
    /// Already logged at that position: a redelivery. Nothing ran.
    Duplicate {
        /// Next position.
        next: u64,
    },
    /// Not run: time is stopped, or the world refused it. Nothing logged.
    Skipped {
        /// Next position.
        next: u64,
        /// Why.
        why: String,
    },
    /// The chronicle has reached its span.
    Complete,
}

/// One family: its stored description and its live world.
#[derive(Debug)]
pub struct Family {
    /// As stored.
    pub row: FamilyRow,
    world: Mutex<World>,
    drifting: AtomicBool,
    applied: AtomicU64,
    /// Set if the rebuild at boot disagreed with the log.
    pub diverged: Option<String>,
}

impl Family {
    /// The world.
    pub fn world(&self) -> MutexGuard<'_, World> {
        // Each command commits as a unit after its effects are computed, so a
        // poisoned lock never exposes a half-run command.
        self.world.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Is time running?
    pub fn drifting(&self) -> bool {
        self.drifting.load(Ordering::Relaxed)
    }

    /// Commands logged so far.
    pub fn applied(&self) -> u64 {
        self.applied.load(Ordering::Relaxed)
    }

    /// The URL base, `/f/<slug>`.
    pub fn base(&self) -> String {
        format!("/f/{}", self.row.slug)
    }
}

/// The gallery.
#[derive(Debug)]
pub struct Atlas {
    store: Store,
    families: RwLock<BTreeMap<String, Arc<Family>>>,
    seq: AtomicU64,
}

/// A request for a new family.
#[derive(Debug, Clone)]
pub struct NewFamily {
    /// Display title; the slug comes from it.
    pub title: String,
    /// The seed word.
    pub seed: String,
    /// A word list, or `None` for the standard one.
    pub vocab: Option<String>,
    /// The century it ends.
    pub span: Option<u32>,
    /// Seconds per century.
    pub century_secs: u64,
}

/// A URL-safe name: lower-case letters, digits and single hyphens.
pub fn slugify(title: &str) -> String {
    let mut s = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s = s.trim_matches('-').chars().take(40).collect::<String>();
    if s.is_empty() { "family".to_owned() } else { s }
}

impl Atlas {
    /// Open the gallery over a log, rebuilding every family by replay.
    pub fn open(store: Store) -> Result<Self, AtlasError> {
        let atlas = Self {
            store,
            families: RwLock::new(BTreeMap::new()),
            seq: AtomicU64::new(0),
        };
        for row in atlas.store.families()? {
            let family = atlas.rebuild(row)?;
            atlas.insert(family);
        }
        Ok(atlas)
    }

    fn insert(&self, f: Family) {
        let mut map = self
            .families
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        map.insert(f.row.slug.clone(), Arc::new(f));
    }

    /// Rebuild one family from its log. A digest that disagrees is reported
    /// on the family, not fatal: the world is still the one this engine makes
    /// from that log.
    fn rebuild(&self, row: FamilyRow) -> Result<Family, AtlasError> {
        let vocab = Arc::new(Vocabulary::parse(&row.vocab)?);
        let log = self.store.commands(&row.slug)?;
        let cmds: Vec<Command> = log.iter().map(|l| l.cmd.clone()).collect();
        let mut diverged = None;
        let world = match World::replay_commands(&row.seed, vocab, row.span, &cmds) {
            Ok(w) => {
                if let Some(last) = log.last()
                    && last.digest != w.digest()
                {
                    let e = AtlasError::Diverged {
                        slug: row.slug.clone(),
                        seq: last.seq,
                        logged: last.digest,
                        got: w.digest(),
                    };
                    tracing::warn!(error = %e, "a family rebuilt differently");
                    diverged = Some(e.to_string());
                }
                w
            }
            Err((i, e)) => {
                return Err(AtlasError::Diverged {
                    slug: row.slug.clone(),
                    seq: i as u64,
                    logged: 0,
                    got: u64::from(matches!(e, WorldError::Complete)),
                });
            }
        };
        Ok(Family {
            drifting: AtomicBool::new(row.drifting),
            applied: AtomicU64::new(log.len() as u64),
            world: Mutex::new(world),
            row,
            diverged,
        })
    }

    /// Grow a new family and log it.
    pub fn create(&self, req: &NewFamily, now: i64) -> Result<Arc<Family>, AtlasError> {
        let vocab = match &req.vocab {
            Some(text) if !text.trim().is_empty() => Vocabulary::parse(text)?,
            _ => Vocabulary::standard(),
        };
        let base = slugify(&req.title);
        let mut slug = base.clone();
        let mut n = 2;
        while self.store.exists(&slug)? {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let row = FamilyRow {
            slug: slug.clone(),
            title: req.title.trim().to_owned(),
            seed: req.seed.trim().to_owned(),
            vocab: vocab.to_text(),
            span: req.span,
            century_secs: req.century_secs.max(1),
            drifting: true,
            created_at: now,
        };
        self.store.create(&row)?;
        let family = self.rebuild(row)?;
        self.insert(family);
        self.get(&slug)
    }

    /// One family.
    pub fn get(&self, slug: &str) -> Result<Arc<Family>, AtlasError> {
        self.families
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(slug)
            .cloned()
            .ok_or_else(|| AtlasError::NoFamily(slug.to_owned()))
    }

    /// Every family, oldest first.
    pub fn all(&self) -> Vec<Arc<Family>> {
        let mut v: Vec<Arc<Family>> = self
            .families
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        v.sort_by(|a, b| {
            a.row
                .created_at
                .cmp(&b.row.created_at)
                .then(a.row.slug.cmp(&b.row.slug))
        });
        v
    }

    /// A broadcast for a family.
    pub fn broadcast(&self, f: &Family, kind: &'static str, lines: Vec<FeedLine>) -> Broadcast {
        Broadcast {
            seq: self.seq.fetch_add(1, Ordering::Relaxed) + 1,
            family: f.row.slug.clone(),
            century: f.world().century,
            kind,
            lines,
            drifting: f.drifting(),
        }
    }

    /// Start or stop a family's time.
    pub fn set_drifting(&self, slug: &str, on: bool) -> Result<Broadcast, AtlasError> {
        let f = self.get(slug)?;
        self.store.set_drifting(slug, on)?;
        f.drifting.store(on, Ordering::Relaxed);
        Ok(self.broadcast(&f, "drift", Vec::new()))
    }

    /// Offer a command at position `seq` — the one path every command takes,
    /// from a Harvest activity or straight from a test.
    ///
    /// * `seq` below the log's length is a redelivery: nothing runs.
    /// * A scheduled `Advance` while time is stopped is skipped.
    /// * Otherwise the command runs at the log's length, and is logged only
    ///   if it succeeded, so the log holds exactly the commands that changed
    ///   the world, in order.
    pub fn apply_at(
        &self,
        slug: &str,
        seq: u64,
        cmd: &Command,
        scheduled: bool,
    ) -> Result<(Outcome, Option<Broadcast>), AtlasError> {
        let f = self.get(slug)?;
        let mut w = f.world();
        let applied = f.applied();
        if seq < applied {
            return Ok((Outcome::Duplicate { next: applied }, None));
        }
        if w.complete() {
            return Ok((Outcome::Complete, None));
        }
        if scheduled && !f.drifting() {
            return Ok((
                Outcome::Skipped {
                    next: applied,
                    why: "time is stopped".into(),
                },
                None,
            ));
        }
        let lines = match w.run(cmd) {
            Ok(lines) => lines,
            Err(e) => {
                return Ok((
                    Outcome::Skipped {
                        next: applied,
                        why: e.to_string(),
                    },
                    None,
                ));
            }
        };
        if let Err(e) = self.store.append(slug, applied, cmd, w.digest()) {
            // The world ran a command the log did not keep: put the world back
            // to what the log says.
            tracing::error!(error = %e, slug, "a command was not logged; rebuilding from the log");
            drop(w);
            let rebuilt = self.rebuild(f.row.clone())?;
            *f.world() = rebuilt
                .world
                .into_inner()
                .unwrap_or_else(PoisonError::into_inner);
            return Err(e.into());
        }
        f.applied.store(applied + 1, Ordering::Relaxed);
        let complete = w.complete();
        drop(w);
        let kind = match (cmd, complete) {
            (_, true) => "complete",
            (Command::Advance, false) => "century",
            (Command::Hand { .. }, false) => "hand",
        };
        let b = self.broadcast(&f, kind, lines);
        Ok((Outcome::Applied { next: applied + 1 }, Some(b)))
    }

    /// Run a command at the end of the log, now. For tests and for running
    /// without a Harvest driver.
    pub fn apply_now(
        &self,
        slug: &str,
        cmd: &Command,
    ) -> Result<(Outcome, Option<Broadcast>), AtlasError> {
        let applied = self.get(slug)?.applied();
        self.apply_at(slug, applied, cmd, false)
    }

    /// The log, for tests and verification.
    pub const fn store(&self) -> &Store {
        &self.store
    }

    /// Rebuild a family from the log alone and compare digests.
    ///
    /// Holds the family's lock throughout: [`Atlas::apply_at`] logs under the
    /// same lock, so the log cannot grow between the rebuild and the compare.
    pub fn verify(&self, slug: &str) -> Result<bool, AtlasError> {
        let f = self.get(slug)?;
        let live = f.world();
        let rebuilt = self.rebuild(f.row.clone())?;
        let same = rebuilt.world().digest() == live.digest() && rebuilt.applied() == f.applied();
        drop(live);
        Ok(same)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas() -> Atlas {
        Atlas::open(Store::in_memory().unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn new(title: &str) -> NewFamily {
        NewFamily {
            title: title.into(),
            seed: title.into(),
            vocab: None,
            span: Some(40),
            century_secs: 6,
        }
    }

    #[test]
    fn slugs_are_url_safe_and_unique() {
        assert_eq!(slugify("The Tolkien Family!"), "the-tolkien-family");
        assert_eq!(slugify("  ***  "), "family");
        let a = atlas();
        let x = a.create(&new("Ash"), 1).unwrap_or_else(|e| panic!("{e}"));
        let y = a.create(&new("Ash"), 2).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(x.row.slug, "ash");
        assert_eq!(y.row.slug, "ash-2");
        assert_eq!(a.all().len(), 2);
    }

    #[test]
    fn redelivery_is_harmless_and_failures_are_not_logged() {
        let a = atlas();
        a.create(&new("salt"), 1).unwrap_or_else(|e| panic!("{e}"));
        let (o, b) = a
            .apply_at("salt", 0, &Command::Advance, true)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(o, Outcome::Applied { next: 1 });
        assert!(b.is_some());
        let (o, b) = a
            .apply_at("salt", 0, &Command::Advance, true)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(o, Outcome::Duplicate { next: 1 });
        assert!(b.is_none());
        let bad = Command::Hand {
            rule: "p >".into(),
            target: None,
            hue: 0,
        };
        let (o, _) = a
            .apply_at("salt", 1, &bad, false)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(o, Outcome::Skipped { next: 1, .. }));
        assert_eq!(a.store().commands("salt").map_or(0, |v| v.len()), 1);
        assert!(a.verify("salt").unwrap_or(false));
    }

    #[test]
    fn stopped_time_skips_scheduled_centuries_only() {
        let a = atlas();
        a.create(&new("ice"), 1).unwrap_or_else(|e| panic!("{e}"));
        a.set_drifting("ice", false)
            .unwrap_or_else(|e| panic!("{e}"));
        let (o, _) = a
            .apply_at("ice", 0, &Command::Advance, true)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(o, Outcome::Skipped { .. }));
        let (o, _) = a
            .apply_at("ice", 0, &Command::Advance, false)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(o, Outcome::Applied { next: 1 });
    }

    #[test]
    fn a_chronicle_completes_at_its_span() {
        let a = atlas();
        a.create(&new("brief"), 1).unwrap_or_else(|e| panic!("{e}"));
        let mut last = None;
        for _ in 0..20 {
            let (o, b) = a
                .apply_now("brief", &Command::Advance)
                .unwrap_or_else(|e| panic!("{e}"));
            if let Some(b) = b {
                last = Some(b.kind);
            }
            if o == Outcome::Complete {
                break;
            }
        }
        assert_eq!(last, Some("complete"));
        assert_eq!(a.get("brief").map_or(0, |f| f.world().century), 40);
    }

    #[test]
    fn reopening_rebuilds_every_family_exactly() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("log.db");
        let digest = {
            let a = Atlas::open(Store::open(&path).unwrap_or_else(|e| panic!("{e}")))
                .unwrap_or_else(|e| panic!("{e}"));
            a.create(&new("ember"), 1).unwrap_or_else(|e| panic!("{e}"));
            for i in 0..6 {
                a.apply_now("ember", &Command::Advance)
                    .unwrap_or_else(|e| panic!("{e}"));
                if i == 2 {
                    let hand = Command::Hand {
                        rule: "V > [+nasal] / _N".into(),
                        target: None,
                        hue: 9,
                    };
                    a.apply_now("ember", &hand)
                        .unwrap_or_else(|e| panic!("{e}"));
                }
            }
            a.get("ember").map_or(0, |f| f.world().digest())
        };
        let a = Atlas::open(Store::open(&path).unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"));
        let f = a.get("ember").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(f.world().digest(), digest);
        assert_eq!(f.applied(), 7);
        assert!(f.diverged.is_none());
    }

    #[test]
    fn a_custom_word_list_grows_its_own_family() {
        let a = atlas();
        let req = NewFamily {
            vocab: Some("moon = mena\nstar\nPL = s\nstar.PL".into()),
            ..new("mine")
        };
        let f = a.create(&req, 1).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(f.world().vocab.len(), 6);
        let bad = NewFamily {
            vocab: Some("star.PL".into()),
            ..new("broken")
        };
        assert!(matches!(a.create(&bad, 2), Err(AtlasError::Vocab(_))));
    }
}
