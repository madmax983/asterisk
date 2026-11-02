//! A family of languages and the centuries that happen to it.
//!
//! The world is a tree. Every language but the first has a parent with a
//! smaller id, so following parents always reaches the proto-language (the
//! Verus model proves this for `parent(k) < k`). A language that splits stops
//! changing; its two daughters start from its lexicon. A language that falls
//! silent stops changing too, but stays on the tree.
//!
//! Every change to every lexicon is an [`Event`] in the history of the
//! language it happened to, so every word's etymology can be read back — and
//! [`World::replay`] re-derives every lexicon from the proto-language and the
//! recorded rules alone, which the property tests hold to.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::WorldError;
use crate::lexicon::{self, Kind, Lexicon, Meaning, Vocabulary, Word};
use crate::library::LIBRARY;
use crate::phon;
use crate::rng::{Rng, Seed};
use crate::rule::{self, MAX_LEN, Rule};

/// A language's id: its index in [`World::langs`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct LangId(pub u16);

impl LangId {
    /// As an index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for LangId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What became of a language.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Fate {
    /// Still spoken, still changing.
    Spoken,
    /// Split into daughters in this century.
    Split(u32),
    /// Fell silent in this century.
    Silent(u32),
}

/// Who made a sound change happen.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Source {
    /// Time.
    Drift,
    /// Somebody at the workbench, in their colour.
    Hand {
        /// Hue in degrees.
        hue: u16,
    },
}

/// One word's change.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WordChange {
    /// Which word.
    pub meaning: Meaning,
    /// Its form before.
    pub before: Word,
    /// Its form after.
    pub after: Word,
}

/// What happened.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Happening {
    /// A sound change, applied to the whole lexicon.
    Sound {
        /// Its name.
        name: String,
        /// The rules, in order.
        rules: Vec<Rule>,
        /// Who.
        source: Source,
    },
    /// A worn-out word was rebuilt as a compound.
    Renewal {
        /// The word.
        meaning: Meaning,
        /// What it was compounded with.
        partner: Meaning,
    },
    /// An irregular inflected form was rebuilt by analogy from its base and
    /// suffix, the way English *kine* became *cows*.
    Levelling {
        /// The inflected form.
        meaning: Meaning,
    },
    /// A word was borrowed from a sister.
    Loan {
        /// The word.
        meaning: Meaning,
        /// From where.
        from: LangId,
    },
}

/// One event in one language's history.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Event {
    /// When.
    pub century: u32,
    /// What.
    pub what: Happening,
    /// Every word it changed.
    pub changes: Vec<WordChange>,
}

/// A language.
#[derive(Clone, Debug)]
pub struct Language {
    /// Its id.
    pub id: LangId,
    /// Where it came from.
    pub parent: Option<LangId>,
    /// What it split into.
    pub children: Vec<LangId>,
    /// The century it was born.
    pub born: u32,
    /// What became of it.
    pub fate: Fate,
    /// Highland, Coastal, …
    pub epithet: &'static str,
    /// Its colour, in degrees. Daughters keep close to their mother.
    pub hue: u16,
    /// One word per meaning.
    pub lexicon: Lexicon,
    /// Everything that happened to it, oldest first.
    pub history: Vec<Event>,
    /// Which meaning names it.
    pub tongue: Meaning,
}

impl Language {
    /// What its speakers call it: their own word for *tongue*, romanised.
    /// Names drift with the language.
    pub fn name(&self) -> String {
        let form = self
            .lexicon
            .get(self.tongue.index())
            .map(|w| phon::romanise(w.segs()))
            .unwrap_or_default();
        match self.fate {
            _ if self.parent.is_none() => format!("Proto-{form}"),
            Fate::Split(_) => format!("Common {} {form}", self.epithet),
            _ => format!("{} {form}", self.epithet),
        }
    }

    /// Still changing?
    pub const fn spoken(&self) -> bool {
        matches!(self.fate, Fate::Spoken)
    }

    /// The century it stopped changing, or `now`.
    pub const fn until(&self, now: u32) -> u32 {
        match self.fate {
            Fate::Spoken => now,
            Fate::Split(c) | Fate::Silent(c) => c,
        }
    }

    /// The word for a meaning.
    pub fn word(&self, m: Meaning) -> Option<&Word> {
        self.lexicon.get(m.index())
    }
}

/// A line in the chronicle.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FeedLine {
    /// When.
    pub century: u32,
    /// Which language, if one.
    pub lang: Option<LangId>,
    /// A colour for it.
    pub hue: u16,
    /// `sound`, `hand`, `split`, `renewal`, `loan`, `silent` or `world`.
    pub kind: String,
    /// What to say.
    pub text: String,
    /// One example, `before → after`, if there is one.
    pub example: Option<String>,
}

