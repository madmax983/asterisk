//! The command log: every family's seed, vocabulary and span, and every
//! command it has run, in one `SQLite` file.
//!
//! This is the whole durable state. A world is never stored: it is rebuilt at
//! boot by replaying its commands through the engine
//! (`World::replay_commands`), and the digest stored with each command says
//! whether the rebuild came out the same.
//!
//! `(family, seq)` is the primary key, so a command can be logged once at a
//! given position and never twice — that is what makes a retried Harvest
//! activity harmless.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use asterisk::world::Command;
use rusqlite::{Connection, OptionalExtension as _, params};

/// Errors from the log.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// `SQLite` failed.
    #[error("the log: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A stored command did not decode.
    #[error("a stored command did not decode: {0}")]
    Json(#[from] serde_json::Error),
    /// The slot was taken.
    #[error("command {seq} of {slug} is already logged")]
    Taken {
        /// The family.
        slug: String,
        /// The position.
        seq: u64,
    },
}

/// A family as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyRow {
    /// URL name.
    pub slug: String,
    /// Display title.
    pub title: String,
    /// The seed word.
    pub seed: String,
    /// The vocabulary, in `Vocabulary::parse` syntax.
    pub vocab: String,
    /// The century it ends.
    pub span: Option<u32>,
    /// Seconds per century.
    pub century_secs: u64,
    /// Is time running?
    pub drifting: bool,
    /// Unix seconds.
    pub created_at: i64,
}

/// A logged command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged {
    /// Its position, from 0.
    pub seq: u64,
    /// What it was.
    pub cmd: Command,
    /// The world's digest after it ran.
    pub digest: u64,
}

