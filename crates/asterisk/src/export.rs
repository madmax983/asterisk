//! Taking a family home: Markdown for reading, JSON for tools.
//!
//! Both are pure functions of a [`World`]. The Markdown is a small grammar
//! book: the family tree (as a Mermaid diagram), a cognate table, and for each
//! language that is not a mere ancestor — its sound changes in the order they
//! happened (its ancestors' first), its inventory, its productive endings and
//! irregular forms, and its whole lexicon with every word's history.

use std::fmt::Write as _;

use serde::Serialize;

use crate::lexicon::{Kind, Meaning, Word};
use crate::phon::{self, Seg};
use crate::world::{Fate, Happening, LangId, Language, Source, World};

/// The format tag in the JSON.
pub const FORMAT: &str = "asterisk-family/1";

/// Lower-case romanisation: `ʃaːŋ` → `shāng`.
pub fn transliterate(segs: &[Seg]) -> String {
    segs.iter().map(|s| s.info().roman.as_str()).collect()
}

/// One vocabulary entry.
#[derive(Debug, Clone, Serialize)]
pub struct VocabEntry {
    /// `moon`, `PL`, `foot.PL`.
    pub gloss: String,
    /// `word`, `suffix` or `inflected`.
    pub kind: &'static str,
}

/// One step of a word's history.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryStep {
    /// When.
    pub century: u32,
    /// Where.
    pub language: String,
    /// What did it.
    pub cause: String,
    /// The rule, for sound changes.
    pub rule: Option<String>,
    /// Typed by a person?
    pub by_hand: bool,
    /// Before, IPA.
    pub from: String,
    /// After, IPA.
    pub to: String,
}

/// One word in one language.
#[derive(Debug, Clone, Serialize)]
pub struct LexEntry {
    /// The gloss.
    pub gloss: String,
    /// IPA.
    pub ipa: String,
    /// Romanised.
    pub roman: String,
    /// The proto-form it descends from, IPA.
    pub proto: String,
    /// Every change on the way, oldest first (leaves only).
    pub history: Vec<HistoryStep>,
}

/// One inflected form.
#[derive(Debug, Clone, Serialize)]
pub struct ParadigmEntry {
    /// `foot.PL`.
    pub gloss: String,
    /// The form, IPA.
    pub ipa: String,
    /// What analogy would make it, IPA.
    pub regular: Option<String>,
    /// Does it differ?
    pub irregular: bool,
}

/// A suffix and the ending speakers actually use.
#[derive(Debug, Clone, Serialize)]
pub struct Ending {
    /// `PL`.
    pub suffix: String,
    /// The productive ending, IPA; empty means none at all.
    pub ending: String,
}

/// One sound change, as it reached a language.
#[derive(Debug, Clone, Serialize)]
pub struct SoundChange {
    /// When.
    pub century: u32,
    /// In which language (an ancestor, or this one).
    pub language: String,
    /// Its name.
    pub name: String,
    /// The rules.
    pub rules: Vec<String>,
    /// Typed by a person?
    pub by_hand: bool,
    /// Words it changed there.
    pub words: usize,
}

/// One language.
#[derive(Debug, Clone, Serialize)]
pub struct LangExport {
    /// Id within the family.
    pub id: u16,
    /// Name.
    pub name: String,
    /// Parent id.
    pub parent: Option<u16>,
    /// `spoken`, `split` or `silent`.
    pub fate: &'static str,
    /// Born.
    pub born: u32,
    /// Stopped changing, if it has.
    pub ended: Option<u32>,
    /// The segments its words use.
    pub inventory: Vec<String>,
    /// Sound changes from the proto-language down, in order (leaves only).
    pub sound_changes: Vec<SoundChange>,
    /// Productive endings.
    pub endings: Vec<Ending>,
    /// Inflected forms.
    pub paradigms: Vec<ParadigmEntry>,
    /// Every word.
    pub lexicon: Vec<LexEntry>,
}

