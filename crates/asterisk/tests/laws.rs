//! The laws in docs/SPEC.md, held to over arbitrary inputs.

use std::fmt::Write as _;
use std::sync::Arc;

use asterisk::lexicon::{Vocabulary, Word};
use asterisk::phon::{self, COUNT, Edit, Seg};
use asterisk::rule::{Atom, Class, Context, Effect, MAX_CONTEXT, MAX_LEN, Rule, Test};
use asterisk::world::{Command, LangId, World};
use proptest::prelude::*;

fn seg() -> impl Strategy<Value = Seg> {
    (0..COUNT).prop_filter_map("in range", Seg::from_index)
}

fn word() -> impl Strategy<Value = Vec<Seg>> {
    prop::collection::vec(seg(), 1..=MAX_LEN)
}

fn atom() -> impl Strategy<Value = Atom> {
    prop_oneof![
        3 => seg().prop_map(Atom::Seg),
        2 => prop::sample::select(Class::ALL.iter().map(|c| c.0).collect::<Vec<_>>()).prop_map(Atom::Class),
        1 => prop::collection::vec(
            (any::<bool>(), prop::sample::select(Test::ALL.iter().map(|t| t.0).collect::<Vec<_>>())),
            1..=3
        ).prop_map(Atom::Feats),
    ]
}

fn effect() -> impl Strategy<Value = Effect> {
    prop_oneof![
        1 => Just(Effect::Delete),
        3 => prop::collection::vec(seg(), 1..=2).prop_map(Effect::Become),
        3 => prop::collection::vec(
            prop::sample::select(Edit::ALL.iter().map(|e| e.0).collect::<Vec<_>>()),
            1..=3
        ).prop_map(Effect::Edit),
    ]
}

fn context() -> impl Strategy<Value = Context> {
    (
        any::<bool>(),
        prop::collection::vec(atom(), 0..=MAX_CONTEXT),
        prop::collection::vec(atom(), 0..=MAX_CONTEXT),
        any::<bool>(),
    )
        .prop_map(|(left_edge, left, right, right_edge)| Context {
            left_edge,
            left,
            right,
            right_edge,
        })
}

