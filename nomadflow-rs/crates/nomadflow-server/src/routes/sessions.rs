use std::sync::Arc;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};

use nomadflow_core::models::{
    AgentStateKind, CloseSessionRequest, CloseSessionResponse, CreateSessionRequest,
    CreateSessionResponse, ListSessionsResponse, SelectSessionRequest, SelectSessionResponse,
    Session, SessionWithState,
};
use nomadflow_core::services::tmux::session_window_name;

use crate::state::AppState;

async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListSessionsResponse>, (StatusCode, Json<Value>)> {
    let sessions = state.tmux.list_sessions().await;
    let states = state
        .agent_state
        .list_all_states(&sessions, &state.tmux)
        .await;

    let enriched: Vec<SessionWithState> = sessions
        .iter()
        .map(|session| {
            let (agent_state, state_timestamp) = states
                .iter()
                .find(|s| s.session_id == session.session_id)
                .map(|s| (s.state, Some(s.timestamp.clone())))
                .unwrap_or((AgentStateKind::Unknown, None));

            SessionWithState {
                session_id: session.session_id.clone(),
                window_name: session.window_name.clone(),
                repo: session.repo.clone(),
                worktree: session.worktree.clone(),
                agent_type: session.agent_type.clone(),
                agent_number: session.agent_number,
                agent_state,
                state_timestamp,
            }
        })
        .collect();

    Ok(Json(ListSessionsResponse { sessions: enriched }))
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

    // Ensure tmux session exists
    state.tmux.ensure_session().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "detail": e.to_string() })),
        )
    })?;

    let agent_type = request.agent_type.unwrap_or_else(|| "agent".to_string());
    let n = state
        .tmux
        .next_agent_number(&repo_name, &worktree_name)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    let window = session_window_name(&repo_name, &worktree_name, &agent_type, n);
    let session_id = window.replace(':', "-");

    state
        .tmux
        .create_window(&window, Some(&request.worktree_path))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "detail": e.to_string() })),
            )
        })?;

    // Inject Claude Code hooks if agent type is claude-code (AC #2)
    if agent_type == "claude-code" {
        // Ensure hook script exists on disk (idempotent, guards against startup failure)
        let _ = state.agent_state.ensure_hook_scripts().await;
        let worktree_path = std::path::Path::new(&request.worktree_path);
        if let Err(e) = state
            .agent_state
            .inject_hooks(worktree_path, &session_id)
            .await
        {
            tracing::warn!("Failed to inject hooks for session {session_id}: {e}");
        }
        // Persist worktree path for reliable cleanup on close
        if let Err(e) = state
            .agent_state
            .save_worktree_path(&session_id, worktree_path)
            .await
        {
            tracing::warn!("Failed to persist worktree path for session {session_id}: {e}");
        }
    }

    // Set env vars in the tmux window for agent state tracking (AC #2, #3)
    let state_dir = state.agent_state.sessions_dir().join(&session_id);
    let env_ok = state
        .tmux
        .send_keys(
            &window,
            &format!(
                "export NOMADFLOW_SESSION_ID='{}' NOMADFLOW_STATE_DIR='{}' && mkdir -p '{}'",
                session_id,
                state_dir.display(),
                state_dir.display()
            ),
            true,
        )
        .await;
    if !env_ok {
        tracing::warn!("Failed to set env vars in tmux window for session {session_id}");
    }

    let session = Session {
        session_id,
        window_name: window,
        repo: repo_name,
        worktree: worktree_name,
        agent_type,
        agent_number: n,
    };

    Ok(Json(CreateSessionResponse { session }))
}

async fn select_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SelectSessionRequest>,
) -> Result<Json<SelectSessionResponse>, (StatusCode, Json<Value>)> {
    let sessions = state.tmux.list_sessions().await;
    let found = sessions.iter().find(|s| s.session_id == request.session_id);

    let window_name = match found {
        Some(s) => s.window_name.clone(),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "detail": format!("session not found: {}", request.session_id) })),
            ));
        }
    };

    // If linkedSession is provided, target that specific linked session
    // for independent cursor. Otherwise fall back to base session.
    let selected = state
        .tmux
        .select_window_in(&window_name, request.linked_session.as_deref())
        .await;
    Ok(Json(SelectSessionResponse { selected }))
}

