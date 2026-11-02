# Asterisk — the rules of the thing

## 1. Segments

The inventory is a closed table of **85 segments** (`phon::COUNT`): 33
consonants described by place, manner and voicing, and 52 vowels — 13
qualities (`i y ɨ u e ø ə o ɛ œ ɔ æ a`), each plain, long (`ː`), nasal (`◌̃`),
or both. A `Seg` is an index `< 85`, and nothing else can construct one.

A **word** is 1 to `MAX_LEN = 12` segments. A **lexicon** has exactly one word
for each meaning of its family's **vocabulary**.

**Spelling.** Words and rules are written in IPA. Where two glyphs would fuse
on re-reading (`t` + `ʃ` reads back as `tʃ`) a `.` is written between them.

## 2. Feature edits

Fifteen edits (`+voice -voice +cont -cont +pal +raise +lower +front +back
+round -round +long -long +nasal -nasal`). Each is a **total** function on
segments: a segment the edit does not apply to, or whose image the
inventory lacks, is returned unchanged. Each maps the inventory into itself.

## 3. The sound-change language

```text
rule    := atom ">" result [ "/" context ]
result  := "∅" | "0" | seg [seg] | "[" edit+ "]"
context := ["#"] atom{0,2} "_" atom{0,2} ["#"]
atom    := seg | "C" | "V" | "P" | "F" | "N" | "L" | "G" | "[" (±feature)+ "]"
```

Matchable features: `voice son stop cont lab long nasal round front back high
low`. Whitespace and `.` between tokens are ignored. Several rules separated
by `;` or newlines form a **chain**, applied in order.

**Application is simultaneous.** For input word `w` and rule `r`:

1. `fires[i]` is computed for every position `i` against `w` itself: the
   target matches `w[i]`, the left atoms match the segments immediately before
   (starting the word if `#`), the right atoms match the segments immediately
   after (ending the word if `#`).
2. The raw output is the concatenation over `i` of: `effect(w[i])` if
   `fires[i]`, else `[w[i]]`. `effect` yields 0 (`∅`), 1 (edit, or one
   literal) or 2 (two literals) segments.
3. **The guard.** If the raw output is empty, longer than `MAX_LEN`, or equal
   to `w`, the word is unchanged. Otherwise it becomes the raw output.

## 3½. Vocabularies

A vocabulary is per family. The standard one is the Swadesh-207 list, 34
extras, and a grammar: three suffixes (`PL`, `PAST`, `GEN`) and 33 inflected
forms (`foot.PL`, `sing.PAST`, `I.GEN`, …) — 277 meanings. Anyone may bring
their own, one entry per line:

```text
moon            a word
moon = mena     a word with a fixed proto-form
PL = s          a suffix: two or more capitals
foot.PL         an inflected form: an existing word . an existing suffix
# comment
```

`tongue` (which names every language) and `little` (which renews worn-out
words) are added if missing. At most 600 entries. `parse(to_text(v)) == v`.

**Inflected forms** are born as base ++ suffix and from then on drift as one
word. Renewal and borrowing touch only plain words; when a word is replaced
wholesale, its inflected forms are rebuilt regular from the new word.

The **productive ending** of a suffix in a language is what speakers abstract:
every inflected form whose current base is a prefix of it votes for the rest
(possibly nothing); the ending with the most votes (≥ 2) wins, ties to the
shorter then the lesser; with no winner, the suffix as it stands alone. The
**regular** form is base ++ productive ending; a form that differs is
**irregular**. **Levelling** (p = 0.25 per language per century) rebuilds one
irregular form as its regular one.

## 4. The world

* A world is seeded by a word (FNV-1a → SplitMix64). The seed invents a
  phonology (6 core consonants + 5–9 more, 5–10 vowels, sometimes long ones)
  and a proto-lexicon of 1–3-syllable words.
* Language 0 is the proto-language. Every other language has a parent with a
  **smaller id**. A language that splits stops changing (`Fate::Split`); its
  two daughters start from its lexicon. A language may fall silent.
* Each century, each spoken language, independently:
  * with p = 0.56, undergoes a sound change drawn from a library of 51
    attested changes (only changes that would alter at least one word count);
  * with p = 0.30, if it has words of ≤ 2 segments, renews one by compounding
    it with a partner meaning (*sun* → *sun-day*, else *X-little*), if the
    result fits `MAX_LEN`;
  * with p = 0.035, borrows one word from a sister;
  * with p = 0.12, if ≥ 3 centuries old and fewer than 10 are spoken and
    fewer than 61 have ever existed, splits;
  * with p = 0.25, if it has irregular forms, levels one by analogy;
  * with p = 0.014, if more than 5 are spoken, falls silent.
* People may apply any rule to any spoken language (or all) at any time.
* The world first runs a prelude of 32 centuries. A family may have a span;
  at its span the chronicle is complete and accepts no more commands.

## 4½. The command log

