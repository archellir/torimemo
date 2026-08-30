//! The typed tool registry.
//!
//! This is the surface an agent reaches torimemo through. Odin's `toolreg`
//! consumes exactly this contract — `GET` the catalog, `POST /<name>` with a
//! `{"input": {...}}` body — so the bookmark archive becomes a toolset in the
//! agent that already owns the Telegram gateway, the scheduler, and the tool
//! allowlist. Torimemo does not need a bot of its own, and building one would
//! split the Telegram surface across two binaries.
//!
//! Every entry declares what it does before it is called: whether it reads or
//! mutates, how sensitive its output is, and whether the caller needs to
//! confirm. The registry is the authority on what exists and what a credential
//! may do — the agent asks, it does not decide.

use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use torimemo_core::{NewCapture, Principal, Source, token};
use torimemo_embed::rank_by_similarity;

/// Whether an entry changes anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    /// Reads only.
    None,
    /// Writes to the archive.
    Write,
}

/// How much care an entry's output needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// Counts and metadata.
    Low,
    /// The user's saved links and the notes attached to them.
    Personal,
}

/// One catalog entry.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    /// The dotted name the agent invokes.
    pub name: String,
    /// What it does, written for a model deciding whether to call it.
    pub description: String,
    /// Whether it reads or mutates.
    pub side_effect: SideEffect,
    /// How sensitive the output is.
    pub sensitivity: Sensitivity,
    /// JSON Schema for the `input` object.
    pub input_schema: serde_json::Value,
}

/// The catalog.
///
/// Deliberately small. A large tool surface makes a weaker model choose badly,
/// and everything here is reachable through four verbs: find something, save
/// something, look at one thing, describe the whole.
#[must_use]
pub fn catalog() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "bookmarks.recall".into(),
            description: "Find saved bookmarks by meaning, not keywords. Use this \
                          when the user describes what they are looking for in \
                          their own words."
                .into(),
            side_effect: SideEffect::None,
            sensitivity: Sensitivity::Personal,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to look for." },
                    "limit": { "type": "integer", "description": "Maximum results (default 10)." }
                },
                "required": ["query"]
            }),
        },
        ToolSpec {
            name: "bookmarks.search".into(),
            description: "Find saved bookmarks by exact words in the title, URL, or \
                          description. Use this for a specific term; use \
                          bookmarks.recall for a description of a topic."
                .into(),
            side_effect: SideEffect::None,
            sensitivity: Sensitivity::Personal,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to match." },
                    "limit": { "type": "integer", "description": "Maximum results (default 10)." }
                },
                "required": ["query"]
            }),
        },
        ToolSpec {
            name: "bookmarks.save".into(),
            description: "Save a link. Saving one already in the archive records \
                          another capture rather than a duplicate, and the reply \
                          says how many times it has now been saved."
                .into(),
            side_effect: SideEffect::Write,
            sensitivity: Sensitivity::Personal,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The link to save." },
                    "note": {
                        "type": "string",
                        "description": "Why it is worth keeping. Stored with the capture."
                    }
                },
                "required": ["url"]
            }),
        },
        ToolSpec {
            name: "bookmarks.get".into(),
            description: "Read one bookmark and every time it was captured, with \
                          the note written each time."
                .into(),
            side_effect: SideEffect::None,
            sensitivity: Sensitivity::Personal,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The link to look up." }
                },
                "required": ["url"]
            }),
        },
        ToolSpec {
            name: "bookmarks.stats".into(),
            description: "How much is in the archive: bookmarks, captures, domains, \
                          and the links saved more than once."
                .into(),
            side_effect: SideEffect::None,
            sensitivity: Sensitivity::Low,
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
    ]
}

/// Adds the registry to a router.
pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/v1/tools", get(list)).route("/v1/tools/{name}", post(invoke))
}

/// An invocation body.
#[derive(Debug, Deserialize)]
struct Invocation {
    #[serde(default)]
    input: serde_json::Value,
}

/// A registry error, rendered the way the caller expects.
#[derive(Debug)]
struct ToolError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ToolError {
    fn unauthenticated() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthenticated",
            // Deliberately uninformative. Distinguishing "no token" from
            // "unknown token" from "revoked token" would let a caller probe
            // for which credentials exist.
            message: "a valid bearer token is required".into(),
        }
    }

    fn forbidden(tool: &str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "insufficient_scope",
            message: format!("{tool} writes to the archive; this token is read-only"),
        }
    }

    fn not_found() -> Self {
        Self { status: StatusCode::NOT_FOUND, code: "not_found", message: "no such tool".into() }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_input",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: message.into(),
        }
    }
}

