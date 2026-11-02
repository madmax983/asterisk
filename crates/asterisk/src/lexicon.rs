//! Vocabularies, words, and the invention of a proto-language.
//!
//! A [`Vocabulary`] is the list of meanings a family keeps a word for. It is
//! per world: the standard one is the Swadesh-207 list plus a few dozen extras
//! and a small grammar, and anyone can bring their own.
//!
//! Three kinds of entry:
//!
//! * a **word** (`moon`);
//! * a **suffix**, glossed in capitals Leipzig-style (`PL`, `PAST`, `GEN`);
//! * an **inflected form** (`foot.PL`), born as the base plus the suffix and
//!   from then on drifting as one word. So when i-umlaut fronts the vowel of
//!   `foot.PL` and apocope eats its ending, the family grows an irregular
//!   plural — *foot*, *feet* — that nobody wrote a rule for.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::VocabError;
use crate::phon::{self, Seg};
use crate::rng::Rng;
use crate::rule::MAX_LEN;

/// The Swadesh-207 list, in Swadesh's order.
pub const SWADESH: &[&str] = &[
    "I",
    "you",
    "he",
    "we",
    "you all",
    "they",
    "this",
    "that",
    "here",
    "there",
    "who",
    "what",
    "where",
    "when",
    "how",
    "not",
    "all",
    "many",
    "some",
    "few",
    "other",
    "one",
    "two",
    "three",
    "four",
    "five",
    "big",
    "long",
    "wide",
    "thick",
    "heavy",
    "small",
    "short",
    "narrow",
    "thin",
    "woman",
    "man",
    "person",
    "child",
    "wife",
    "husband",
    "mother",
    "father",
    "animal",
    "fish",
    "bird",
    "dog",
    "louse",
    "snake",
    "worm",
    "tree",
    "forest",
    "stick",
    "fruit",
    "seed",
    "leaf",
    "root",
    "bark",
    "flower",
    "grass",
    "rope",
    "skin",
    "meat",
    "blood",
    "bone",
    "fat",
    "egg",
    "horn",
    "tail",
    "feather",
    "hair",
    "head",
    "ear",
    "eye",
    "nose",
    "mouth",
    "tooth",
    "tongue",
    "fingernail",
    "foot",
    "leg",
    "knee",
    "hand",
    "wing",
    "belly",
    "guts",
    "neck",
    "back",
    "breast",
    "heart",
    "liver",
    "drink",
    "eat",
    "bite",
    "suck",
    "spit",
    "vomit",
    "blow",
    "breathe",
    "laugh",
    "see",
    "hear",
    "know",
    "think",
    "smell",
    "fear",
    "sleep",
    "live",
    "die",
    "kill",
    "fight",
    "hunt",
    "hit",
    "cut",
    "split",
    "stab",
    "scratch",
    "dig",
    "swim",
    "fly",
    "walk",
    "come",
    "lie",
    "sit",
    "stand",
    "turn",
    "fall",
    "give",
    "hold",
    "squeeze",
    "rub",
    "wash",
    "wipe",
    "pull",
    "push",
    "throw",
    "tie",
    "sew",
    "count",
    "say",
    "sing",
    "play",
    "float",
    "flow",
    "freeze",
    "swell",
    "sun",
    "moon",
    "star",
    "water",
    "rain",
    "river",
    "lake",
    "sea",
    "salt",
    "stone",
    "sand",
    "dust",
    "earth",
    "cloud",
    "fog",
    "sky",
    "wind",
    "snow",
    "ice",
    "smoke",
    "fire",
    "ash",
    "burn",
    "road",
    "mountain",
    "red",
    "green",
    "yellow",
    "white",
    "black",
    "night",
    "day",
    "year",
    "warm",
    "cold",
    "full",
    "new",
    "old",
    "good",
    "bad",
    "rotten",
    "dirty",
    "straight",
    "round",
    "sharp",
    "dull",
    "smooth",
    "wet",
    "dry",
    "correct",
    "near",
    "far",
    "right",
    "left",
    "at",
    "in",
    "with",
    "and",
    "if",
    "because",
    "name",
];

