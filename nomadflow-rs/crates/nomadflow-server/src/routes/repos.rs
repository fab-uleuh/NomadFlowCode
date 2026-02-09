use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

use nomadflow_core::error::NomadError;
use nomadflow_core::models::{CloneRepoRequest, CloneRepoResponse, ListReposResponse};

use crate::state::AppState;

async fn list_repos(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListReposResponse>, (StatusCode, Json<Value>)> {
    match state.git.list_repos().await {
        Ok(repos) => Ok(Json(ListReposResponse { repos })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )),
    }
}

async fn clone_repo(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CloneRepoRequest>,
) -> Result<Json<CloneRepoResponse>, (StatusCode, Json<Value>)> {
    match state
        .git
        .clone_repo(&request.url, request.token.as_deref(), request.name.as_deref())
        .await
    {
        Ok((name, path, branch)) => Ok(Json(CloneRepoResponse { name, path, branch })),
        Err(NomadError::AlreadyExists(msg)) => Err((
            StatusCode::CONFLICT,
            Json(json!({ "detail": msg })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/list-repos", post(list_repos))
        .route("/api/clone-repo", post(clone_repo))
}
