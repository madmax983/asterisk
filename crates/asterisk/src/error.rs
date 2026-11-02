//! Errors.

use thiserror::Error;

/// A rule that did not parse.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// Something unexpected at a position.
    #[error("at character {at}: expected {expected}, found {found}")]
    Syntax {
        /// Character offset.
        at: usize,
        /// What the parser wanted.
        expected: String,
        /// What it saw.
        found: String,
    },
    /// An edit token nobody defined.
    #[error(
        "there is no edit called {0} (try +voice, -voice, +cont, -cont, +pal, +raise, +lower, +front, +back, ±round, ±long, ±nasal)"
    )]
    UnknownEdit(String),
    /// A feature name nobody defined.
    #[error(
        "there is no feature called {0} (try voice, son, stop, cont, lab, long, nasal, round, front, back, high, low)"
    )]
    UnknownFeature(String),
    /// `[]`.
    #[error("an empty bracket matches nothing")]
    EmptyBracket,
    /// Too many atoms beside the `_`.
    #[error("at most {0} segments on each side of the _")]
    ContextTooLong(usize),
    /// No rules at all.
    #[error("no rule given")]
    Empty,
}

/// Something the world refused to do.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorldError {
    /// The rule did not parse.
    #[error(transparent)]
    Rule(#[from] RuleError),
    /// No such language.
    #[error("there is no language {0}")]
    NoLanguage(u16),
    /// The language no longer changes.
    #[error("{0} is no longer spoken")]
    NotSpoken(String),
    /// No such meaning.
    #[error("there is no word for {0}")]
    NoMeaning(String),
    /// The world has lived its span.
    #[error("this chronicle is complete")]
    Complete,
}

/// A word list that did not parse. Line numbers are 1-based.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VocabError {
    /// A gloss with characters other than letters, digits, spaces, `-` or `'`.
    #[error("line {0}: ‘{1}’ is not a gloss (letters, digits, spaces, - and ' only, at most 32)")]
    BadGloss(usize, String),
    /// A proto-form that is not a word of the inventory.
    #[error("line {0}: ‘{1}’ is not a word in this inventory (1–12 IPA segments)")]
    BadForm(usize, String),
    /// The same gloss twice.
    #[error("line {0}: ‘{1}’ is already in the list")]
    Duplicate(usize, String),
    /// `x.PL` where `x` is not a word in the list.
    #[error("line {0}: there is no word ‘{1}’ to inflect")]
    NoBase(usize, String),
    /// `x.PL` where `PL` is not a suffix in the list.
    #[error("line {0}: there is no suffix ‘{1}’ (declare it, e.g. `{1} = s`)")]
    NoSuffix(usize, String),
    /// Too long.
    #[error("a list has at most {0} entries")]
    TooMany(usize),
}
