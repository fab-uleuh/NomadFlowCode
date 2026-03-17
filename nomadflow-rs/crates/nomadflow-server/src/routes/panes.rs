use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use tokio::sync::mpsc;
use tracing::{info, warn};

use nomadflow_pty::protocol::{ControlMsg, PaneInfoDto, WsFrame};
use nomadflow_pty::{ClientId, PaneId};

use crate::frame_handler::ForwardingTasks;
use crate::state::AppState;

static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);

fn next_client_id() -> ClientId {
    ClientId(NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

pub fn ws_router() -> Router<Arc<AppState>> {
    Router::new().route("/ws/panes", get(ws_handler))
}

/// REST endpoints for pane management (used by home screen where no WS connection exists).
pub fn rest_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/list-panes", post(list_panes))
        .route("/api/destroy-pane", post(destroy_pane))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ListPanesResponse {
    panes: Vec<PaneInfoDto>,
}

async fn list_panes(
    State(state): State<Arc<AppState>>,
) -> Json<ListPanesResponse> {
    let manager = state.pane_manager.lock().await;
    let panes: Vec<PaneInfoDto> = manager.list_panes().iter().map(PaneInfoDto::from).collect();
    Json(ListPanesResponse { panes })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DestroyPaneRequest {
    pane_id: u16,
}

async fn destroy_pane(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DestroyPaneRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut manager = state.pane_manager.lock().await;
    // Get CWD before destroying so we can clean up the state file
    let cwd = manager.get_pane_info(PaneId(request.pane_id)).map(|info| info.cwd.clone());
    match manager.destroy_pane(PaneId(request.pane_id)) {
        Ok(()) => {
            drop(manager);
            if let Some(cwd) = cwd {
                state.agent_state.delete_state_file(&cwd).await;
            }
            Ok(Json(json!({ "destroyed": true })))
        }
        Err(_) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": format!("pane not found: {}", request.pane_id) })),
        )),
    }
}

async fn ws_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WsQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let secret = &state.settings.auth.secret;
    if !secret.is_empty() {
        // Try query param first (backward compat)
        let query_auth: bool = match &query.token {
            Some(token) => token.as_bytes().ct_eq(secret.as_bytes()).into(),
            None => false,
        };

        // Try Sec-WebSocket-Protocol: bearer.{token}
        let (proto_auth, matched_protocol) = check_ws_protocol_auth(&headers, secret);

        if !query_auth && !proto_auth {
            return StatusCode::UNAUTHORIZED.into_response();
        }

        // If authenticated via subprotocol, echo it back in the upgrade response
        if let Some(protocol) = matched_protocol {
            return ws
                .protocols([protocol])
                .on_upgrade(move |socket| handle_ws_connection(socket, state));
        }
    }
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Check Sec-WebSocket-Protocol header for `bearer.{token}` pattern.
/// Returns (authenticated, matched_protocol_to_echo).
fn check_ws_protocol_auth(headers: &HeaderMap, secret: &str) -> (bool, Option<String>) {
    let header_val = match headers.get(header::SEC_WEBSOCKET_PROTOCOL) {
        Some(v) => match v.to_str() {
            Ok(s) => s,
            Err(_) => return (false, None),
        },
        None => return (false, None),
    };

    // The header may contain multiple comma-separated protocols
    for protocol in header_val.split(',') {
        let protocol = protocol.trim();
        if let Some(token) = protocol.strip_prefix("bearer.") {
            if token.as_bytes().ct_eq(secret.as_bytes()).into() {
                return (true, Some(protocol.to_string()));
            }
        }
    }

    (false, None)
}

async fn handle_ws_connection(socket: WebSocket, state: Arc<AppState>) {
    let client_id = next_client_id();
    info!(%client_id, "WebSocket client connected");

    let (ws_sender, ws_receiver) = socket.split();

    // Single writer channel: all output forwarding tasks send encoded frames here.
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(256);

    // Writer task: receives encoded frames and sends them on the WS.
    let writer = tokio::spawn(ws_writer(ws_sender, write_rx));

    // Agent state update forwarder
    let mut agent_state_rx = state.agent_state_broadcast.subscribe();
    let state_writer_tx = write_tx.clone();
    let state_forwarder = tokio::spawn(async move {
        while let Ok((pane_id, agent_state)) = agent_state_rx.recv().await {
            let msg = WsFrame::Control {
                payload: ControlMsg::PaneStateUpdated {
                    pane_id,
                    agent_state,
                },
            };
            if state_writer_tx.send(msg.encode()).await.is_err() {
                break;
            }
        }
    });

    // Task 4.1: Send pane list automatically on new connection.
    {
        let panes = {
            let mut manager = state.pane_manager.lock().await;
            manager.cleanup_dead_panes();
            manager
                .list_panes()
                .iter()
                .map(PaneInfoDto::from)
                .collect::<Vec<_>>()
        };
        let resp = WsFrame::Control {
            payload: ControlMsg::PaneList { panes },
        };
        let _ = write_tx.send(resp.encode()).await;
    }

    // Reader task: receives from WS, routes to PaneManager.
    client_reader(ws_receiver, state.clone(), client_id, write_tx).await;

    // Client disconnected — cleanup.
    info!(%client_id, "WebSocket client disconnected, cleaning up");
    state_forwarder.abort();
    let mut manager = state.pane_manager.lock().await;
    manager.remove_client(client_id);
    drop(manager);

    // writer will stop when all write_tx senders are dropped
    let _ = writer.await;
}

/// Single writer loop: serializes all outgoing frames through one task.
async fn ws_writer(
    mut sender: futures_util::stream::SplitSink<WebSocket, Message>,
    mut rx: mpsc::Receiver<Vec<u8>>,
) {
    while let Some(frame_bytes) = rx.recv().await {
        if sender.send(Message::Binary(frame_bytes.into())).await.is_err() {
            break;
        }
    }
}

/// Main read loop: decodes WS binary frames and routes them.
async fn client_reader(
    mut receiver: futures_util::stream::SplitStream<WebSocket>,
    state: Arc<AppState>,
    client_id: ClientId,
    write_tx: mpsc::Sender<Vec<u8>>,
) {
    let mut forwarding = ForwardingTasks::new();

    while let Some(msg_result) = receiver.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                warn!(%client_id, "WS read error: {e}");
                break;
            }
        };

        // Task 4.2: Clean up finished forwarding tasks (pane destroyed by another client).
        forwarding.cleanup_finished();

        match msg {
            Message::Binary(data) => {
                let frame = match WsFrame::decode(&data) {
                    Ok(f) => f,
                    Err(e) => {
                        warn!(%client_id, "Invalid frame: {e}");
                        let err_frame = WsFrame::Control {
                            payload: ControlMsg::Error {
                                message: e.to_string(),
                            },
                        };
                        let _ = write_tx.send(err_frame.encode()).await;
                        continue;
                    }
                };

                crate::frame_handler::handle_frame(
                    frame,
                    &state,
                    client_id,
                    &write_tx,
                    &mut forwarding,
                )
                .await;
            }
            Message::Close(_) => break,
            _ => {} // Ignore text, ping, pong
        }
    }

    // Cleanup: abort all forwarding tasks on disconnect.
    forwarding.abort_all();
}