/// One step of an etymology.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Step {
    /// When.
    pub century: u32,
    /// In which language.
    pub lang: LangId,
    /// Its name.
    pub lang_name: String,
    /// What did it: a change's name, or a renewal or loan.
    pub cause: String,
    /// The single rule responsible, for sound changes.
    pub rule: Option<String>,
    /// Who.
    pub hand: Option<u16>,
    /// Before.
    pub before: Word,
    /// After.
    pub after: Word,
}

/// Tunables. All rates are per mille, per language, per century.
#[derive(Clone, Copy, Debug)]
pub struct Rates {
    /// A sound change happens.
    pub sound: u32,
    /// A short word is renewed.
    pub renew: u32,
    /// A word is borrowed.
    pub loan: u32,
    /// The language splits.
    pub split: u32,
    /// The language falls silent.
    pub silence: u32,
    /// An irregular form is levelled.
    pub level: u32,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            sound: 560,
            renew: 300,
            loan: 35,
            split: 120,
            silence: 14,
            level: 250,
        }
    }
}

/// Most languages spoken at once.
pub const MAX_SPOKEN: usize = 10;
/// Fewest spoken before silence stops striking.
pub const MIN_SPOKEN: usize = 5;
/// Most languages a world will ever hold; after this nothing splits.
pub const MAX_LANGS: usize = 61;
/// Centuries a world runs before anybody sees it.
pub const PRELUDE: u32 = 32;
/// A language must be this many centuries old to split.
pub const SPLIT_AGE: u32 = 3;
/// The chronicle keeps this many lines.
pub const FEED_LEN: usize = 80;

const EPITHETS: &[&str] = &[
    "High", "Low", "Coastal", "Inland", "Northern", "Southern", "Eastern", "Western", "Island",
    "River", "Hill", "Steppe", "Forest", "Lake", "Old", "Valley", "Marsh", "Desert", "Upper",
    "Lower", "Far", "Near",
];

/// A family of languages.
#[derive(Clone, Debug)]
pub struct World {
    /// The word it was seeded from.
    pub seed_word: String,
    rng: Rng,
    /// How many centuries have passed.
    pub century: u32,
    /// The proto-language's lexicon, as invented.
    pub proto: Lexicon,
    /// Every language that ever was, by id.
    pub langs: Vec<Language>,
    /// The chronicle, newest last.
    pub feed: Vec<FeedLine>,
    /// Tunables.
    pub rates: Rates,
    /// The meanings this family keeps words for.
    pub vocab: Arc<Vocabulary>,
    /// The century the chronicle ends, if it ends.
    pub span: Option<u32>,
    /// Lines said since the last `advance` or `apply_hand` began.
    fresh: Vec<FeedLine>,
}

impl World {
    /// A fresh family with the standard vocabulary, seeded from a word, run
    /// through the prelude.
    pub fn new(seed_word: &str) -> Self {
        Self::grown(seed_word, Arc::new(Vocabulary::standard()), None)
    }

    /// A fresh family with any vocabulary and span, run through the prelude.
    pub fn grown(seed_word: &str, vocab: Arc<Vocabulary>, span: Option<u32>) -> Self {
        let mut w = Self::unrun(seed_word, vocab, span);
        for _ in 0..PRELUDE {
            w.advance();
        }
        w
    }

    /// A fresh family at century zero: just the proto-language.
    pub fn unrun(seed_word: &str, vocab: Arc<Vocabulary>, span: Option<u32>) -> Self {
        let mut rng = Rng::new(Seed::from_word(seed_word));
        let proto = lexicon::invent(&mut rng, &vocab);
        let hue = u16::try_from(rng.below(360)).unwrap_or(0);
        let root = Language {
            id: LangId(0),
            parent: None,
            children: Vec::new(),
            born: 0,
            fate: Fate::Spoken,
            epithet: "",
            hue,
            lexicon: proto.clone(),
            history: Vec::new(),
            tongue: vocab.tongue(),
        };
        let mut w = Self {
            seed_word: seed_word.trim().to_owned(),
            rng,
            century: 0,
            proto,
            langs: vec![root],
            feed: Vec::new(),
            rates: Rates::default(),
            vocab,
            span: span.map(|s| s.max(PRELUDE)),
            fresh: Vec::new(),
        };
        let name = w.langs[0].name();
        w.say(FeedLine {
            century: 0,
            lang: Some(LangId(0)),
            hue,
            kind: "world".into(),
            text: format!("{name} is spoken, somewhere, by somebody"),
            example: None,
        });
        w
    }

    /// Has the chronicle reached its span?
    pub fn complete(&self) -> bool {
        self.span.is_some_and(|s| self.century >= s)
    }

    /// A gloss.
    pub fn gloss(&self, m: Meaning) -> &str {
        self.vocab.gloss(m)
    }

