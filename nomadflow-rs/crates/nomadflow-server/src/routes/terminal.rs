use std::sync::Arc;

use axum::{
    body::Body,
    extract::{
        ws::WebSocket,
        Path, Query, State, WebSocketUpgrade,
    },
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::Engine;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::connect_async;
use tracing::{error, warn};

use crate::state::AppState;

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
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
    let secret = &state.settings.auth.secret;
    if !secret.is_empty() {
        let token = query.token.unwrap_or_default();
        let matches: bool = token.as_bytes().ct_eq(secret.as_bytes()).into();
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

    ws.protocols(["tty"])
        .on_upgrade(move |socket| handle_ws(socket, ttyd_port, auth_secret))
}

async fn handle_ws(client_ws: WebSocket, ttyd_port: u16, auth_secret: String) {
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

    if !auth_secret.is_empty() {
        let creds = base64::engine::general_purpose::STANDARD
            .encode(format!("nomadflow:{auth_secret}"));
        request
            .headers_mut()
            .insert("Authorization", format!("Basic {creds}").parse().unwrap());
    }

    let ttyd_ws = match connect_async(request).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            error!("Failed to connect to ttyd: {e}");
            return;
        }
    };

    nomadflow_ws::bridge(client_ws, ttyd_ws).await;
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
async fn proxy_ttyd_request(
    state: &AppState,
    path: &str,
) -> Result<impl IntoResponse, StatusCode> {
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
