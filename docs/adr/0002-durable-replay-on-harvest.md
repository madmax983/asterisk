# ADR 0002 — Durable replay: a command log in SQLite, a clock on Harvest

**Status:** accepted, 2026-11-01. Supersedes the "lives in memory" half of
ADR 0001; its other half — *a world is a seed and a history* — is now the
design.

## Context

Version one kept one world in memory. A restart regrew it from its seed and
lost every hand-typed rule. Families should now run for hundreds of
centuries, overnight, several at once, and end in an export, so they must
survive restarts. The engine already had the property that makes this cheap:
it is deterministic, and every lexicon re-derives from the proto-language and
its recorded history (property-tested since day one).

## Decision

**The command log is the only durable state.** One SQLite file
(`asterisk-data/log.db`, rusqlite, WAL) holds:

* `families(slug, title, seed, vocab, span, century_secs, drifting, created_at)`
  — `vocab` is the word list in its own text syntax;
* `commands(family, seq, cmd, digest)` with primary key `(family, seq)` —
  `cmd` is `Advance` or `Hand { rule, target, hue }` as JSON, `digest` is the
  world's FNV-1a fingerprint after the command ran.

No world is ever stored. At boot every family is rebuilt with
`World::replay_commands(seed, vocab, span, commands)`, and the last digest is
compared; a mismatch (a log written by a different engine) is reported on the
family's page rather than refused.

**Only successful commands are logged.** `World::run` returns `Err` without
changing anything when a command fails (bad rule, target no longer spoken,
chronicle complete), and `Atlas::apply_at` logs only on `Ok`. So the log is
exactly the commands that changed the world, in order — which is what makes
replay exact (`a_command_log_replays_to_the_same_world`, 16 random worlds ×
up to 40 mixed commands).

**Harvest is the clock and the single writer.** Each family has one
`chronicle` workflow on `autumn-harvest-sqlite` (`asterisk-data/harvest.db`),
id `family:<slug>`:

```text
loop 200 times {
    race a `cmd` signal against a durable timer of one century
    signal → that command;  timer → a scheduled Advance
    activity apply_command(slug, next, cmd) → Outcome { next | Complete }
}
continue_as_new(next)
```

Every write that changes a world — a century, a typed rule, `+1 century` —
goes through this loop, so commands to one family are totally ordered
without any locking discipline in the web layer. Form resubmits are deduped
by the signal's idempotency key.

**Exactly-once by position.** The activity offers its command at position
`next`. `apply_at` treats an offer below the log's length as a redelivery and
runs nothing; otherwise it runs the command at the log's length and returns
the new length, which becomes the workflow's `next`. If the process dies
after a century was logged but before Harvest recorded the activity's result,
Harvest re-runs the activity on reopen and the log turns it into a no-op.
This offer semantics is modelled in Verus (`theorem_offer`, `theorem_log`,
`theorem_redelivery`) and exercised by a test that drops the driver
mid-chronicle and reopens the same Harvest file
(`tests/chronicle.rs`), and by `verify.sh`, which `kill -9`s the real binary.

**Stopped time** is a flag on the family (`families.drifting`), checked by the
activity for *scheduled* advances only. The workflow keeps waking each
century and the activity returns `Skipped`; `continue_as_new` every 200 turns
keeps the history bounded.

## Consequences

* A restart loses nothing logged. A family whose server slept overnight wakes
  to its next century: Harvest's timers are durable but there is no catch-up,
  by choice — drift should be watched, not back-filled.
* Boot time grows with the log (each command is replayed). A family at its
  default span of 300 centuries rebuilds in well under a second; very long
  spans would want snapshots (a world and the `seq` it reflects), which the
  digest column already makes safe to verify.
* The engine is now part of the storage format. Changing the generator, the
  library, or a rate changes what a log means; the digest check makes that
  loud instead of silent. A log carries no engine version yet.
* Two SQLite files, both single-writer, one process. Two replicas would need
  a different Harvest backend and a different log store.
* The activity finds the gallery through a process-wide slot set when the
  driver starts (activities are free functions Harvest calls by name).