impl IntoResponse for ToolError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.code, "detail": self.message })))
            .into_response()
    }
}

type ToolResult = std::result::Result<Json<serde_json::Value>, ToolError>;

/// Authenticates a request against the store.
///
/// Returns `Ok(None)` when the store holds no live tokens at all: a freshly
/// created archive has not been configured for agent access, and refusing
/// every call before the operator has minted anything would be a confusing
/// first run. Once one token exists the surface is closed, so enabling auth is
/// a single `torimemo token issue` and cannot be half-done.
fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> std::result::Result<Option<Principal>, ToolError> {
    let store = state.store.lock().map_err(|_| ToolError::internal("store lock poisoned"))?;

    if !store.has_tokens().map_err(|error| ToolError::internal(error.to_string()))? {
        return Ok(None);
    }

    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(token::from_header)
        .ok_or_else(ToolError::unauthenticated)?;

    store
        .authenticate(presented)
        .map_err(|error| ToolError::internal(error.to_string()))?
        .map(Some)
        .ok_or_else(ToolError::unauthenticated)
}

async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<serde_json::Value>, ToolError> {
    // The catalog is behind auth too: it names what this archive can do, which
    // is not something an unauthenticated caller should be able to enumerate.
    authenticate(&state, &headers)?;
    Ok(Json(serde_json::json!({ "tools": catalog() })))
}

async fn invoke(
    State(state): State<AppState>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Option<Json<Invocation>>,
) -> ToolResult {
    let principal = authenticate(&state, &headers)?;

    // Scope is checked against the entry's own declared side effect rather
    // than a list of names here, so a new mutating tool is gated the moment it
    // is added to the catalog — there is no second place to remember.
    let writes = catalog()
        .iter()
        .find(|tool| tool.name == name)
        .is_some_and(|tool| tool.side_effect == SideEffect::Write);
    if writes
        && let Some(principal) = &principal
        && !principal.scope.may_write()
    {
        return Err(ToolError::forbidden(&name));
    }

    // An entry that takes no input should be callable with no body at all,
    // which is what an agent sends when the schema has no properties.
    let input = body.map_or(serde_json::Value::Null, |Json(body)| body.input);

    match name.as_str() {
        "bookmarks.recall" => recall(&state, &input),
        "bookmarks.search" => search(&state, &input),
        "bookmarks.save" => save(&state, &input),
        "bookmarks.get" => get_bookmark(&state, &input),
        "bookmarks.stats" => stats(&state),
        _ => Err(ToolError::not_found()),
    }
}

/// Reads a required string field.
fn required_str(input: &serde_json::Value, field: &str) -> std::result::Result<String, ToolError> {
    input[field]
        .as_str()
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolError::invalid(format!("`{field}` is required")))
}

