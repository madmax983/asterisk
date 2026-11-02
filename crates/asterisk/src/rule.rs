//! The sound-change language.
//!
//! ```text
//! rule    := atom ">" result [ "/" context ]
//! result  := "∅" | "0" | seg [seg] | "[" edit+ "]"
//! context := ["#"] atom{0,2} "_" atom{0,2} ["#"]
//! atom    := seg | class | "[" test+ "]"
//! class   := "C" | "V" | "P" | "F" | "N" | "L" | "G"
//! ```
//!
//! A rule applies **simultaneously**: every position of the input word is
//! tested against the *input* word, and the output is the concatenation of
//! what every position becomes. So `V > ∅ / C_#` deletes one final vowel, not
//! a cascade of them, and the order the scanner runs in cannot matter.
//!
//! Each position becomes zero, one, or two segments, so a rule is total,
//! terminates in one pass, and the output is at most twice as long as the
//! input. The guarded step ([`Rule::apply`]) then refuses outputs that would
//! leave a word empty or longer than [`MAX_LEN`]: that is the invariant the
//! Verus model proves survives any history.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::RuleError;
use crate::phon::{self, Back, Edit, Feat, Height, Manner, Place, Seg};

/// No word ever grows past this many segments.
pub const MAX_LEN: usize = 12;
/// At most this many atoms on each side of the `_`.
pub const MAX_CONTEXT: usize = 2;

/// A named natural class.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Class {
    /// `C` — any consonant.
    C,
    /// `V` — any vowel.
    V,
    /// `P` — stops (plosives).
    P,
    /// `F` — fricatives.
    F,
    /// `N` — nasal consonants.
    N,
    /// `L` — liquids.
    L,
    /// `G` — glides.
    G,
}

impl Class {
    /// Every class with its letter.
    pub const ALL: &'static [(Self, char)] = &[
        (Self::C, 'C'),
        (Self::V, 'V'),
        (Self::P, 'P'),
        (Self::F, 'F'),
        (Self::N, 'N'),
        (Self::L, 'L'),
        (Self::G, 'G'),
    ];

    /// The letter.
    pub fn letter(self) -> char {
        Self::ALL
            .iter()
            .find(|(c, _)| *c == self)
            .map_or('?', |(_, l)| *l)
    }

    /// Does `s` belong?
    pub fn contains(self, s: Seg) -> bool {
        match (self, s.feat()) {
            (Self::V, Feat::Vowel { .. }) | (Self::C, Feat::Cons { .. }) => true,
            (Self::P, Feat::Cons { manner, .. }) => manner == Manner::Stop,
            (Self::F, Feat::Cons { manner, .. }) => manner == Manner::Fricative,
            (Self::N, Feat::Cons { manner, .. }) => manner == Manner::Nasal,
            (Self::L, Feat::Cons { manner, .. }) => {
                matches!(manner, Manner::Lateral | Manner::Rhotic)
            }
            (Self::G, Feat::Cons { manner, .. }) => manner == Manner::Glide,
            _ => false,
        }
    }
}

/// A binary feature a bracketed atom can test.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Test {
    /// Voiced (vowels are voiced).
    Voice,
    /// Sonorant: vowels, nasals, liquids, glides.
    Son,
    /// Stop.
    Stop,
    /// Fricative.
    Cont,
    /// Labial or labiodental.
    Lab,
    /// A long vowel.
    Long,
    /// A nasal vowel or nasal consonant.
    Nasal,
    /// A rounded vowel (and w).
    Round,
    /// A front vowel.
    Front,
    /// A back vowel.
    Back,
    /// A high vowel.
    High,
    /// A low vowel.
    Low,
}