async fn close_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CloseSessionRequest>,
) -> Result<Json<CloseSessionResponse>, (StatusCode, Json<Value>)> {
    // Look up the session by ID to get the window name (session_id is not reversible)
    let sessions = state.tmux.list_sessions().await;
    let found = sessions.iter().find(|s| s.session_id == request.session_id);

    let (window_name, agent_type) = match found {
        Some(s) => (s.window_name.clone(), s.agent_type.clone()),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "detail": format!("session not found: {}", request.session_id) })),
            ));
        }
    };

    // Clean up hooks before killing the window (AC #2 — cleanup)
    if agent_type == "claude-code" {
        // Prefer persisted worktree path (reliable), fall back to tmux pane cwd
        let worktree_path = match state
            .agent_state
            .read_worktree_path(&request.session_id)
            .await
        {
            Some(p) => Some(p),
            None => state.tmux.get_pane_cwd(&window_name).await,
        };

        if let Some(ref path) = worktree_path {
            if let Err(e) = state
                .agent_state
                .cleanup_hooks(std::path::Path::new(path))
                .await
            {
                tracing::warn!(
                    "Failed to cleanup hooks for session {}: {e}",
                    request.session_id
                );
            }
        }
    }

    let closed = state.tmux.kill_window(&window_name).await;

    // Clean up session state directory (agent-state.json, worktree-path)
    let session_state_dir = state.agent_state.sessions_dir().join(&request.session_id);
    if session_state_dir.is_dir() {
        if let Err(e) = tokio::fs::remove_dir_all(&session_state_dir).await {
            tracing::warn!(
                "Failed to clean up session state dir for {}: {e}",
                request.session_id
            );
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
    use nomadflow_core::shell::run;
    use tower::ServiceExt;

    use crate::auth::auth_middleware;
    use crate::state::AppState;

    fn tmux_available() -> bool {
        std::process::Command::new("which")
            .arg("tmux")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Build a test router with auth, using a unique tmux session.
    fn test_app(settings: Settings) -> (Router, String) {
        let session_name = settings.tmux.session.clone();
        let state = Arc::new(AppState::new(settings));
        let app = router()
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);
        (app, session_name)
    }

    fn test_settings(session_name: &str) -> Settings {
        let mut settings = Settings::default();
        settings.tmux.session = session_name.to_string();
        settings.auth.secret = "test-secret".to_string();
        settings
    }

    fn auth_header() -> (&'static str, String) {
        ("Authorization", "Bearer test-secret".to_string())
    }

    #[tokio::test]
    async fn test_list_sessions_handler() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-ls-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        // Create a tmux session with a session window
        run(
            &format!("tmux new-session -d -s \"{session_name}\" -n \"myapp:feat:agent-1\""),
            None,
        )
        .await;

        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

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
        assert!(!sessions.is_empty());
        // Verify camelCase
        assert!(sessions[0].get("sessionId").is_some());
        assert!(sessions[0].get("windowName").is_some());
        assert!(sessions[0].get("agentType").is_some());
        assert!(sessions[0].get("agentNumber").is_some());
        // Verify enriched agent state fields are present
        assert!(
            sessions[0].get("agentState").is_some(),
            "response should include agentState field"
        );
        // agent_type "agent" (generic) → detect_process_state → idle shell = waiting_for_input
        let state_val = sessions[0]["agentState"].as_str().unwrap();
        assert_eq!(
            state_val, "waiting_for_input",
            "generic session with idle shell should have 'waiting_for_input', got: {state_val}"
        );
        // Generic adapter always provides a timestamp
        assert!(
            sessions[0]["stateTimestamp"].is_string(),
            "generic session should have a stateTimestamp"
        );

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
    }

    #[tokio::test]
    async fn test_list_sessions_unknown_state_for_claude_code_without_file() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-unk-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        // Use a temp base dir with NO state files
        let tmp_dir = std::env::temp_dir().join(format!("nf-test-unk-{}", std::process::id()));
        std::fs::create_dir_all(tmp_dir.join("sessions")).unwrap();

        // Create a claude-code session (no state file exists for it)
        run(
            &format!("tmux new-session -d -s \"{session_name}\" -n \"myapp:feat:claude-code-1\""),
            None,
        )
        .await;

        let mut settings = test_settings(&session_name);
        settings.paths.base_dir = tmp_dir.to_str().unwrap().to_string();
        let (app, _) = test_app(settings);

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
        assert!(!sessions.is_empty());

        let cc_session = sessions
            .iter()
            .find(|s| s["agentType"].as_str() == Some("claude-code"))
            .expect("should find claude-code session");

        // claude-code with no state file → Unknown
        assert_eq!(
            cc_session["agentState"].as_str().unwrap(),
            "unknown",
            "claude-code session without state file should be 'unknown'"
        );
        assert!(
            cc_session["stateTimestamp"].is_null(),
            "stateTimestamp should be null when state is unknown"
        );

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[tokio::test]
    async fn test_list_sessions_claude_code_state_from_file() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-cc-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        // Create temp base dir for agent state files
        let tmp_dir = std::env::temp_dir().join(format!("nf-test-cc-{}", std::process::id()));
        let session_id = "myapp-feat-claude-code-1";
        let state_dir = tmp_dir.join("sessions").join(session_id);
        std::fs::create_dir_all(&state_dir).unwrap();

        // Write a state file for the claude-code session
        let state_json = json!({
            "sessionId": session_id,
            "agentType": "claude-code",
            "state": "waiting_for_input",
            "timestamp": "2026-02-15T14:30:00Z",
            "lastEvent": "TaskComplete"
        });
        std::fs::write(
            state_dir.join("agent-state.json"),
            serde_json::to_string(&state_json).unwrap(),
        )
        .unwrap();

        // Create tmux session with a window matching the session
        run(
            &format!("tmux new-session -d -s \"{session_name}\" -n \"myapp:feat:claude-code-1\""),
            None,
        )
        .await;

        // Configure settings to use temp base dir so AgentStateService reads our file
        let mut settings = test_settings(&session_name);
        settings.paths.base_dir = tmp_dir.to_str().unwrap().to_string();
        let (app, _) = test_app(settings);

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
        assert!(!sessions.is_empty());

        // Find the claude-code session
        let cc_session = sessions
            .iter()
            .find(|s| s["sessionId"].as_str() == Some(session_id))
            .expect("should find claude-code session");

        // Verify state was read from file (not defaulting to unknown)
        assert_eq!(
            cc_session["agentState"].as_str().unwrap(),
            "waiting_for_input",
            "claude-code session should have state from agent-state.json file"
        );
        assert_eq!(
            cc_session["stateTimestamp"].as_str().unwrap(),
            "2026-02-15T14:30:00Z",
            "claude-code session should have timestamp from agent-state.json file"
        );

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[tokio::test]
    async fn test_create_session_handler() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-cs-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

        // Use /tmp as a valid directory
        let (hdr, val) = auth_header();
        let body = json!({ "worktreePath": "/tmp", "agentType": "claude" });
        let req = Request::builder()
            .method("POST")
            .uri("/api/create-session")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let session = &json["session"];
        assert!(session.get("sessionId").is_some());
        assert!(session.get("windowName").is_some());
        assert_eq!(session["agentType"], "claude");
        assert_eq!(session["agentNumber"], 1);

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
    }

    #[tokio::test]
    async fn test_create_session_claude_code_injects_hooks() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-cc-cs-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        // Use a temp dir as worktree so we can check for .claude/settings.local.json
        let tmp_dir = std::env::temp_dir().join(format!("nf-test-cc-cs-{}", std::process::id()));
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let mut settings = test_settings(&session_name);
        // Use a separate base dir for hook scripts and session state
        let base_dir =
            std::env::temp_dir().join(format!("nf-test-cc-cs-base-{}", std::process::id()));
        settings.paths.base_dir = base_dir.to_str().unwrap().to_string();
        let (app, _) = test_app(settings);

        let (hdr, val) = auth_header();
        let body = json!({
            "worktreePath": tmp_dir.to_str().unwrap(),
            "agentType": "claude-code"
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/create-session")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let session = &json["session"];
        assert_eq!(session["agentType"], "claude-code");

        let session_id = session["sessionId"].as_str().unwrap();

        // Verify .claude/settings.local.json was created with hook entries
        let settings_path = tmp_dir.join(".claude").join("settings.local.json");
        assert!(
            settings_path.exists(),
            "inject_hooks should create .claude/settings.local.json"
        );
        let settings_content: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert!(
            settings_content.get("hooks").is_some(),
            "settings.local.json should contain hooks"
        );

        // Verify worktree-path was persisted for cleanup
        let worktree_path_file = base_dir
            .join("sessions")
            .join(session_id)
            .join("worktree-path");
        assert!(
            worktree_path_file.exists(),
            "worktree-path should be persisted in session dir"
        );
        let persisted_path = std::fs::read_to_string(&worktree_path_file).unwrap();
        assert_eq!(persisted_path, tmp_dir.to_str().unwrap());

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
        let _ = std::fs::remove_dir_all(&tmp_dir);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[tokio::test]
    async fn test_create_session_default_agent_type() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-da-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

        // Omit agentType — should default to "agent"
        let (hdr, val) = auth_header();
        let body = json!({ "worktreePath": "/tmp" });
        let req = Request::builder()
            .method("POST")
            .uri("/api/create-session")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let session = &json["session"];
        assert_eq!(session["agentType"], "agent");
        assert_eq!(session["agentNumber"], 1);

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
    }

    #[tokio::test]
    async fn test_close_session_handler() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-cl-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        // Create a session with 2 windows (killing one won't destroy the session)
        run(
            &format!("tmux new-session -d -s \"{session_name}\" -n \"myapp:feat:agent-1\""),
            None,
        )
        .await;
        run(
            &format!("tmux new-window -t \"{session_name}\" -n \"myapp:feat:agent-2\""),
            None,
        )
        .await;

        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

        let (hdr, val) = auth_header();
        let body = json!({ "sessionId": "myapp-feat-agent-1" });
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
        assert_eq!(json["closed"], true);

        // Verify window was actually removed by checking tmux directly
        let check = run(
            &format!("tmux list-windows -t \"{session_name}\" -F \"#{{window_name}}\""),
            None,
        )
        .await;
        let windows: Vec<&str> = check.stdout.trim().lines().collect();
        assert!(
            !windows.contains(&"myapp:feat:agent-1"),
            "window should have been removed"
        );
        assert!(
            windows.contains(&"myapp:feat:agent-2"),
            "other window should still exist"
        );

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
    }

    #[tokio::test]
    async fn test_close_session_not_found() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-nf-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        // Create a session with one window (so tmux session exists)
        run(
            &format!("tmux new-session -d -s \"{session_name}\" -n \"placeholder\""),
            None,
        )
        .await;

        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

        let (hdr, val) = auth_header();
        let body = json!({ "sessionId": "nonexistent-session-id" });
        let req = Request::builder()
            .method("POST")
            .uri("/api/close-session")
            .header("Content-Type", "application/json")
            .header(hdr, val)
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Cleanup
        run(&format!("tmux kill-session -t \"{session_name}\""), None).await;
    }

    #[tokio::test]
    async fn test_auth_required_returns_401() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-auth-{}", std::process::id());
        run(
            &format!("tmux kill-session -t \"{session_name}\" 2>/dev/null"),
            None,
        )
        .await;

        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

        // No auth header
        let req = Request::builder()
            .method("POST")
            .uri("/api/list-sessions")
            .header("Content-Type", "application/json")
            .body(Body::from("{}"))
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Verify WWW-Authenticate header is present
        let www_auth = response.headers().get("www-authenticate");
        assert!(www_auth.is_some());
        assert!(www_auth.unwrap().to_str().unwrap().contains("Basic"));

        // No cleanup needed - tmux session was never created
    }

    #[tokio::test]
    async fn test_create_session_bad_path() {
        if !tmux_available() {
            eprintln!("Skipping: tmux not available");
            return;
        }

        let session_name = format!("nf-test-handler-bp-{}", std::process::id());
        let settings = test_settings(&session_name);
        let (app, _) = test_app(settings);

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
}