/// Words beyond Swadesh's list: what a saga needs.
pub const EXTRAS: &[&str] = &[
    "little", "honey", "wolf", "song", "house", "mouse", "goose", "ox", "horse", "king", "god",
    "bread", "milk", "wine", "iron", "gold", "ship", "sword", "spear", "six", "seven", "eight",
    "nine", "ten", "brother", "sister", "son", "daughter", "morning", "evening", "summer",
    "winter", "dream", "word",
];

/// The grammar: suffixes, and the forms born from them.
pub const GRAMMAR: &[&str] = &[
    "PL",
    "PAST",
    "GEN",
    "foot.PL",
    "tooth.PL",
    "man.PL",
    "woman.PL",
    "child.PL",
    "mouse.PL",
    "goose.PL",
    "ox.PL",
    "eye.PL",
    "hand.PL",
    "house.PL",
    "star.PL",
    "stone.PL",
    "bird.PL",
    "tree.PL",
    "eat.PAST",
    "drink.PAST",
    "sleep.PAST",
    "die.PAST",
    "come.PAST",
    "give.PAST",
    "see.PAST",
    "sing.PAST",
    "swim.PAST",
    "fly.PAST",
    "know.PAST",
    "say.PAST",
    "fall.PAST",
    "sit.PAST",
    "I.GEN",
    "you.GEN",
    "we.GEN",
    "he.GEN",
];

/// Every vocabulary keeps these: *tongue* names the language, *little* renews
/// worn-out words.
pub const REQUIRED: &[&str] = &["tongue", "little"];

/// Most entries a vocabulary may have.
pub const MAX_ENTRIES: usize = 600;
/// Longest gloss.
pub const MAX_GLOSS: usize = 32;

/// A meaning: an index into its world's [`Vocabulary`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Meaning(u16);

impl Meaning {
    /// The index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    fn at(i: usize) -> Option<Self> {
        u16::try_from(i).ok().map(Self)
    }
}

/// What an entry is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    /// A word.
    Word,
    /// A bound suffix.
    Suffix,
    /// A base with a suffix, fused at birth.
    Inflected {
        /// The base word.
        base: Meaning,
        /// The suffix.
        affix: Meaning,
    },
}

/// One meaning.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Entry {
    /// `moon`, `PL`, `foot.PL`.
    pub gloss: String,
    /// What it is.
    pub kind: Kind,
    /// A fixed proto-form, if the vocabulary gave one.
    pub proto: Option<Word>,
}

/// The meanings a family keeps a word for.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Vocabulary {
    entries: Vec<Entry>,
}

/// A suffix is glossed in two or more capitals (`PL`); a lone `I` is a word.
fn is_suffix_gloss(g: &str) -> bool {
    g.len() >= 2 && g.chars().all(|c| c.is_ascii_uppercase())
}

fn gloss_ok(g: &str) -> bool {
    !g.is_empty()
        && g.chars().count() <= MAX_GLOSS
        && g.chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '\'')
}

impl Vocabulary {
    /// Swadesh-207, the extras, and the grammar.
    pub fn standard() -> Self {
        let text: Vec<&str> = SWADESH
            .iter()
            .chain(EXTRAS)
            .chain(GRAMMAR)
            .copied()
            .collect();
        // The constant lists are checked by the tests; an error here is
        // impossible, and an empty fallback would still be a usable world.
        Self::parse(&text.join("\n")).unwrap_or_else(|_| Self::minimal())
    }

    /// Only the required words.
    pub fn minimal() -> Self {
        Self {
            entries: REQUIRED
                .iter()
                .map(|g| Entry {
                    gloss: (*g).to_owned(),
                    kind: Kind::Word,
                    proto: None,
                })
                .collect(),
        }
    }