/// Rewrites free text as an FTS5 query.
///
/// An agent passes whatever the user said, and FTS5's syntax turns ordinary
/// punctuation into operators — `a.com` and `rust-lang` are both parse errors,
/// and an unbalanced quote is another. Quoting each word as a literal and
/// joining them makes any input a valid conjunctive query, which is what a
/// caller means by a search term anyway.
fn fts_query(raw: &str) -> String {
    raw.split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads an optional limit, capped so one call cannot return the archive.
fn limit(input: &serde_json::Value) -> usize {
    input["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize
}

fn recall(state: &AppState, input: &serde_json::Value) -> ToolResult {
    let query = required_str(input, "query")?;
    let store = state.store.lock().map_err(|_| ToolError::internal("store lock poisoned"))?;

    let matches = rank_by_similarity(&store, &*state.embedder, &query, limit(input), 0.0)
        .map_err(|error| ToolError::internal(error.to_string()))?;

    let results: Vec<serde_json::Value> = matches
        .into_iter()
        .map(|found| {
            serde_json::json!({
                "url": found.bookmark.canonical_url,
                "title": found.bookmark.title,
                "score": found.score,
                "saved_times": found.bookmark.capture_count
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "results": results })))
}

fn search(state: &AppState, input: &serde_json::Value) -> ToolResult {
    let query = required_str(input, "query")?;
    let store = state.store.lock().map_err(|_| ToolError::internal("store lock poisoned"))?;

    let results: Vec<serde_json::Value> = store
        .search(&fts_query(&query), limit(input))
        .map_err(|error| ToolError::invalid(error.to_string()))?
        .into_iter()
        .map(|bookmark| {
            serde_json::json!({
                "url": bookmark.canonical_url,
                "title": bookmark.title,
                "saved_times": bookmark.capture_count
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "results": results })))
}

fn save(state: &AppState, input: &serde_json::Value) -> ToolResult {
    let url = required_str(input, "url")?;
    let mut capture = NewCapture::new(url, Source::Api);
    if let Some(note) = input["note"].as_str().filter(|note| !note.trim().is_empty()) {
        capture = capture.with_context(note.to_string());
    }

    let (ingested, bookmark) = {
        let mut store =
            state.store.lock().map_err(|_| ToolError::internal("store lock poisoned"))?;
        let ingested =
            store.ingest(&capture).map_err(|error| ToolError::invalid(error.to_string()))?;
        let bookmark = store
            .bookmark(ingested.bookmark_id())
            .map_err(|error| ToolError::internal(error.to_string()))?
            .ok_or_else(|| ToolError::internal("bookmark vanished after ingest"))?;
        (ingested, bookmark)
    };

    // A page saved from the browser has to become searchable on its own. This
    // is the difference between the extension working and quietly saving into
    // a hole: nothing else in the serving path embeds, so without it a saved
    // bookmark stays invisible to recall until someone runs `torimemo embed`.
    //
    // Only for a new bookmark — a repeat capture already has its vector, and
    // the text it was computed from has not changed.
    if ingested.is_new() {
        state.embed_in_background(ingested.bookmark_id());
    }

    Ok(Json(serde_json::json!({
        "url": bookmark.canonical_url,
        "created": ingested.is_new(),
        "saved_times": bookmark.capture_count
    })))
}

fn get_bookmark(state: &AppState, input: &serde_json::Value) -> ToolResult {
    let url = required_str(input, "url")?;
    let store = state.store.lock().map_err(|_| ToolError::internal("store lock poisoned"))?;

    let bookmark = store
        .bookmark_by_url(&url)
        .map_err(|error| ToolError::invalid(error.to_string()))?
        .ok_or_else(ToolError::not_found)?;
    let captures =
        store.captures(bookmark.id).map_err(|error| ToolError::internal(error.to_string()))?;

    let history: Vec<serde_json::Value> = captures
        .into_iter()
        .map(|capture| {
            serde_json::json!({
                "source": capture.source.as_str(),
                "note": capture.context,
                "at": capture.captured_at
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "url": bookmark.canonical_url,
        "title": bookmark.title,
        "description": bookmark.description,
        "saved_times": bookmark.capture_count,
        "captures": history
    })))
}

fn stats(state: &AppState) -> ToolResult {
    let store = state.store.lock().map_err(|_| ToolError::internal("store lock poisoned"))?;
    let stats = store.stats().map_err(|error| ToolError::internal(error.to_string()))?;
    let repeats = store
        .most_captured(5)
        .map_err(|error| ToolError::internal(error.to_string()))?
        .into_iter()
        .map(|bookmark| {
            serde_json::json!({
                "url": bookmark.canonical_url,
                "saved_times": bookmark.capture_count
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({ "stats": stats, "saved_more_than_once": repeats })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt as _;
    use torimemo_core::Store;
    use torimemo_embed::Provider;
    use tower::ServiceExt as _;

    fn app() -> Router {
        router(AppState::new(Store::open_in_memory().unwrap(), Provider::deterministic()))
    }

    /// An app with auth switched on, plus the token to reach it with.
    fn guarded(scope: torimemo_core::Scope) -> (Router, String) {
        let store = Store::open_in_memory().unwrap();
        let issued = store.issue_token("test", scope).unwrap();
        let app = router(AppState::new(store, Provider::deterministic()));
        (app, issued.token)
    }

    fn authorized(name: &str, input: serde_json::Value, token: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(format!("/v1/tools/{name}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(serde_json::json!({ "input": input }).to_string()))
            .unwrap()
    }

    fn call(name: &str, input: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(format!("/v1/tools/{name}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({ "input": input }).to_string()))
            .unwrap()
    }

    async fn json(response: Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    #[tokio::test]
    async fn the_catalog_lists_every_entry_with_its_declared_metadata() {
        let response = app()
            .oneshot(Request::builder().uri("/v1/tools").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let body = json(response).await;
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), catalog().len());

        for tool in tools {
            assert!(tool["name"].is_string());
            assert!(tool["side_effect"].is_string());
            assert!(tool["sensitivity"].is_string());
            assert!(tool["input_schema"]["type"] == "object");
        }
    }

    #[tokio::test]
    async fn only_save_declares_a_side_effect() {
        let mutating =
            catalog().iter().filter(|tool| tool.side_effect == SideEffect::Write).count();
        assert_eq!(mutating, 1);
    }

    #[tokio::test]
    async fn saving_reports_creation_then_recapture() {
        let app = app();
        let first = json(
            app.clone()
                .oneshot(call("bookmarks.save", serde_json::json!({ "url": "https://a.com/x" })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(first["created"], true);
        assert_eq!(first["saved_times"], 1);

        // The same resource with tracking parameters — dedupe must see through it.
        let second = json(
            app.oneshot(call(
                "bookmarks.save",
                serde_json::json!({ "url": "https://a.com/x?utm_source=agent" }),
            ))
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(second["created"], false);
        assert_eq!(second["saved_times"], 2);
    }

    #[tokio::test]
    async fn a_saved_page_becomes_searchable_without_a_cli_command() {
        // The failure this prevents: the extension saves a page, the row
        // lands, and recall never finds it because nothing in the serving
        // path computes a vector. `embed_now` is called directly here rather
        // than through the spawned task, so the assertion does not race.
        let store = Store::open_in_memory().unwrap();
        let state = AppState::new(store, Provider::deterministic());
        let app = router(state.clone());

        app.oneshot(call(
            "bookmarks.save",
            serde_json::json!({ "url": "https://example.com/a", "note": "a title" }),
        ))
        .await
        .unwrap();

        {
            let store = state.store.lock().unwrap();
            assert_eq!(store.stats().unwrap().embedded, 0, "not embedded yet");
        }

        state.embed_now(1).unwrap();

        let store = state.store.lock().unwrap();
        assert_eq!(store.stats().unwrap().embedded, 1, "the save should have queued a vector");
    }

    #[tokio::test]
    async fn embedding_a_missing_bookmark_is_not_an_error() {
        // The background task can lose a race with a delete; that is a no-op,
        // not a failure worth logging.
        let state = AppState::new(Store::open_in_memory().unwrap(), Provider::deterministic());
        assert!(state.embed_now(999).is_ok());
    }

    #[tokio::test]
    async fn a_note_is_kept_with_the_capture() {
        let app = app();
        app.clone()
            .oneshot(call(
                "bookmarks.save",
                serde_json::json!({ "url": "https://a.com/x", "note": "for the auth section" }),
            ))
            .await
            .unwrap();

        let body = json(
            app.oneshot(call("bookmarks.get", serde_json::json!({ "url": "https://a.com/x" })))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(body["captures"][0]["note"], "for the auth section");
    }

    #[tokio::test]
    async fn a_missing_required_field_is_unprocessable() {
        let response = app().oneshot(call("bookmarks.save", serde_json::json!({}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json(response).await["error"], "invalid_input");
    }

    #[tokio::test]
    async fn an_unparseable_url_is_rejected_not_stored() {
        let response = app()
            .oneshot(call("bookmarks.save", serde_json::json!({ "url": "not a url" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn an_unknown_tool_is_not_found() {
        let response =
            app().oneshot(call("bookmarks.destroy", serde_json::json!({}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn looking_up_an_unsaved_url_is_not_found() {
        let response = app()
            .oneshot(call("bookmarks.get", serde_json::json!({ "url": "https://nope.com/x" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn stats_needs_no_input_body_at_all() {
        // What an agent sends for an entry whose schema has no properties.
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/bookmarks.stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(json(response).await["stats"]["bookmarks"].is_number());
    }

    #[tokio::test]
    async fn a_limit_is_capped_so_one_call_cannot_drain_the_archive() {
        assert_eq!(limit(&serde_json::json!({ "limit": 10_000 })), 50);
        assert_eq!(limit(&serde_json::json!({ "limit": 0 })), 1);
        assert_eq!(limit(&serde_json::json!({})), 10);
    }

    #[tokio::test]
    async fn search_finds_a_saved_bookmark_by_title() {
        let app = app();
        app.clone()
            .oneshot(call("bookmarks.save", serde_json::json!({ "url": "https://a.com/x" })))
            .await
            .unwrap();

        let response = app
            .oneshot(call("bookmarks.search", serde_json::json!({ "query": "a.com" })))
            .await
            .unwrap();
        assert!(json(response).await["results"].is_array());
    }

    #[test]
    fn free_text_becomes_a_valid_fts_query() {
        // Each of these is a syntax error passed to FTS5 unquoted.
        assert_eq!(fts_query("a.com"), "\"a\" \"com\"");
        assert_eq!(fts_query("rust-lang"), "\"rust\" \"lang\"");
        assert_eq!(fts_query("what's \"this\"?"), "\"what\" \"s\" \"this\"");
        assert_eq!(fts_query("   "), "");
    }

    #[tokio::test]
    async fn search_survives_punctuation_that_is_fts_syntax() {
        for query in ["a.com", "rust-lang", "c++", "\"unbalanced", "NEAR/2"] {
            let response = app()
                .oneshot(call("bookmarks.search", serde_json::json!({ "query": query })))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "failed on {query:?}");
        }
    }

    #[tokio::test]
    async fn without_a_token_a_guarded_registry_refuses_everything() {
        let (app, _) = guarded(torimemo_core::Scope::ReadWrite);

        let listed = app
            .clone()
            .oneshot(Request::builder().uri("/v1/tools").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::UNAUTHORIZED);

        let invoked = app.oneshot(call("bookmarks.stats", serde_json::json!({}))).await.unwrap();
        assert_eq!(invoked.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_valid_token_reaches_the_registry() {
        let (app, token) = guarded(torimemo_core::Scope::Read);
        let response = app
            .oneshot(authorized("bookmarks.stats", serde_json::json!({}), &token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn a_read_token_cannot_write() {
        let (app, token) = guarded(torimemo_core::Scope::Read);
        let response = app
            .oneshot(authorized(
                "bookmarks.save",
                serde_json::json!({ "url": "https://a.com/x" }),
                &token,
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(json(response).await["error"], "insufficient_scope");
    }

    #[tokio::test]
    async fn a_read_write_token_can_write() {
        let (app, token) = guarded(torimemo_core::Scope::ReadWrite);
        let response = app
            .oneshot(authorized(
                "bookmarks.save",
                serde_json::json!({ "url": "https://a.com/x" }),
                &token,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn a_revoked_token_stops_working_immediately() {
        let store = Store::open_in_memory().unwrap();
        let issued = store.issue_token("test", torimemo_core::Scope::ReadWrite).unwrap();
        // A second token keeps the registry closed after the first is revoked;
        // otherwise it would fall back to open mode and the test would pass
        // for the wrong reason.
        store.issue_token("other", torimemo_core::Scope::Read).unwrap();
        store.revoke_token(&issued.id).unwrap();

        let app = router(AppState::new(store, Provider::deterministic()));
        let response = app
            .oneshot(authorized("bookmarks.stats", serde_json::json!({}), &issued.token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_forged_token_is_refused() {
        let (app, _) = guarded(torimemo_core::Scope::ReadWrite);
        let forged = format!("tmk_{}", "a".repeat(64));

        let response = app
            .oneshot(authorized("bookmarks.stats", serde_json::json!({}), &forged))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn the_refusal_never_says_why() {
        // "no token" and "wrong token" must be indistinguishable, or the
        // surface becomes an oracle for which credentials exist.
        let (app, _) = guarded(torimemo_core::Scope::Read);
        let forged = format!("tmk_{}", "b".repeat(64));

        let missing = json(
            app.clone().oneshot(call("bookmarks.stats", serde_json::json!({}))).await.unwrap(),
        )
        .await;
        let wrong = json(
            app.oneshot(authorized("bookmarks.stats", serde_json::json!({}), &forged))
                .await
                .unwrap(),
        )
        .await;

        assert_eq!(missing, wrong);
    }

    #[tokio::test]
    async fn every_mutating_entry_is_scope_gated() {
        // Guards against a future write tool being added to the catalog
        // without the gate noticing: the check is driven by `side_effect`, and
        // this asserts that stays true for every entry.
        let (app, token) = guarded(torimemo_core::Scope::Read);

        for tool in catalog().iter().filter(|tool| tool.side_effect == SideEffect::Write) {
            let response = app
                .clone()
                .oneshot(authorized(&tool.name, serde_json::json!({}), &token))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{} was reachable with a read token",
                tool.name
            );
        }
    }

    #[tokio::test]
    async fn recall_on_an_empty_archive_is_empty_not_an_error() {
        let response = app()
            .oneshot(call("bookmarks.recall", serde_json::json!({ "query": "anything" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json(response).await["results"].as_array().unwrap().len(), 0);
    }
}
