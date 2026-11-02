//! Pages, fragments, and the family tree drawn as SVG.
//!
//! The look is a night library: ink-dark ground, parchment text, and the
//! asterisk in gold. Every language has a hue; daughters keep close to their
//! mother's, so a whole branch reads as one colour family. A word's changes
//! are beads on the branch where they happened, and a bead struck by hand is
//! in the colour of the hand that struck it.
//!
//! Autumn sets `script-src 'self'`, so the script is served from
//! `/asterisk.js` and wired by `data-` attributes, never inline.

use asterisk::lexicon::{Kind, Meaning, Vocabulary};
use asterisk::library::LIBRARY;
use asterisk::phon::{self, Edit};
use asterisk::rule::{Class, MAX_LEN, Test};
use asterisk::world::{
    Event, Fate, FeedLine, Happening, LangId, Language, Source, Step, WordChange, World,
};
use std::fmt::Write as _;

use autumn_web::{Markup, PreEscaped, html};

use crate::atlas::Family;

/// The stylesheet.
pub const STYLE: &str = include_str!("asterisk.css");
/// The script.
pub const SCRIPT: &str = include_str!("asterisk.js");

/// The meaning the tree shows when nobody has picked one and nothing has
/// drifted yet.
pub const DEFAULT_MEANING: &str = "moon";

/// Glyphs on the workbench palette.
const PALETTE: &[&str] = &[
    "∅", "#", "_", "ʃ", "ʒ", "tʃ", "dʒ", "θ", "ð", "ɲ", "ŋ", "ʎ", "x", "ɣ", "χ", "ʔ", "ə", "ɛ",
    "ɔ", "æ", "ɨ", "y", "ø", "œ", "ː", "\u{303}",
];

/// Rules to try, one click away.
const TRY: &[&str] = &[
    "V > ∅ / C_#",
    "k > tʃ / _[+front]",
    "P > [+cont] / V_V",
    "a > o",
    "s > h / #_",
    "[+stop -voice] > [+voice]",
    "V > [+nasal] / _N",
    "e > je",
    "C > ∅ / #_C",
    "[+long] > [-long]",
];

/// An example word list for the new-family form.
pub const EXAMPLE_LIST: &str = "# One meaning per line. Fix a proto-form with `= ipa`.\n\
moon = mena\nstar\nsea\nship\nsong\nking\nqueen\nsword\nlight\ndark\n\
# Suffixes are glossed in capitals; inflected forms are base.SUFFIX\n\
PL = ni\nstar.PL\nship.PL\nking.PL\n";

fn hsl(hue: u16, s: u8, l: u8) -> String {
    format!("hsl({hue} {s}% {l}%)")
}

/// Percent-encode a path segment (glosses can hold spaces).
pub fn seg(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn page(title: &str, here: &str, csrf: Option<&str>, data_page: &str, content: &Markup) -> Markup {
    let nav = [
        ("/", "Families"),
        ("/new", "Grow one"),
        ("/about", "The notation"),
    ];
    html! {
        (PreEscaped("<!DOCTYPE html>"))
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " — Asterisk" }
                @if let Some(t) = csrf { meta name="csrf-token" content=(t); }
                style { (PreEscaped(STYLE)) }
                script src="/asterisk.js" defer {}
            }
            body data-page=(data_page) {
                div class="wrap" {
                    header class="top" {
                        h1 { a href="/" { span class="star" { "*" } "Asterisk" } }
                        span class="tag" { "language families that drift while you watch" }
                        nav {
                            @for (href, label) in nav {
                                a href=(href) class=(if href == here { "here" } else { "" }) { (label) }
                            }
                        }
                    }
                    (content)
                    footer {
                        "A starred form is a word nobody ever heard: a reconstruction. Here they are "
                        "invented, and time runs forwards from them. Every word in every language is "
                        "one to " (MAX_LEN) " sounds long, for ever — proved in Verus. Every family is "
                        "its seed and its log of commands, replayed at every boot."
                    }
                }
            }
        }
    }
}

// ── The tree ────────────────────────────────────────────────────────────────

const VIEW_W: f64 = 1100.0;
const LEFT: f64 = 24.0;
const RIGHT: f64 = 250.0;
const TOP: f64 = 44.0;
const ROW: f64 = 50.0;