    /// Parse a word list, one entry per line:
    ///
    /// ```text
    /// moon            a word
    /// moon = mena     a word with a fixed proto-form (IPA)
    /// PL = s          a suffix (glossed in two or more capitals)
    /// foot.PL         an inflected form: base . suffix
    /// # a comment
    /// ```
    ///
    /// Suffixes may be declared after the forms that use them. *tongue* and
    /// *little* are added if missing.
    pub fn parse(text: &str) -> Result<Self, VocabError> {
        let mut raw: Vec<(usize, String, Option<Word>)> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (gloss, proto) = match line.split_once('=') {
                Some((g, p)) => {
                    let w = Word::parse(p.trim())
                        .filter(Word::is_valid)
                        .ok_or_else(|| VocabError::BadForm(n + 1, p.trim().to_owned()))?;
                    (g.trim().to_owned(), Some(w))
                }
                None => (line.to_owned(), None),
            };
            raw.push((n + 1, gloss, proto));
        }
        for g in REQUIRED {
            if !raw.iter().any(|(_, x, _)| x == g) {
                raw.push((0, (*g).to_owned(), None));
            }
        }
        if raw.len() > MAX_ENTRIES {
            return Err(VocabError::TooMany(MAX_ENTRIES));
        }
        let mut entries: Vec<Entry> = Vec::with_capacity(raw.len());
        for (n, gloss, proto) in &raw {
            if entries.iter().any(|e| e.gloss == *gloss) {
                return Err(VocabError::Duplicate(*n, gloss.clone()));
            }
            let kind = if gloss.contains('.') {
                Kind::Inflected {
                    base: Meaning(0),
                    affix: Meaning(0),
                }
            } else if is_suffix_gloss(gloss) {
                Kind::Suffix
            } else if gloss_ok(gloss) {
                Kind::Word
            } else {
                return Err(VocabError::BadGloss(*n, gloss.clone()));
            };
            entries.push(Entry {
                gloss: gloss.clone(),
                kind,
                proto: proto.clone(),
            });
        }
        // Second pass: resolve inflected forms now that every entry exists.
        let index = |g: &str| entries.iter().position(|e| e.gloss == g);
        let mut resolved = Vec::with_capacity(entries.len());
        for (i, e) in entries.iter().enumerate() {
            let n = raw.get(i).map_or(0, |r| r.0);
            let kind = match e.kind {
                Kind::Inflected { .. } => {
                    let (b, a) = e
                        .gloss
                        .split_once('.')
                        .ok_or_else(|| VocabError::BadGloss(n, e.gloss.clone()))?;
                    let base = index(b)
                        .filter(|&j| entries.get(j).is_some_and(|x| x.kind == Kind::Word))
                        .ok_or_else(|| VocabError::NoBase(n, b.to_owned()))?;
                    let affix = index(a)
                        .filter(|&j| entries.get(j).is_some_and(|x| x.kind == Kind::Suffix))
                        .ok_or_else(|| VocabError::NoSuffix(n, a.to_owned()))?;
                    Kind::Inflected {
                        base: Meaning::at(base).ok_or(VocabError::TooMany(MAX_ENTRIES))?,
                        affix: Meaning::at(affix).ok_or(VocabError::TooMany(MAX_ENTRIES))?,
                    }
                }
                k => k,
            };
            resolved.push(Entry {
                gloss: e.gloss.clone(),
                kind,
                proto: e.proto.clone(),
            });
        }
        Ok(Self { entries: resolved })
    }

    /// The list in the syntax [`Vocabulary::parse`] reads.
    pub fn to_text(&self) -> String {
        self.entries
            .iter()
            .map(|e| {
                e.proto
                    .as_ref()
                    .map_or_else(|| e.gloss.clone(), |w| format!("{} = {w}", e.gloss))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// How many meanings.
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Never true: the required words are always there.
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every meaning, in order.
    pub fn all(&self) -> impl Iterator<Item = Meaning> + '_ {
        (0..self.entries.len()).filter_map(Meaning::at)
    }

    /// One entry.
    pub fn entry(&self, m: Meaning) -> Option<&Entry> {
        self.entries.get(m.index())
    }

    /// The gloss.
    pub fn gloss(&self, m: Meaning) -> &str {
        self.entry(m).map_or("?", |e| e.gloss.as_str())
    }

    /// The kind.
    pub fn kind(&self, m: Meaning) -> Kind {
        self.entry(m).map_or(Kind::Word, |e| e.kind)
    }

    /// By gloss.
    pub fn named(&self, gloss: &str) -> Option<Meaning> {
        self.entries
            .iter()
            .position(|e| e.gloss == gloss)
            .and_then(Meaning::at)
    }

    /// The word that names a language.
    pub fn tongue(&self) -> Meaning {
        self.named("tongue").unwrap_or(Meaning(0))
    }

    /// The word a language reaches for when this one wears away: compounds
    /// with a real history behind them (*eye of day* is the Malay word for
    /// sun), and otherwise *little*, the diminutive — which is how Latin
    /// *sol* became French *soleil*.
    pub fn partner(&self, m: Meaning) -> Meaning {
        let p = match self.gloss(m) {
            "sun" => "day",
            "moon" | "star" => "night",
            "sea" | "river" | "rain" | "fish" | "lake" => "water",
            "tooth" | "horn" => "bone",
            "tongue" => "mouth",
            "egg" | "feather" | "wing" => "bird",
            "tree" | "flower" => "leaf",
            "little" => "child",
            "snow" | "ice" => "cold",
            "fog" | "cloud" => "sky",
            _ => "little",
        };
        self.named(p).filter(|&x| x != m).unwrap_or_else(|| {
            self.named("little")
                .filter(|&x| x != m)
                .unwrap_or_else(|| self.tongue())
        })
    }

    /// The inflected forms, and their base and suffix.
    pub fn paradigm_entries(&self) -> impl Iterator<Item = (Meaning, Meaning, Meaning)> + '_ {
        self.all().filter_map(|m| match self.kind(m) {
            Kind::Inflected { base, affix } => Some((m, base, affix)),
            _ => None,
        })
    }
}