/// A whole family.
#[derive(Debug, Clone, Serialize)]
pub struct FamilyExport {
    /// [`FORMAT`].
    pub format: &'static str,
    /// The family's title.
    pub title: String,
    /// The seed word.
    pub seed: String,
    /// Centuries passed.
    pub century: u32,
    /// When it ends.
    pub span: Option<u32>,
    /// The vocabulary.
    pub vocabulary: Vec<VocabEntry>,
    /// The tree, as Mermaid source.
    pub tree_mermaid: String,
    /// Every language.
    pub languages: Vec<LangExport>,
}

const fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Word => "word",
        Kind::Suffix => "suffix",
        Kind::Inflected { .. } => "inflected",
    }
}

const fn fate_name(f: Fate) -> &'static str {
    match f {
        Fate::Spoken => "spoken",
        Fate::Split(_) => "split",
        Fate::Silent(_) => "silent",
    }
}

const fn ended(f: Fate) -> Option<u32> {
    match f {
        Fate::Spoken => None,
        Fate::Split(c) | Fate::Silent(c) => Some(c),
    }
}

const fn is_leaf(l: &Language) -> bool {
    l.children.is_empty()
}

fn word_of(l: &Language, m: Meaning) -> String {
    l.word(m).map(Word::to_string).unwrap_or_default()
}

/// The family tree as Mermaid `graph LR` source.
pub fn mermaid(w: &World) -> String {
    let mut s = String::from("graph LR\n");
    for l in &w.langs {
        let span = format!(
            "c.{}–{}",
            l.born,
            ended(l.fate).map_or_else(|| "now".to_owned(), |c| format!("c.{c}"))
        );
        let dagger = if matches!(l.fate, Fate::Silent(_)) {
            " †"
        } else {
            ""
        };
        let _ = writeln!(
            s,
            "  L{}[\"{}{}<br/>{}\"]",
            l.id,
            l.name().replace('"', "'"),
            dagger,
            span
        );
    }
    for l in &w.langs {
        for c in &l.children {
            let _ = writeln!(s, "  L{} --> L{}", l.id, c);
        }
    }
    s
}

fn sound_changes(w: &World, id: LangId) -> Vec<SoundChange> {
    let mut out = Vec::new();
    for anc in w.lineage(id) {
        let Ok(l) = w.lang(anc) else { continue };
        for e in &l.history {
            if let Happening::Sound {
                name,
                rules,
                source,
            } = &e.what
            {
                out.push(SoundChange {
                    century: e.century,
                    language: l.name(),
                    name: name.clone(),
                    rules: rules.iter().map(ToString::to_string).collect(),
                    by_hand: matches!(source, Source::Hand { .. }),
                    words: e.changes.len(),
                });
            }
        }
    }
    out
}

fn inventory(l: &Language) -> Vec<Seg> {
    let mut v: Vec<Seg> = l.lexicon.iter().flat_map(|w| w.0.iter().copied()).collect();
    v.sort();
    v.dedup();
    v
}

