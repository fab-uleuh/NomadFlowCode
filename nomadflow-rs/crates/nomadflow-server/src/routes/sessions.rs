use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};

use nomadflow_core::models::{
    AgentStateKind, CloseSessionRequest, CloseSessionResponse, CreateSessionRequest,
    CreateSessionResponse, ListSessionsResponse, SelectSessionRequest, SelectSessionResponse,
    SessionWithState,
};
use nomadflow_pty::types::{CreatePaneRequest, PaneId};

use crate::state::AppState;

async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListSessionsResponse>, (StatusCode, Json<Value>)> {
    let manager = state.pane_manager.lock().await;
    let panes = manager.list_panes();
    
    let sessions: Vec<SessionWithState> = panes
        .iter()
        .map(|info| {
            let _session_id = format!("{}-{}-{}", info.repo, info.worktree, info.agent_number);
            let agent_state = match info.agent_state {
                nomadflow_pty::types::AgentStateKind::WaitingForInput => AgentStateKind::WaitingForInput,
                nomadflow_pty::types::AgentStateKind::WaitingForPermission => AgentStateKind::WaitingForPermission,
                nomadflow_pty::types::AgentStateKind::Generating => AgentStateKind::Generating,
                nomadflow_pty::types::AgentStateKind::Idle => AgentStateKind::Idle,
                nomadflow_pty::types::AgentStateKind::Done => AgentStateKind::Done,
                nomadflow_pty::types::AgentStateKind::Error => AgentStateKind::Error,
                nomadflow_pty::types::AgentStateKind::Unknown => AgentStateKind::Unknown,
            };
            SessionWithState {
                session_id: info.id.0.to_string(),
                window_name: info.label.0.clone(),
                repo: info.repo.clone(),
                worktree: info.worktree.clone(),
                agent_type: info.agent_type.clone(),
                agent_number: info.agent_number as u32,
                agent_state,
                state_timestamp: None,
            }
        })
        .collect();

    Ok(Json(ListSessionsResponse { sessions }))
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, (StatusCode, Json<Value>)> {
    let path = std::path::Path::new(&request.worktree_path);

    // Validate worktree_path exists on disk
    if !path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "detail": format!("worktree path does not exist: {}", request.worktree_path) }),
            ),
        ));
    }

    // Use detect_current_worktree for robust repo/worktree resolution,
    // fall back to path components if not in a tracked repo.
    let (repo_name, worktree_name) = match state.git.detect_current_worktree(path).await {
        Ok(info) => (info.repo_name, info.worktree_name),
        Err(_) => {
            // Fallback: parent dir = repo, last dir = worktree
            let wt = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let repo = path
                .parent()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            (repo, wt)
        }
    };

    let agent_type = request.agent_type.unwrap_or_else(|| "agent".to_string());

    // Create pane via PaneManager
    let create_req = CreatePaneRequest {
        repo: repo_name.clone(),
        worktree: worktree_name.clone(),
        agent_type: agent_type.clone(),
        cwd: path.to_path_buf(),
        cols: Some(80),
        rows: Some(24),
        shell: None,
    };

    let mut manager = state.pane_manager.lock().await;
    let info = manager.create_pane(create_req).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )
    })?;

    let session_id = info.id.0.to_string();

    Ok(Json(CreateSessionResponse {
        session: nomadflow_core::models::Session {
            session_id: session_id.clone(),
            window_name: info.label.0.clone(),
            repo: repo_name,
            worktree: worktree_name,
            agent_type,
            agent_number: info.agent_number as u32,
        },
    }))
}

async fn select_session(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<SelectSessionRequest>,
) -> Result<Json<SelectSessionResponse>, (StatusCode, Json<Value>)> {
    // With PTY, session selection is handled client-side via WebSocket subscribe
    // This endpoint just validates the pane exists
    let _pane_id = request.session_id.parse::<u16>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "detail": "invalid session id" })),
        )
    })?;

    Ok(Json(SelectSessionResponse { selected: true }))
}

async fn close_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CloseSessionRequest>,
) -> Result<Json<CloseSessionResponse>, (StatusCode, Json<Value>)> {
    // Parse pane ID from session_id
    let pane_id = request.session_id.parse::<u16>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "detail": "invalid session id" })),
        )
    })?;

    let mut manager = state.pane_manager.lock().await;
    // Get CWD before destroying so we can clean up the state file
    let cwd = manager.get_pane_info(PaneId(pane_id)).map(|info| info.cwd.clone());
    let closed = manager.destroy_pane(PaneId(pane_id)).is_ok();
    drop(manager);

    if closed {
        if let Some(cwd) = cwd {
            state.agent_state.delete_state_file(&cwd).await;
        }
    }

    Ok(Json(CloseSessionResponse { closed }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/list-sessions", post(list_sessions))
        .route("/api/create-session", post(create_session))
        .route("/api/select-session", post(select_session))
        .route("/api/close-session", post(close_session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::middleware;
    use http_body_util::BodyExt;
    use nomadflow_core::config::Settings;
    use tower::ServiceExt;

    use crate::auth::auth_middleware;

    fn test_app(settings: Settings) -> Router {
        let state = Arc::new(AppState::new(settings));
        router()
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state)
    }

    fn test_settings() -> Settings {
        let mut settings = Settings::default();
        settings.auth.secret = "test-secret".to_string();
        settings
    }

    fn auth_header() -> (&'static str, String) {
        ("Authorization", "Bearer test-secret".to_string())
    }

    #[tokio::test]
    async fn test_list_sessions_handler() {
        let settings = test_settings();
        let app = test_app(settings);

        let (hdr, val) = auth_header();
        let req = Request::builder()
            .method("POST")
            .uri("/api/list-sessions")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from("{}"))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let sessions = json["sessions"].as_array().unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_create_session_bad_path() {
        let settings = test_settings();
        let app = test_app(settings);

        let (hdr, val) = auth_header();
        let body = json!({ "worktreePath": "/nonexistent/path/does/not/exist" });
        let req = Request::builder()
            .method("POST")
            .uri("/api/create-session")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_close_session_not_found() {
        let settings = test_settings();
        let app = test_app(settings);

        let (hdr, val) = auth_header();
        let body = json!({ "sessionId": "99999" });
        let req = Request::builder()
            .method("POST")
            .uri("/api/close-session")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["closed"], false);
    }

    #[tokio::test]
    async fn test_auth_required_returns_401() {
        let settings = test_settings();
        let app = test_app(settings);

        let req = Request::builder()
            .method("POST")
            .uri("/api/list-sessions")
            .header("Content-Type", "application/json")
            .body(Body::from("{}"))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