    /// A language by id.
    pub fn lang(&self, id: LangId) -> Result<&Language, WorldError> {
        self.langs
            .get(id.index())
            .ok_or(WorldError::NoLanguage(id.0))
    }

    /// The ids of every language still spoken.
    pub fn spoken(&self) -> Vec<LangId> {
        self.langs
            .iter()
            .filter(|l| l.spoken())
            .map(|l| l.id)
            .collect()
    }

    /// The root first, `id` last.
    pub fn lineage(&self, id: LangId) -> Vec<LangId> {
        let mut chain = vec![id];
        let mut cur = id;
        // Parents have smaller ids, so this takes at most `id` steps.
        while let Some(p) = self.langs.get(cur.index()).and_then(|l| l.parent) {
            if p >= cur {
                break;
            }
            chain.push(p);
            cur = p;
        }
        chain.reverse();
        chain
    }

    fn say(&mut self, line: FeedLine) {
        self.fresh.push(line.clone());
        self.feed.push(line);
        if self.feed.len() > FEED_LEN {
            let drop = self.feed.len() - FEED_LEN;
            self.feed.drain(..drop);
        }
    }

    /// What a chain of rules would do to one language, without doing it.
    pub fn changes_for(&self, id: LangId, rules: &[Rule]) -> Vec<WordChange> {
        let Some(lang) = self.langs.get(id.index()) else {
            return Vec::new();
        };
        self.vocab
            .all()
            .filter_map(|m| {
                let before = lang.word(m)?;
                let after = Word(rule::step_chain(rules, before.segs()));
                (after != *before).then(|| WordChange {
                    meaning: m,
                    before: before.clone(),
                    after,
                })
            })
            .collect()
    }

    fn commit(&mut self, id: LangId, what: Happening, changes: Vec<WordChange>) {
        let century = self.century;
        if let Some(lang) = self.langs.get_mut(id.index()) {
            for c in &changes {
                if let Some(slot) = lang.lexicon.get_mut(c.meaning.index()) {
                    *slot = c.after.clone();
                }
            }
            lang.history.push(Event {
                century,
                what,
                changes,
            });
        }
    }

    fn example(&self, changes: &[WordChange]) -> Option<String> {
        changes
            .iter()
            .max_by_key(|c| c.before.0.len().abs_diff(c.after.0.len()) + c.before.0.len())
            .map(|c| format!("{} *{} → {}", self.gloss(c.meaning), c.before, c.after))
    }

    /// One century passes.
    /// A complete chronicle does not advance.
    pub fn advance(&mut self) -> Vec<FeedLine> {
        if self.complete() {
            return Vec::new();
        }
        self.century += 1;
        self.fresh.clear();
        let spoken = self.spoken();
        for &id in &spoken {
            self.drift(id);
            self.renew(id);
            self.borrow(id, &spoken);
            self.level(id);
        }
        let force_first_split = self.langs.len() == 1 && self.century >= 2;
        for &id in &spoken {
            self.maybe_split(id, force_first_split);
        }
        for &id in &spoken {
            self.maybe_fall_silent(id);
        }
        std::mem::take(&mut self.fresh)
    }

    fn drift(&mut self, id: LangId) {
        if !self.rng.chance(self.rates.sound) {
            return;
        }
        for _ in 0..10 {
            let Some(change) = self.rng.pick(&LIBRARY) else {
                return;
            };
            let changes = self.changes_for(id, &change.rules);
            if changes.is_empty() {
                continue;
            }
            let (name, where_, rules) = (change.name, change.where_, change.rules.clone());
            let n = changes.len();
            let example = self.example(&changes);
            let (lname, hue) = self
                .langs
                .get(id.index())
                .map(|l| (l.name(), l.hue))
                .unwrap_or_default();
            self.commit(
                id,
                Happening::Sound {
                    name: name.to_owned(),
                    rules,
                    source: Source::Drift,
                },
                changes,
            );
            self.say(FeedLine {
                century: self.century,
                lang: Some(id),
                hue,
                kind: "sound".into(),
                text: format!("{lname}: {name} (as in {where_}), {n} word{}", plural(n)),
                example,
            });
            return;
        }
    }