fn rule() -> impl Strategy<Value = Rule> {
    (atom(), effect(), context()).prop_map(|(target, effect, context)| Rule {
        target,
        effect,
        context,
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(600))]

    /// parse(print(r)) == r.
    #[test]
    fn rules_round_trip(r in rule()) {
        let printed = r.to_string();
        let back = Rule::parse(&printed);
        prop_assert_eq!(back.as_ref(), Ok(&r), "printed as {}", printed);
    }

    /// Printing is a fixed point: print(parse(print(r))) == print(r).
    #[test]
    fn printing_is_stable(r in rule()) {
        let once = r.to_string();
        let twice = Rule::parse(&once).map(|x| x.to_string());
        prop_assert_eq!(twice, Ok(once));
    }

    /// Words spell back to themselves.
    #[test]
    fn words_round_trip(w in word()) {
        prop_assert_eq!(phon::parse_segments(&phon::spell(&w)), Some(w));
    }

    /// Closure and growth (Verus theorem_closure_and_growth).
    #[test]
    fn raw_application_is_closed_and_bounded(r in rule(), w in word()) {
        let out = r.apply_raw(&w);
        prop_assert!(out.iter().all(|s| s.index() < COUNT));
        prop_assert!(out.len() <= 2 * w.len());
        let two = matches!(&r.effect, Effect::Become(v) if v.len() == 2);
        if !two {
            prop_assert!(out.len() <= w.len());
        }
    }

    /// The guard (Verus theorem_guard).
    #[test]
    fn the_guarded_step_keeps_words_valid(r in rule(), w in word()) {
        let out = Word(r.step(&w));
        prop_assert!(out.is_valid());
    }

    /// History (Verus theorem_history): any sequence of rules.
    #[test]
    fn no_history_breaks_a_word(rules in prop::collection::vec(rule(), 0..30), w in word()) {
        let out = rules.iter().fold(w, |acc, r| r.step(&acc));
        prop_assert!(Word(out).is_valid());
    }

    /// Where the rule does not fire, the segment is copied — and with a
    /// one-for-one effect, in place.
    #[test]
    fn non_firing_positions_are_untouched(r in rule(), w in word()) {
        if let Effect::Edit(_) = r.effect {
            let fires = r.firing(&w);
            let out = r.apply_raw(&w);
            prop_assert_eq!(out.len(), w.len());
            for i in 0..w.len() {
                if !fires[i] {
                    prop_assert_eq!(out[i], w[i]);
                }
            }
        }
    }

    /// Simultaneity: the result does not depend on scanning order. Applying
    /// to the reversed word with the mirrored rule gives the reversed result.
    #[test]
    fn application_is_order_free(r in rule(), w in word()) {
        let mirrored = Rule {
            target: r.target.clone(),
            effect: match &r.effect {
                Effect::Become(v) => Effect::Become(v.iter().rev().copied().collect()),
                e => e.clone(),
            },
            context: Context {
                left_edge: r.context.right_edge,
                left: r.context.right.iter().rev().cloned().collect(),
                right: r.context.left.iter().rev().cloned().collect(),
                right_edge: r.context.left_edge,
            },
        };
        let mut rev = w.clone();
        rev.reverse();
        let mut forward = r.apply_raw(&w);
        forward.reverse();
        prop_assert_eq!(mirrored.apply_raw(&rev), forward);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Every lexicon replays from the proto-language and the recorded
    /// history, with arbitrary hand rules mixed into the drift.
    #[test]
    fn worlds_replay(seed in "[a-z]{1,8}", centuries in 0u32..30, hands in prop::collection::vec((rule(), 0usize..12), 0..6)) {
        let mut w = World::new(&seed);
        for (i, (r, pick)) in hands.iter().enumerate() {
            for _ in 0..(centuries / 6) { w.advance(); }
            let spoken = w.spoken();
            let target = if i % 2 == 0 { None } else { spoken.get(pick % spoken.len().max(1)).copied() };
            let _ = w.apply_hand(&r.to_string(), target, 120);
        }
        for l in &w.langs {
            prop_assert_eq!(&w.replay(l.id), &l.lexicon);
            prop_assert!(l.lexicon.iter().all(Word::is_valid));
            prop_assert_eq!(l.lexicon.len(), w.vocab.len());
            if let Some(p) = l.parent {
                prop_assert!(p < l.id);
            }
        }
        // Lineage reaches the root (Verus theorem_lineage) within id steps.
        for l in &w.langs {
            let chain = w.lineage(l.id);
            prop_assert_eq!(chain.first(), Some(&LangId(0)));
            prop_assert!(chain.len() <= l.id.index() + 1);
        }
        // Etymologies are contiguous and end at the current form.
        for &id in &w.spoken() {
            for m in w.vocab.all() {
                let steps = w.etymology(id, m);
                let mut cur = w.proto[m.index()].clone();
                for s in &steps {
                    prop_assert_eq!(&s.before, &cur);
                    cur = s.after.clone();
                }
                prop_assert_eq!(&cur, &w.langs[id.index()].lexicon[m.index()]);
            }
        }
    }
}

fn command() -> impl Strategy<Value = Command> {
    prop_oneof![
        4 => Just(Command::Advance),
        1 => (rule(), prop::option::of(0u16..24), 0u16..360).prop_map(|(r, t, hue)| Command::Hand {
            rule: r.to_string(),
            target: t.map(LangId),
            hue,
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Durable replay: a world rebuilt from its seed and the commands that
    /// succeeded is the same world — same digest, same lexicons, same feed.
    #[test]
    fn a_command_log_replays_to_the_same_world(
        seed in "[a-z]{1,8}",
        cmds in prop::collection::vec(command(), 0..40),
        span in prop::option::of(40u32..70),
    ) {
        let vocab = Arc::new(Vocabulary::standard());
        let mut live = World::grown(&seed, Arc::clone(&vocab), span);
        let mut log = Vec::new();
        for c in &cmds {
            let before = live.digest();
            if live.run(c).is_ok() {
                log.push(c.clone());
            } else {
                prop_assert_eq!(live.digest(), before, "a failed command changed the world");
            }
        }
        let rebuilt = World::replay_commands(&seed, vocab, span, &log);
        prop_assert!(rebuilt.is_ok());
        let rebuilt = rebuilt.unwrap_or_else(|_| World::new("unreachable"));
        prop_assert_eq!(rebuilt.digest(), live.digest());
        prop_assert_eq!(rebuilt.century, live.century);
        prop_assert_eq!(&rebuilt.feed, &live.feed);
        for (a, b) in rebuilt.langs.iter().zip(&live.langs) {
            prop_assert_eq!(&a.lexicon, &b.lexicon);
        }
        if let Some(s) = span {
            prop_assert!(live.century <= s.max(32));
        }
    }

    /// A custom word list prints back to itself and grows a world whose
    /// fixed proto-forms are the ones given.
    #[test]
    fn custom_vocabularies_round_trip(
        glosses in prop::collection::btree_set("[a-z]{2,8}", 1..30),
        forms in prop::collection::vec(prop::collection::vec(seg(), 1..=5), 30),
    ) {
        let mut text = String::new();
        for (i, g) in glosses.iter().enumerate() {
            if i % 3 == 0 {
                let _ = writeln!(text, "{g} = {}", Word(forms[i].clone()));
            } else {
                let _ = writeln!(text, "{g}");
            }
        }
        let v = Vocabulary::parse(&text);
        prop_assert!(v.is_ok(), "{:?}", v);
        let v = v.unwrap_or_else(|_| Vocabulary::minimal());
        prop_assert_eq!(Vocabulary::parse(&v.to_text()), Ok(v.clone()));
        let w = World::grown("x", Arc::new(v.clone()), None);
        for (i, g) in glosses.iter().enumerate() {
            if i % 3 == 0 {
                let m = v.named(g);
                prop_assert!(m.is_some());
                prop_assert_eq!(m.map(|m| w.proto[m.index()].clone()), Some(Word(forms[i].clone())));
            }
        }
    }
}
