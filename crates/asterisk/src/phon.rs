//! Segments, their features, and the edits a sound change can make to them.
//!
//! The inventory is a fixed, closed table of [`COUNT`] segments: 33 consonants
//! described by place, manner and voicing, and 52 vowels — 13 qualities, each
//! plain, long, nasal, or both. A [`Seg`] is an index into that table, and
//! every function in this module maps a valid index to a valid index. That
//! closure is what the Verus model in `verus/src/asterisk.rs` calls `seg_ok`.

use std::fmt;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// One segment: an index into the inventory. Always `< COUNT`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Seg(u8);

/// Place of articulation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// p b m w
    Labial,
    /// f v
    LabioDental,
    /// θ ð
    Dental,
    /// t d s z n l r
    Alveolar,
    /// tʃ dʒ ʃ ʒ
    PostAlveolar,
    /// ɲ j ʎ
    Palatal,
    /// k g x ɣ ŋ
    Velar,
    /// q χ
    Uvular,
    /// ʔ h
    Glottal,
}

/// Manner of articulation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Manner {
    /// Full closure.
    Stop,
    /// Closure released into friction.
    Affricate,
    /// Friction.
    Fricative,
    /// Nasal stop.
    Nasal,
    /// l, ʎ
    Lateral,
    /// r
    Rhotic,
    /// j, w
    Glide,
}

/// Vowel height, four steps.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Height {
    /// i y ɨ u
    High,
    /// e ø ə o
    MidHigh,
    /// ɛ œ ɔ
    MidLow,
    /// æ a
    Low,
}

/// Vowel backness.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Back {
    /// i e ɛ æ y ø œ
    Front,
    /// ɨ ə a
    Central,
    /// u o ɔ
    Back,
}

/// What a segment is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feat {
    /// A consonant.
    Cons {
        /// Where.
        place: Place,
        /// How.
        manner: Manner,
        /// Voiced?
        voiced: bool,
    },
    /// A vowel.
    Vowel {
        /// Height.
        height: Height,
        /// Backness.
        back: Back,
        /// Rounded?
        round: bool,
        /// Long?
        long: bool,
        /// Nasalised?
        nasal: bool,
    },
}

/// A row of the inventory.
#[derive(Clone, Debug)]
pub struct SegInfo {
    /// How it is written, in IPA.
    pub glyph: String,
    /// How it is romanised in language names.
    pub roman: String,
    /// What it is.
    pub feat: Feat,
}

const CONSONANTS: &[(&str, &str, Place, Manner, bool)] = &[
    ("p", "p", Place::Labial, Manner::Stop, false),
    ("b", "b", Place::Labial, Manner::Stop, true),
    ("m", "m", Place::Labial, Manner::Nasal, true),
    ("w", "w", Place::Labial, Manner::Glide, true),
    ("f", "f", Place::LabioDental, Manner::Fricative, false),
    ("v", "v", Place::LabioDental, Manner::Fricative, true),
    ("θ", "th", Place::Dental, Manner::Fricative, false),
    ("ð", "dh", Place::Dental, Manner::Fricative, true),
    ("t", "t", Place::Alveolar, Manner::Stop, false),
    ("d", "d", Place::Alveolar, Manner::Stop, true),
    ("ts", "ts", Place::Alveolar, Manner::Affricate, false),
    ("dz", "dz", Place::Alveolar, Manner::Affricate, true),
    ("s", "s", Place::Alveolar, Manner::Fricative, false),
    ("z", "z", Place::Alveolar, Manner::Fricative, true),
    ("n", "n", Place::Alveolar, Manner::Nasal, true),
    ("l", "l", Place::Alveolar, Manner::Lateral, true),
    ("r", "r", Place::Alveolar, Manner::Rhotic, true),
    ("tʃ", "ch", Place::PostAlveolar, Manner::Affricate, false),
    ("dʒ", "j", Place::PostAlveolar, Manner::Affricate, true),
    ("ʃ", "sh", Place::PostAlveolar, Manner::Fricative, false),
    ("ʒ", "zh", Place::PostAlveolar, Manner::Fricative, true),
    ("ɲ", "ny", Place::Palatal, Manner::Nasal, true),
    ("j", "y", Place::Palatal, Manner::Glide, true),
    ("ʎ", "ly", Place::Palatal, Manner::Lateral, true),
    ("k", "k", Place::Velar, Manner::Stop, false),
    ("g", "g", Place::Velar, Manner::Stop, true),
    ("x", "kh", Place::Velar, Manner::Fricative, false),
    ("ɣ", "gh", Place::Velar, Manner::Fricative, true),
    ("ŋ", "ng", Place::Velar, Manner::Nasal, true),
    ("q", "q", Place::Uvular, Manner::Stop, false),
    ("χ", "qh", Place::Uvular, Manner::Fricative, false),
    ("ʔ", "'", Place::Glottal, Manner::Stop, false),
    ("h", "h", Place::Glottal, Manner::Fricative, false),
];