    fn renew(&mut self, id: LangId) {
        let Some(lang) = self.langs.get(id.index()) else {
            return;
        };
        let worn: Vec<Meaning> = self
            .vocab
            .all()
            .filter(|&m| self.vocab.kind(m) == Kind::Word)
            .filter(|&m| lang.word(m).is_some_and(|w| w.0.len() <= 2))
            .collect();
        if worn.is_empty() || !self.rng.chance(self.rates.renew) {
            return;
        }
        let Some(&m) = self.rng.pick(&worn) else {
            return;
        };
        let p = self.vocab.partner(m);
        let (Some(before), Some(with)) = (lang.word(m).cloned(), lang.word(p).cloned()) else {
            return;
        };
        let mut segs = before.0.clone();
        segs.extend_from_slice(&with.0);
        if segs.len() > MAX_LEN {
            return;
        }
        let after = Word(segs);
        let (lname, hue) = (lang.name(), lang.hue);
        let (g, pg) = (self.gloss(m).to_owned(), self.gloss(p).to_owned());
        let example = Some(format!("{g} *{before} → {after}"));
        let changes = self.with_paradigm(
            id,
            WordChange {
                meaning: m,
                before,
                after,
            },
        );
        self.commit(
            id,
            Happening::Renewal {
                meaning: m,
                partner: p,
            },
            changes,
        );
        self.say(FeedLine {
            century: self.century,
            lang: Some(id),
            hue,
            kind: "renewal".into(),
            text: format!("{lname} rebuilds its worn-down word for ‘{g}’ as ‘{g}-{pg}’"),
            example,
        });
    }

    fn borrow(&mut self, id: LangId, spoken: &[LangId]) {
        if spoken.len() < 2 || !self.rng.chance(self.rates.loan) {
            return;
        }
        let others: Vec<LangId> = spoken.iter().copied().filter(|&o| o != id).collect();
        let (Some(&from), Some(m)) = (
            self.rng.pick(&others),
            self.vocab.all().nth(self.rng.below(self.vocab.len())),
        ) else {
            return;
        };
        if self.vocab.kind(m) != Kind::Word {
            return;
        }
        let (Ok(to), Ok(src)) = (self.lang(id), self.lang(from)) else {
            return;
        };
        let (Some(before), Some(after)) = (to.word(m).cloned(), src.word(m).cloned()) else {
            return;
        };
        if before == after {
            return;
        }
        let (lname, hue, sname) = (to.name(), to.hue, src.name());
        let g = self.gloss(m).to_owned();
        let example = Some(format!("{g} *{before} → {after}"));
        let changes = self.with_paradigm(
            id,
            WordChange {
                meaning: m,
                before,
                after,
            },
        );
        self.commit(id, Happening::Loan { meaning: m, from }, changes);
        self.say(FeedLine {
            century: self.century,
            lang: Some(id),
            hue,
            kind: "loan".into(),
            text: format!("{lname} borrows ‘{g}’ from {sname}"),
            example,
        });
    }

    /// The productive ending for a suffix in one language: what speakers
    /// abstract from the forms they hear. Every inflected form whose base is
    /// still a prefix of it votes for the rest of it (possibly nothing, as in
    /// *sheep*); the most common ending with at least two votes wins, ties to
    /// the shorter, then the earlier in the inventory. With no such ending,
    /// the suffix as it stands alone.
    pub fn productive(&self, id: LangId, affix: Meaning) -> Option<Vec<phon::Seg>> {
        let l = self.langs.get(id.index())?;
        let mut votes: Vec<(Vec<phon::Seg>, usize)> = Vec::new();
        for (m, base, a) in self.vocab.paradigm_entries() {
            if a != affix {
                continue;
            }
            let (Some(f), Some(b)) = (l.word(m), l.word(base)) else {
                continue;
            };
            if let Some(tail) = f.segs().strip_prefix(b.segs()) {
                match votes.iter_mut().find(|(t, _)| t == tail) {
                    Some((_, n)) => *n += 1,
                    None => votes.push((tail.to_vec(), 1)),
                }
            }
        }
        votes
            .into_iter()
            .filter(|(_, n)| *n >= 2)
            .max_by(|(ta, na), (tb, nb)| {
                na.cmp(nb)
                    .then_with(|| tb.len().cmp(&ta.len()))
                    .then_with(|| tb.cmp(ta))
            })
            .map(|(t, _)| t)
            .or_else(|| l.word(affix).map(|w| w.0.clone()))
    }

    /// The regular form of an inflected meaning in one language: its base as
    /// it is now with the productive ending, if that is still a word.
    pub fn regular(&self, id: LangId, m: Meaning) -> Option<Word> {
        let Kind::Inflected { base, affix } = self.vocab.kind(m) else {
            return None;
        };
        let l = self.langs.get(id.index())?;
        let mut segs = l.word(base)?.0.clone();
        segs.extend(self.productive(id, affix)?);
        let w = Word(segs);
        w.is_valid().then_some(w)
    }

    /// The inflected forms of a language that no longer match their base
    /// and suffix.
    pub fn irregulars(&self, id: LangId) -> Vec<Meaning> {
        let Some(l) = self.langs.get(id.index()) else {
            return Vec::new();
        };
        self.vocab
            .paradigm_entries()
            .filter(|&(m, _, _)| self.regular(id, m).is_some_and(|r| l.word(m) != Some(&r)))
            .map(|(m, _, _)| m)
            .collect()
    }

