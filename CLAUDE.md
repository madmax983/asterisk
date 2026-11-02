# Asterisk — notes for agents

* Engine in `crates/asterisk` stays pure: no IO, async, clock, or randomness
  beyond `rng::Rng` seeded from the world's word.
* Every lexicon change must go through `World::commit` as an `Event`, or
  `World::replay` (and the `worlds_replay` property test) will catch it.
* Rules are printed canonically; if you change the grammar, keep
  `tests/laws.rs::rules_round_trip` green and update `docs/SPEC.md` §3.
* If you change `rule::Rule::apply`'s guard or `MAX_LEN`, update
  `verus/src/asterisk.rs` first.
* Lints: clippy pedantic + nursery, `-D warnings`. No `unwrap`/`expect` outside tests.
* Every command that changes a world goes through `Atlas::apply_at` (from the
  Harvest `chronicle` activity, or `apply_now` in tests). Never mutate a
  world anywhere else, or the command log stops being the truth.
* Changing the generator, library or rates changes what an existing log
  means; the digest check will flag old families as diverged.
* `./verify.sh` is the gate. No services: two SQLite files under ASTERISK_DATA.