const QUALITIES: &[(&str, &str, Height, Back, bool)] = &[
    ("i", "i", Height::High, Back::Front, false),
    ("y", "ü", Height::High, Back::Front, true),
    ("ɨ", "y", Height::High, Back::Central, false),
    ("u", "u", Height::High, Back::Back, true),
    ("e", "e", Height::MidHigh, Back::Front, false),
    ("ø", "ö", Height::MidHigh, Back::Front, true),
    ("ə", "ë", Height::MidHigh, Back::Central, false),
    ("o", "o", Height::MidHigh, Back::Back, true),
    ("ɛ", "e", Height::MidLow, Back::Front, false),
    ("œ", "oe", Height::MidLow, Back::Front, true),
    ("ɔ", "o", Height::MidLow, Back::Back, true),
    ("æ", "ae", Height::Low, Back::Front, false),
    ("a", "a", Height::Low, Back::Central, false),
];

/// Combining tilde, written after a nasal vowel's base letter.
pub const NASAL_MARK: char = '\u{303}';
/// The IPA length mark.
pub const LONG_MARK: char = 'ː';

/// How many segments the inventory has.
pub const COUNT: usize = CONSONANTS.len() + QUALITIES.len() * 4;

static INVENTORY: LazyLock<Vec<SegInfo>> = LazyLock::new(|| {
    let mut v = Vec::with_capacity(COUNT);
    for &(g, r, place, manner, voiced) in CONSONANTS {
        v.push(SegInfo {
            glyph: g.to_owned(),
            roman: r.to_owned(),
            feat: Feat::Cons {
                place,
                manner,
                voiced,
            },
        });
    }
    for nasal in [false, true] {
        for long in [false, true] {
            for &(g, r, height, back, round) in QUALITIES {
                let mut glyph = g.to_owned();
                let mut roman = r.to_owned();
                if nasal {
                    glyph.push(NASAL_MARK);
                    roman.push('n');
                }
                if long {
                    glyph.push(LONG_MARK);
                    roman = roman.replacen(r, &lengthen(r), 1);
                }
                v.push(SegInfo {
                    glyph,
                    roman,
                    feat: Feat::Vowel {
                        height,
                        back,
                        round,
                        long,
                        nasal,
                    },
                });
            }
        }
    }
    v
});

/// A long vowel's romanisation: a macron on its first letter.
fn lengthen(r: &str) -> String {
    let mut chars = r.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let marked = match first {
        'a' => 'ā',
        'e' => 'ē',
        'i' => 'ī',
        'o' => 'ō',
        'u' => 'ū',
        'y' => 'ȳ',
        'ü' => 'ǖ',
        other => other,
    };
    let mut s = String::new();
    s.push(marked);
    s.extend(chars);
    s
}

/// Glyphs sorted longest first, for greedy tokenising.
static BY_LENGTH: LazyLock<Vec<(String, Seg)>> = LazyLock::new(|| {
    let mut v: Vec<(String, Seg)> = all().map(|s| (s.info().glyph.clone(), s)).collect();
    v.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    v
});

/// Every segment, in inventory order.
pub fn all() -> impl Iterator<Item = Seg> {
    // COUNT is 85, comfortably inside u8.
    (0..COUNT).filter_map(|i| u8::try_from(i).ok().map(Seg))
}

impl Seg {
    /// The segment at an inventory index, if there is one.
    pub fn from_index(i: usize) -> Option<Self> {
        if i < COUNT {
            u8::try_from(i).ok().map(Self)
        } else {
            None
        }
    }

    /// The inventory index.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The inventory row. Every `Seg` is constructed in range, so the
    /// fallback row is unreachable; it exists so this never panics.
    pub fn info(self) -> &'static SegInfo {
        let inv: &'static [SegInfo] = &INVENTORY;
        inv.get(self.index()).unwrap_or(&inv[0]) // inv[0] exists: COUNT > 0
    }

    /// Features.
    pub fn feat(self) -> Feat {
        self.info().feat
    }

    /// IPA.
    pub fn glyph(self) -> &'static str {
        &self.info().glyph
    }

    /// Is this a vowel?
    pub fn is_vowel(self) -> bool {
        matches!(self.feat(), Feat::Vowel { .. })
    }

    /// Find the segment with exactly these features.
    pub fn find(feat: Feat) -> Option<Self> {
        all().find(|s| s.feat() == feat)
    }

    /// A consonant by glyph-free description.
    pub fn cons(place: Place, manner: Manner, voiced: bool) -> Option<Self> {
        Self::find(Feat::Cons {
            place,
            manner,
            voiced,
        })
    }
}