/// A word: `1..=MAX_LEN` segments.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Word(pub Vec<Seg>);

impl Word {
    /// The segments.
    pub fn segs(&self) -> &[Seg] {
        &self.0
    }

    /// The invariant every word in every language keeps for ever.
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= MAX_LEN
            && self.0.iter().all(|s| s.index() < phon::COUNT)
    }

    /// From IPA.
    pub fn parse(s: &str) -> Option<Self> {
        phon::parse_segments(s).map(Self)
    }

    /// `self` with `suffix` after it, if that is still a word.
    pub fn suffixed(&self, suffix: &Self) -> Option<Self> {
        let mut v = self.0.clone();
        v.extend_from_slice(&suffix.0);
        let w = Self(v);
        w.is_valid().then_some(w)
    }
}

impl fmt::Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&phon::spell(&self.0))
    }
}

/// A lexicon: one word per meaning, in vocabulary order.
pub type Lexicon = Vec<Word>;

fn segs(glyphs: &[&str]) -> Vec<Seg> {
    glyphs
        .iter()
        .filter_map(|g| phon::parse_segments(g))
        .filter(|v| v.len() == 1)
        .map(|v| v[0])
        .collect()
}

/// What a proto-language's words are built from.
#[derive(Clone, Debug)]
pub struct Phonotactics {
    /// Consonants that can start a syllable.
    pub onsets: Vec<Seg>,
    /// Consonants that can end one.
    pub codas: Vec<Seg>,
    /// Second members of onset clusters (liquids, glides).
    pub glides: Vec<Seg>,
    /// Vowels, short and (sometimes) long.
    pub vowels: Vec<Seg>,
}