fn f(n: usize) -> f64 {
    u32::try_from(n).map_or_else(|_| f64::from(u32::MAX), f64::from)
}

struct Placed {
    x0: f64,
    x1: f64,
    y: f64,
}

fn xpos(c: u32, span: f64) -> f64 {
    (f64::from(c) / span).mul_add(VIEW_W - LEFT - RIGHT, LEFT)
}

fn layout(w: &World) -> (Vec<Placed>, f64) {
    let span = f64::from(w.century.max(1));
    let mut ys = vec![0.0_f64; w.langs.len()];
    let leaves = w.leaf_order();
    for (k, id) in leaves.iter().enumerate() {
        if let Some(y) = ys.get_mut(id.index()) {
            *y = f(k).mul_add(ROW, TOP);
        }
    }
    // Children have larger ids than their parents, so a reverse sweep sees
    // every child before its parent.
    for l in w.langs.iter().rev() {
        if let (Some(a), Some(b)) = (l.children.first(), l.children.last()) {
            let ya = ys.get(a.index()).copied().unwrap_or(TOP);
            let yb = ys.get(b.index()).copied().unwrap_or(TOP);
            if let Some(y) = ys.get_mut(l.id.index()) {
                *y = f64::midpoint(ya, yb);
            }
        }
    }
    let placed = w
        .langs
        .iter()
        .map(|l| Placed {
            x0: xpos(l.born, span),
            x1: xpos(l.until(w.century), span),
            y: ys.get(l.id.index()).copied().unwrap_or(TOP),
        })
        .collect();
    let height = f(leaves.len()).mul_add(ROW, TOP + 30.0);
    (placed, height)
}

fn n1(v: f64) -> String {
    format!("{v:.1}")
}

fn word(l: &Language, m: Meaning) -> String {
    l.word(m).map(ToString::to_string).unwrap_or_default()
}