impl Test {
    /// Every test with its name.
    pub const ALL: &'static [(Self, &'static str)] = &[
        (Self::Voice, "voice"),
        (Self::Son, "son"),
        (Self::Stop, "stop"),
        (Self::Cont, "cont"),
        (Self::Lab, "lab"),
        (Self::Long, "long"),
        (Self::Nasal, "nasal"),
        (Self::Round, "round"),
        (Self::Front, "front"),
        (Self::Back, "back"),
        (Self::High, "high"),
        (Self::Low, "low"),
    ];

    /// The name.
    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(t, _)| *t == self)
            .map_or("?", |(_, n)| n)
    }

    /// Does `s` have the feature?
    pub fn holds(self, s: Seg) -> bool {
        match s.feat() {
            Feat::Cons {
                place,
                manner,
                voiced,
            } => match self {
                Self::Voice => voiced,
                Self::Son => matches!(
                    manner,
                    Manner::Nasal | Manner::Lateral | Manner::Rhotic | Manner::Glide
                ),
                Self::Stop => manner == Manner::Stop,
                Self::Cont => manner == Manner::Fricative,
                Self::Lab => matches!(place, Place::Labial | Place::LabioDental),
                Self::Nasal => manner == Manner::Nasal,
                Self::Round => place == Place::Labial && manner == Manner::Glide,
                _ => false,
            },
            Feat::Vowel {
                height,
                back,
                round,
                long,
                nasal,
            } => match self {
                Self::Voice | Self::Son => true,
                Self::Long => long,
                Self::Nasal => nasal,
                Self::Round => round,
                Self::Front => back == Back::Front,
                Self::Back => back == Back::Back,
                Self::High => height == Height::High,
                Self::Low => height == Height::Low,
                _ => false,
            },
        }
    }
}

/// Something that matches one segment.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Atom {
    /// Exactly this segment.
    Seg(Seg),
    /// Any member of a class.
    Class(Class),
    /// Every test in the bracket, `+` meaning has and `-` meaning lacks.
    Feats(Vec<(bool, Test)>),
}

impl Atom {
    /// Does `s` match?
    pub fn matches(&self, s: Seg) -> bool {
        match self {
            Self::Seg(t) => *t == s,
            Self::Class(c) => c.contains(s),
            Self::Feats(tests) => tests.iter().all(|&(plus, t)| t.holds(s) == plus),
        }
    }
}

/// What the target becomes.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Effect {
    /// `∅` — it is lost.
    Delete,
    /// One or two literal segments.
    Become(Vec<Seg>),
    /// A sequence of feature edits applied to the target itself.
    Edit(Vec<Edit>),
}

impl Effect {
    /// What one matching segment becomes. Length 0, 1 or 2 — never more.
    pub fn on(&self, s: Seg) -> Vec<Seg> {
        match self {
            Self::Delete => Vec::new(),
            Self::Become(v) => v.iter().copied().take(2).collect(),
            Self::Edit(edits) => vec![edits.iter().fold(s, |acc, e| e.apply(acc))],
        }
    }
}

/// The environment: what must stand to the left and right of the target.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct Context {
    /// `#` before the left atoms: the left atoms start the word.
    pub left_edge: bool,
    /// Atoms immediately to the left, in order.
    pub left: Vec<Atom>,
    /// Atoms immediately to the right, in order.
    pub right: Vec<Atom>,
    /// `#` after the right atoms: the right atoms end the word.
    pub right_edge: bool,
}

impl Context {
    /// Is this the empty context (`_`, applies everywhere)?
    pub const fn is_empty(&self) -> bool {
        !self.left_edge && !self.right_edge && self.left.is_empty() && self.right.is_empty()
    }
}

/// One sound change.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Rule {
    /// Which segment changes.
    pub target: Atom,
    /// What it becomes.
    pub effect: Effect,
    /// Where.
    pub context: Context,
}

impl Rule {
    /// Does the rule fire at position `i` of `w`? Tested against `w` itself.
    pub fn fires(&self, word: &[Seg], at: usize) -> bool {
        let Some(&here) = word.get(at) else {
            return false;
        };
        if !self.target.matches(here) {
            return false;
        }
        let ctx = &self.context;
        let Some(start) = at.checked_sub(ctx.left.len()) else {
            return false;
        };
        if ctx.left_edge && start != 0 {
            return false;
        }
        if !ctx
            .left
            .iter()
            .zip(&word[start..at])
            .all(|(a, &x)| a.matches(x))
        {
            return false;
        }
        let end = at + 1 + ctx.right.len();
        if end > word.len() || (ctx.right_edge && end != word.len()) {
            return false;
        }
        ctx.right
            .iter()
            .zip(&word[at + 1..end])
            .all(|(a, &x)| a.matches(x))
    }