/// The structured export.
pub fn family(w: &World, title: &str) -> FamilyExport {
    let languages = w
        .langs
        .iter()
        .map(|l| {
            let leaf = is_leaf(l);
            let lexicon = w
                .vocab
                .all()
                .map(|m| LexEntry {
                    gloss: w.gloss(m).to_owned(),
                    ipa: word_of(l, m),
                    roman: l
                        .word(m)
                        .map(|x| transliterate(x.segs()))
                        .unwrap_or_default(),
                    proto: w
                        .proto
                        .get(m.index())
                        .map(Word::to_string)
                        .unwrap_or_default(),
                    history: if leaf {
                        w.etymology(l.id, m)
                            .into_iter()
                            .map(|s| HistoryStep {
                                century: s.century,
                                language: s.lang_name,
                                cause: s.cause,
                                rule: s.rule,
                                by_hand: s.hand.is_some(),
                                from: s.before.to_string(),
                                to: s.after.to_string(),
                            })
                            .collect()
                    } else {
                        Vec::new()
                    },
                })
                .collect();
            let paradigms = w
                .vocab
                .paradigm_entries()
                .map(|(m, _, _)| {
                    let regular = w.regular(l.id, m);
                    ParadigmEntry {
                        gloss: w.gloss(m).to_owned(),
                        ipa: word_of(l, m),
                        irregular: regular.as_ref().is_some_and(|r| l.word(m) != Some(r)),
                        regular: regular.map(|r| r.to_string()),
                    }
                })
                .collect();
            let endings = w
                .vocab
                .all()
                .filter(|&m| w.vocab.kind(m) == Kind::Suffix)
                .map(|m| Ending {
                    suffix: w.gloss(m).to_owned(),
                    ending: w
                        .productive(l.id, m)
                        .map(|t| phon::spell(&t))
                        .unwrap_or_default(),
                })
                .collect();
            LangExport {
                id: l.id.0,
                name: l.name(),
                parent: l.parent.map(|p| p.0),
                fate: fate_name(l.fate),
                born: l.born,
                ended: ended(l.fate),
                inventory: inventory(l).iter().map(|s| s.glyph().to_owned()).collect(),
                sound_changes: if leaf {
                    sound_changes(w, l.id)
                } else {
                    Vec::new()
                },
                endings,
                paradigms,
                lexicon,
            }
        })
        .collect();
    FamilyExport {
        format: FORMAT,
        title: title.to_owned(),
        seed: w.seed_word.clone(),
        century: w.century,
        span: w.span,
        vocabulary: w
            .vocab
            .all()
            .map(|m| VocabEntry {
                gloss: w.gloss(m).to_owned(),
                kind: kind_name(w.vocab.kind(m)),
            })
            .collect(),
        tree_mermaid: mermaid(w),
        languages,
    }
}

/// The structured export, as pretty JSON.
pub fn json(w: &World, title: &str) -> String {
    serde_json::to_string_pretty(&family(w, title))
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
        .replace('*', "\\*")
        .replace('_', "\\_")
}

fn code(s: &str) -> String {
    format!("`{}`", s.replace('`', "'"))
}

/// One language's chapter of the grammar book, on its own.
pub fn language_markdown(w: &World, title: &str, id: LangId) -> Option<String> {
    let fam = family(w, title);
    let l = fam.languages.iter().find(|l| l.id == id.0)?;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "*From “{}”, grown by Asterisk from the word “{}”, at c.{}.*\n",
        md_escape(title),
        md_escape(&fam.seed),
        fam.century
    );
    write_language(&mut s, w, l);
    Some(s)
}

/// The grammar book, as Markdown.
pub fn markdown(w: &World, title: &str) -> String {
    let fam = family(w, title);
    let mut s = String::new();
    let leaves: Vec<&LangExport> = fam
        .languages
        .iter()
        .filter(|l| l.fate != "split" && w.lang(LangId(l.id)).is_ok_and(is_leaf))
        .collect();
    let spoken = leaves.iter().filter(|l| l.fate == "spoken").count();
    let _ = writeln!(s, "# {}\n", md_escape(title));
    let _ = writeln!(
        s,
        "*Grown from the word “{}” by Asterisk. {} centuries{}; {} languages ever, {} spoken now. \
         Every form is IPA; a starred form is an ancestor's.*\n",
        md_escape(&fam.seed),
        fam.century,
        fam.span.map_or_else(String::new, |sp| format!(" of {sp}")),
        fam.languages.len(),
        spoken
    );
    let _ = writeln!(s, "## Contents\n");
    let _ = writeln!(
        s,
        "- [The family tree](#the-family-tree)\n- [Cognates](#cognates)\n- [The proto-language](#the-proto-language)"
    );
    for l in &leaves {
        let _ = writeln!(s, "- [{}](#{})", md_escape(&l.name), anchor(&l.name));
    }
    let _ = writeln!(
        s,
        "\n## The family tree\n\n```mermaid\n{}```\n",
        fam.tree_mermaid
    );

    // Cognates: the most-drifted words across every leaf.
    let _ = writeln!(s, "## Cognates\n");
    let _ = writeln!(
        s,
        "The core vocabulary in every language at the ends of the tree, spoken or silent.\n"
    );
    let mut header = String::from("| meaning | proto |");
    let mut rule = String::from("|---|---|");
    for l in &leaves {
        let _ = write!(header, " {} |", md_escape(&l.name));
        rule.push_str("---|");
    }
    let _ = writeln!(s, "{header}\n{rule}");
    for m in w
        .vocab
        .all()
        .filter(|&m| w.vocab.kind(m) == Kind::Word)
        .take(120)
    {
        let mut row = format!(
            "| {} | \\*{} |",
            md_escape(w.gloss(m)),
            w.proto
                .get(m.index())
                .map(Word::to_string)
                .unwrap_or_default()
        );
        for l in &leaves {
            let f = l
                .lexicon
                .get(m.index())
                .map(|e| e.ipa.clone())
                .unwrap_or_default();
            let _ = write!(row, " {f} |");
        }
        let _ = writeln!(s, "{row}");
    }
    let _ = writeln!(s);

    // The proto-language.
    let _ = writeln!(s, "## The proto-language\n");
    if let Some(root) = fam.languages.first() {
        let _ = writeln!(
            s,
            "{} — spoken from c.0 until it split. Its inventory: {}.\n",
            md_escape(&root.name),
            root.inventory.join(" ")
        );
    }
    let _ = writeln!(s, "| meaning | form | romanised |\n|---|---|---|");
    for m in w.vocab.all() {
        let p = w.proto.get(m.index());
        let _ = writeln!(
            s,
            "| {} | \\*{} | {} |",
            md_escape(w.gloss(m)),
            p.map(Word::to_string).unwrap_or_default(),
            p.map(|x| transliterate(x.segs())).unwrap_or_default()
        );
    }
    let _ = writeln!(s);

    for l in &leaves {
        write_language(&mut s, w, l);
    }
    s
}

