//! The HTTP surface.
//!
//! Reads go straight to a family's live world. Every write that changes a
//! world — a century by hand, a typed rule — goes to [`crate::submit`], which
//! hands it to the family's Harvest chronicle, the single writer.

use std::convert::Infallible;
use std::sync::Arc;

use asterisk::WorldError;
use asterisk::export;
use asterisk::lexicon::{Meaning, Vocabulary};
use asterisk::world::{Command, LangId, World};
use autumn_web::error::AutumnError;
use autumn_web::prelude::*;
use autumn_web::sse::{Sse, from_subscriber};
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;

use crate::atlas::{AtlasError, Family, NewFamily, TOPIC};
use crate::view;

/// Longest rule source accepted.
pub const MAX_RULE: usize = 400;
/// Longest word list accepted, in bytes.
pub const MAX_VOCAB: usize = 24_000;

fn atlas_error(e: AtlasError) -> AutumnError {
    match e {
        AtlasError::NoFamily(_) => AutumnError::not_found(e),
        AtlasError::Vocab(_) | AtlasError::World(_) => AutumnError::bad_request(e),
        _ => AutumnError::internal_server_error_msg(e.to_string()),
    }
}

fn family(state: &AppState, slug: &str) -> AutumnResult<Arc<Family>> {
    crate::atlas(state)
        .ok_or_else(|| AutumnError::service_unavailable_msg("the families are still being born"))?
        .get(slug)
        .map_err(atlas_error)
}

fn meaning(w: &World, gloss: Option<&str>) -> AutumnResult<Meaning> {
    gloss.map(str::trim).map_or_else(
        || {
            w.liveliest()
                .or_else(|| w.vocab.named(view::DEFAULT_MEANING))
                .or_else(|| w.vocab.all().next())
                .ok_or_else(|| AutumnError::not_found_msg("this family has no words"))
        },
        |g| {
            w.vocab.named(g).ok_or_else(|| {
                AutumnError::not_found_msg(format!("this family has no word for ‘{g}’"))
            })
        },
    )
}

fn lang(w: &World, id: u16) -> AutumnResult<LangId> {
    w.lang(LangId(id)).map(|l| l.id).map_err(|e| match e {
        WorldError::NoLanguage(_) => AutumnError::not_found(e),
        other => AutumnError::bad_request(other),
    })
}

/// `?m=moon`.
#[derive(Debug, Clone, Deserialize)]
pub struct MeaningQuery {
    /// A gloss.
    pub m: Option<String>,
}

/// The gallery.
#[get("/")]
#[public]
pub async fn gallery(State(state): State<AppState>) -> AutumnResult<Markup> {
    let a = crate::atlas(&state)
        .ok_or_else(|| AutumnError::service_unavailable_msg("the families are still being born"))?;
    Ok(view::gallery_page(&a.all()))
}

/// The new-family form.
#[get("/new")]
#[public]
pub async fn new_form(csrf: Option<CsrfToken>) -> Markup {
    view::new_page(csrf.as_ref().map(CsrfToken::token), &Vocabulary::standard())
}

/// A new family, from the form.
#[derive(Debug, Clone, Deserialize)]
pub struct NewForm {
    /// Title.
    pub title: String,
    /// Seed word.
    pub seed: String,
    /// Centuries.
    #[serde(default)]
    pub span: Option<u32>,
    /// Seconds per century.
    #[serde(default)]
    pub century_secs: Option<u64>,
    /// A word list.
    #[serde(default)]
    pub vocab: Option<String>,
}

/// Where the new family lives.
#[derive(Debug, Clone, Serialize)]
pub struct Created {
    /// Its slug.
    pub slug: String,
    /// Its page.
    pub href: String,
}

