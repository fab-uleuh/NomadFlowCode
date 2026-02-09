use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite};
use tracing::{error, info, warn};

use crate::state::AppState;

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

async fn ws_proxy(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    // Verify auth via query parameter
    let secret = &state.settings.auth.secret;
    if !secret.is_empty() {
        let token = query.token.unwrap_or_default();
        if token != *secret {
            warn!("WebSocket auth failed: invalid token");
            return Response::builder()
                .status(403)
                .body("Authentication required".into())
                .unwrap();
        }
    }

    let ttyd_port = state.settings.ttyd.port;
    let auth_secret = state.settings.auth.secret.clone();

    info!(ttyd_port, "WebSocket proxy upgrade requested");

    // Accept the "tty" subprotocol (required by ttyd)
    ws.protocols(["tty"])
        .on_upgrade(move |socket| handle_ws(socket, ttyd_port, auth_secret))
}

async fn handle_ws(client_ws: WebSocket, ttyd_port: u16, auth_secret: String) {
    let ws_url = format!("ws://127.0.0.1:{ttyd_port}/ws");

    info!(%ws_url, "Connecting to ttyd");

    // Build a proper WebSocket client request (auto-generates handshake headers)
    let mut request = match ws_url.into_client_request() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to build ttyd request: {e}");
            return;
        }
    };

    // Add the "tty" subprotocol (required by ttyd)
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        "tty".parse().unwrap(),
    );

    // Add basic auth header if secret is configured
    if !auth_secret.is_empty() {
        let creds = base64::engine::general_purpose::STANDARD
            .encode(format!("nomadflow:{auth_secret}"));
        request.headers_mut().insert(
            "Authorization",
            format!("Basic {creds}").parse().unwrap(),
        );
    }

    let ttyd_ws = match connect_async(request).await {
        Ok((ws, _)) => {
            info!("Connected to ttyd WebSocket");
            ws
        }
        Err(e) => {
            error!("Failed to connect to ttyd at ws://127.0.0.1:{ttyd_port}/ws: {e}");
            return;
        }
    };

    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut ttyd_tx, mut ttyd_rx) = ttyd_ws.split();

    // Client -> ttyd
    let client_to_ttyd = async {
        while let Some(msg) = client_rx.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let s: String = text.to_string();
                    if ttyd_tx
                        .send(tungstenite::Message::Text(s.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    let bytes: Vec<u8> = data.to_vec();
                    if ttyd_tx
                        .send(tungstenite::Message::Binary(bytes.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    };

    // ttyd -> Client
    let ttyd_to_client = async {
        while let Some(msg) = ttyd_rx.next().await {
            match msg {
                Ok(tungstenite::Message::Text(text)) => {
                    let s: String = text.to_string();
                    if client_tx.send(Message::Text(s.into())).await.is_err() {
                        break;
                    }
                }
                Ok(tungstenite::Message::Binary(data)) => {
                    let bytes: Vec<u8> = data.to_vec();
                    if client_tx.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                Ok(tungstenite::Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = client_to_ttyd => {},
        _ = ttyd_to_client => {},
    }

    info!("WebSocket proxy session ended");
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/terminal/ws", get(ws_proxy))
}
