use std::sync::Arc;

use axum::{
    body::Body,
    extract::{ws::WebSocket, Path, Query, State, WebSocketUpgrade},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::Engine;
use futures_util::SinkExt;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::{error, info, warn};

use nomadflow_core::shell::run;

use crate::state::AppState;

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
    /// Optional tmux window name to select in the linked session.
    /// Passed by the mobile client so the correct window is active
    /// before the first byte of terminal output reaches the client.
    window: Option<String>,
}

/// WebSocket proxy: mobile connects here, we forward to ttyd with Basic Auth.
/// The mobile loads the ttyd HTML page directly (with basicAuthCredential),
/// but WKWebView does not send Basic Auth on WebSocket upgrades,
/// so the WS connection must go through this proxy.
async fn ws_proxy(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let auth_token = query.token.unwrap_or_default();
    let secret = &state.settings.auth.secret;
    if !secret.is_empty() {
        let matches: bool = auth_token.as_bytes().ct_eq(secret.as_bytes()).into();
        if !matches {
            warn!("WebSocket auth failed: invalid token");
            return Response::builder()
                .status(403)
                .body("Authentication required".into())
                .unwrap();
        }
    }

    let ttyd_port = state.settings.ttyd.port;
    let auth_secret = state.settings.auth.secret.clone();
    let session_name = state.settings.tmux.session.clone();
    let discovery_lock = state.session_discovery_lock.clone();
    let desired_window = query.window;

    ws.protocols(["tty"])
        .on_upgrade(move |socket| {
            handle_ws(
                socket,
                ttyd_port,
                auth_secret,
                session_name,
                discovery_lock,
                desired_window,
            )
        })
}

async fn handle_ws(
    mut client_ws: WebSocket,
    ttyd_port: u16,
    auth_secret: String,
    session_name: String,
    discovery_lock: Arc<tokio::sync::Mutex<()>>,
    desired_window: Option<String>,
) {
    // Phase 1: serialized discovery (prevents race condition when multiple clients connect)
    let ttyd_ws = {
        let _guard = discovery_lock.lock().await;

        // Snapshot existing linked sessions before connecting (to detect the new one after)
        let sessions_before = list_linked_sessions(&session_name).await;

        let ws_url = format!("ws://127.0.0.1:{ttyd_port}/ws");

        let mut request = match ws_url.into_client_request() {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to build ttyd request: {e}");
                return;
            }
        };

        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", "tty".parse().unwrap());

        // Pre-compute base64 credentials (used for both HTTP header and WS auth message)
        let creds = if !auth_secret.is_empty() {
            Some(
                base64::engine::general_purpose::STANDARD
                    .encode(format!("nomadflow:{auth_secret}")),
            )
        } else {
            None
        };

        if let Some(ref creds) = creds {
            request
                .headers_mut()
                .insert("Authorization", format!("Basic {creds}").parse().unwrap());
        }

        let mut ttyd_ws = match connect_async(request).await {
            Ok((ws, _)) => ws,
            Err(e) => {
                error!("Failed to connect to ttyd: {e}");
                return;
            }
        };

        // ttyd requires a WebSocket-level auth token as the first message.
        if let Some(ref creds) = creds {
            let auth_msg = format!(r#"{{"AuthToken":"{creds}"}}"#);
            if let Err(e) = ttyd_ws
                .send(tokio_tungstenite::tungstenite::Message::Text(
                    auth_msg.into(),
                ))
                .await
            {
                error!("Failed to send auth token to ttyd: {e}");
                return;
            }
        }

        // Wait for tmux to create the linked session (retry up to 5 times)
        let mut linked_session = None;
        for _ in 0..5 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            linked_session = discover_new_linked_session(&session_name, &sessions_before).await;
            if linked_session.is_some() {
                break;
            }
        }

        if linked_session.is_none() {
            warn!(session = %session_name, "Failed to discover linked tmux session after 5 retries");
        }

        if let Some(ref name) = linked_session {
            info!(session = %name, "Discovered linked tmux session");
            // Hide tmux status bar on the linked session — it doesn't inherit
            // the base session's `status off` option (tmux options are per-session).
            let result = run(
                &format!("tmux set-option -t \"{name}\" status off"),
                None,
            )
            .await;
            if !result.success() {
                warn!(session = %name, "Failed to set status off on linked session");
            }

            // Select the desired window in the linked session. tmux new-session
            // does NOT inherit the base session's active window — it defaults to
            // window 0. Without this, the client would briefly see the wrong window.
            if let Some(ref win) = desired_window {
                info!(session = %name, window = %win, "Selecting desired window in linked session");
                let result = run(
                    &format!("tmux select-window -t \"{name}:{win}\""),
                    None,
                )
                .await;
                if !result.success() {
                    warn!(session = %name, window = %win, "Failed to select window in linked session");
                }
            }
        }

        // Phase 2: push linked session name to client BEFORE bridge starts
        if let Some(ref name) = linked_session {
            let msg = format!(r#"{{"linkedSession":"{name}"}}"#);
            if let Err(e) = client_ws
                .send(axum::extract::ws::Message::Text(msg.into()))
                .await
            {
                warn!(session = %name, "Failed to send linkedSession to client: {e}");
            }
        }

        ttyd_ws
    }; // discovery_lock released here

    // Phase 3: bridge client ↔ ttyd (blocks until one side closes)
    nomadflow_ws::bridge(client_ws, ttyd_ws).await;
}