    /// When a word is replaced wholesale (renewed or borrowed), its inflected
    /// forms are rebuilt regular from the new word — a borrowed word takes the
    /// borrower's endings.
    fn with_paradigm(&self, id: LangId, change: WordChange) -> Vec<WordChange> {
        let Some(l) = self.langs.get(id.index()) else {
            return vec![change];
        };
        let mut out = Vec::new();
        for (m, base, affix) in self.vocab.paradigm_entries() {
            if base != change.meaning {
                continue;
            }
            let (Some(before), Some(tail)) = (l.word(m), self.productive(id, affix)) else {
                continue;
            };
            if let Some(after) = change.after.suffixed(&Word(tail))
                && after != *before
            {
                out.push(WordChange {
                    meaning: m,
                    before: before.clone(),
                    after,
                });
            }
        }
        out.insert(0, change);
        out
    }

    fn level(&mut self, id: LangId) {
        let irregular = self.irregulars(id);
        if irregular.is_empty() || !self.rng.chance(self.rates.level) {
            return;
        }
        let Some(&m) = self.rng.pick(&irregular) else {
            return;
        };
        let (Some(after), Some(before)) = (
            self.regular(id, m),
            self.langs.get(id.index()).and_then(|l| l.word(m)).cloned(),
        ) else {
            return;
        };
        let (lname, hue) = self
            .langs
            .get(id.index())
            .map(|l| (l.name(), l.hue))
            .unwrap_or_default();
        let g = self.gloss(m).to_owned();
        let example = Some(format!("{g} *{before} → {after}"));
        self.commit(
            id,
            Happening::Levelling { meaning: m },
            vec![WordChange {
                meaning: m,
                before,
                after,
            }],
        );
        self.say(FeedLine {
            century: self.century,
            lang: Some(id),
            hue,
            kind: "levelling".into(),
            text: format!("{lname} levels ‘{g}’ by analogy"),
            example,
        });
    }

    fn depth(&self, id: LangId) -> usize {
        self.lineage(id).len()
    }

    fn maybe_split(&mut self, id: LangId, force: bool) {
        let Some(lang) = self.langs.get(id.index()) else {
            return;
        };
        let room = self.spoken().len() < MAX_SPOKEN && self.langs.len() + 2 <= MAX_LANGS;
        let old_enough = self.century.saturating_sub(lang.born) >= SPLIT_AGE;
        if !(lang.spoken() && room && (force || (old_enough && self.rng.chance(self.rates.split))))
        {
            return;
        }
        let (lexicon, hue, pname) = (lang.lexicon.clone(), lang.hue, lang.name());
        let spread: i32 = match self.depth(id) {
            1 => 120,
            2 => 46,
            3 => 22,
            _ => 10,
        };
        let a = self.rng.below(EPITHETS.len());
        let b = (a + 1 + self.rng.below(EPITHETS.len() - 1)) % EPITHETS.len();
        let mut names = Vec::new();
        for (k, sign) in [(a, -1), (b, 1)] {
            let child = LangId(u16::try_from(self.langs.len()).unwrap_or(u16::MAX));
            let h = (i32::from(hue) + sign * spread).rem_euclid(360);
            let l = Language {
                id: child,
                parent: Some(id),
                children: Vec::new(),
                born: self.century,
                fate: Fate::Spoken,
                epithet: EPITHETS.get(k).copied().unwrap_or("New"),
                hue: u16::try_from(h).unwrap_or(0),
                lexicon: lexicon.clone(),
                history: Vec::new(),
                tongue: self.vocab.tongue(),
            };
            names.push(l.name());
            self.langs.push(l);
            if let Some(p) = self.langs.get_mut(id.index()) {
                p.children.push(child);
            }
        }
        if let Some(p) = self.langs.get_mut(id.index()) {
            p.fate = Fate::Split(self.century);
        }
        let pname_now = self.langs.get(id.index()).map_or(pname, Language::name);
        self.say(FeedLine {
            century: self.century,
            lang: Some(id),
            hue,
            kind: "split".into(),
            text: format!("{pname_now} splits into {}", names.join(" and ")),
            example: None,
        });
    }

    fn maybe_fall_silent(&mut self, id: LangId) {
        let spoken = self.spoken().len();
        let Some(lang) = self.langs.get(id.index()) else {
            return;
        };
        if !lang.spoken() || spoken <= MIN_SPOKEN || !self.rng.chance(self.rates.silence) {
            return;
        }
        let (name, hue) = (lang.name(), lang.hue);
        if let Some(l) = self.langs.get_mut(id.index()) {
            l.fate = Fate::Silent(self.century);
        }
        self.say(FeedLine {
            century: self.century,
            lang: Some(id),
            hue,
            kind: "silent".into(),
            text: format!("the last speaker of {name} dies"),
            example: None,
        });
    }