fn anchor(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c)
            } else if c == ' ' || c == '-' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn write_language(s: &mut String, w: &World, l: &LangExport) {
    let _ = writeln!(s, "## {}\n", md_escape(&l.name));
    let lineage: Vec<String> = w
        .lineage(LangId(l.id))
        .iter()
        .filter_map(|a| w.lang(*a).ok().map(Language::name))
        .collect();
    let status = match (l.fate, l.ended) {
        ("silent", Some(c)) => format!("spoken c.{}–c.{c}; no one speaks it now", l.born),
        _ => format!("spoken since c.{}", l.born),
    };
    let _ = writeln!(
        s,
        "*{}.* Descent: {}.\n",
        status,
        lineage
            .iter()
            .map(|n| md_escape(n))
            .collect::<Vec<_>>()
            .join(" › ")
    );

    let (cons, vows): (Vec<&String>, Vec<&String>) = l.inventory.iter().partition(|g| {
        phon::parse_segments(g)
            .and_then(|v| v.first().copied())
            .is_some_and(|x| !x.is_vowel())
    });
    let _ = writeln!(
        s,
        "### Sounds\n\nConsonants: {}  \nVowels: {}\n",
        cons.iter()
            .map(|g| g.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        vows.iter()
            .map(|g| g.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    );

    let _ = writeln!(s, "### Sound changes, in order\n");
    if l.sound_changes.is_empty() {
        let _ = writeln!(s, "None yet.\n");
    }
    for (i, c) in l.sound_changes.iter().enumerate() {
        let rules: Vec<String> = c.rules.iter().map(|r| code(r)).collect();
        let who = if c.by_hand { " — *by hand*" } else { "" };
        let _ = writeln!(
            s,
            "{}. c.{} · {} — {}: {} ({} word{}){}",
            i + 1,
            c.century,
            md_escape(&c.language),
            md_escape(&c.name),
            rules.join("; "),
            c.words,
            if c.words == 1 { "" } else { "s" },
            who
        );
    }
    let _ = writeln!(s);

    write_grammar(s, l);
    write_lexicon(s, l);
}

fn write_grammar(s: &mut String, l: &LangExport) {
    if !l.paradigms.is_empty() {
        let _ = writeln!(s, "### Grammar\n");
        for e in &l.endings {
            let ending = if e.ending.is_empty() {
                "nothing at all (zero-marked)".to_owned()
            } else {
                code(&format!("-{}", e.ending))
            };
            let _ = writeln!(s, "- **{}** is marked by {}", e.suffix, ending);
        }
        let irregular = l.paradigms.iter().filter(|p| p.irregular).count();
        let _ = writeln!(
            s,
            "\n{} of {} inflected forms are irregular.\n",
            irregular,
            l.paradigms.len()
        );
        let _ = writeln!(s, "| form | actual | by analogy |\n|---|---|---|");
        for p in &l.paradigms {
            let mark = if p.irregular { " **(irregular)**" } else { "" };
            let _ = writeln!(
                s,
                "| {} | {}{} | {} |",
                md_escape(&p.gloss),
                p.ipa,
                mark,
                p.regular.clone().unwrap_or_default()
            );
        }
        let _ = writeln!(s);
    }
}

fn write_lexicon(s: &mut String, l: &LangExport) {
    let silent = l.fate == "silent";
    if silent {
        let _ = writeln!(
            s,
            "### Last words\n\nThe core of its vocabulary when it fell silent.\n\n| meaning | word | romanised | from |\n|---|---|---|---|"
        );
        for e in l.lexicon.iter().take(60) {
            let _ = writeln!(
                s,
                "| {} | {} | {} | \\*{} |",
                md_escape(&e.gloss),
                e.ipa,
                md_escape(&e.roman),
                e.proto
            );
        }
        let _ = writeln!(s);
        return;
    }
    let _ = writeln!(
        s,
        "### Lexicon\n\n| meaning | word | romanised | from | how |\n|---|---|---|---|---|"
    );
    for e in &l.lexicon {
        let how: Vec<String> = e
            .history
            .iter()
            .map(|h| {
                let why = h.rule.as_ref().map_or_else(|| h.cause.clone(), |r| code(r));
                format!("→ {} ({}, c.{})", h.to, why, h.century)
            })
            .collect();
        let _ = writeln!(
            s,
            "| {} | {} | {} | \\*{} | {} |",
            md_escape(&e.gloss),
            e.ipa,
            md_escape(&e.roman),
            e.proto,
            if how.is_empty() {
                "unchanged".to_owned()
            } else {
                how.join(" ").replace('|', "\\|")
            }
        );
    }
    let _ = writeln!(s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_has_every_part() {
        let w = World::new("tolkien");
        let md = markdown(&w, "The Tolkien Family");
        assert!(md.starts_with("# The Tolkien Family"));
        for part in [
            "```mermaid",
            "graph LR",
            "## Cognates",
            "## The proto-language",
            "### Sound changes, in order",
            "### Grammar",
            "### Lexicon",
            "irregular",
        ] {
            assert!(md.contains(part), "missing {part}");
        }
        for l in w.spoken() {
            assert!(md.contains(&format!("## {}", w.langs[l.index()].name())));
        }
    }

    #[test]
    fn json_round_trips_through_serde_and_matches_the_world() {
        let w = World::new("beren");
        let v: serde_json::Value = serde_json::from_str(&json(&w, "Beren")).unwrap_or_default();
        assert_eq!(v["format"], FORMAT);
        assert_eq!(v["century"], w.century);
        let langs = v["languages"].as_array().map_or(0, Vec::len);
        assert_eq!(langs, w.langs.len());
        let first = &v["languages"][w.spoken()[0].index()];
        assert_eq!(
            first["lexicon"].as_array().map_or(0, Vec::len),
            w.vocab.len()
        );
        // A leaf's history ends at its current form.
        for e in first["lexicon"].as_array().into_iter().flatten() {
            if let Some(last) = e["history"].as_array().and_then(|h| h.last()) {
                assert_eq!(last["to"], e["ipa"]);
            }
        }
    }

    #[test]
    fn mermaid_names_every_edge() {
        let w = World::new("luthien");
        let m = mermaid(&w);
        let edges = m.lines().filter(|l| l.contains("-->")).count();
        assert_eq!(edges, w.langs.len() - 1);
    }

    #[test]
    fn transliteration_is_lower_case() {
        let segs = phon::parse_segments("ʃaːŋ").unwrap_or_default();
        assert_eq!(transliterate(&segs), "shāng");
    }
}