    /// Where the rule fires, one flag per position.
    pub fn firing(&self, w: &[Seg]) -> Vec<bool> {
        (0..w.len()).map(|i| self.fires(w, i)).collect()
    }

    /// The unguarded simultaneous application: every position replaced by its
    /// image (itself when the rule does not fire there).
    pub fn apply_raw(&self, w: &[Seg]) -> Vec<Seg> {
        let fires = self.firing(w);
        let mut out = Vec::with_capacity(w.len() * 2);
        for (&s, &f) in w.iter().zip(&fires) {
            if f {
                out.extend(self.effect.on(s));
            } else {
                out.push(s);
            }
        }
        out
    }

    /// The guarded step: `Some(new)` if the rule changes `w` and the result is
    /// still a word (`1..=MAX_LEN` segments); `None` if it changes nothing or
    /// would wreck the word, in which case the word keeps its old form.
    pub fn apply(&self, w: &[Seg]) -> Option<Vec<Seg>> {
        let out = self.apply_raw(w);
        if out.as_slice() == w || out.is_empty() || out.len() > MAX_LEN {
            None
        } else {
            Some(out)
        }
    }

    /// The guarded step as a total function.
    pub fn step(&self, w: &[Seg]) -> Vec<Seg> {
        self.apply(w).unwrap_or_else(|| w.to_vec())
    }

    /// Parse one rule.
    pub fn parse(src: &str) -> Result<Self, RuleError> {
        Parser::new(src).rule()
    }
}

/// Apply a chain of rules in order, each one guarded.
pub fn step_chain(rules: &[Rule], w: &[Seg]) -> Vec<Seg> {
    rules.iter().fold(w.to_vec(), |acc, r| r.step(&acc))
}

/// Parse several rules separated by newlines or `;`.
pub fn parse_chain(src: &str) -> Result<Vec<Rule>, RuleError> {
    let rules = src
        .split(['\n', ';'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(Rule::parse)
        .collect::<Result<Vec<_>, _>>()?;
    if rules.is_empty() {
        return Err(RuleError::Empty);
    }
    Ok(rules)
}

// ── Printing ────────────────────────────────────────────────────────────────

fn write_atoms(f: &mut fmt::Formatter<'_>, atoms: &[Atom]) -> fmt::Result {
    for (i, a) in atoms.iter().enumerate() {
        if i > 0
            && let (Atom::Seg(x), Atom::Seg(y)) = (&atoms[i - 1], a)
            && phon::needs_separator(*x, *y)
        {
            f.write_str(".")?;
        }
        write!(f, "{a}")?;
    }
    Ok(())
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seg(s) => f.write_str(s.glyph()),
            Self::Class(c) => write!(f, "{}", c.letter()),
            Self::Feats(tests) => {
                f.write_str("[")?;
                for (i, (plus, t)) in tests.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{}{}", if *plus { '+' } else { '-' }, t.name())?;
                }
                f.write_str("]")
            }
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Delete => f.write_str("∅"),
            Self::Become(v) => f.write_str(&phon::spell(v)),
            Self::Edit(edits) => {
                f.write_str("[")?;
                for (i, e) in edits.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    f.write_str(e.token())?;
                }
                f.write_str("]")
            }
        }
    }
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.left_edge {
            f.write_str("#")?;
        }
        write_atoms(f, &self.left)?;
        f.write_str("_")?;
        write_atoms(f, &self.right)?;
        if self.right_edge {
            f.write_str("#")?;
        }
        Ok(())
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} > {}", self.target, self.effect)?;
        if !self.context.is_empty() {
            write!(f, " / {}", self.context)?;
        }
        Ok(())
    }
}

// ── Parsing ─────────────────────────────────────────────────────────────────

struct Parser {
    src: String,
    pos: usize,
}

impl Parser {
    fn new(src: &str) -> Self {
        Self {
            src: phon::normalise(src),
            pos: 0,
        }
    }

    fn rest(&self) -> &str {
        &self.src[self.pos..]
    }