A family is its seed, vocabulary, span, and its log of commands — `Advance`
or `Hand { rule, target, hue }`. `World::run(cmd)` either succeeds or returns
`Err` with the world unchanged; only successful commands are logged, each
with the world's digest after it. `World::replay_commands` rebuilds the world
from those alone.

Commands reach a family only through its Harvest `chronicle` workflow, which
races a `cmd` signal against a one-century durable timer and runs each turn
as the activity `apply_command(slug, next, cmd)`. The activity calls
`Atlas::apply_at(slug, seq, cmd)`:

* `seq < len(log)` → a redelivery: nothing runs; returns `next = len(log)`;
* a scheduled `Advance` while time is stopped → skipped;
* otherwise the command runs; on success it is logged at `len(log)` and
  `next = len(log) + 1`; on failure nothing is logged.

Every change is an `Event` in the history of the language it changed.

## 5. What must be true

### Formally (Verus, `verus/src/asterisk.rs`)

| # | Obligation | Confidence |
|---|---|---|
| F1 | **Closure** — raw output segments are all `< COUNT`, for any effect and any firing pattern | high |
| F2 | **Growth** — `|raw| ≤ 2|w|`, and `|raw| ≤ |w|` unless the effect writes two segments | high |
| F3 | **Guard** — the guarded step maps valid words to valid words | high (follows from F1, F2 by case split) |
| F4 | **History** — any finite sequence of guarded steps keeps a valid word valid | medium-high: induction over `subrange`; the trigger on `es[k]` may need a nudge |
| F5 | **Exec** — `apply_raw`/`step` are proved equal to the spec | **medium**: the loop's `out@ == out_prefix(i+1)` relies on Verus unfolding `out_prefix` one level and on `=~=` for push-vs-concat; the `match e` on `&Effect` inside a loop is the line most likely to need restating |
| F6 | **Lineage** — `parent[k] < k` for all k ⇒ following parents reaches 0 within k steps | high |
| F7 | **The log is exactly-once** — an offer below the log's length changes nothing; any offer adds at most one entry and only extends the log; any history of offers only extends it, by at most one per offer; a redelivered batch is a no-op | high: plain `Seq` reasoning; the `push` index fact in `theorem_offer` is the one place a trigger may need spelling out |

Abstractions, stated plainly: *where* a rule fires is an arbitrary
`Seq<bool>` (context matching is not modelled, so F1–F4 hold for whatever the
matcher computes); a chain of feature edits is a lookup table of 85 entries.
That runtime edits are closed over the inventory is checked exhaustively
(85 × 15) by a unit test, not proved.

None of this was run through Verus during the build: there is no Verus in
the build environment. `./verify.sh` on the Mac is the first time it is
checked.

### Observably (tests)

| Property | Where |
|---|---|
| `parse(print(r)) == r` for arbitrary rules (600 cases) | `tests/laws.rs::rules_round_trip` |
| printing is a fixed point | `printing_is_stable` |
| words spell back to themselves | `words_round_trip` |
| F1, F2 at runtime | `raw_application_is_closed_and_bounded` |
| F3, F4 at runtime (sequences of up to 30 arbitrary rules) | `the_guarded_step_keeps_words_valid`, `no_history_breaks_a_word` |
| non-firing positions are copied | `non_firing_positions_are_untouched` |
| simultaneity: mirror(rule)(reverse(w)) = reverse(rule(w)) | `application_is_order_free` |
| **replay**: every lexicon re-derives from the proto-language and the recorded rules, with arbitrary hand rules mixed in | `worlds_replay` |
| etymologies are contiguous and end at the current form | `worlds_replay`, `every_etymology_ends_where_the_word_is` |
| every library entry parses and prints back exactly as written | `library::tests` |
| worlds stay bounded over 600 centuries | `worlds_stay_bounded_over_a_long_time` |
| the HTTP surface, preview-is-pure, apply-is-recorded, 404s, 400s | `crates/asterisk-app/tests/http.rs` |
| a command log rebuilds the same world (digest, lexicons, chronicle), and a failed command changes nothing | `a_command_log_replays_to_the_same_world` |
| custom word lists print back to themselves and keep their fixed proto-forms | `custom_vocabularies_round_trip`, `lexicon::tests` |
| inflected forms are born fused | `inflected_forms_are_born_fused` |
| a log slot is taken once; a redelivered offer runs nothing; failures are not logged; stopped time skips only scheduled centuries | `store::tests`, `atlas::tests` |
| a family reopened from its log file is the same world | `reopening_rebuilds_every_family_exactly` |
| **Harvest**: the clock ticks, a typed rule arrives as a signal (and a resubmit is not delivered twice), the driver dies mid-chronicle, a new one reopens the same file and attaches, the chronicle completes at its span with no gaps or repeats in the log, and the log alone rebuilds the world | `tests/chronicle.rs` |
| exports: Markdown has the tree, cognates, every spoken language; JSON matches the world and each history ends at its word | `export::tests` |
| CSRF refuses a tokenless apply; the real binary boots; **kill -9 mid-chronicle loses nothing and resumes** | `verify.sh` end-to-end gate |
