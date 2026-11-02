//! # asterisk
//!
//! A family of languages drifting through time. No IO, no async, no clock:
//! a world is a pure function of the word it was seeded with and the rules
//! people have typed into it.
//!
//! * [`phon`] — the segment inventory, features, and feature edits.
//! * [`rule`] — the sound-change language: parse, print, apply.
//! * [`library`] — sound changes attested on Earth, in that language.
//! * [`lexicon`] — meanings, words, and inventing a proto-language.
//! * [`world`] — the tree of languages and the centuries that pass over it, and
//!   the command log that rebuilds it.
//! * [`export`] — a family as a Markdown grammar book, or as JSON.
//!
//! ```
//! use asterisk::rule::Rule;
//! use asterisk::lexicon::Word;
//!
//! let grimm = Rule::parse("p > f / #_").unwrap();
//! let pater = Word::parse("pater").unwrap();
//! assert_eq!(Word(grimm.step(pater.segs())).to_string(), "fater");
//! ```

pub mod error;
pub mod export;
pub mod lexicon;
pub mod library;
pub mod phon;
pub mod rng;
pub mod rule;
pub mod world;

pub use error::{RuleError, VocabError, WorldError};
