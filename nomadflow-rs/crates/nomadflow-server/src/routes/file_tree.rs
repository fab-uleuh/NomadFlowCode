use std::path::PathBuf;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};

use nomadflow_core::models::{ListDirRequest, ListDirResponse};
use nomadflow_core::services::file_tree::FileTreeService;

use crate::state::AppState;

fn map_error(e: nomadflow_core::error::NomadError) -> (StatusCode, Json<Value>) {
    match &e {
        nomadflow_core::error::NomadError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": e.to_string() })),
        ),
        nomadflow_core::error::NomadError::InvalidInput(_) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "detail": e.to_string() })),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        ),
    }
}

fn map_join_error(e: tokio::task::JoinError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "detail": e.to_string() })),
    )
}

async fn list_dir(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<ListDirRequest>,
) -> Result<Json<ListDirResponse>, (StatusCode, Json<Value>)> {
    let path = PathBuf::from(&request.worktree_path);
    let relative_path = request.relative_path.clone();
    let svc = FileTreeService::new();
    let result = tokio::task::spawn_blocking(move || svc.list_dir(&path, &relative_path))
        .await
        .map_err(map_join_error)?
        .map_err(map_error)?;

    Ok(Json(result))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/list-dir", post(list_dir))
}