    /// Which languages a hand rule would reach.
    fn targets(&self, target: Option<LangId>) -> Result<Vec<LangId>, WorldError> {
        match target {
            None => Ok(self.spoken()),
            Some(id) => {
                let l = self.lang(id)?;
                if l.spoken() {
                    Ok(vec![id])
                } else {
                    Err(WorldError::NotSpoken(l.name()))
                }
            }
        }
    }

    /// What a typed rule would do, language by language, without doing it.
    pub fn preview(
        &self,
        src: &str,
        target: Option<LangId>,
    ) -> Result<Vec<(LangId, Vec<WordChange>)>, WorldError> {
        let rules = rule::parse_chain(src)?;
        Ok(self
            .targets(target)?
            .into_iter()
            .map(|id| (id, self.changes_for(id, &rules)))
            .collect())
    }

    /// Apply a typed rule. Languages it changes nothing in get no event.
    pub fn apply_hand(
        &mut self,
        src: &str,
        target: Option<LangId>,
        hue: u16,
    ) -> Result<Vec<FeedLine>, WorldError> {
        let rules = rule::parse_chain(src)?;
        let printed: Vec<String> = rules.iter().map(ToString::to_string).collect();
        let name = printed.join("; ");
        self.fresh.clear();
        let mut total = 0;
        for id in self.targets(target)? {
            let changes = self.changes_for(id, &rules);
            if changes.is_empty() {
                continue;
            }
            total += changes.len();
            let n = changes.len();
            let example = self.example(&changes);
            let lname = self
                .langs
                .get(id.index())
                .map(Language::name)
                .unwrap_or_default();
            self.commit(
                id,
                Happening::Sound {
                    name: name.clone(),
                    rules: rules.clone(),
                    source: Source::Hand { hue },
                },
                changes,
            );
            self.say(FeedLine {
                century: self.century,
                lang: Some(id),
                hue,
                kind: "hand".into(),
                text: format!("{lname}: {name}, by hand, {n} word{}", plural(n)),
                example,
            });
        }
        if total == 0 {
            self.say(FeedLine {
                century: self.century,
                lang: target,
                hue,
                kind: "hand".into(),
                text: format!("{name} — changed nothing; the language shrugs"),
                example: None,
            });
        }
        Ok(std::mem::take(&mut self.fresh))
    }

    /// Re-derive a lexicon from the proto-language and the recorded history
    /// of every ancestor. Sound changes are re-applied from their rules, not
    /// copied from their records.
    pub fn replay(&self, id: LangId) -> Lexicon {
        let mut lex = self.proto.clone();
        for anc in self.lineage(id) {
            let Some(lang) = self.langs.get(anc.index()) else {
                continue;
            };
            for e in &lang.history {
                match &e.what {
                    Happening::Sound { rules, .. } => {
                        for w in &mut lex {
                            *w = Word(rule::step_chain(rules, w.segs()));
                        }
                    }
                    Happening::Renewal { .. }
                    | Happening::Loan { .. }
                    | Happening::Levelling { .. } => {
                        for c in &e.changes {
                            if let Some(slot) = lex.get_mut(c.meaning.index()) {
                                *slot = c.after.clone();
                            }
                        }
                    }
                }
            }
        }
        lex
    }