impl Phonotactics {
    /// Invent a sound system: a core every family has, plus a random handful.
    pub fn invent(rng: &mut Rng) -> Self {
        let mut cons = segs(&["p", "t", "k", "m", "n", "s"]);
        let mut pool = segs(&[
            "b", "d", "g", "l", "r", "j", "w", "h", "q", "ʔ", "x", "f", "ts", "ŋ", "z", "ʃ", "tʃ",
            "ɲ", "θ", "ɣ",
        ]);
        let extra = 5 + rng.below(5);
        for _ in 0..extra {
            if pool.is_empty() {
                break;
            }
            cons.push(pool.swap_remove(rng.below(pool.len())));
        }
        let mut vowels = segs(&["i", "e", "a", "o", "u"]);
        for (g, p) in [("ə", 350), ("ɛ", 250), ("ɔ", 250), ("y", 120), ("æ", 120)] {
            if rng.chance(p) {
                vowels.extend(segs(&[g]));
            }
        }
        if rng.chance(500) {
            // Collected first: `vowels` is borrowed by the map and extended after.
            #[allow(clippy::needless_collect)]
            let longs: Vec<Seg> = vowels
                .iter()
                .map(|&v| phon::Edit::Lengthen.apply(v))
                .collect();
            vowels.extend(longs.into_iter().take(3 + rng.below(3)));
        }
        let codas: Vec<Seg> = cons
            .iter()
            .copied()
            .filter(|s| ["m", "n", "s", "r", "l", "k", "t", "ŋ", "h", "q"].contains(&s.glyph()))
            .collect();
        let glides: Vec<Seg> = cons
            .iter()
            .copied()
            .filter(|s| ["l", "r", "j", "w"].contains(&s.glyph()))
            .collect();
        Self {
            onsets: cons,
            codas,
            glides,
            vowels,
        }
    }

    fn syllable(&self, rng: &mut Rng, first: bool, out: &mut Vec<Seg>) {
        // Shapes: V, CV, CVC, CCV. Vowel-initial syllables only begin words.
        let w = [
            if first { 12 } else { 0 },
            55,
            24,
            if self.glides.is_empty() { 0 } else { 9 },
        ];
        let shape = rng.weighted(&w);
        if shape != 0
            && let Some(&c) = rng.pick(&self.onsets)
        {
            out.push(c);
        }
        if shape == 3
            && let Some(&g) = rng.pick(&self.glides)
            && out.last() != Some(&g)
        {
            out.push(g);
        }
        if let Some(&v) = rng.pick(&self.vowels) {
            out.push(v);
        }
        if shape == 2
            && let Some(&c) = rng.pick(&self.codas)
        {
            out.push(c);
        }
    }

    /// One word of one to three syllables.
    pub fn word(&self, rng: &mut Rng) -> Word {
        let syllables = 1 + rng.weighted(&[4, 7, 2]);
        let mut out = Vec::new();
        for i in 0..syllables {
            self.syllable(rng, i == 0, &mut out);
        }
        out.truncate(7);
        if out.is_empty() {
            out.extend(segs(&["a"]));
        }
        Word(out)
    }

    /// A suffix: one light syllable — V, VC or CV.
    pub fn suffix(&self, rng: &mut Rng) -> Word {
        let mut out = Vec::new();
        let shape = rng.weighted(&[3, 3, 4]);
        if shape == 2
            && let Some(&c) = rng.pick(&self.onsets)
        {
            out.push(c);
        }
        if let Some(&v) = rng.pick(&self.vowels) {
            out.push(v);
        }
        if shape == 1
            && let Some(&c) = rng.pick(&self.codas)
        {
            out.push(c);
        }
        if out.is_empty() {
            out.extend(segs(&["a"]));
        }
        Word(out)
    }
}