/// The log.
#[derive(Debug)]
pub struct Store {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS families (
    slug         TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    seed         TEXT NOT NULL,
    vocab        TEXT NOT NULL,
    span         INTEGER,
    century_secs INTEGER NOT NULL,
    drifting     INTEGER NOT NULL DEFAULT 1,
    created_at   INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS commands (
    family  TEXT NOT NULL REFERENCES families(slug),
    seq     INTEGER NOT NULL,
    cmd     TEXT NOT NULL,
    digest  TEXT NOT NULL,
    PRIMARY KEY (family, seq)
);
";

fn u32_of(v: Option<i64>) -> Option<u32> {
    v.and_then(|x| u32::try_from(x).ok())
}

impl Store {
    /// Open or create the log file.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// A log that lives only as long as the process, for tests.
    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic mid-statement leaves SQLite consistent (each statement is
        // atomic), so a poisoned lock is safe to reuse.
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Record a new family.
    pub fn create(&self, f: &FamilyRow) -> Result<(), StoreError> {
        self.conn().execute(
            "INSERT INTO families (slug, title, seed, vocab, span, century_secs, drifting, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                f.slug,
                f.title,
                f.seed,
                f.vocab,
                f.span.map(i64::from),
                i64::try_from(f.century_secs).unwrap_or(i64::MAX),
                f.drifting,
                f.created_at
            ],
        )?;
        Ok(())
    }

    /// Every family, oldest first.
    pub fn families(&self) -> Result<Vec<FamilyRow>, StoreError> {
        let conn = self.conn();
        let mut st = conn.prepare_cached(
            "SELECT slug, title, seed, vocab, span, century_secs, drifting, created_at
             FROM families ORDER BY created_at, slug",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok(FamilyRow {
                    slug: r.get(0)?,
                    title: r.get(1)?,
                    seed: r.get(2)?,
                    vocab: r.get(3)?,
                    span: u32_of(r.get(4)?),
                    century_secs: u64::try_from(r.get::<_, i64>(5)?).unwrap_or(6),
                    drifting: r.get(6)?,
                    created_at: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(st);
        drop(conn);
        Ok(rows)
    }

    /// Is this slug taken?
    pub fn exists(&self, slug: &str) -> Result<bool, StoreError> {
        Ok(self
            .conn()
            .query_row("SELECT 1 FROM families WHERE slug = ?1", [slug], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Start or stop a family's time.
    pub fn set_drifting(&self, slug: &str, on: bool) -> Result<(), StoreError> {
        self.conn().execute(
            "UPDATE families SET drifting = ?2 WHERE slug = ?1",
            params![slug, on],
        )?;
        Ok(())
    }

    /// A family's commands, in order.
    pub fn commands(&self, slug: &str) -> Result<Vec<Logged>, StoreError> {
        let conn = self.conn();
        let mut st =
            conn.prepare("SELECT seq, cmd, digest FROM commands WHERE family = ?1 ORDER BY seq")?;
        let rows = st
            .query_map([slug], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(st);
        drop(conn);
        rows.into_iter()
            .map(|(seq, cmd, digest)| {
                Ok(Logged {
                    seq: u64::try_from(seq).unwrap_or(0),
                    cmd: serde_json::from_str(&cmd)?,
                    digest: u64::from_str_radix(&digest, 16).unwrap_or(0),
                })
            })
            .collect()
    }

    /// Log a command at `seq`. Refuses a slot already taken.
    pub fn append(
        &self,
        slug: &str,
        seq: u64,
        cmd: &Command,
        digest: u64,
    ) -> Result<(), StoreError> {
        let json = serde_json::to_string(cmd)?;
        let n = self.conn().execute(
            "INSERT OR IGNORE INTO commands (family, seq, cmd, digest) VALUES (?1, ?2, ?3, ?4)",
            params![
                slug,
                i64::try_from(seq).unwrap_or(i64::MAX),
                json,
                format!("{digest:016x}")
            ],
        )?;
        if n == 0 {
            return Err(StoreError::Taken {
                slug: slug.to_owned(),
                seq,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asterisk::world::LangId;

    fn row(slug: &str) -> FamilyRow {
        FamilyRow {
            slug: slug.into(),
            title: "T".into(),
            seed: "s".into(),
            vocab: "moon".into(),
            span: Some(100),
            century_secs: 6,
            drifting: true,
            created_at: 1,
        }
    }

    #[test]
    fn families_and_commands_round_trip() {
        let s = Store::in_memory().unwrap_or_else(|e| panic!("{e}"));
        s.create(&row("a")).unwrap_or_else(|e| panic!("{e}"));
        assert!(s.exists("a").unwrap_or(false));
        assert_eq!(s.families().unwrap_or_default(), vec![row("a")]);
        let hand = Command::Hand {
            rule: "p > f".into(),
            target: Some(LangId(3)),
            hue: 7,
        };
        s.append("a", 0, &Command::Advance, 0xdead)
            .unwrap_or_else(|e| panic!("{e}"));
        s.append("a", 1, &hand, 0xbeef)
            .unwrap_or_else(|e| panic!("{e}"));
        let log = s.commands("a").unwrap_or_default();
        assert_eq!(log.len(), 2);
        assert_eq!(log[1].cmd, hand);
        assert_eq!(log[1].digest, 0xbeef);
        s.set_drifting("a", false).unwrap_or_else(|e| panic!("{e}"));
        assert!(!s.families().unwrap_or_default()[0].drifting);
    }

    #[test]
    fn a_slot_is_taken_once() {
        let s = Store::in_memory().unwrap_or_else(|e| panic!("{e}"));
        s.create(&row("a")).unwrap_or_else(|e| panic!("{e}"));
        s.append("a", 0, &Command::Advance, 1)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(
            s.append("a", 0, &Command::Advance, 2),
            Err(StoreError::Taken { seq: 0, .. })
        ));
        assert_eq!(s.commands("a").unwrap_or_default()[0].digest, 1);
    }

    #[test]
    fn a_file_survives_reopening() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let path = dir.path().join("log.db");
        {
            let s = Store::open(&path).unwrap_or_else(|e| panic!("{e}"));
            s.create(&row("a")).unwrap_or_else(|e| panic!("{e}"));
            s.append("a", 0, &Command::Advance, 9)
                .unwrap_or_else(|e| panic!("{e}"));
        }
        let s = Store::open(&path).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(s.commands("a").unwrap_or_default().len(), 1);
    }
}
