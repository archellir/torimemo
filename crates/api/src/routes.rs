//! Route definitions.
//!
//! The endpoints are shaped for an agent rather than for a UI: a search
//! returns whole bookmarks with their scores rather than a page of ids, and
//! `POST /captures` is idempotent on the canonical URL, so an agent that
//! re-sends a link it already saved gets the same bookmark back instead of a
//! conflict to reason about.

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use torimemo_core::{Bookmark, NewCapture, Source};
use torimemo_embed::rank_by_similarity;

/// Builds the router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/captures", post(create_capture))
        .route("/bookmarks/{id}", get(bookmark))
        .route("/search", get(search))
        .route("/recall", get(recall))
        .route("/events", post(record_event))
        .merge(crate::tools::routes())
        .with_state(state)
}

/// An error rendered as JSON.
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: message.into() }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: message.into() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}

/// A poisoned store mutex means a handler panicked while holding it; the
/// process is no longer trustworthy, so this reports rather than papers over it.
fn lock_error() -> ApiError {
    ApiError::internal("store lock was poisoned")
}

type ApiResult<T> = std::result::Result<T, ApiError>;

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn stats(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let store = state.store.lock().map_err(|_| lock_error())?;
    let stats = store.stats().map_err(|error| ApiError::internal(error.to_string()))?;
    let domains = store.top_domains(10).map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(serde_json::json!({ "stats": stats, "top_domains": domains })))
}

/// A link an agent is handing over.
#[derive(Debug, Deserialize)]
struct CaptureRequest {
    url: String,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

/// What happened to it.
#[derive(Debug, Serialize)]
struct CaptureResponse {
    bookmark: Bookmark,
    /// False when this URL was already known — the agent's cue that it is
    /// looking at a link it saved before, not a new one.
    created: bool,
}

async fn create_capture(
    State(state): State<AppState>,
    Json(request): Json<CaptureRequest>,
) -> ApiResult<(StatusCode, Json<CaptureResponse>)> {
    let source = match request.source.as_deref() {
        None => Source::Api,
        Some(raw) => {
            raw.parse::<Source>().map_err(|error| ApiError::bad_request(error.to_string()))?
        }
    };

    let mut capture = NewCapture::new(request.url, source);
    if let Some(context) = request.context {
        capture = capture.with_context(context);
    }

    let mut store = state.store.lock().map_err(|_| lock_error())?;
    let ingested =
        store.ingest(&capture).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let bookmark = store
        .bookmark(ingested.bookmark_id())
        .map_err(|error| ApiError::internal(error.to_string()))?
        .ok_or_else(|| ApiError::internal("bookmark vanished after ingest"))?;

    let status = if ingested.is_new() { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(CaptureResponse { bookmark, created: ingested.is_new() })))
}

async fn bookmark(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = state.store.lock().map_err(|_| lock_error())?;
    let bookmark = store
        .bookmark(id)
        .map_err(|error| ApiError::internal(error.to_string()))?
        .ok_or(ApiError { status: StatusCode::NOT_FOUND, message: "no such bookmark".into() })?;
    let captures = store.captures(id).map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(serde_json::json!({ "bookmark": bookmark, "captures": captures })))
}

/// Query parameters shared by both search endpoints.
#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    floor: f32,
}

fn default_limit() -> usize {
    20
}

async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = state.store.lock().map_err(|_| lock_error())?;
    let results = store
        .search(&query.q, query.limit)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(serde_json::json!({ "results": results })))
}

/// A recall hit, flattened so an agent gets the score beside the bookmark.
#[derive(Debug, Serialize)]
struct RecallHit {
    #[serde(flatten)]
    bookmark: Bookmark,
    score: f32,
}

async fn recall(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let store = state.store.lock().map_err(|_| lock_error())?;
    let matches = rank_by_similarity(&store, &*state.embedder, &query.q, query.limit, query.floor)
        .map_err(|error| ApiError::internal(error.to_string()))?;

    let results: Vec<RecallHit> = matches
        .into_iter()
        .map(|found| RecallHit { bookmark: found.bookmark, score: found.score })
        .collect();
    Ok(Json(serde_json::json!({ "results": results })))
}