    /// Skip whitespace and the `.` separator.
    fn skip(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if c.is_whitespace() || c == '.' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip();
        self.rest().chars().next()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn err(&self, what: &str) -> RuleError {
        let found = self
            .rest()
            .chars()
            .next()
            .map_or_else(|| "the end of the rule".to_owned(), |c| format!("‘{c}’"));
        RuleError::Syntax {
            at: self.src[..self.pos].chars().count(),
            expected: what.to_owned(),
            found,
        }
    }

    fn rule(&mut self) -> Result<Rule, RuleError> {
        let target = self
            .atom()?
            .ok_or_else(|| self.err("a segment, a class or a [feature]"))?;
        if !self.eat('>') {
            return Err(self.err("‘>’"));
        }
        let effect = self.effect()?;
        let context = if self.eat('/') {
            self.context()?
        } else {
            Context::default()
        };
        if self.peek().is_some() {
            return Err(self.err("the end of the rule"));
        }
        Ok(Rule {
            target,
            effect,
            context,
        })
    }

    fn effect(&mut self) -> Result<Effect, RuleError> {
        if self.eat('∅') || self.eat('0') {
            return Ok(Effect::Delete);
        }
        if self.eat('[') {
            let mut edits = Vec::new();
            loop {
                if self.eat(']') {
                    break;
                }
                let word = self
                    .signed_word()
                    .ok_or_else(|| self.err("an edit like +voice"))?;
                let e = Edit::from_token(&word).ok_or(RuleError::UnknownEdit(word))?;
                edits.push(e);
            }
            if edits.is_empty() {
                return Err(RuleError::EmptyBracket);
            }
            return Ok(Effect::Edit(edits));
        }
        let mut segs = Vec::new();
        while segs.len() < 2 {
            match self.seg() {
                Some(s) => segs.push(s),
                None => break,
            }
        }
        if segs.is_empty() {
            return Err(self.err("∅, one or two segments, or [edits]"));
        }
        Ok(Effect::Become(segs))
    }

    fn context(&mut self) -> Result<Context, RuleError> {
        let mut c = Context {
            left_edge: self.eat('#'),
            ..Context::default()
        };
        while let Some(a) = self.atom()? {
            c.left.push(a);
        }
        if !self.eat('_') {
            return Err(self.err("‘_’ to mark where the target stands"));
        }
        while let Some(a) = self.atom()? {
            c.right.push(a);
        }
        c.right_edge = self.eat('#');
        if c.left.len() > MAX_CONTEXT || c.right.len() > MAX_CONTEXT {
            return Err(RuleError::ContextTooLong(MAX_CONTEXT));
        }
        Ok(c)
    }

    /// An atom, or `None` if the next thing is not one.
    fn atom(&mut self) -> Result<Option<Atom>, RuleError> {
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        if let Some(&(class, _)) = Class::ALL.iter().find(|(_, l)| *l == c) {
            self.pos += c.len_utf8();
            return Ok(Some(Atom::Class(class)));
        }
        if c == '[' {
            self.pos += 1;
            let mut tests = Vec::new();
            loop {
                if self.eat(']') {
                    break;
                }
                let word = self
                    .signed_word()
                    .ok_or_else(|| self.err("a feature like +voice"))?;
                let (plus, name) = word.split_at(1);
                let t = Test::ALL
                    .iter()
                    .find(|(_, n)| *n == name)
                    .map(|(t, _)| *t)
                    .ok_or_else(|| RuleError::UnknownFeature(word.clone()))?;
                tests.push((plus == "+", t));
            }
            if tests.is_empty() {
                return Err(RuleError::EmptyBracket);
            }
            return Ok(Some(Atom::Feats(tests)));
        }
        Ok(self.seg().map(Atom::Seg))
    }

    fn seg(&mut self) -> Option<Seg> {
        self.skip();
        let (s, n) = phon::longest_prefix(self.rest())?;
        self.pos += n;
        Some(s)
    }

    /// `+word` or `-word`.
    fn signed_word(&mut self) -> Option<String> {
        let sign = self.peek().filter(|c| *c == '+' || *c == '-')?;
        self.pos += 1;
        let word: String = self
            .rest()
            .chars()
            .take_while(char::is_ascii_lowercase)
            .collect();
        if word.is_empty() {
            return None;
        }
        self.pos += word.len();
        Some(format!("{sign}{word}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<Seg> {
        phon::parse_segments(s).unwrap_or_default()
    }

    fn r(s: &str) -> Rule {
        match Rule::parse(s) {
            Ok(r) => r,
            Err(e) => panic!("{s}: {e}"),
        }
    }

    #[test]
    fn grimm_initial_p() {
        let rule = r("p > f / #_");
        assert_eq!(rule.step(&w("pater")), w("fater"));
        assert_eq!(rule.step(&w("apa")), w("apa"));
    }

    #[test]
    fn apocope_takes_one_vowel_not_a_cascade() {
        let rule = r("V > ∅ / _#");
        assert_eq!(rule.step(&w("kaia")), w("kai"));
    }

    #[test]
    fn simultaneous_means_contexts_read_the_input() {
        // Every vowel between consonants deletes — judged on the input, so
        // deleting one does not stop or start its neighbours.
        let rule = r("V > ∅ / C_C");
        assert_eq!(rule.step(&w("tatata")), w("ttta"));
    }

    #[test]
    fn lenition_by_class_and_edit() {
        let rule = r("P > [+cont] / V_V");
        assert_eq!(rule.step(&w("akata")), w("axaθa"));
    }

    #[test]
    fn palatalisation_before_front_vowels() {
        let rule = r("k > tʃ / _[+front]");
        assert_eq!(rule.step(&w("kika")), w("tʃika"));
    }

    #[test]
    fn breaking_grows_a_word() {
        let rule = r("ɛ > jɛ");
        assert_eq!(rule.step(&w("tɛra")), w("tjɛra"));
    }

    #[test]
    fn umlaut() {
        let rule = r("V > [+front] / _Ci");
        assert_eq!(rule.step(&w("musi")), w("mysi"));
        assert_eq!(rule.step(&w("fotiz")), w("føtiz"));
    }

    #[test]
    fn the_guard_keeps_words_alive() {
        let rule = r("V > ∅");
        assert_eq!(rule.apply(&w("a")), None);
        assert_eq!(rule.step(&w("a")), w("a"));
        let grow = r("a > aa");
        let long = w("aaaaaaaa");
        assert_eq!(grow.apply(&long), None, "would be 16 > MAX_LEN");
    }

    #[test]
    fn unchanged_is_none() {
        assert_eq!(r("q > k").apply(&w("tata")), None);
    }

    #[test]
    fn edges_and_two_atom_contexts() {
        let rule = r("s > h / #_V");
        assert_eq!(rule.step(&w("sasa")), w("hasa"));
        let rule = r("V > [+long] / _CV#");
        assert_eq!(rule.step(&w("pata")), w("paːta"));
        assert_eq!(rule.step(&w("patak")), w("patak"));
    }

    #[test]
    fn printing_is_canonical() {
        assert_eq!(r("p>f/#_").to_string(), "p > f / #_");
        assert_eq!(r("V>0/C_#").to_string(), "V > ∅ / C_#");
        assert_eq!(
            r("[-voice +stop] > [+voice] / V_V").to_string(),
            "[-voice +stop] > [+voice] / V_V"
        );
        assert_eq!(r("q > k").to_string(), "q > k");
        assert_eq!(r("a: > o:").to_string(), "aː > oː");
    }

    #[test]
    fn printing_separates_glyphs_that_would_fuse() {
        let rule = r("a > i / t.ʃ_");
        let printed = rule.to_string();
        assert_eq!(printed, "a > i / t.ʃ_");
        assert_eq!(r(&printed), rule);
    }

    #[test]
    fn errors_say_where() {
        let e = Rule::parse("p f")
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(e.contains("‘>’"), "{e}");
        assert!(matches!(
            Rule::parse("p > [+bogus]"),
            Err(RuleError::UnknownEdit(_))
        ));
        assert!(matches!(
            Rule::parse("[+wet] > p"),
            Err(RuleError::UnknownFeature(_))
        ));
        assert!(matches!(
            Rule::parse("p > f / VVV_"),
            Err(RuleError::ContextTooLong(2))
        ));
        assert!(Rule::parse("p > f / V").is_err());
        assert!(Rule::parse("p > f g h").is_err());
        assert!(matches!(parse_chain(" ; "), Err(RuleError::Empty)));
    }

    #[test]
    fn chains_apply_in_order() {
        let grimm =
            parse_chain("[+stop -voice] > [+cont]; [+stop +voice] > [-voice]").unwrap_or_default();
        assert_eq!(step_chain(&grimm, &w("bapa")), w("pafa"));
    }
}
