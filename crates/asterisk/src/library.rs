//! The changes that happen on their own: a library of sound changes attested
//! somewhere on Earth, written in the language they are applied in.
//!
//! Every entry is parsed at startup, and the tests check that every entry
//! parses and prints back to itself.

use std::sync::LazyLock;

use crate::rule::{Rule, parse_chain};

/// Name, where it happened, and the rules (applied in order).
pub const SOURCES: &[(&str, &str, &str)] = &[
    (
        "Grimm's law",
        "Proto-Germanic",
        "[+stop -voice] > [+cont]; [+stop +voice] > [-voice]",
    ),
    ("lenition", "Western Romance", "P > [+cont] / V_V"),
    (
        "intervocalic voicing",
        "Spanish",
        "[+stop -voice] > [+voice] / V_V",
    ),
    (
        "final devoicing",
        "German, Russian",
        "[-son] > [-voice] / _#",
    ),
    ("apocope", "Middle English", "V > ∅ / VC_#"),
    ("syncope", "Latin", "V > ∅ / VC_CV"),
    (
        "palatalisation of k",
        "Italian, English",
        "k > tʃ / _[+front]; g > dʒ / _[+front]",
    ),
    (
        "assibilation",
        "Japanese, Quebec French",
        "t > ts / _[+high]; d > dz / _[+high]",
    ),
    ("hissing palatalisation", "French", "k > s / _[+front]"),
    ("s before i", "Japanese", "s > ʃ / _[+high +front]"),
    (
        "nasal vowels",
        "French, Portuguese",
        "V > [+nasal] / _N#; N > ∅ / [+nasal]_#",
    ),
    (
        "medial nasal vowels",
        "Polish",
        "V > [+nasal] / _NC; N > ∅ / [+nasal]_C",
    ),
    ("denasalisation", "Late Latin", "[+nasal] > [-nasal] / _#"),
    ("breaking", "Castilian", "ɛ > jɛ; ɔ > wɛ"),
    (
        "open-syllable lengthening",
        "Middle English",
        "[-long] > [+long] / #C_CV",
    ),
    ("shortening", "Late Latin", "[+long] > [-long]"),
    (
        "compensatory lengthening",
        "Old French",
        "V > [+long] / _sC; s > ∅ / [+long]_C",
    ),
    ("h-dropping", "Romance", "h > ∅"),
    ("initial s to h", "Greek, Persian", "s > h / #_V"),
    ("initial f to h", "Spanish", "f > h / #_"),
    (
        "the great vowel shift",
        "English",
        "iː > aj; uː > aw; eː > iː; oː > uː; aː > eː",
    ),
    ("final raising", "Portuguese", "e > i / _#; o > u / _#"),
    ("i-umlaut", "Old English, German", "V > [+front] / _Ci"),
    ("reduction", "English", "[-long -nasal] > ə / VC_#"),
    ("rhotacism", "Latin", "z > r / V_V; s > r / V_V"),
    (
        "l-vocalisation",
        "Brazilian Portuguese",
        "l > w / V_C; l > w / V_#",
    ),
    ("glottal stops", "Cockney", "t > ʔ / V_#; t > ʔ / V_C"),
    ("th-fronting", "Cockney", "θ > f; ð > v"),
    ("loss of θ", "Romance, German", "θ > t; ð > d"),
    ("w becomes v", "German, Russian", "w > v"),
    ("j hardens", "Italian", "j > dʒ / #_"),
    (
        "nasal assimilation",
        "everywhere",
        "n > m / _[+lab]; n > ŋ / _k; n > ŋ / _g",
    ),
    ("cluster simplification", "Romance", "C > ∅ / VC_C"),
    ("final stop loss", "French", "P > ∅ / V_#"),
    ("prothesis", "Spanish", "s > es / #_C"),
    (
        "excrescent stops",
        "English, Spanish",
        "n > nd / _r; m > mb / _r; m > mb / _l",
    ),
    ("ny", "Romance", "n > ɲ / _j; j > ∅ / ɲ_"),
    ("fronting of u", "French, Dutch", "u > y"),
    ("rounding of ā", "Middle English", "aː > ɔː"),
    ("uvulars soften", "Arabic dialects", "q > ʔ; χ > h"),
    ("glottal loss", "everywhere", "ʔ > ∅ / V_V"),
    ("affrication before u", "Japanese", "t > ts / _u"),
    ("velar softening", "Danish", "g > ɣ / V_; ɣ > j / [+front]_"),
    ("vowel lowering", "Danish", "e > ɛ / _r; o > ɔ / _r"),
    (
        "monophthongs",
        "Sanskrit, French",
        "a > eː / _j; j > ∅ / eː_",
    ),
    ("rounding", "Swedish", "a > ɔ / _ŋ"),
    (
        "raising before nasals",
        "Greek, Latin",
        "e > i / _N; o > u / _N",
    ),
    ("unrounding", "Middle English", "y > i; ø > e"),
    ("x weakens", "Dutch, Spanish", "x > h"),
    ("tʃ to ʃ", "French, Andalusian", "tʃ > ʃ; dʒ > ʒ"),
    ("final vowel lengthening", "Hawaiian", "V > [+long] / #C_#"),
];

/// A library entry, parsed.
#[derive(Clone, Debug)]
pub struct Change {
    /// What linguists call it.
    pub name: &'static str,
    /// Where something like it happened.
    pub where_: &'static str,
    /// The rules, in order.
    pub rules: Vec<Rule>,
}

/// The parsed library. An entry that does not parse is left out rather than
/// panicking at startup — and the tests prove there are none.
pub static LIBRARY: LazyLock<Vec<Change>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .filter_map(|&(name, where_, src)| {
            parse_chain(src).ok().map(|rules| Change {
                name,
                where_,
                rules,
            })
        })
        .collect()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_parses() {
        for &(name, _, src) in SOURCES {
            assert!(
                parse_chain(src).is_ok(),
                "{name}: {src}: {:?}",
                parse_chain(src).err()
            );
        }
        assert_eq!(LIBRARY.len(), SOURCES.len());
    }

    #[test]
    fn every_entry_prints_as_written() {
        for c in LIBRARY.iter() {
            let printed: Vec<String> = c.rules.iter().map(ToString::to_string).collect();
            let src = SOURCES.iter().find(|s| s.0 == c.name).map_or("", |s| s.2);
            assert_eq!(printed.join("; "), src, "{}", c.name);
        }
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = SOURCES.iter().map(|s| s.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SOURCES.len());
    }
}