/// An interaction worth learning from.
#[derive(Debug, Deserialize)]
struct EventRequest {
    bookmark_id: i64,
    kind: String,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    position: Option<i64>,
}

async fn record_event(
    State(state): State<AppState>,
    Json(request): Json<EventRequest>,
) -> ApiResult<StatusCode> {
    let store = state.store.lock().map_err(|_| lock_error())?;
    store
        .record_event(
            request.bookmark_id,
            &request.kind,
            request.query.as_deref(),
            request.position,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt as _;
    use torimemo_core::Store;
    use torimemo_embed::Provider;
    use tower::ServiceExt as _;

    fn app() -> Router {
        let store = Store::open_in_memory().unwrap();
        router(AppState::new(store, Provider::deterministic()))
    }

    async fn json(response: Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    fn post_json(path: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn health_reports_ok() {
        let response = app()
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn capturing_a_new_link_creates_it() {
        let response = app()
            .oneshot(post_json("/captures", serde_json::json!({ "url": "https://example.com/a" })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = json(response).await;
        assert_eq!(body["created"], true);
        assert_eq!(body["bookmark"]["canonical_url"], "https://example.com/a");
    }

    #[tokio::test]
    async fn recapturing_a_known_link_merges_instead_of_conflicting() {
        let app = app();
        let first = post_json("/captures", serde_json::json!({ "url": "https://example.com/a" }));
        app.clone().oneshot(first).await.unwrap();

        // Same resource, different tracking parameters and a different source.
        let second = post_json(
            "/captures",
            serde_json::json!({
                "url": "https://example.com/a?utm_source=telegram",
                "source": "telegram"
            }),
        );
        let response = app.oneshot(second).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json(response).await;
        assert_eq!(body["created"], false);
        assert_eq!(body["bookmark"]["capture_count"], 2);
    }

    #[tokio::test]
    async fn an_unparseable_url_is_a_bad_request_not_a_panic() {
        let response = app()
            .oneshot(post_json("/captures", serde_json::json!({ "url": "not a url" })))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn an_unknown_source_is_rejected() {
        let response = app()
            .oneshot(post_json(
                "/captures",
                serde_json::json!({ "url": "https://example.com/a", "source": "carrier-pigeon" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_missing_bookmark_is_not_found() {
        let response = app()
            .oneshot(Request::builder().uri("/bookmarks/999").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_bookmark_carries_its_captures() {
        let app = app();
        app.clone()
            .oneshot(post_json("/captures", serde_json::json!({ "url": "https://example.com/a" })))
            .await
            .unwrap();

        let response = app
            .oneshot(Request::builder().uri("/bookmarks/1").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let body = json(response).await;
        assert_eq!(body["captures"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn recall_returns_scores_beside_bookmarks() {
        let app = app();
        app.clone()
            .oneshot(post_json(
                "/captures",
                serde_json::json!({ "url": "https://rust-lang.org/x" }),
            ))
            .await
            .unwrap();

        // Nothing is embedded yet, so recall is empty rather than an error —
        // an agent querying a fresh store must not see a failure.
        let response = app
            .oneshot(Request::builder().uri("/recall?q=rust").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(json(response).await["results"].is_array());
    }

    #[tokio::test]
    async fn an_event_is_recorded() {
        let app = app();
        app.clone()
            .oneshot(post_json("/captures", serde_json::json!({ "url": "https://example.com/a" })))
            .await
            .unwrap();

        let response = app
            .oneshot(post_json(
                "/events",
                serde_json::json!({ "bookmark_id": 1, "kind": "opened" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn an_unknown_event_kind_is_rejected_by_the_schema_check() {
        let app = app();
        app.clone()
            .oneshot(post_json("/captures", serde_json::json!({ "url": "https://example.com/a" })))
            .await
            .unwrap();

        let response = app
            .oneshot(post_json(
                "/events",
                serde_json::json!({ "bookmark_id": 1, "kind": "levitated" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
