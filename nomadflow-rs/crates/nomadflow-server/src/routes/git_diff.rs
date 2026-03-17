use std::path::PathBuf;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::Value;

use nomadflow_core::models::{
    FileContentRequest, FileContentResponse, FileDiffRequest, FileDiffResponse,
    WorktreeStatusRequest, WorktreeStatusResponse,
};

use super::{map_error, map_join_error};
use crate::state::AppState;

async fn worktree_status(
    State(state): State<Arc<AppState>>,
    Json(request): Json<WorktreeStatusRequest>,
) -> Result<Json<WorktreeStatusResponse>, (StatusCode, Json<Value>)> {
    let path = PathBuf::from(&request.worktree_path);
    let git_diff = state.git_diff.clone();
    let result = tokio::task::spawn_blocking(move || git_diff.worktree_status(&path))
        .await
        .map_err(map_join_error)?
        .map_err(map_error)?;

    Ok(Json(result))
}

async fn file_diff(
    State(state): State<Arc<AppState>>,
    Json(request): Json<FileDiffRequest>,
) -> Result<Json<FileDiffResponse>, (StatusCode, Json<Value>)> {
    let path = PathBuf::from(&request.worktree_path);
    let file_path = request.file_path.clone();
    let git_diff = state.git_diff.clone();
    let result = tokio::task::spawn_blocking(move || git_diff.file_diff(&path, &file_path))
        .await
        .map_err(map_join_error)?
        .map_err(map_error)?;

    Ok(Json(result))
}

async fn file_content(
    State(state): State<Arc<AppState>>,
    Json(request): Json<FileContentRequest>,
) -> Result<Json<FileContentResponse>, (StatusCode, Json<Value>)> {
    let path = PathBuf::from(&request.worktree_path);
    let file_path = request.file_path.clone();
    let git_diff = state.git_diff.clone();
    let result = tokio::task::spawn_blocking(move || git_diff.file_content(&path, &file_path))
        .await
        .map_err(map_join_error)?
        .map_err(map_error)?;

    Ok(Json(FileContentResponse {
        file_path: request.file_path,
        content: result,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/worktree-status", post(worktree_status))
        .route("/api/file-diff", post(file_diff))
        .route("/api/file-content", post(file_content))
}