impl fmt::Display for Seg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.glyph())
    }
}

/// Fold precomposed nasal vowels into base + combining tilde, and ASCII
/// stand-ins into IPA, so that typed rules match the inventory's spelling.
pub fn normalise(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ã' => out.push_str("a\u{303}"),
            'ẽ' => out.push_str("e\u{303}"),
            'ĩ' => out.push_str("i\u{303}"),
            'õ' => out.push_str("o\u{303}"),
            'ũ' => out.push_str("u\u{303}"),
            'ỹ' => out.push_str("y\u{303}"),
            '~' => out.push(NASAL_MARK),
            ':' => out.push(LONG_MARK),
            'ɡ' => out.push('g'),
            other => out.push(other),
        }
    }
    out
}

/// The longest segment glyph that `s` starts with.
pub fn longest_prefix(s: &str) -> Option<(Seg, usize)> {
    BY_LENGTH
        .iter()
        .find(|(g, _)| s.starts_with(g.as_str()))
        .map(|(g, seg)| (*seg, g.len()))
}

/// Tokenise a whole string of segments. `.` and whitespace separate segments
/// and are otherwise ignored.
pub fn parse_segments(s: &str) -> Option<Vec<Seg>> {
    let s = normalise(s);
    let mut rest = s.as_str();
    let mut out = Vec::new();
    while let Some(c) = rest.chars().next() {
        if c == '.' || c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let (seg, n) = longest_prefix(rest)?;
        out.push(seg);
        rest = &rest[n..];
    }
    Some(out)
}

/// Would writing `a` straight after `b`'s glyph re-tokenise differently?
/// (`t` then `ʃ` reads back as `tʃ`.)
pub fn needs_separator(a: Seg, b: Seg) -> bool {
    let joined = format!("{}{}", a.glyph(), b.glyph());
    parse_segments(&joined).as_deref() != Some(&[a, b][..])
}

/// Write segments so that they tokenise back to themselves, with `.` only
/// where two glyphs would otherwise fuse.
pub fn spell(segs: &[Seg]) -> String {
    let mut s = String::new();
    for (i, &seg) in segs.iter().enumerate() {
        if i > 0 && needs_separator(segs[i - 1], seg) {
            s.push('.');
        }
        s.push_str(seg.glyph());
    }
    s
}

/// Romanise for a language name: capitalised, no IPA.
pub fn romanise(segs: &[Seg]) -> String {
    let raw: String = segs.iter().map(|s| s.info().roman.as_str()).collect();
    let raw = raw.trim_matches('\'').to_owned();
    let mut chars = raw.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().chain(chars).collect::<String>()
    })
}

// ── Edits ───────────────────────────────────────────────────────────────────

/// A change of features a sound change can make. Each is total: a segment it
/// does not apply to, or whose result the inventory lacks, is left alone.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Edit {
    /// `+voice`
    Voice,
    /// `-voice`
    Devoice,
    /// `+cont` — stops and affricates become fricatives.
    Spirantise,
    /// `-cont` — fricatives become stops or affricates; j becomes dʒ.
    Harden,
    /// `+pal` — velars and alveolars move to the palate.
    Palatalise,
    /// `+raise` — one step higher.
    Raise,
    /// `+lower` — one step lower.
    Lower,
    /// `+front`
    Front,
    /// `+back`
    Backen,
    /// `+round`
    Round,
    /// `-round`
    Unround,
    /// `+long`
    Lengthen,
    /// `-long`
    Shorten,
    /// `+nasal`
    Nasalise,
    /// `-nasal`
    Denasalise,
}

impl Edit {
    /// Every edit with its token.
    pub const ALL: &'static [(Self, &'static str)] = &[
        (Self::Voice, "+voice"),
        (Self::Devoice, "-voice"),
        (Self::Spirantise, "+cont"),
        (Self::Harden, "-cont"),
        (Self::Palatalise, "+pal"),
        (Self::Raise, "+raise"),
        (Self::Lower, "+lower"),
        (Self::Front, "+front"),
        (Self::Backen, "+back"),
        (Self::Round, "+round"),
        (Self::Unround, "-round"),
        (Self::Lengthen, "+long"),
        (Self::Shorten, "-long"),
        (Self::Nasalise, "+nasal"),
        (Self::Denasalise, "-nasal"),
    ];

    /// The token, e.g. `+voice`.
    pub fn token(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(e, _)| *e == self)
            .map_or("?", |(_, t)| t)
    }

    /// Parse a token.
    pub fn from_token(t: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, s)| *s == t).map(|(e, _)| *e)
    }

    /// Apply to one segment. Total, and closed over the inventory.
    pub fn apply(self, s: Seg) -> Seg {
        match s.feat() {
            Feat::Cons {
                place,
                manner,
                voiced,
            } => cons_edit(self, place, manner, voiced).unwrap_or(s),
            Feat::Vowel {
                height,
                back,
                round,
                long,
                nasal,
            } => vowel_edit(self, height, back, round, long, nasal).unwrap_or(s),
        }
    }
}