/// Start a test server on a random port and return the address.
#[cfg(test)]
async fn start_test_server(secret: &str) -> std::net::SocketAddr {
    use nomadflow_core::config::Settings;

    let mut settings = Settings::default();
    settings.auth.secret = secret.to_string();
    let state = Arc::new(crate::state::AppState::new(settings));
    let app = ws_router().with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use nomadflow_pty::protocol::{ControlMsg, WsFrame};
    use tokio_tungstenite::tungstenite;

    async fn ws_connect(
        addr: std::net::SocketAddr,
        token: Option<&str>,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tungstenite::Error,
    > {
        let url = match token {
            Some(t) => format!("ws://{addr}/ws/panes?token={t}"),
            None => format!("ws://{addr}/ws/panes"),
        };
        let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
        Ok(ws)
    }

    /// Helper: receive the next binary frame from WS, decode it.
    async fn recv_frame(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> WsFrame {
        loop {
            let msg = ws.next().await.unwrap().unwrap();
            if let tungstenite::Message::Binary(data) = msg {
                return WsFrame::decode(&data).unwrap();
            }
        }
    }

    /// Helper: send a frame.
    async fn send_frame(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        frame: &WsFrame,
    ) {
        ws.send(tungstenite::Message::Binary(frame.encode().into()))
            .await
            .unwrap();
    }

    // --- AC #4: Auth rejection tests ---

    #[tokio::test]
    async fn test_ws_auth_rejection_no_token() {
        let addr = start_test_server("my-secret").await;
        let result = ws_connect(addr, None).await;
        // Server returns 401 before upgrade → connection error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ws_auth_rejection_wrong_token() {
        let addr = start_test_server("my-secret").await;
        let result = ws_connect(addr, Some("wrong-token")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ws_auth_no_secret_allows_connection() {
        let addr = start_test_server("").await;
        let result = ws_connect(addr, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ws_auth_valid_token() {
        let addr = start_test_server("my-secret").await;
        let result = ws_connect(addr, Some("my-secret")).await;
        assert!(result.is_ok());
    }

    // --- AC #1, #2, #3: Control messages and PTY data ---

    #[tokio::test]
    async fn test_auto_pane_list_on_connect() {
        let addr = start_test_server("secret").await;
        let mut ws = ws_connect(addr, Some("secret")).await.unwrap();

        // Task 4.1: Server sends PaneList automatically on connect
        let response = recv_frame(&mut ws).await;
        match response {
            WsFrame::Control {
                payload: ControlMsg::PaneList { panes },
            } => {
                assert!(panes.is_empty(), "should have no panes initially");
            }
            other => panic!("expected auto PaneList on connect, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_control_list_panes_empty() {
        let addr = start_test_server("secret").await;
        let mut ws = ws_connect(addr, Some("secret")).await.unwrap();

        // Consume auto PaneList from connect (Task 4.1)
        let _ = recv_frame(&mut ws).await;

        // Send explicit List control message
        let list_frame = WsFrame::Control {
            payload: ControlMsg::List,
        };
        send_frame(&mut ws, &list_frame).await;

        let response = recv_frame(&mut ws).await;
        match response {
            WsFrame::Control {
                payload: ControlMsg::PaneList { panes },
            } => {
                assert!(panes.is_empty(), "should have no panes initially");
            }
            other => panic!("expected PaneList, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_ping_pong() {
        let addr = start_test_server("secret").await;
        let mut ws = ws_connect(addr, Some("secret")).await.unwrap();

        // Consume auto PaneList from connect (Task 4.1)
        let _ = recv_frame(&mut ws).await;

        send_frame(&mut ws, &WsFrame::Ping).await;
        let response = recv_frame(&mut ws).await;
        assert_eq!(response, WsFrame::Ping);
    }

    #[tokio::test]
    async fn test_client_disconnect_cleanup() {
        let addr = start_test_server("secret").await;

        // Connect and immediately disconnect
        {
            let ws = ws_connect(addr, Some("secret")).await.unwrap();
            drop(ws);
        }

        // Give server time to clean up
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // A new client should still be able to connect (server didn't crash)
        let result = ws_connect(addr, Some("secret")).await;
        assert!(result.is_ok());
    }
}
