//! axum HTTP API over the engine.
//!
//! Document ids are u64 snowflakes (crate `beakid`); on the wire they are
//! base62 strings. Routes:
//! ```text
//! GET    /docs               -> ["<base62-id>", …]
//! PUT    /docs               body: markdown -> server assigns a snowflake id
//! PUT    /docs?id=<base62>   body: markdown -> deterministic upsert
//! GET    /docs?id=<base62>   -> markdown (404 when absent)
//! DELETE /docs?id=<base62>   -> 204 | 404
//! GET    /search?q=…&limit=N -> [{"id","score"}]
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use beakid::{BeakId, BeakIdGenerator};
use serde::Serialize;

use core::error::MdbError;
use shard::Engine;

// ponytail: handlers call the blocking engine inline; wrap in spawn_blocking
// if tantivy commits ever stall the tokio runtime.

pub struct AppState {
    pub engine: Arc<Engine>,
    pub gen: Arc<BeakIdGenerator>,
}

impl AppState {
    /// Fixed epoch 2024-01-01Z keeps ms timestamps inside the 41 snowflake
    /// bits until ~2093.
    pub fn new(engine: Arc<Engine>, worker_id: u16) -> Self {
        let epoch = SystemTime::UNIX_EPOCH + Duration::from_millis(1_704_067_200_000);
        Self {
            engine,
            gen: Arc::new(BeakIdGenerator::new(
                worker_id,
                epoch,
                Duration::from_secs(1),
            )),
        }
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/docs", get(get_doc).put(put_doc).delete(delete_doc))
        .route("/search", get(search))
        .with_state(state)
}

/// Serves the API on an already-bound listener until the process stops.
pub async fn serve(state: Arc<AppState>, listener: tokio::net::TcpListener) -> std::io::Result<()> {
    axum::serve(listener, router(state)).await
}

/// GET /docs       -> id list (base62)
/// GET /docs?id=X  -> document body
async fn get_doc(
    State(st): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    match param(&params, "id") {
        Ok(raw) => {
            let doc = st.engine.get(parse_id(&raw)?)?;
            Ok((
                [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                doc.content,
            )
                .into_response())
        }
        Err(_) => {
            Ok(Json(st.engine.ids()?.into_iter().map(base62).collect::<Vec<_>>()).into_response())
        }
    }
}

async fn put_doc(
    State(st): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    content: String,
) -> Result<Json<AssignedId>, ApiError> {
    // Explicit base62 id -> deterministic upsert; otherwise a fresh snowflake.
    let id = match params.get("id").filter(|s| !s.is_empty()) {
        Some(raw) => parse_id(raw)?,
        None => st.gen.next_id().await.to_u64(),
    };
    st.engine.put(id, &content)?;
    Ok(Json(AssignedId { id: base62(id) }))
}

async fn delete_doc(
    State(st): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<StatusCode, ApiError> {
    let id = parse_id(&param(&params, "id")?)?;
    if st.engine.delete(id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

async fn search(
    State(st): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Hit>>, ApiError> {
    let q = param(&params, "q")?;
    let limit = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    Ok(Json(
        st.engine
            .search(&q, limit)?
            .into_iter()
            .map(|h| Hit {
                id: base62(h.id),
                score: h.score,
            })
            .collect(),
    ))
}

fn param(params: &HashMap<String, String>, key: &str) -> Result<String, ApiError> {
    params
        .get(key)
        .filter(|s| !s.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::BadRequest(format!("missing {key}")))
}

fn base62(id: u64) -> String {
    BeakId::from_u64(id).to_base62()
}

fn parse_id(s: &str) -> Result<u64, ApiError> {
    BeakId::from_base62(s)
        .map_err(|e| ApiError::BadRequest(e.to_string()))
        .map(|b| b.to_u64())
}

#[derive(Debug)]
enum ApiError {
    Db(MdbError),
    BadRequest(String),
}

impl From<MdbError> for ApiError {
    fn from(e: MdbError) -> Self {
        Self::Db(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ApiError::Db(MdbError::NotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("not found: {id}"))
            }
            ApiError::Db(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, msg).into_response()
    }
}

/// Wire formats: ids are always base62 strings.
#[derive(Serialize)]
struct AssignedId {
    id: String,
}

#[derive(Serialize)]
struct Hit {
    id: String,
    score: f32,
}