fn cons_edit(e: Edit, place: Place, manner: Manner, voiced: bool) -> Option<Seg> {
    use Manner::{Affricate, Fricative, Glide, Nasal, Stop};
    use Place::{Alveolar, Dental, Glottal, Labial, LabioDental, Palatal, PostAlveolar, Velar};
    match e {
        Edit::Voice => Seg::cons(place, manner, true),
        Edit::Devoice => Seg::cons(place, manner, false),
        Edit::Spirantise => match (place, manner) {
            (Labial, Stop) => Seg::cons(LabioDental, Fricative, voiced),
            (Alveolar, Stop) => Seg::cons(Dental, Fricative, voiced),
            (Glottal, Stop) => Seg::cons(Glottal, Fricative, false),
            (p, Stop | Affricate) => Seg::cons(p, Fricative, voiced),
            _ => None,
        },
        Edit::Harden => match (place, manner) {
            (LabioDental, Fricative) => Seg::cons(Labial, Stop, voiced),
            (Dental, Fricative) => Seg::cons(Alveolar, Stop, voiced),
            (Alveolar | PostAlveolar, Fricative) => Seg::cons(place, Affricate, voiced),
            (Glottal, Fricative) => Seg::cons(Glottal, Stop, false),
            (Palatal, Glide) => Seg::cons(PostAlveolar, Affricate, true),
            (p, Fricative) => Seg::cons(p, Stop, voiced),
            _ => None,
        },
        Edit::Palatalise => match (place, manner) {
            (Velar | Alveolar, Stop | Affricate) => Seg::cons(PostAlveolar, Affricate, voiced),
            (Velar | Alveolar, Fricative) => Seg::cons(PostAlveolar, Fricative, voiced),
            (Alveolar, Nasal) => Seg::cons(Palatal, Nasal, true),
            (Alveolar, Manner::Lateral) => Seg::cons(Palatal, Manner::Lateral, true),
            _ => None,
        },
        _ => None,
    }
}

fn vowel(height: Height, back: Back, round: bool, long: bool, nasal: bool) -> Option<Seg> {
    Seg::find(Feat::Vowel {
        height,
        back,
        round,
        long,
        nasal,
    })
}