/// Grow a family and start its chronicle.
#[post("/families")]
#[public]
pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<NewForm>,
) -> AutumnResult<Json<Created>> {
    let title = form.title.trim();
    let seed = form.seed.trim();
    if title.is_empty() || title.chars().count() > 60 {
        return Err(AutumnError::bad_request_msg(
            "a family's name is 1 to 60 characters",
        ));
    }
    if seed.is_empty() || seed.chars().count() > 40 {
        return Err(AutumnError::bad_request_msg(
            "a seed is a word of 1 to 40 characters",
        ));
    }
    if form.vocab.as_ref().is_some_and(|v| v.len() > MAX_VOCAB) {
        return Err(AutumnError::bad_request_msg(format!(
            "a word list is at most {MAX_VOCAB} bytes"
        )));
    }
    let span = form.span.unwrap_or(crate::SPAN).clamp(40, 5000);
    let century_secs = form
        .century_secs
        .unwrap_or(crate::CENTURY_SECS)
        .clamp(1, 86_400);
    let a = crate::atlas(&state)
        .ok_or_else(|| AutumnError::service_unavailable_msg("the families are still being born"))?;
    let req = NewFamily {
        title: title.to_owned(),
        seed: seed.to_owned(),
        vocab: form.vocab.clone(),
        span: Some(span),
        century_secs,
    };
    let f = a.create(&req, crate::now()).map_err(atlas_error)?;
    crate::start_chronicle(&state, &f.row.slug).await;
    let b = a.broadcast(&f, "born", Vec::new());
    crate::publish(&state, &b);
    Ok(Json(Created {
        href: f.base(),
        slug: f.row.slug.clone(),
    }))
}

/// One family.
#[get("/f/{slug}")]
#[public]
pub async fn family_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(q): Query<MeaningQuery>,
    csrf: Option<CsrfToken>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let m = meaning(&w, q.m.as_deref())?;
    let html = view::family_page(&f, &w, m, csrf.as_ref().map(CsrfToken::token));
    drop(w);
    Ok(html)
}

/// The tree alone, for live refresh.
#[get("/f/{slug}/_tree")]
#[public]
pub async fn tree(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(q): Query<MeaningQuery>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let m = meaning(&w, q.m.as_deref())?;
    let html = view::tree(&f.base(), &w, m);
    drop(w);
    Ok(html)
}

/// The chronicle alone.
#[get("/f/{slug}/_feed")]
#[public]
pub async fn feed(State(state): State<AppState>, Path(slug): Path<String>) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let html = view::feed(&f.base(), &w.feed, w.century);
    drop(w);
    Ok(html)
}

/// The workbench's language list alone.
#[get("/f/{slug}/_targets")]
#[public]
pub async fn targets(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let html = view::targets(&w);
    drop(w);
    Ok(html)
}

/// A summary, for scripts and tests.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// The family.
    pub slug: String,
    /// The seed word.
    pub seed: String,
    /// Centuries passed.
    pub century: u32,
    /// When it ends.
    pub span: Option<u32>,
    /// Languages ever.
    pub languages: usize,
    /// Languages spoken.
    pub spoken: usize,
    /// Meanings.
    pub meanings: usize,
    /// Commands in the log.
    pub commands: u64,
    /// Is time running?
    pub drifting: bool,
    /// Has it ended?
    pub complete: bool,
    /// Does the log alone rebuild exactly this world?
    pub replays: bool,
}

fn summary(state: &AppState, slug: &str) -> AutumnResult<Summary> {
    let a = crate::atlas(state)
        .ok_or_else(|| AutumnError::service_unavailable_msg("the families are still being born"))?;
    let f = a.get(slug).map_err(atlas_error)?;
    let replays = a.verify(slug).map_err(atlas_error)?;
    let w = f.world();
    Ok(Summary {
        slug: slug.to_owned(),
        seed: w.seed_word.clone(),
        century: w.century,
        span: w.span,
        languages: w.langs.len(),
        spoken: w.spoken().len(),
        meanings: w.vocab.len(),
        commands: f.applied(),
        drifting: f.drifting(),
        complete: w.complete(),
        replays,
    })
}