/// Invent a proto-lexicon for a vocabulary: one distinct word per word,
/// one light syllable per suffix, and each inflected form fused from its
/// base and suffix. Fixed proto-forms are kept as given.
pub fn invent(rng: &mut Rng, vocab: &Vocabulary) -> Lexicon {
    let ph = Phonotactics::invent(rng);
    let mut lex: Lexicon = Vec::with_capacity(vocab.len());
    for m in vocab.all() {
        let Some(e) = vocab.entry(m) else { continue };
        let w = match (&e.proto, e.kind) {
            (Some(w), _) => w.clone(),
            (None, Kind::Suffix) => ph.suffix(rng),
            (None, Kind::Inflected { .. }) => Word(Vec::new()),
            (None, Kind::Word) => {
                let mut w = ph.word(rng);
                let mut tries = 0;
                while lex.contains(&w) && tries < 20 {
                    w = ph.word(rng);
                    tries += 1;
                }
                w
            }
        };
        lex.push(w);
    }
    for (m, base, affix) in vocab.paradigm_entries() {
        if vocab.entry(m).is_some_and(|e| e.proto.is_some()) {
            continue;
        }
        let fused = match (lex.get(base.index()), lex.get(affix.index())) {
            (Some(b), Some(a)) => b.suffixed(a).unwrap_or_else(|| b.clone()),
            (Some(b), None) => b.clone(),
            _ => Word(segs(&["a"])),
        };
        if let Some(slot) = lex.get_mut(m.index()) {
            *slot = fused;
        }
    }
    lex
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Seed;

    #[test]
    fn the_standard_vocabulary_parses_with_its_grammar() {
        assert_eq!(SWADESH.len(), 207);
        let v = Vocabulary::standard();
        assert_eq!(v.len(), SWADESH.len() + EXTRAS.len() + GRAMMAR.len());
        assert_eq!(Vocabulary::parse(&v.to_text()), Ok(v.clone()));
        let feet = v.named("foot.PL").map(|m| v.kind(m));
        assert!(matches!(feet, Some(Kind::Inflected { .. })));
        assert_eq!(v.named("PL").map(|m| v.kind(m)), Some(Kind::Suffix));
        assert!(v.paradigm_entries().count() >= 30);
    }

    #[test]
    fn every_meaning_has_a_partner_other_than_itself() {
        let v = Vocabulary::standard();
        for m in v.all() {
            assert_ne!(v.partner(m), m, "{}", v.gloss(m));
        }
        assert_eq!(v.named("sun").map(|m| v.partner(m)), v.named("day"));
    }

    #[test]
    fn a_custom_list_reads_its_own_syntax() {
        let v = Vocabulary::parse("# my conlang\nmoon = mena\nstar\nPL = s\nstar.PL\n")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v.len(), 6, "tongue and little are added");
        let lex = invent(&mut Rng::new(Seed(1)), &v);
        let g = |s: &str| lex[v.named(s).map_or(0, Meaning::index)].to_string();
        assert_eq!(g("moon"), "mena");
        assert_eq!(g("PL"), "s");
        assert_eq!(g("star.PL"), format!("{}s", g("star")));
    }

    #[test]
    fn bad_lists_say_which_line() {
        assert_eq!(
            Vocabulary::parse("moon\nmoon"),
            Err(VocabError::Duplicate(2, "moon".into()))
        );
        assert_eq!(
            Vocabulary::parse("star.PL"),
            Err(VocabError::NoBase(1, "star".into()))
        );
        assert_eq!(
            Vocabulary::parse("star\nstar.PL"),
            Err(VocabError::NoSuffix(2, "PL".into()))
        );
        assert!(matches!(
            Vocabulary::parse("moon = ???"),
            Err(VocabError::BadForm(1, _))
        ));
        assert!(matches!(
            Vocabulary::parse("m<o>on"),
            Err(VocabError::BadGloss(1, _))
        ));
    }

    #[test]
    fn proto_words_are_valid_and_mostly_distinct() {
        let v = Vocabulary::standard();
        for seed in 0..20 {
            let lex = invent(&mut Rng::new(Seed(seed)), &v);
            assert_eq!(lex.len(), v.len());
            assert!(lex.iter().all(Word::is_valid), "seed {seed}");
        }
    }

    #[test]
    fn inflected_forms_are_born_fused() {
        let v = Vocabulary::standard();
        let lex = invent(&mut Rng::new(Seed(5)), &v);
        for (m, b, a) in v.paradigm_entries() {
            let born = lex[b.index()]
                .suffixed(&lex[a.index()])
                .unwrap_or_else(|| lex[b.index()].clone());
            assert_eq!(lex[m.index()], born, "{}", v.gloss(m));
        }
    }

    #[test]
    fn the_same_seed_invents_the_same_language() {
        let v = Vocabulary::standard();
        assert_eq!(
            invent(&mut Rng::new(Seed(9)), &v),
            invent(&mut Rng::new(Seed(9)), &v)
        );
        assert_ne!(
            invent(&mut Rng::new(Seed(9)), &v),
            invent(&mut Rng::new(Seed(10)), &v)
        );
    }
}