fn vowel_edit(
    e: Edit,
    height: Height,
    back: Back,
    round: bool,
    long: bool,
    nasal: bool,
) -> Option<Seg> {
    // Nearest vowel at a new height: same backness and rounding if the
    // inventory has it, otherwise the nearest that does.
    let at_height = |h: Height| {
        vowel(h, back, round, long, nasal)
            .or_else(|| vowel(h, Back::Front, round, long, nasal))
            .or_else(|| vowel(h, back, !round, long, nasal))
            .or_else(|| vowel(h, Back::Front, !round, long, nasal))
    };
    match e {
        Edit::Raise => {
            let h = match height {
                Height::High | Height::MidHigh => Height::High,
                Height::MidLow => Height::MidHigh,
                Height::Low => Height::MidLow,
            };
            if h == height { None } else { at_height(h) }
        }
        Edit::Lower => {
            let h = match height {
                Height::High => Height::MidHigh,
                Height::MidHigh => Height::MidLow,
                Height::MidLow | Height::Low => Height::Low,
            };
            if h == height { None } else { at_height(h) }
        }
        Edit::Front => match back {
            Back::Front => None,
            Back::Central => vowel(height, Back::Front, false, long, nasal),
            Back::Back => vowel(height, Back::Front, round, long, nasal),
        },
        Edit::Backen => match back {
            Back::Back => None,
            Back::Central => vowel(height, Back::Back, true, long, nasal),
            Back::Front if round => vowel(height, Back::Back, true, long, nasal),
            Back::Front => vowel(height, Back::Central, false, long, nasal),
        },
        Edit::Round if !round => vowel(height, back, true, long, nasal)
            .or_else(|| vowel(height, Back::Back, true, long, nasal)),
        Edit::Unround if round => vowel(height, back, false, long, nasal)
            .or_else(|| vowel(height, Back::Central, false, long, nasal)),
        Edit::Lengthen if !long => vowel(height, back, round, true, nasal),
        Edit::Shorten if long => vowel(height, back, round, false, nasal),
        Edit::Nasalise if !nasal => vowel(height, back, round, long, true),
        Edit::Denasalise if nasal => vowel(height, back, round, long, false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(g: &str) -> Seg {
        let v = parse_segments(g).expect("a glyph");
        assert_eq!(v.len(), 1, "{g} is one segment");
        v[0]
    }

    #[test]
    fn inventory_has_the_advertised_size() {
        assert_eq!(COUNT, 85);
        assert_eq!(all().count(), COUNT);
    }

    #[test]
    fn every_glyph_is_unique_and_tokenises_to_itself() {
        for seg in all() {
            assert_eq!(
                parse_segments(seg.glyph()),
                Some(vec![seg]),
                "{}",
                seg.glyph()
            );
        }
    }

    #[test]
    fn affricates_win_over_their_parts() {
        assert_eq!(parse_segments("tʃa"), Some(vec![s("tʃ"), s("a")]));
        assert_eq!(parse_segments("t.ʃa"), Some(vec![s("t"), s("ʃ"), s("a")]));
        assert!(needs_separator(s("t"), s("ʃ")));
        assert!(!needs_separator(s("a"), s("t")));
        assert_eq!(spell(&[s("t"), s("ʃ"), s("a")]), "t.ʃa");
    }

    #[test]
    fn precomposed_and_ascii_marks_normalise() {
        assert_eq!(parse_segments("ã"), parse_segments("a\u{303}"));
        assert_eq!(parse_segments("a:"), Some(vec![s("aː")]));
        assert_eq!(parse_segments("o~:"), Some(vec![s("õː")]));
        assert_eq!(parse_segments("ɡ"), Some(vec![s("g")]));
        assert_eq!(parse_segments("?"), None);
    }

    #[test]
    fn grimm_and_friends() {
        assert_eq!(Edit::Spirantise.apply(s("p")), s("f"));
        assert_eq!(Edit::Spirantise.apply(s("t")), s("θ"));
        assert_eq!(Edit::Spirantise.apply(s("k")), s("x"));
        assert_eq!(Edit::Spirantise.apply(s("tʃ")), s("ʃ"));
        assert_eq!(Edit::Devoice.apply(s("b")), s("p"));
        assert_eq!(Edit::Voice.apply(s("s")), s("z"));
        assert_eq!(Edit::Voice.apply(s("h")), s("h"), "no voiced h: unchanged");
        assert_eq!(Edit::Harden.apply(s("j")), s("dʒ"));
        assert_eq!(Edit::Palatalise.apply(s("k")), s("tʃ"));
        assert_eq!(Edit::Palatalise.apply(s("n")), s("ɲ"));
        assert_eq!(Edit::Palatalise.apply(s("p")), s("p"));
    }

    #[test]
    fn vowels_move_through_the_space() {
        assert_eq!(Edit::Front.apply(s("u")), s("y"));
        assert_eq!(Edit::Front.apply(s("a")), s("æ"));
        assert_eq!(Edit::Raise.apply(s("eː")), s("iː"));
        assert_eq!(Edit::Raise.apply(s("a")), s("ɛ"));
        assert_eq!(Edit::Lower.apply(s("i")), s("e"));
        assert_eq!(Edit::Nasalise.apply(s("o")), s("õ"));
        assert_eq!(Edit::Lengthen.apply(s("õ")), s("õː"));
        assert_eq!(Edit::Round.apply(s("i")), s("y"));
        assert_eq!(Edit::Backen.apply(s("ø")), s("o"));
        assert_eq!(Edit::Nasalise.apply(s("k")), s("k"));
    }

    #[test]
    fn every_edit_is_closed_over_the_inventory() {
        for seg in all() {
            for &(e, _) in Edit::ALL {
                assert!(e.apply(seg).index() < COUNT);
            }
        }
    }

    #[test]
    fn edit_tokens_round_trip() {
        for &(e, t) in Edit::ALL {
            assert_eq!(Edit::from_token(t), Some(e));
            assert_eq!(e.token(), t);
        }
    }

    #[test]
    fn romanisation_reads_like_a_name() {
        assert_eq!(
            romanise(&parse_segments("ʃaːŋ").unwrap_or_default()),
            "Shāng"
        );
        assert_eq!(romanise(&parse_segments("ʔõ").unwrap_or_default()), "On");
    }
}
