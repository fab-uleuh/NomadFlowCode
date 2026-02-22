use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};

use nomadflow_core::models::{
    AttachBranchRequest, AttachBranchResponse, CreateFeatureRequest, CreateFeatureResponse,
    DeleteFeatureRequest, DeleteFeatureResponse, ListBranchesRequest, ListBranchesResponse,
    ListFeaturesRequest, ListFeaturesResponse, SwitchFeatureRequest, SwitchFeatureResponse,
};

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

    // Inject hooks so Claude Code state tracking works in the new worktree
    let wt_path = std::path::Path::new(&worktree_path);
    if let Err(e) = state.agent_state.inject_hooks(wt_path).await {
        tracing::warn!(worktree = %worktree_path, "Failed to inject hooks on create: {e}");
    }

    let worktree_name = std::path::Path::new(&worktree_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Ok(Json(CreateFeatureResponse {
        worktree_path,
        branch,
        worktree_name,
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

    // Resolve worktree path before deletion so we can clean up hooks and state files
    let repo_name = std::path::Path::new(&request.repo_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let worktree_path = state.settings.worktrees_dir().join(&repo_name).join(&request.feature_name);

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

    // Clean up hooks and state file for the deleted worktree
    if deleted {
        if let Err(e) = state.agent_state.cleanup_hooks(&worktree_path).await {
            tracing::warn!(worktree = %worktree_path.display(), "Failed to cleanup hooks on delete: {e}");
        }
        let cwd = worktree_path.to_string_lossy().to_string();
        state.agent_state.delete_state_file(&cwd).await;
    }

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

    // Inject hooks into the worktree so Claude Code state tracking works
    let wt_path = std::path::Path::new(&worktree_path);
    if let Err(e) = state.agent_state.inject_hooks(wt_path).await {
        tracing::warn!(worktree = %worktree_path, "Failed to inject hooks on switch: {e}");
    }

    let worktree_name = std::path::Path::new(&worktree_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Ok(Json(SwitchFeatureResponse {
        switched: true,
        worktree_path,
        worktree_name,
    }))
}

async fn list_branches(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ListBranchesRequest>,
) -> Result<Json<ListBranchesResponse>, (StatusCode, Json<Value>)> {
    let (branches, default_branch) =
        state
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

    // Inject hooks so Claude Code state tracking works in the attached worktree
    let wt_path = std::path::Path::new(&worktree_path);
    if let Err(e) = state.agent_state.inject_hooks(wt_path).await {
        tracing::warn!(worktree = %worktree_path, "Failed to inject hooks on attach: {e}");
    }

    let worktree_name = std::path::Path::new(&worktree_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Ok(Json(AttachBranchResponse {
        worktree_path,
        branch,
        worktree_name,
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