    /// Every step that changed one word on its way from the proto-language to
    /// `id`, oldest first. Chains are broken into their individual rules.
    pub fn etymology(&self, id: LangId, m: Meaning) -> Vec<Step> {
        let mut steps = Vec::new();
        for anc in self.lineage(id) {
            let Some(lang) = self.langs.get(anc.index()) else {
                continue;
            };
            let lang_name = lang.name();
            for e in &lang.history {
                let Some(c) = e.changes.iter().find(|c| c.meaning == m) else {
                    continue;
                };
                match &e.what {
                    Happening::Sound {
                        name,
                        rules,
                        source,
                    } => {
                        let hand = match source {
                            Source::Hand { hue } => Some(*hue),
                            Source::Drift => None,
                        };
                        let mut cur = c.before.clone();
                        for r in rules {
                            let next = Word(r.step(cur.segs()));
                            if next != cur {
                                steps.push(Step {
                                    century: e.century,
                                    lang: anc,
                                    lang_name: lang_name.clone(),
                                    cause: name.clone(),
                                    rule: Some(r.to_string()),
                                    hand,
                                    before: cur.clone(),
                                    after: next.clone(),
                                });
                                cur = next;
                            }
                        }
                    }
                    Happening::Levelling { .. } => steps.push(Step {
                        century: e.century,
                        lang: anc,
                        lang_name: lang_name.clone(),
                        cause: "levelled by analogy with its base and suffix".to_owned(),
                        rule: None,
                        hand: None,
                        before: c.before.clone(),
                        after: c.after.clone(),
                    }),
                    Happening::Renewal { meaning, partner } => steps.push(Step {
                        century: e.century,
                        lang: anc,
                        lang_name: lang_name.clone(),
                        cause: if *meaning == m {
                            format!("renewed as ‘{}-{}’", self.gloss(m), self.gloss(*partner))
                        } else {
                            format!(
                                "rebuilt regular when ‘{0}’ was renewed as ‘{0}-{1}’",
                                self.gloss(*meaning),
                                self.gloss(*partner)
                            )
                        },
                        rule: None,
                        hand: None,
                        before: c.before.clone(),
                        after: c.after.clone(),
                    }),
                    Happening::Loan { from, meaning } => steps.push(Step {
                        century: e.century,
                        lang: anc,
                        lang_name: lang_name.clone(),
                        cause: {
                            let src = self
                                .langs
                                .get(from.index())
                                .map(Language::name)
                                .unwrap_or_default();
                            if *meaning == m {
                                format!("borrowed from {src}")
                            } else {
                                format!(
                                    "rebuilt regular when ‘{}’ was borrowed from {src}",
                                    self.gloss(*meaning)
                                )
                            }
                        },
                        rule: None,
                        hand: None,
                        before: c.before.clone(),
                        after: c.after.clone(),
                    }),
                }
            }
        }
        steps
    }

    /// The meaning whose word has drifted furthest apart: the most distinct
    /// forms among the spoken languages, ties broken by the most changes.
    /// The front page opens on it.
    pub fn liveliest(&self) -> Option<Meaning> {
        let spoken = self.spoken();
        self.vocab
            .all()
            .filter(|&m| self.vocab.kind(m) == Kind::Word)
            .max_by_key(|&m| {
                let mut forms: Vec<&Word> = spoken
                    .iter()
                    .filter_map(|id| self.langs.get(id.index()).and_then(|l| l.word(m)))
                    .collect();
                forms.sort_by_key(|w| w.segs().to_vec());
                forms.dedup();
                let changes: usize = spoken.iter().map(|&id| self.etymology(id, m).len()).sum();
                (forms.len(), changes, std::cmp::Reverse(m.index()))
            })
    }

    /// Leaves in drawing order: a depth-first walk, daughters in birth order.
    pub fn leaf_order(&self) -> Vec<LangId> {
        let mut out = Vec::new();
        let mut stack = vec![LangId(0)];
        while let Some(id) = stack.pop() {
            let Some(l) = self.langs.get(id.index()) else {
                continue;
            };
            if l.children.is_empty() {
                out.push(id);
            } else {
                stack.extend(l.children.iter().rev().copied());
            }
        }
        out
    }
}

/// Something that happens to a world from outside: time, or a hand.
///
/// A world is its seed, its vocabulary, its span, and the list of commands it has
/// run — [`World::replay_commands`] rebuilds it exactly from those, because
/// the engine is deterministic and a command that fails changes nothing.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Command {
    /// One century passes.
    Advance,
    /// A typed sound change.
    Hand {
        /// The rule source.
        rule: String,
        /// One language, or every spoken one.
        target: Option<LangId>,
        /// The hand's colour.
        hue: u16,
    },
}

impl World {
    /// Run one command. An `Err` leaves the world exactly as it was, so a
    /// log need only record the commands that returned `Ok`.
    pub fn run(&mut self, cmd: &Command) -> Result<Vec<FeedLine>, WorldError> {
        if self.complete() {
            return Err(WorldError::Complete);
        }
        match cmd {
            Command::Advance => Ok(self.advance()),
            Command::Hand { rule, target, hue } => self.apply_hand(rule, *target, *hue),
        }
    }

