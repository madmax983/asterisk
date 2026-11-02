// Asterisk — the verified core.
//
//   verus verus/src/asterisk.rs
//
// What is proved here is why no word in any language ever breaks, however
// many sound changes — drifted or typed by hand — happen to it:
//
//   1. closure    — a sound change maps inventory segments to inventory
//                   segments: every segment of the output is < COUNT;
//   2. growth     — the unguarded output is at most twice as long as the
//                   input, and no longer than the input unless the effect
//                   writes two segments;
//   3. the guard  — the guarded step (keep the old form if the new one would
//                   be empty or longer than MAX_LEN) maps valid words to
//                   valid words, where valid means 1..=MAX_LEN segments, all
//                   in the inventory;
//   4. history    — any finite sequence of guarded steps, with any effects
//                   and ANY firing patterns, keeps a valid word valid;
//   5. exec       — `apply_raw` and `step` below are executable Rust whose
//                   results are proved equal to the spec functions;
//   6. lineage    — in a family where every language's parent has a smaller
//                   id, following parents from any language reaches the
//                   proto-language within `id` steps;
//   7. the log    — offering commands to the command log (Atlas::apply_at)
//                   is exactly-once: a redelivered position changes nothing,
//                   an offer adds at most one entry, and the log only ever
//                   grows by extension — whatever order, duplication or
//                   failure the offers come with. So a Harvest activity
//                   retried after a crash cannot run a century twice.
//
// This file is standalone: no dependency on the `asterisk` crate and no IO.
//
// ── The modelling assumptions ───────────────────────────────────────────────
//
// * A segment is a u8; the inventory is 0..85 (`phon::COUNT` in
//   crates/asterisk/src/phon.rs).
// * WHERE a rule fires is abstracted to an arbitrary `Seq<bool>`. Context
//   matching (`Rule::fires`) is not modelled; properties 1–4 hold for every
//   firing pattern, so they hold for whatever the matcher computes. The
//   matcher is unit- and property-tested instead.
// * A chain of feature edits (`Effect::Edit`) is modelled as a lookup table
//   of 85 entries, each < 85. That the runtime edits are closed over the
//   inventory is checked exhaustively by `every_edit_is_closed_over_the_
//   inventory` in phon.rs (85 segments × 15 edits).
// * `Effect::Become` with one or two segments is `One` / `Two`.

use vstd::prelude::*;