/// The family, in numbers.
#[get("/f/{slug}/_world.json")]
#[public]
pub async fn world_json(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AutumnResult<Json<Summary>> {
    Ok(Json(summary(&state, &slug)?))
}

/// One language.
#[get("/f/{slug}/lang/{id}")]
#[public]
pub async fn language(
    State(state): State<AppState>,
    Path((slug, id)): Path<(String, u16)>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let id = lang(&w, id)?;
    let html = view::language_page(&f, &w, id);
    drop(w);
    Ok(html)
}

/// One word in one language: its whole life.
#[get("/f/{slug}/lang/{id}/word/{gloss}")]
#[public]
pub async fn word(
    State(state): State<AppState>,
    Path((slug, id, gloss)): Path<(String, u16, String)>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let id = lang(&w, id)?;
    let m = meaning(&w, Some(&gloss))?;
    let html = view::etymology_page(&f, &w, id, m);
    drop(w);
    Ok(html)
}

fn download(body: String, kind: &'static str, name: &str) -> impl IntoResponse + use<> {
    (
        [
            ("content-type", kind.to_owned()),
            (
                "content-disposition",
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        body,
    )
}

/// The whole family as a Markdown grammar book.
#[get("/f/{slug}/export.md")]
#[public]
pub async fn export_md(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AutumnResult<impl IntoResponse> {
    let f = family(&state, &slug)?;
    let w = f.world().clone();
    let title = f.row.title.clone();
    let body = tokio::task::spawn_blocking(move || export::markdown(&w, &title))
        .await
        .map_err(|e| AutumnError::internal_server_error_msg(e.to_string()))?;
    Ok(download(
        body,
        "text/markdown; charset=utf-8",
        &format!("{slug}.md"),
    ))
}

/// The whole family as JSON.
#[get("/f/{slug}/export.json")]
#[public]
pub async fn export_json(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AutumnResult<impl IntoResponse> {
    let f = family(&state, &slug)?;
    let w = f.world().clone();
    let title = f.row.title.clone();
    let body = tokio::task::spawn_blocking(move || export::json(&w, &title))
        .await
        .map_err(|e| AutumnError::internal_server_error_msg(e.to_string()))?;
    Ok(download(body, "application/json", &format!("{slug}.json")))
}

/// One language's chapter.
#[get("/f/{slug}/lang/{id}/export.md")]
#[public]
pub async fn export_lang(
    State(state): State<AppState>,
    Path((slug, id)): Path<(String, u16)>,
) -> AutumnResult<impl IntoResponse> {
    let f = family(&state, &slug)?;
    let w = f.world();
    let id = lang(&w, id)?;
    let body = export::language_markdown(&w, &f.row.title, id)
        .ok_or_else(|| AutumnError::not_found_msg("no such language"))?;
    let name = format!(
        "{slug}-{}.md",
        view::seg(&w.langs[id.index()].name().to_lowercase())
    );
    drop(w);
    Ok(download(body, "text/markdown; charset=utf-8", &name))
}

/// A rule from the workbench.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleForm {
    /// The source.
    pub rule: String,
    /// A language id, or `all`.
    #[serde(default)]
    pub target: Option<String>,
    /// The hand's colour.
    #[serde(default)]
    pub hue: Option<u16>,
}

impl RuleForm {
    fn checked(&self) -> AutumnResult<(&str, Option<LangId>)> {
        let src = self.rule.trim();
        if src.chars().count() > MAX_RULE {
            return Err(AutumnError::bad_request_msg(format!(
                "a rule is at most {MAX_RULE} characters"
            )));
        }
        let target = match self.target.as_deref().map(str::trim) {
            None | Some("" | "all") => None,
            Some(t) => Some(LangId(t.parse().map_err(|_| {
                AutumnError::bad_request_msg(format!("{t} is not a language"))
            })?)),
        };
        Ok((src, target))
    }
}

fn previewed(
    f: &Family,
    src: &str,
    target: Option<LangId>,
    sent: bool,
) -> AutumnResult<Result<Markup, Markup>> {
    let w = f.world();
    let html = match w.preview(src, target) {
        Ok(rows) => Ok(view::preview(&w, &rows, src, sent)),
        Err(e @ (WorldError::Rule(_) | WorldError::NotSpoken(_))) => {
            Err(view::rule_error(&e.to_string()))
        }
        Err(e @ WorldError::NoLanguage(_)) => return Err(AutumnError::not_found(e)),
        Err(e) => Err(view::rule_error(&e.to_string())),
    };
    drop(w);
    Ok(html)
}

/// What a rule would do. A malformed rule is reported in the fragment, not
/// as an error status: the workbench shows it where the preview would be.
#[post("/f/{slug}/preview")]
#[public]
pub async fn preview(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Form(form): Form<RuleForm>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let (src, target) = form.checked()?;
    Ok(previewed(&f, src, target, false)?.unwrap_or_else(|e| e))
}

/// Make a rule happen: it joins the family's log through its chronicle.
#[post("/f/{slug}/apply")]
#[public]
pub async fn apply(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Form(form): Form<RuleForm>,
) -> AutumnResult<Markup> {
    let f = family(&state, &slug)?;
    let (src, target) = form.checked()?;
    if f.world().complete() {
        return Ok(view::rule_error(
            "this chronicle is complete; export it, or grow another",
        ));
    }
    match previewed(&f, src, target, true)? {
        Err(err) => Ok(err),
        Ok(html) => {
            let cmd = Command::Hand {
                rule: src.to_owned(),
                target,
                hue: form.hue.unwrap_or(48) % 360,
            };
            crate::submit(&state, &slug, cmd)
                .await
                .map_err(atlas_error)?;
            Ok(html)
        }
    }
}

/// Start or stop a family's time.
#[post("/f/{slug}/drift/{on}")]
#[public]
pub async fn drift(
    State(state): State<AppState>,
    Path((slug, on)): Path<(String, String)>,
) -> AutumnResult<Json<Summary>> {
    let on = match on.as_str() {
        "on" => true,
        "off" => false,
        other => {
            return Err(AutumnError::bad_request_msg(format!(
                "time is on or off, not {other}"
            )));
        }
    };
    let a = crate::atlas(&state)
        .ok_or_else(|| AutumnError::service_unavailable_msg("the families are still being born"))?;
    let b = a.set_drifting(&slug, on).map_err(atlas_error)?;
    crate::publish(&state, &b);
    Ok(Json(summary(&state, &slug)?))
}

/// One century, now.
#[post("/f/{slug}/century")]
#[public]
pub async fn century(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AutumnResult<Json<Summary>> {
    family(&state, &slug)?;
    crate::submit(&state, &slug, Command::Advance)
        .await
        .map_err(atlas_error)?;
    Ok(Json(summary(&state, &slug)?))
}

/// The live feed: one message per event in any family.
#[get("/stream")]
#[public]
pub async fn stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<autumn_web::sse::Event, Infallible>>> {
    from_subscriber(state.channels().subscribe(TOPIC))
}

/// The notation.
#[get("/about")]
#[public]
pub async fn about() -> Markup {
    view::about_page()
}

/// The one script.
#[get("/asterisk.js")]
#[public]
pub async fn script() -> impl IntoResponse {
    (
        [
            ("content-type", "text/javascript; charset=utf-8"),
            ("cache-control", "no-cache"),
        ],
        view::SCRIPT,
    )
}

/// Every route, for `main` and the tests.
#[macro_export]
macro_rules! all_routes {
    () => {
        autumn_web::routes![
            $crate::routes::gallery,
            $crate::routes::new_form,
            $crate::routes::create,
            $crate::routes::family_page,
            $crate::routes::tree,
            $crate::routes::feed,
            $crate::routes::targets,
            $crate::routes::world_json,
            $crate::routes::language,
            $crate::routes::word,
            $crate::routes::export_md,
            $crate::routes::export_json,
            $crate::routes::export_lang,
            $crate::routes::preview,
            $crate::routes::apply,
            $crate::routes::drift,
            $crate::routes::century,
            $crate::routes::stream,
            $crate::routes::about,
            $crate::routes::script,
        ]
    };
}