    /// A fingerprint of everything a command can change: the century and
    /// every language's fate and lexicon. FNV-1a, 64-bit.
    pub fn digest(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: u64| {
            for byte in b.to_le_bytes() {
                h ^= u64::from(byte);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(u64::from(self.century));
        for l in &self.langs {
            eat(u64::from(l.id.0));
            eat(match l.fate {
                Fate::Spoken => 0,
                Fate::Split(c) => 1 << 32 | u64::from(c),
                Fate::Silent(c) => 2 << 32 | u64::from(c),
            });
            for w in &l.lexicon {
                eat(w.0.len() as u64);
                for s in &w.0 {
                    eat(s.index() as u64);
                }
            }
        }
        h
    }

    /// Rebuild a world from its seed, vocabulary, span and command log. The
    /// error names the first command that failed on replay — which means the
    /// log was not written by this engine.
    pub fn replay_commands(
        seed_word: &str,
        vocab: Arc<Vocabulary>,
        span: Option<u32>,
        cmds: &[Command],
    ) -> Result<Self, (usize, WorldError)> {
        let mut w = Self::grown(seed_word, vocab, span);
        for (i, c) in cmds.iter().enumerate() {
            w.run(c).map_err(|e| (i, e))?;
        }
        Ok(w)
    }
}

const fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_world_grows_a_tree() {
        let w = World::new("moon");
        assert_eq!(w.century, PRELUDE);
        assert!(w.langs.len() >= 3, "only {} languages", w.langs.len());
        assert!(w.spoken().len() >= 2);
        assert!(w.spoken().len() <= MAX_SPOKEN);
        for l in &w.langs {
            if let Some(p) = l.parent {
                assert!(p < l.id);
                assert!(w.langs[p.index()].children.contains(&l.id));
            }
        }
    }

    #[test]
    fn the_same_word_grows_the_same_world() {
        let a = World::new("salt");
        let b = World::new("salt");
        assert_eq!(a.langs.len(), b.langs.len());
        for (x, y) in a.langs.iter().zip(&b.langs) {
            assert_eq!(x.lexicon, y.lexicon);
        }
    }

    #[test]
    fn every_lexicon_replays() {
        let mut w = World::new("ash");
        for _ in 0..40 {
            w.advance();
        }
        for l in &w.langs {
            assert_eq!(w.replay(l.id), l.lexicon, "{}", l.name());
        }
    }

    #[test]
    fn every_etymology_ends_where_the_word_is() {
        let w = World::new("honey");
        for &id in &w.spoken() {
            for m in w.vocab.all() {
                let steps = w.etymology(id, m);
                let start = &w.proto[m.index()];
                let end = steps.last().map_or(start, |s| &s.after);
                assert_eq!(end, &w.langs[id.index()].lexicon[m.index()]);
                if let Some(first) = steps.first() {
                    assert_eq!(&first.before, start);
                }
                for pair in steps.windows(2) {
                    assert_eq!(pair[0].after, pair[1].before);
                }
            }
        }
    }

    #[test]
    fn a_hand_rule_previews_then_applies() {
        let mut w = World::new("wolf");
        let id = w.spoken()[0];
        let preview = w.preview("V > ∅ / C_#", Some(id)).unwrap_or_default();
        let n: usize = preview.iter().map(|(_, c)| c.len()).sum();
        let lines = w
            .apply_hand("V > ∅ / C_#", Some(id), 200)
            .unwrap_or_default();
        assert!(!lines.is_empty());
        let applied = w.langs[id.index()]
            .history
            .last()
            .map_or(0, |e| e.changes.len());
        if n > 0 {
            assert_eq!(applied, n);
        }
        assert_eq!(w.replay(id), w.langs[id.index()].lexicon);
    }

    #[test]
    fn hand_rules_refuse_the_dead_and_the_malformed() {
        let mut w = World::new("bone");
        assert!(matches!(
            w.apply_hand("p > ", None, 0),
            Err(WorldError::Rule(_))
        ));
        assert!(matches!(
            w.preview("p > f", Some(LangId(0))),
            Err(WorldError::NotSpoken(_))
        ));
        assert!(matches!(
            w.preview("p > f", Some(LangId(999))),
            Err(WorldError::NoLanguage(999))
        ));
    }

    #[test]
    fn the_liveliest_word_has_the_most_forms() {
        let w = World::new("salt");
        let Some(best) = w.liveliest() else {
            panic!("no meanings")
        };
        let distinct = |m: Meaning| {
            let mut v: Vec<String> = w
                .spoken()
                .iter()
                .map(|id| w.langs[id.index()].lexicon[m.index()].to_string())
                .collect();
            v.sort();
            v.dedup();
            v.len()
        };
        assert!(
            w.vocab
                .all()
                .filter(|&m| w.vocab.kind(m) == Kind::Word)
                .all(|m| distinct(m) <= distinct(best))
        );
    }

    #[test]
    fn leaves_are_every_childless_language_once() {
        let w = World::new("river");
        let leaves = w.leaf_order();
        let childless = w.langs.iter().filter(|l| l.children.is_empty()).count();
        assert_eq!(leaves.len(), childless);
        let mut sorted = leaves.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), leaves.len());
    }

    #[test]
    fn worlds_stay_bounded_over_a_long_time() {
        let mut w = World::new("star");
        for _ in 0..600 {
            w.advance();
        }
        assert!(w.langs.len() <= MAX_LANGS);
        assert!(w.spoken().len() <= MAX_SPOKEN);
        assert!(!w.spoken().is_empty());
        assert!(w.feed.len() <= FEED_LEN);
        assert!(w.langs.iter().all(|l| l.lexicon.iter().all(Word::is_valid)));
    }
}