verus! {

pub const COUNT: u8 = 85;
pub const MAX_LEN: usize = 12;

// ── Words ───────────────────────────────────────────────────────────────────

pub open spec fn seg_ok(s: u8) -> bool {
    s < COUNT
}

pub open spec fn segs_ok(w: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < w.len() ==> seg_ok(#[trigger] w[i])
}

pub open spec fn valid_word(w: Seq<u8>) -> bool {
    1 <= w.len() <= MAX_LEN && segs_ok(w)
}

proof fn lemma_single_ok(x: u8)
    requires
        seg_ok(x),
    ensures
        segs_ok(seq![x]),
{
    assert(seq![x][0] == x);
}

proof fn lemma_concat_ok(a: Seq<u8>, b: Seq<u8>)
    requires
        segs_ok(a),
        segs_ok(b),
    ensures
        segs_ok(a + b),
{
    assert forall|i: int| 0 <= i < (a + b).len() implies seg_ok(#[trigger] (a + b)[i]) by {
        if i < a.len() {
            assert((a + b)[i] == a[i]);
        } else {
            assert((a + b)[i] == b[i - a.len()]);
        }
    }
}

// ── Effects ─────────────────────────────────────────────────────────────────

pub enum Effect {
    Delete,
    One(u8),
    Two(u8, u8),
    Map(Vec<u8>),
}

pub open spec fn effect_wf(e: Effect) -> bool {
    match e {
        Effect::Delete => true,
        Effect::One(a) => seg_ok(a),
        Effect::Two(a, b) => seg_ok(a) && seg_ok(b),
        Effect::Map(t) => t@.len() == COUNT as nat && forall|s: int|
            0 <= s < COUNT ==> seg_ok(#[trigger] t@[s]),
    }
}

pub open spec fn is_two(e: Effect) -> bool {
    match e {
        Effect::Two(_, _) => true,
        _ => false,
    }
}

/// What one firing position becomes.
pub open spec fn piece(e: Effect, s: u8) -> Seq<u8> {
    match e {
        Effect::Delete => Seq::<u8>::empty(),
        Effect::One(a) => seq![a],
        Effect::Two(a, b) => seq![a, b],
        Effect::Map(t) => seq![t@[s as int]],
    }
}

proof fn lemma_piece(e: Effect, s: u8)
    requires
        effect_wf(e),
        seg_ok(s),
    ensures
        segs_ok(piece(e, s)),
        piece(e, s).len() <= 2,
        !is_two(e) ==> piece(e, s).len() <= 1,
{
    match e {
        Effect::Delete => {},
        Effect::One(a) => {
            lemma_single_ok(a);
        },
        Effect::Two(a, b) => {
            assert(seq![a, b][0] == a);
            assert(seq![a, b][1] == b);
        },
        Effect::Map(t) => {
            assert(seg_ok(t@[s as int]));
            lemma_single_ok(t@[s as int]);
        },
    }
}

// ── Simultaneous application ───────────────────────────────────────────────

pub open spec fn fires_at(fires: Seq<bool>, i: int) -> bool {
    0 <= i < fires.len() && fires[i]
}

/// The output for the first n positions of w.
pub open spec fn out_prefix(e: Effect, fires: Seq<bool>, w: Seq<u8>, n: nat) -> Seq<u8>
    decreases n,
{
    if n == 0 {
        Seq::<u8>::empty()
    } else {
        let i = (n - 1) as int;
        let prev = out_prefix(e, fires, w, (n - 1) as nat);
        if fires_at(fires, i) {
            prev + piece(e, w[i])
        } else {
            prev + seq![w[i]]
        }
    }
}

pub open spec fn raw(e: Effect, fires: Seq<bool>, w: Seq<u8>) -> Seq<u8> {
    out_prefix(e, fires, w, w.len())
}

/// The guarded step: keep the old form when the new one is not a word.
pub open spec fn guarded(e: Effect, fires: Seq<bool>, w: Seq<u8>) -> Seq<u8> {
    let r = raw(e, fires, w);
    if r.len() == 0 || r.len() > MAX_LEN {
        w
    } else {
        r
    }
}

proof fn lemma_prefix(e: Effect, fires: Seq<bool>, w: Seq<u8>, n: nat)
    requires
        effect_wf(e),
        segs_ok(w),
        n <= w.len(),
    ensures
        segs_ok(out_prefix(e, fires, w, n)),
        out_prefix(e, fires, w, n).len() <= 2 * n,
        !is_two(e) ==> out_prefix(e, fires, w, n).len() <= n,
    decreases n,
{
    if n > 0 {
        let i = (n - 1) as int;
        lemma_prefix(e, fires, w, (n - 1) as nat);
        let prev = out_prefix(e, fires, w, (n - 1) as nat);
        assert(seg_ok(w[i]));
        lemma_piece(e, w[i]);
        if fires_at(fires, i) {
            lemma_concat_ok(prev, piece(e, w[i]));
        } else {
            lemma_single_ok(w[i]);
            lemma_concat_ok(prev, seq![w[i]]);
        }
    }
}

/// Properties 1 and 2.
pub proof fn theorem_closure_and_growth(e: Effect, fires: Seq<bool>, w: Seq<u8>)
    requires
        effect_wf(e),
        segs_ok(w),
    ensures
        segs_ok(raw(e, fires, w)),
        raw(e, fires, w).len() <= 2 * w.len(),
        !is_two(e) ==> raw(e, fires, w).len() <= w.len(),
{
    lemma_prefix(e, fires, w, w.len());
}

/// Property 3.
pub proof fn theorem_guard(e: Effect, fires: Seq<bool>, w: Seq<u8>)
    requires
        effect_wf(e),
        valid_word(w),
    ensures
        valid_word(guarded(e, fires, w)),
{
    theorem_closure_and_growth(e, fires, w);
}

// ── Histories ───────────────────────────────────────────────────────────────

/// Apply es[0], es[1], … in order, the k-th with firing pattern fs[k].
pub open spec fn run(es: Seq<Effect>, fs: Seq<Seq<bool>>, w: Seq<u8>) -> Seq<u8>
    decreases es.len(),
{
    if es.len() == 0 {
        w
    } else {
        let k = (es.len() - 1) as int;
        guarded(es[k], fs[k], run(es.subrange(0, k), fs, w))
    }
}

/// Property 4: no history ever breaks a word.
pub proof fn theorem_history(es: Seq<Effect>, fs: Seq<Seq<bool>>, w: Seq<u8>)
    requires
        forall|k: int| 0 <= k < es.len() ==> effect_wf(#[trigger] es[k]),
        valid_word(w),
    ensures
        valid_word(run(es, fs, w)),
    decreases es.len(),
{
    if es.len() > 0 {
        let k = (es.len() - 1) as int;
        let pre = es.subrange(0, k);
        assert forall|j: int| 0 <= j < pre.len() implies effect_wf(#[trigger] pre[j]) by {
            assert(pre[j] == es[j]);
        }
        theorem_history(pre, fs, w);
        assert(effect_wf(es[k]));
        theorem_guard(es[k], fs[k], run(pre, fs, w));
    }
}

// ── Executable, proved equal to the spec ───────────────────────────────────

pub fn apply_raw(e: &Effect, fires: &Vec<bool>, w: &Vec<u8>) -> (out: Vec<u8>)
    requires
        effect_wf(*e),
        fires.len() == w.len(),
        segs_ok(w@),
    ensures
        out@ == raw(*e, fires@, w@),
        segs_ok(out@),
        out.len() <= 2 * w.len(),
{
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < w.len()
        invariant
            0 <= i <= w.len(),
            fires.len() == w.len(),
            effect_wf(*e),
            segs_ok(w@),
            out@ == out_prefix(*e, fires@, w@, i as nat),
        decreases w.len() - i,
    {
        let ghost before = out@;
        let s = w[i];
        proof {
            assert(seg_ok(w@[i as int]));
        }
        if fires[i] {
            match e {
                Effect::Delete => {},
                Effect::One(a) => {
                    out.push(*a);
                },
                Effect::Two(a, b) => {
                    out.push(*a);
                    out.push(*b);
                },
                Effect::Map(t) => {
                    out.push(t[s as usize]);
                },
            }
            proof {
                assert(out@ =~= before + piece(*e, w@[i as int]));
            }
        } else {
            out.push(s);
            proof {
                assert(out@ =~= before + seq![w@[i as int]]);
            }
        }
        proof {
            assert(fires_at(fires@, i as int) == fires@[i as int]);
            assert(out@ == out_prefix(*e, fires@, w@, (i + 1) as nat));
        }
        i = i + 1;
    }
    proof {
        theorem_closure_and_growth(*e, fires@, w@);
    }
    out
}

fn copy_word(w: &Vec<u8>) -> (out: Vec<u8>)
    ensures
        out@ == w@,
{
    let mut out: Vec<u8> = Vec::new();
    let mut i: usize = 0;
    while i < w.len()
        invariant
            0 <= i <= w.len(),
            out@ == w@.subrange(0, i as int),
        decreases w.len() - i,
    {
        out.push(w[i]);
        proof {
            assert(w@.subrange(0, i as int + 1) =~= w@.subrange(0, i as int).push(w@[i as int]));
        }
        i = i + 1;
    }
    proof {
        assert(w@.subrange(0, w.len() as int) =~= w@);
    }
    out
}

/// The guarded step, executable.
pub fn step(e: &Effect, fires: &Vec<bool>, w: &Vec<u8>) -> (out: Vec<u8>)
    requires
        effect_wf(*e),
        fires.len() == w.len(),
        valid_word(w@),
    ensures
        out@ == guarded(*e, fires@, w@),
        valid_word(out@),
{
    let r = apply_raw(e, fires, w);
    proof {
        theorem_guard(*e, fires@, w@);
    }
    if r.len() == 0 || r.len() > MAX_LEN {
        copy_word(w)
    } else {
        r
    }
}

// ── Lineage ─────────────────────────────────────────────────────────────────

pub open spec fn parents_ok(parent: Seq<int>) -> bool {
    forall|k: int| 1 <= k < parent.len() ==> 0 <= #[trigger] parent[k] < k
}

/// Following parents from k reaches 0 within `steps` steps.
pub open spec fn reaches_root(parent: Seq<int>, k: int, steps: nat) -> bool
    decreases steps,
{
    if k == 0 {
        true
    } else if steps == 0 {
        false
    } else {
        reaches_root(parent, parent[k], (steps - 1) as nat)
    }
}

proof fn lemma_more_steps(parent: Seq<int>, k: int, a: nat, b: nat)
    requires
        parents_ok(parent),
        0 <= k < parent.len(),
        reaches_root(parent, k, a),
        a <= b,
    ensures
        reaches_root(parent, k, b),
    decreases a,
{
    if k != 0 {
        assert(a > 0);
        assert(0 <= parent[k] < k);
        lemma_more_steps(parent, parent[k], (a - 1) as nat, (b - 1) as nat);
    }
}

/// Property 6.
pub proof fn theorem_lineage(parent: Seq<int>, k: int)
    requires
        parents_ok(parent),
        0 <= k < parent.len(),
    ensures
        reaches_root(parent, k, k as nat),
    decreases k,
{
    if k > 0 {
        let p = parent[k];
        assert(0 <= p < k);
        theorem_lineage(parent, p);
        lemma_more_steps(parent, p, p as nat, (k - 1) as nat);
    }
}

// ── The command log ─────────────────────────────────────────────────────────
//
// Commands are abstract ints. An offer carries the position the chronicle
// believes is next, the command, and whether the world accepted it. The
// runtime (Atlas::apply_at) logs an accepted command at the log's length, not
// at the offered position, and ignores an offer below the length.

pub open spec fn offer(log: Seq<int>, seq: nat, c: int, accepted: bool) -> Seq<int> {
    if seq < log.len() {
        log
    } else if accepted {
        log.push(c)
    } else {
        log
    }
}

pub open spec fn is_prefix(a: Seq<int>, b: Seq<int>) -> bool {
    a.len() <= b.len() && forall|i: int| 0 <= i < a.len() ==> #[trigger] a[i] == b[i]
}

/// One offer: at most one entry, by extension; a redelivery is a no-op.
pub proof fn theorem_offer(log: Seq<int>, seq: nat, c: int, accepted: bool)
    ensures
        offer(log, seq, c, accepted).len() <= log.len() + 1,
        is_prefix(log, offer(log, seq, c, accepted)),
        seq < log.len() ==> offer(log, seq, c, accepted) == log,
        !accepted ==> offer(log, seq, c, accepted) == log,
{
    let r = offer(log, seq, c, accepted);
    assert forall|i: int| 0 <= i < log.len() implies #[trigger] log[i] == r[i] by {
        if !(seq < log.len()) && accepted {
            assert(r == log.push(c));
            assert(log.push(c)[i] == log[i]);
        }
    }
}

proof fn lemma_prefix_trans(a: Seq<int>, b: Seq<int>, c: Seq<int>)
    requires
        is_prefix(a, b),
        is_prefix(b, c),
    ensures
        is_prefix(a, c),
{
    assert forall|i: int| 0 <= i < a.len() implies #[trigger] a[i] == c[i] by {
        assert(a[i] == b[i]);
        assert(b[i] == c[i]);
    }
}

/// Offers, in order.
pub open spec fn offers(log: Seq<int>, seqs: Seq<nat>, cs: Seq<int>, oks: Seq<bool>) -> Seq<int>
    decreases seqs.len(),
{
    if seqs.len() == 0 {
        log
    } else {
        let k = (seqs.len() - 1) as int;
        offer(offers(log, seqs.subrange(0, k), cs, oks), seqs[k], cs[k], oks[k])
    }
}

/// Property 7: any history of offers only extends the log, by at most one
/// entry per offer.
pub proof fn theorem_log(log: Seq<int>, seqs: Seq<nat>, cs: Seq<int>, oks: Seq<bool>)
    ensures
        is_prefix(log, offers(log, seqs, cs, oks)),
        offers(log, seqs, cs, oks).len() <= log.len() + seqs.len(),
    decreases seqs.len(),
{
    if seqs.len() > 0 {
        let k = (seqs.len() - 1) as int;
        let pre = seqs.subrange(0, k);
        theorem_log(log, pre, cs, oks);
        let mid = offers(log, pre, cs, oks);
        theorem_offer(mid, seqs[k], cs[k], oks[k]);
        lemma_prefix_trans(log, mid, offers(log, seqs, cs, oks));
    }
}

/// A redelivered batch — the same offers again, every position now below the
/// log's length — changes nothing.
pub proof fn theorem_redelivery(log: Seq<int>, seqs: Seq<nat>, cs: Seq<int>, oks: Seq<bool>)
    requires
        forall|k: int| 0 <= k < seqs.len() ==> #[trigger] seqs[k] < log.len(),
    ensures
        offers(log, seqs, cs, oks) == log,
    decreases seqs.len(),
{
    if seqs.len() > 0 {
        let k = (seqs.len() - 1) as int;
        let pre = seqs.subrange(0, k);
        assert forall|j: int| 0 <= j < pre.len() implies #[trigger] pre[j] < log.len() by {
            assert(pre[j] == seqs[j]);
        }
        theorem_redelivery(log, pre, cs, oks);
        assert(seqs[k] < log.len());
    }
}

fn main() {
}

} // verus!