/// List all tmux sessions in the same group as `session_name`.
async fn list_linked_sessions(session_name: &str) -> Vec<String> {
    let result = run(
        &format!("tmux list-sessions -F \"#{{session_name}} #{{session_group}}\" 2>/dev/null"),
        None,
    )
    .await;

    if !result.success() {
        return Vec::new();
    }

    result
        .stdout
        .trim()
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, ' ');
            let name = parts.next()?;
            let group = parts.next().unwrap_or("");
            // Sessions in the same group as our base session
            if group == session_name || name == session_name {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Discover a newly created linked session by comparing before/after snapshots.
async fn discover_new_linked_session(
    session_name: &str,
    sessions_before: &[String],
) -> Option<String> {
    let sessions_after = list_linked_sessions(session_name).await;
    // Find sessions that are new (not in the before list) and not the base session
    sessions_after
        .into_iter()
        .find(|s| s != session_name && !sessions_before.contains(s))
}

/// Proxy GET /terminal → ttyd HTML page
async fn terminal_html_proxy(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    proxy_ttyd_request(&state, "/").await
}

/// Proxy GET /terminal/*path → ttyd assets (JS, CSS, etc.)
async fn terminal_asset_proxy(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    proxy_ttyd_request(&state, &format!("/{path}")).await
}

/// Proxy an HTTP request to the local ttyd instance.
async fn proxy_ttyd_request(state: &AppState, path: &str) -> Result<impl IntoResponse, StatusCode> {
    let ttyd_port = state.settings.ttyd.port;
    let url = format!("http://127.0.0.1:{ttyd_port}{path}");

    let mut req = state.http_client.get(&url);

    // Add Basic Auth if secret is configured
    if !state.settings.auth.secret.is_empty() {
        req = req.basic_auth("nomadflow", Some(&state.settings.auth.secret));
    }

    let resp = req.send().await.map_err(|e| {
        error!("Failed to proxy to ttyd: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let bytes = resp.bytes().await.map_err(|e| {
        error!("Failed to read ttyd response: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .unwrap())
}

pub fn ws_router() -> Router<Arc<AppState>> {
    Router::new().route("/terminal/ws", get(ws_proxy))
}

pub fn http_proxy_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/terminal", get(terminal_html_proxy))
        .route("/terminal/{*path}", get(terminal_asset_proxy))
}