/// The family tree for one meaning, as an SVG fragment with its caption.
pub fn tree(base: &str, w: &World, m: Meaning) -> Markup {
    let (placed, height) = layout(w);
    let span = f64::from(w.century.max(1));
    let x = |c: u32| xpos(c, span);
    let step = if w.century > 120 {
        40
    } else if w.century > 60 {
        20
    } else if w.century > 30 {
        10
    } else {
        5
    };
    let at = |id: LangId| placed.get(id.index());
    let gloss = w.gloss(m).to_owned();
    let proto = w
        .proto
        .get(m.index())
        .map(ToString::to_string)
        .unwrap_or_default();
    html! {
        div class="tree-head" {
            span class="gloss" { "‘" (gloss) "’" }
            span class="proto" { "*" (proto) }
            span class="count" { (w.spoken().len()) " languages spoken · " (w.langs.len()) " ever · c. " (w.century) }
        }
        svg class="tree" viewBox=(format!("0 0 {VIEW_W} {}", n1(height))) role="img"
            aria-label=(format!("Family tree: the word for {gloss} in every language")) {
            g class="axis" {
                @for c in (0..=w.century).step_by(step) {
                    line x1=(n1(x(c))) x2=(n1(x(c))) y1="16" y2=(n1(height - 18.0)) {}
                    text x=(n1(x(c))) y=(n1(height - 4.0)) { "c." (c) }
                }
            }
            @for l in &w.langs {
                @if let Some(p) = at(l.id) {
                    @let col = hsl(l.hue, 62, 64);
                    @let href = format!("{base}/lang/{}/word/{}", l.id, seg(&gloss));
                    g class=(if l.spoken() { "lang" } else if matches!(l.fate, Fate::Silent(_)) { "lang silent" } else { "lang dead" }) {
                        line class="bar" x1=(n1(p.x0)) x2=(n1(p.x1)) y1=(n1(p.y)) y2=(n1(p.y)) stroke=(col) {}
                        @if let (Some(a), Some(b)) = (l.children.first().and_then(|c| at(*c)), l.children.last().and_then(|c| at(*c))) {
                            line class="fork" x1=(n1(p.x1)) x2=(n1(p.x1)) y1=(n1(a.y)) y2=(n1(b.y)) stroke=(col) {}
                        }
                        @for e in l.history.iter().filter(|e| e.changes.iter().any(|c| c.meaning == m)) {
                            @let hand = match &e.what { Happening::Sound { source: Source::Hand { hue }, .. } => Some(*hue), _ => None };
                            circle class=(if hand.is_some() { "bead hand" } else { "bead" })
                                cx=(n1(x(e.century))) cy=(n1(p.y)) r=(if hand.is_some() { "5" } else { "3.6" })
                                fill=(hand.map_or_else(|| "var(--paper)".to_owned(), |h| hsl(h, 85, 62))) {
                                title { (bead_title(w, e, m)) }
                            }
                        }
                        @if l.parent.is_none() {
                            text class="proto-label" x=(n1(p.x0)) y=(n1(p.y - 12.0)) { "*" (proto) }
                        }
                        @if l.children.is_empty() {
                            @let fresh = l.history.last().is_some_and(|e| e.century == w.century && e.changes.iter().any(|c| c.meaning == m));
                            a href=(href) {
                                text class=(if fresh { "form fresh" } else { "form" }) x=(n1(p.x1 + 12.0)) y=(n1(p.y + 2.0)) fill=(col) {
                                    (word(l, m))
                                    @if matches!(l.fate, Fate::Silent(_)) { " †" }
                                }
                                text class="name" x=(n1(p.x1 + 12.0)) y=(n1(p.y + 19.0)) { (l.name()) }
                            }
                        } @else if l.parent.is_some() || l.word(m) != w.proto.get(m.index()) {
                            a href=(href) {
                                text class="mid-label" x=(n1(p.x1 - 6.0)) y=(n1(p.y - 8.0)) { "*" (word(l, m)) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn bead_title(w: &World, e: &Event, m: Meaning) -> String {
    let change = e.changes.iter().find(|c| c.meaning == m);
    let what = match &e.what {
        Happening::Sound { name, source, .. } => match source {
            Source::Drift => name.clone(),
            Source::Hand { .. } => format!("by hand: {name}"),
        },
        Happening::Renewal { partner, .. } => format!("renewed with ‘{}’", w.gloss(*partner)),
        Happening::Loan { .. } => "borrowed".to_owned(),
        Happening::Levelling { .. } => "levelled by analogy".to_owned(),
    };
    change.map_or_else(String::new, |c| {
        format!("c.{} · {what} · {} → {}", e.century, c.before, c.after)
    })
}

/// The chronicle, newest first.
pub fn feed(base: &str, lines: &[FeedLine], now: u32) -> Markup {
    html! {
        ol class="feed" {
            @for l in lines.iter().rev().take(28) {
                li class=(format!("k-{}{}", l.kind, if l.century == now { " new" } else { "" })) {
                    span class="dot" style=(format!("background:{}", hsl(l.hue, 70, 62))) {}
                    span class="when" { "c." (l.century) }
                    span class="what" {
                        @if let Some(id) = l.lang { a href=(format!("{base}/lang/{id}")) { (l.text) } } @else { (l.text) }
                        @if let Some(ex) = &l.example { span class="ex" { (ex) } }
                    }
                }
            }
        }
    }
}

/// The workbench's language list.
pub fn targets(w: &World) -> Markup {
    html! {
        select name="target" id="target" {
            option value="all" { "every spoken language" }
            @for id in w.leaf_order() {
                @if let Ok(l) = w.lang(id) {
                    @if l.spoken() { option value=(id) { (l.name()) } }
                }
            }
        }
    }
}

/// The words most worth looking at first: the most drifted, then a few
/// everyone wants.
fn featured(w: &World, n: usize) -> Vec<Meaning> {
    let spoken = w.spoken();
    let mut scored: Vec<(usize, Meaning)> = w
        .vocab
        .all()
        .filter(|&m| w.vocab.kind(m) == Kind::Word)
        .map(|m| {
            let mut forms: Vec<String> = spoken
                .iter()
                .filter_map(|id| w.langs.get(id.index()).map(|l| word(l, m)))
                .collect();
            forms.sort();
            forms.dedup();
            (forms.len(), m)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut out: Vec<Meaning> = scored.into_iter().take(n).map(|(_, m)| m).collect();
    for g in [
        "moon", "sun", "water", "mother", "fire", "star", "tongue", "foot.PL",
    ] {
        if let Some(m) = w.vocab.named(g)
            && !out.contains(&m)
        {
            out.push(m);
        }
    }
    out
}

fn picker(w: &World, current: Meaning) -> Markup {
    html! {
        div class="picker" {
            div class="chips" id="meanings" {
                @for m in featured(w, 14) {
                    @let g = w.gloss(m);
                    a class=(if m == current { "chip on" } else { "chip" }) href=(format!("?m={}", seg(g))) data-m=(g) { (g) }
                }
            }
            label class="find" {
                span { "any of " (w.vocab.len()) " words " }
                input id="find" list="glosses" placeholder="foot.PL, river, you all…" autocomplete="off";
                datalist id="glosses" { @for m in w.vocab.all() { option value=(w.gloss(m)) {} } }
            }
        }
    }
}

/// One family's page.
pub fn family_page(fam: &Family, w: &World, m: Meaning, csrf: Option<&str>) -> Markup {
    let base = fam.base();
    let progress = w.span.map(|s| (w.century, s));
    page(
        &fam.row.title,
        "",
        csrf,
        "family",
        &html! {
            section class="clock" data-base=(base) data-family=(fam.row.slug) {
                div class="century" { span class="c" { "c." } span id="century" { (w.century) } }
                div class="seedline" {
                    strong { (fam.row.title) } br;
                    "seeded from " em { "“" (w.seed_word) "”" } " · " (w.vocab.len()) " meanings"
                    @if let Some((c, s)) = progress {
                        " · " span id="span" { (c) " of " (s) " centuries" }
                    }
                }
                @if w.complete() {
                    span class="complete" { "the chronicle is complete" }
                } @else {
                    button id="drift" class=(if fam.drifting() { "drift on" } else { "drift" }) data-on=(fam.drifting()) {
                        @if fam.drifting() { "time is running" } @else { "time is stopped" }
                    }
                    button id="once" { "+1 century" }
                }
                div class="exports" {
                    a href=(format!("{base}/export.md")) download { "export .md" }
                    a href=(format!("{base}/export.json")) download { "export .json" }
                }
            }
            @if let Some(d) = &fam.diverged { p class="warn" { "This family rebuilt differently from its log: " (d) } }
            @if let Some((c, s)) = progress {
                div class="span-bar" { div style=(format!("width:{:.1}%", f64::from(c.min(s)) * 100.0 / f64::from(s.max(1)))) {} }
            }
            (picker(w, m))
            section class="stage" data-m=(w.gloss(m)) {
                div id="tree" { (tree(&base, w, m)) }
            }
            section class="lower" {
                div class="bench" {
                    h2 { "The workbench" }
                    p class="hint" {
                        "Write a sound change the way linguists do — " code { "target > result / left_right" }
                        " — preview what it would do, then make it happen. It joins this family's log; "
                        "everyone watching sees it, struck in your colour."
                    }
                    form id="workbench" {
                        textarea name="rule" id="rule" rows="2" spellcheck="false" placeholder="k > tʃ / _[+front]" {}
                        div class="palette" {
                            @for g in PALETTE {
                                button type="button" data-glyph=(g) title=(format!("insert {g}")) {
                                    @if *g == "\u{303}" { "◌̃" } @else { (g) }
                                }
                            }
                        }
                        div class="row" {
                            div id="targets" { (targets(w)) }
                            button type="button" id="preview" { "preview" }
                            button type="button" id="apply" class="primary" disabled[w.complete()] { "apply" }
                            span id="me" class="me" title="your colour" {}
                        }
                        div class="try" {
                            span { "try " }
                            @for r in TRY { button type="button" data-rule=(r) { (r) } }
                        }
                    }
                    div id="result" {}
                }
                div class="chronicle" {
                    h2 { "The chronicle" }
                    div id="feed" { (feed(&base, &w.feed, w.century)) }
                }
            }
        },
    )
}

/// What a rule would do, language by language.
pub fn preview(w: &World, rows: &[(LangId, Vec<WordChange>)], rule: &str, sent: bool) -> Markup {
    let total: usize = rows.iter().map(|(_, c)| c.len()).sum();
    html! {
        div class="preview" {
            p class="verdict" {
                code { (rule) }
                @if total == 0 { " would change nothing here." }
                @else { " would change " strong { (total) } " word" @if total != 1 { "s" } ":" }
                @if sent { " " span class="sent" { "sent to the chronicle" } }
            }
            @for (id, changes) in rows {
                @if !changes.is_empty() {
                    @if let Ok(l) = w.lang(*id) {
                        div class="pv-lang" {
                            h3 style=(format!("color:{}", hsl(l.hue, 62, 66))) { (l.name()) span class="n" { " · " (changes.len()) } }
                            ul {
                                @for c in changes {
                                    li { span class="g" { (w.gloss(c.meaning)) } span class="b" { (c.before) } " → " span class="a" { (c.after) } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A rule that would not parse.
pub fn rule_error(msg: &str) -> Markup {
    html! { div class="preview error" { p { (msg) } } }
}

fn lineage(base: &str, w: &World, id: LangId) -> Markup {
    html! {
        nav class="lineage" {
            a href=(base) { "the tree" } span class="sep" { " › " }
            @for (i, a) in w.lineage(id).iter().enumerate() {
                @if i > 0 { span class="sep" { " › " } }
                @if let Ok(l) = w.lang(*a) {
                    a href=(format!("{base}/lang/{a}")) style=(format!("color:{}", hsl(l.hue, 62, 66))) { (l.name()) }
                }
            }
        }
    }
}

fn fate(l: &Language, now: u32) -> String {
    match l.fate {
        Fate::Spoken => format!(
            "spoken since c.{} — {} centuries",
            l.born,
            now.saturating_sub(l.born)
        ),
        Fate::Split(c) => format!("spoken c.{}–c.{}, then split", l.born, c),
        Fate::Silent(c) => format!("spoken c.{}–c.{}; no one speaks it now", l.born, c),
    }
}

fn grammar(base: &str, w: &World, id: LangId) -> Markup {
    let Ok(l) = w.lang(id) else { return html! {} };
    let suffixes: Vec<Meaning> = w
        .vocab
        .all()
        .filter(|&m| w.vocab.kind(m) == Kind::Suffix)
        .collect();
    if suffixes.is_empty() {
        return html! {};
    }
    let irregular = w.irregulars(id);
    html! {
        section class="gram" {
            h2 { "Grammar" }
            p class="hint" {
                "Every inflected form was born as its base plus its suffix, then drifted as one word. "
                "Speakers keep rebuilding the odd ones by analogy with the ending most forms share — "
                "the ones that resist are its irregulars. " strong { (irregular.len()) } " of "
                (w.vocab.paradigm_entries().count()) " are irregular now."
            }
            ul class="endings" {
                @for s in &suffixes {
                    @let ending = w.productive(id, *s).map(|t| phon::spell(&t)).unwrap_or_default();
                    li { strong { (w.gloss(*s)) } " is marked by "
                        @if ending.is_empty() { em { "nothing at all" } } @else { span class="f" { "-" (ending) } } }
                }
            }
            div class="paradigms" {
                @for (m, b, _) in w.vocab.paradigm_entries() {
                    @let irr = irregular.contains(&m);
                    a class=(if irr { "para irr" } else { "para" }) href=(format!("{base}/lang/{id}/word/{}", seg(w.gloss(m)))) {
                        span class="g" { (w.gloss(m)) }
                        span class="base" { (word(l, b)) }
                        span class="f" { (word(l, m)) }
                        @if irr { span class="n" { "by analogy " (w.regular(id, m).map(|r| r.to_string()).unwrap_or_default()) } }
                    }
                }
            }
        }
    }
}

/// One language: its lexicon against the proto-language, grammar, history.
pub fn language_page(fam: &Family, w: &World, id: LangId) -> Markup {
    let base = fam.base();
    let Ok(l) = w.lang(id) else {
        return not_found("no such language");
    };
    let col = hsl(l.hue, 62, 66);
    page(
        &l.name(),
        "",
        None,
        "lang",
        &html! {
            section class="lang-head" {
                (lineage(&base, w, id))
                h2 class="lang-name" style=(format!("color:{col}")) { (l.name()) }
                p class="fate" { (fate(l, w.century)) " · " (l.history.len()) " events · "
                    a href=(format!("{base}/lang/{id}/export.md")) download { "export this language" } }
            }
            (grammar(&base, w, id))
            section class="lexicon" {
                @for m in w.vocab.all().filter(|&m| !matches!(w.vocab.kind(m), Kind::Inflected { .. })) {
                    @let n = w.etymology(id, m).len();
                    a class="lex" href=(format!("{base}/lang/{id}/word/{}", seg(w.gloss(m)))) {
                        span class="g" { (w.gloss(m)) }
                        span class="p" { "*" (w.proto.get(m.index()).map(ToString::to_string).unwrap_or_default()) }
                        span class="f" style=(format!("color:{col}")) { (word(l, m)) }
                        span class="n" { @if n == 0 { "unchanged" } @else { (n) " change" @if n != 1 { "s" } } }
                    }
                }
            }
            section class="history" {
                h2 { "What happened to it" }
                @if l.history.is_empty() { p class="hint" { "Nothing yet. Give it time — or give it a rule." } }
                ol {
                    @for e in l.history.iter().rev().take(200) {
                        li {
                            span class="when" { "c." (e.century) }
                            @match &e.what {
                                Happening::Sound { name, rules, source } => {
                                    span class="what" {
                                        @if let Source::Hand { hue } = source {
                                            span class="hand" style=(format!("color:{}", hsl(*hue, 85, 64))) { "by hand " }
                                        } @else {
                                            (name) " "
                                        }
                                        @for r in rules { code { (r) } " " }
                                        span class="n" { (e.changes.len()) " words" }
                                    }
                                }
                                Happening::Renewal { meaning, partner } => {
                                    span class="what" { "‘" (w.gloss(*meaning)) "’ rebuilt as ‘" (w.gloss(*meaning)) "-" (w.gloss(*partner)) "’" }
                                }
                                Happening::Levelling { meaning } => {
                                    span class="what" { "‘" (w.gloss(*meaning)) "’ levelled by analogy" }
                                }
                                Happening::Loan { meaning, from } => {
                                    span class="what" { "‘" (w.gloss(*meaning)) "’ borrowed from "
                                        a href=(format!("{base}/lang/{from}")) { (w.lang(*from).map(Language::name).unwrap_or_default()) } }
                                }
                            }
                            @if let Some(c) = e.changes.first() {
                                span class="ex" { (w.gloss(c.meaning)) " " (c.before) " → " (c.after) }
                            }
                        }
                    }
                }
            }
        },
    )
}

fn step_row(base: &str, s: &Step, w: &World) -> Markup {
    let col = w.lang(s.lang).map_or(0, |l| l.hue);
    html! {
        li class="step" {
            div class="when" { "c." (s.century) }
            div class="body" {
                div class="forms" {
                    span class="b" { (s.before) } span class="arrow" { "→" } span class="a" style=(format!("color:{}", hsl(col, 62, 68))) { (s.after) }
                }
                div class="cause" {
                    a href=(format!("{base}/lang/{}", s.lang)) style=(format!("color:{}", hsl(col, 62, 66))) { (s.lang_name) }
                    " · "
                    @if let Some(h) = s.hand { span class="hand" style=(format!("color:{}", hsl(h, 85, 64))) { "by hand" } }
                    @else { (s.cause) }
                    @if let Some(r) = &s.rule { " " code { (r) } }
                }
            }
        }
    }
}

/// One word's whole life.
pub fn etymology_page(fam: &Family, w: &World, id: LangId, m: Meaning) -> Markup {
    let base = fam.base();
    let Ok(l) = w.lang(id) else {
        return not_found("no such language");
    };
    let steps = w.etymology(id, m);
    let col = hsl(l.hue, 62, 66);
    let gloss = w.gloss(m).to_owned();
    let proto = w
        .proto
        .get(m.index())
        .map(ToString::to_string)
        .unwrap_or_default();
    let regular = w.regular(id, m);
    page(
        &format!("‘{gloss}’ in {}", l.name()),
        "",
        None,
        "word",
        &html! {
            section class="ety" {
                (lineage(&base, w, id))
                p class="ety-gloss" { "the word for ‘" (gloss) "’" }
                div class="ety-ends" {
                    span class="p" { "*" (proto) }
                    span class="arrow" { "⟶" }
                    span class="f" style=(format!("color:{col}")) { (word(l, m)) }
                }
                @if let Some(r) = regular {
                    @if l.word(m) != Some(&r) {
                        p class="irregular-note" { "Irregular: analogy would make it " strong { (r) } "." }
                    }
                }
                p class="hint" {
                    (steps.len()) " step" @if steps.len() != 1 { "s" } " over " (l.until(w.century)) " centuries. "
                    "Every step names the rule that did it."
                }
                ol class="steps" {
                    li class="step origin" {
                        div class="when" { "c.0" }
                        div class="body" { div class="forms" { span class="a" { "*" (proto) } }
                            div class="cause" { "invented: the proto-language’s word for ‘" (gloss) "’" } }
                    }
                    @for s in &steps { (step_row(&base, s, w)) }
                }
            }
            section class="cognates" {
                h2 { "Its cognates" }
                p class="hint" { "The same starred word, in every language at the ends of the tree." }
                div class="cog-grid" {
                    @for other in w.leaf_order() {
                        @if let Ok(o) = w.lang(other) {
                            a class=(if other == id { "cog here" } else { "cog" }) href=(format!("{base}/lang/{other}/word/{}", seg(&gloss))) {
                                span class="f" style=(format!("color:{}", hsl(o.hue, 62, 66))) {
                                    (word(o, m))
                                    @if matches!(o.fate, Fate::Silent(_)) { " †" }
                                }
                                span class="name" { (o.name()) }
                            }
                        }
                    }
                }
                p class="more" {
                    @for g in ["sun", "water", "mother", "fire", "foot.PL", "tongue"] {
                        @if w.vocab.named(g).is_some() { a href=(format!("{base}/lang/{id}/word/{}", seg(g))) { (g) } " " }
                    }
                    a href=(format!("{base}?m={}", seg(&gloss))) { "see ‘" (gloss) "’ on the tree" }
                }
            }
        },
    )
}

/// One family on the gallery wall.
fn card(fam: &Family) -> Markup {
    let w = fam.world();
    let base = fam.base();
    let sample = w.liveliest();
    let leaves: Vec<LangId> = w
        .leaf_order()
        .into_iter()
        .filter(|id| w.lang(*id).is_ok_and(Language::spoken))
        .take(5)
        .collect();
    let pct = w
        .span
        .map(|s| f64::from(w.century.min(s)) * 100.0 / f64::from(s.max(1)));
    let status = if w.complete() {
        "complete"
    } else if fam.drifting() {
        "drifting"
    } else {
        "stopped"
    };
    html! {
        a class="card" href=(base) {
            div class="card-top" {
                span class="title" { (fam.row.title) }
                span class=(format!("status {status}")) { (status) }
            }
            div class="meta" { "“" (w.seed_word) "” · c." (w.century) " · " (w.spoken().len()) " spoken · " (w.vocab.len()) " meanings" }
            @if let Some(m) = sample {
                div class="sample" {
                    span class="g" { "‘" (w.gloss(m)) "’ " span class="p" { "*" (w.proto.get(m.index()).map(ToString::to_string).unwrap_or_default()) } }
                    div class="forms" {
                        @for id in &leaves {
                            @if let Ok(l) = w.lang(*id) {
                                span style=(format!("color:{}", hsl(l.hue, 62, 66))) { (word(l, m)) }
                            }
                        }
                    }
                }
            }
            @if let Some(p) = pct { div class="span-bar" { div style=(format!("width:{p:.1}%")) {} } }
        }
    }
}

/// The gallery.
pub fn gallery_page(families: &[std::sync::Arc<Family>]) -> Markup {
    page(
        "Families",
        "/",
        None,
        "gallery",
        &html! {
            section class="gallery-head" {
                h2 { "The families" }
                p class="hint" {
                    "Every family grows from one word and drifts for its span of centuries, even while "
                    "nobody watches — each one is a durable chronicle that survives restarts by replaying "
                    "its log. When it ends, take it home as Markdown or JSON."
                }
                a class="button primary" href="/new" { "grow a family" }
            }
            div class="cards" { @for fam in families { (card(fam)) } }
        },
    )
}

/// The form for a new family.
pub fn new_page(csrf: Option<&str>, standard: &Vocabulary) -> Markup {
    page(
        "Grow a family",
        "/new",
        csrf,
        "new",
        &html! {
            section class="newfam" {
                h2 { "Grow a family" }
                p class="hint" { "Any word grows a family. Give it a name, a span and a pace — and, if you like, the words it should keep." }
                form id="newform" {
                    div class="field" { label for="title" { "Name" } input id="title" name="title" maxlength="60" placeholder="The Tolkien family" required; }
                    div class="field" { label for="seed" { "Seed word" } input id="seed" name="seed" maxlength="40" placeholder="beren" required; }
                    div class="row" {
                        div class="field" { label for="span" { "Span" }
                            select id="span" name="span" {
                                option value="100" { "100 centuries" }
                                option value="300" selected { "300 centuries" }
                                option value="1000" { "1,000 centuries" }
                            }
                        }
                        div class="field" { label for="century_secs" { "One century every" }
                            select id="century_secs" name="century_secs" {
                                option value="2" { "2 seconds" }
                                option value="6" selected { "6 seconds" }
                                option value="60" { "minute" }
                                option value="3600" { "hour (overnight)" }
                            }
                        }
                    }
                    div class="field" {
                        label for="vocab" { "Words to keep " span class="hint" { "— leave empty for the standard " (standard.len()) ": Swadesh-207, extras, and a grammar of PL, PAST and GEN" } }
                        textarea id="vocab" name="vocab" rows="12" spellcheck="false" placeholder=(EXAMPLE_LIST) {}
                        div class="row" {
                            button type="button" id="use-example" { "use the example" }
                            button type="button" id="use-standard" data-text=(standard.to_text()) { "start from the standard list" }
                        }
                    }
                    button type="submit" class="primary" { "grow" }
                    div id="newresult" {}
                }
            }
        },
    )
}

/// The notation.
pub fn about_page() -> Markup {
    page(
        "The notation",
        "/about",
        None,
        "about",
        &html! {
            section class="about" {
                h2 { "Writing a sound change" }
                pre class="grammar" {
                    "rule    := atom \">\" result [ \"/\" context ]\n"
                    "result  := \"∅\" | seg [seg] | \"[\" edit+ \"]\"\n"
                    "context := [\"#\"] atom{0,2} \"_\" atom{0,2} [\"#\"]\n"
                    "atom    := seg | class | \"[\" (+|-)feature … \"]\""
                }
                p { code { "_" } " is where the target stands; " code { "#" } " is the edge of the word. "
                    "A rule is applied " em { "simultaneously" } ": every position is tested against the word as it was, "
                    "so " code { "V > ∅ / C_C" } " on " em { "tatata" } " gives " em { "ttta" } ", not a chain reaction. "
                    "A rule that would leave a word empty, or longer than " (MAX_LEN) " sounds, leaves that word alone — "
                    "and that is the guarantee proved in Verus." }
                div class="cols" {
                    div { h3 { "Classes" } ul {
                        @for (c, l) in Class::ALL { li { code { (l) } " " (class_doc(*c)) } }
                    } }
                    div { h3 { "Features to match" } p class="mono" {
                        @for (_, n) in Test::ALL { "±" (n) " " }
                    } h3 { "Edits to make" } p class="mono" {
                        @for (_, t) in Edit::ALL { (t) " " }
                    } }
                }
                h2 { "Writing a word list" }
                pre class="grammar" { (EXAMPLE_LIST) }
                p { "Suffixes are glossed in two or more capitals. An inflected form is born as its base plus its suffix "
                    "and drifts as one word from then on, so sound change makes irregulars and analogy unmakes them. "
                    em { "tongue" } " (which names each language) and " em { "little" } " (which renews worn-out words) "
                    "are added if you leave them out." }
                h3 { "The inventory" }
                p class="inventory" { @for s in phon::all() { span { (s) } } }
                h3 { "What time does on its own" }
                p class="hint" { "Drift draws from " (LIBRARY.len()) " changes, every one attested somewhere on Earth." }
                ul class="library" {
                    @for c in LIBRARY.iter() {
                        li { strong { (c.name) } span class="where" { " — " (c.where_) } br;
                            @for r in &c.rules { code { (r) } " " } }
                    }
                }
            }
        },
    )
}

const fn class_doc(c: Class) -> &'static str {
    match c {
        Class::C => "any consonant",
        Class::V => "any vowel",
        Class::P => "stops",
        Class::F => "fricatives",
        Class::N => "nasals",
        Class::L => "liquids, l r ʎ",
        Class::G => "glides, j w",
    }
}

/// A 404 body.
pub fn not_found(what: &str) -> Markup {
    page(
        "Not found",
        "",
        None,
        "missing",
        &html! { section class="about" { h2 { "*" (what) } p { a href="/" { "back to the families" } } } },
    )
}
