use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

use nomadflow_core::models::{
    AttachBranchRequest, AttachBranchResponse, CreateFeatureRequest, CreateFeatureResponse,
    DeleteFeatureRequest, DeleteFeatureResponse, ListBranchesRequest, ListBranchesResponse,
    ListFeaturesRequest, ListFeaturesResponse, SwitchFeatureRequest, SwitchFeatureResponse,
};
use nomadflow_core::services::tmux::window_name;

use crate::state::AppState;

async fn list_features(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ListFeaturesRequest>,
) -> Result<Json<ListFeaturesResponse>, (StatusCode, Json<Value>)> {
    match state.git.list_features(&request.repo_path).await {
        Ok(features) => Ok(Json(ListFeaturesResponse { features })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )),
    }
}

async fn create_feature(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateFeatureRequest>,
) -> Result<Json<CreateFeatureResponse>, (StatusCode, Json<Value>)> {
    let base_branch = if request.base_branch == "main" {
        None
    } else {
        Some(request.base_branch.as_str())
    };

    let (worktree_path, branch) = state
        .git
        .create_feature(&request.repo_path, &request.branch_name, base_branch)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    // Derive the worktree name for tmux window naming
    let wt_name = std::path::Path::new(&worktree_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Ensure tmux session and window
    state.tmux.ensure_session().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )
    })?;

    let win_name = window_name(&request.repo_path, &wt_name);
    state
        .tmux
        .ensure_window(&win_name, Some(&worktree_path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    Ok(Json(CreateFeatureResponse {
        worktree_path,
        branch,
        tmux_window: win_name,
    }))
}

async fn delete_feature(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DeleteFeatureRequest>,
) -> Result<Json<DeleteFeatureResponse>, (StatusCode, Json<Value>)> {
    // Prevent deletion of main branch
    let features = state
        .git
        .list_features(&request.repo_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    if let Some(f) = features.iter().find(|f| f.name == request.feature_name) {
        if f.is_main {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "detail": "Cannot delete the main repository branch" })),
            ));
        }
    }

    // Kill tmux window if it exists
    let win_name = window_name(&request.repo_path, &request.feature_name);
    state.tmux.kill_window(&win_name).await;

    let deleted = state
        .git
        .delete_feature(&request.repo_path, &request.feature_name)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    Ok(Json(DeleteFeatureResponse { deleted }))
}

async fn switch_feature(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SwitchFeatureRequest>,
) -> Result<Json<SwitchFeatureResponse>, (StatusCode, Json<Value>)> {
    let features = state
        .git
        .list_features(&request.repo_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    let feature = features.iter().find(|f| f.name == request.feature_name);

    let worktree_path = if let Some(f) = feature {
        f.worktree_path.clone()
    } else {
        // Feature doesn't exist, create it
        let (wt, _branch) = state
            .git
            .create_feature(&request.repo_path, &request.feature_name, None)
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "detail": e.to_string() })),
                )
            })?;
        wt
    };

    // Ensure tmux session
    state.tmux.ensure_session().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )
    })?;

    // Switch to window
    let win_name = window_name(&request.repo_path, &request.feature_name);
    let (switched, has_running_process) = state
        .tmux
        .switch_to_window(&win_name, Some(&worktree_path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    if !switched {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": format!("Failed to switch to window '{win_name}'") })),
        ));
    }

    Ok(Json(SwitchFeatureResponse {
        switched: true,
        worktree_path,
        tmux_window: win_name,
        has_running_process,
    }))
}

async fn list_branches(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ListBranchesRequest>,
) -> Result<Json<ListBranchesResponse>, (StatusCode, Json<Value>)> {
    let (branches, default_branch) = state
        .git
        .list_branches(&request.repo_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    Ok(Json(ListBranchesResponse {
        branches,
        default_branch,
    }))
}

async fn attach_branch(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AttachBranchRequest>,
) -> Result<Json<AttachBranchResponse>, (StatusCode, Json<Value>)> {
    let (worktree_path, branch) = state
        .git
        .attach_branch(&request.repo_path, &request.branch_name)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    let wt_name = std::path::Path::new(&worktree_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Ensure tmux session and window
    state.tmux.ensure_session().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )
    })?;

    let win_name = window_name(&request.repo_path, &wt_name);
    state
        .tmux
        .ensure_window(&win_name, Some(&worktree_path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    Ok(Json(AttachBranchResponse {
        worktree_path,
        branch,
        tmux_window: win_name,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/list-features", post(list_features))
        .route("/api/create-feature", post(create_feature))
        .route("/api/delete-feature", post(delete_feature))
        .route("/api/switch-feature", post(switch_feature))
        .route("/api/list-branches", post(list_branches))
        .route("/api/attach-branch", post(attach_branch))
}
