# ADR 0001 — A world is a seed and a history; it lives in memory

**Status:** accepted, 2026-11-01; its in-memory half superseded by ADR 0002 the same day.

## Context

Asterisk's state is one family of languages: a tree of at most 61 languages,
each with 55 words of at most 12 segments and a list of events. It is small
(well under a megabyte), and changes every few seconds. Earlier builds on
this stack reached for Postgres or SQLite first. This one does not need to.

The engine already guarantees something stronger than persistence would:
every lexicon is a pure function of the proto-language (itself a pure
function of the seed word) and the recorded rules
(`World::replay`, property-tested). The history *is* the state.

## Decision

* The world lives in memory, in one `Atlas` behind a `std::sync::Mutex`,
  installed as an `AppState` extension. Every operation under the lock is a
  century of a dozen languages or one rule over them: sub-millisecond.
* No database, no Docker, no migrations. `verify.sh` has no services gate.
* Live updates go over one Autumn channel topic (`family`) as small JSON
  notices; browsers refetch server-rendered fragments. A missed message costs
  an animation, never correctness.
* The poisoned-lock case is recovered (`PoisonError::into_inner`): each event
  is committed as a single push after its changes are computed, so a reader
  never sees a half-applied sound change.

## Consequences

* A restart grows the family again from its seed and loses hand-typed rules.
  That is the honest cost; it is also the obvious next step: persist the
  event log (seed + `Vec<(LangId, Event)>`) to SQLite and replay it at boot.
  Because replay is already tested, that is a storage change, not a design
  change.
* One process owns the world. Two replicas would grow two families.
* Rendering the SVG tree on the server keeps the client to one small script
  and makes every tree linkable (`/?m=salt`).
