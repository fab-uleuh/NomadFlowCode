use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use nomadflow_pty::protocol::{ControlMsg, PaneInfoDto, WsFrame};
use nomadflow_pty::ClientId;

use crate::frame_handler::ForwardingTasks;
use crate::state::AppState;

static NEXT_SOCKET_CLIENT_ID: AtomicU64 = AtomicU64::new(100_000);

fn next_client_id() -> ClientId {
    ClientId(NEXT_SOCKET_CLIENT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Start the Unix domain socket listener alongside the TCP server.
/// Accepts connections and spawns a handler task for each.
/// Removes stale socket on startup and cleans up on shutdown.
pub async fn serve_unix_socket(
    socket_path: &Path,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) -> color_eyre::Result<()> {
    // Remove stale socket file on startup (subtask 1.3).
    // Try connecting first to detect if another server is already running.
    if socket_path.exists() {
        match tokio::net::UnixStream::connect(socket_path).await {
            Ok(_) => {
                return Err(color_eyre::eyre::eyre!(
                    "Another server is already running (socket at {} is active).\n\
                     Stop it first with `nomadflow stop`.",
                    socket_path.display()
                ));
            }
            Err(_) => {
                // Stale socket — previous server didn't clean up
                std::fs::remove_file(socket_path)?;
            }
        }
    }

    let listener = UnixListener::bind(socket_path)?;
    info!(path = %socket_path.display(), "Unix socket listener started");

    loop {
        tokio::select! {
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _addr)) => {
                        let state = state.clone();
                        tokio::spawn(handle_socket_connection(stream, state));
                    }
                    Err(e) => {
                        warn!("Unix socket accept error: {e}");
                    }
                }
            }
            _ = shutdown.cancelled() => {
                break;
            }
        }
    }

    // Clean up socket file on shutdown (subtask 1.3)
    let _ = std::fs::remove_file(socket_path);
    info!("Unix socket listener stopped");

    Ok(())
}

/// Handle a single Unix socket connection.
/// Mirrors the WebSocket handler in `routes/panes.rs` but operates on
/// length-prefixed frames over a raw `UnixStream`.
async fn handle_socket_connection(stream: UnixStream, state: Arc<AppState>) {
    let client_id = next_client_id();
    info!(%client_id, "Socket client connected");

    // Wrap the stream with LengthDelimitedCodec for message framing (subtask 1.5)
    let framed = Framed::new(stream, LengthDelimitedCodec::new());
    let (framed_writer, framed_reader) = framed.split();

    // Single writer channel: all output forwarding tasks send encoded frames here.
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(256);

    // Writer task: receives encoded frames and sends them length-prefixed.
    let writer = tokio::spawn(socket_writer(framed_writer, write_rx));

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

    // Send pane list automatically on connect (same as WS handler).
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

    // Reader loop: receives frames, routes to PaneManager (subtask 1.6).
    socket_reader(framed_reader, state.clone(), client_id, write_tx).await;

    // Client disconnected — cleanup.
    info!(%client_id, "Socket client disconnected, cleaning up");
    state_forwarder.abort();
    let mut manager = state.pane_manager.lock().await;
    manager.remove_client(client_id);
    drop(manager);

    let _ = writer.await;
}

/// Writer loop: serializes outgoing frames through one task.
async fn socket_writer(
    mut writer: futures_util::stream::SplitSink<
        Framed<UnixStream, LengthDelimitedCodec>,
        tokio_util::bytes::Bytes,
    >,
    mut rx: mpsc::Receiver<Vec<u8>>,
) {
    while let Some(frame_bytes) = rx.recv().await {
        if writer
            .send(tokio_util::bytes::Bytes::from(frame_bytes))
            .await
            .is_err()
        {
            break;
        }
    }
}

/// Main read loop: decodes length-prefixed frames and routes them.
async fn socket_reader(
    mut reader: futures_util::stream::SplitStream<Framed<UnixStream, LengthDelimitedCodec>>,
    state: Arc<AppState>,
    client_id: ClientId,
    write_tx: mpsc::Sender<Vec<u8>>,
) {
    let mut forwarding = ForwardingTasks::new();

    while let Some(msg_result) = reader.next().await {
        let data = match msg_result {
            Ok(d) => d,
            Err(e) => {
                warn!(%client_id, "Socket read error: {e}");
                break;
            }
        };

        forwarding.cleanup_finished();

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

        crate::frame_handler::handle_frame(frame, &state, client_id, &write_tx, &mut forwarding).await;
    }

    forwarding.abort_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use nomadflow_core::config::Settings;
    use tempfile::TempDir;
    use tokio_util::codec::{Framed, LengthDelimitedCodec};

    /// Helper: start a Unix socket server in a temp directory.
    /// Returns (socket_path, shutdown_token, _temp_dir).
    /// The `TempDir` must be kept alive for the duration of the test (RAII cleanup).
    async fn start_test_socket_server() -> (std::path::PathBuf, CancellationToken, TempDir) {
        let dir = TempDir::new().unwrap();
        let socket_path = dir.path().join("test.sock");

        let settings = Settings::default();
        let state = Arc::new(AppState::new(settings));
        let shutdown = CancellationToken::new();

        let path = socket_path.clone();
        let s = state.clone();
        let sd = shutdown.clone();
        tokio::spawn(async move {
            serve_unix_socket(&path, s, sd).await.unwrap();
        });

        // Wait for socket to be available
        for _ in 0..50 {
            if socket_path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        (socket_path, shutdown, dir)
    }

    /// Helper: connect to socket and return framed stream.
    async fn connect_socket(
        socket_path: &std::path::Path,
    ) -> Framed<tokio::net::UnixStream, LengthDelimitedCodec> {
        let stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .unwrap();
        Framed::new(stream, LengthDelimitedCodec::new())
    }

    /// Helper: receive next frame.
    async fn recv_frame(
        framed: &mut Framed<tokio::net::UnixStream, LengthDelimitedCodec>,
    ) -> WsFrame {
        let data = framed.next().await.unwrap().unwrap();
        WsFrame::decode(&data).unwrap()
    }

    /// Helper: send a frame.
    async fn send_frame(
        framed: &mut Framed<tokio::net::UnixStream, LengthDelimitedCodec>,
        frame: &WsFrame,
    ) {
        framed
            .send(tokio_util::bytes::Bytes::from(frame.encode()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_socket_auto_pane_list_on_connect() {
        let (socket_path, shutdown, _dir) = start_test_socket_server().await;
        let mut framed = connect_socket(&socket_path).await;

        // Server sends PaneList automatically on connect
        let response = recv_frame(&mut framed).await;
        match response {
            WsFrame::Control {
                payload: ControlMsg::PaneList { panes },
            } => {
                assert!(panes.is_empty(), "should have no panes initially");
            }
            other => panic!("expected auto PaneList, got: {other:?}"),
        }

        shutdown.cancel();
    }

    #[tokio::test]
    async fn test_socket_list_control_message() {
        let (socket_path, shutdown, _dir) = start_test_socket_server().await;
        let mut framed = connect_socket(&socket_path).await;

        // Consume auto PaneList
        let _ = recv_frame(&mut framed).await;

        // Send explicit List
        send_frame(
            &mut framed,
            &WsFrame::Control {
                payload: ControlMsg::List,
            },
        )
        .await;

        let response = recv_frame(&mut framed).await;
        match response {
            WsFrame::Control {
                payload: ControlMsg::PaneList { panes },
            } => {
                assert!(panes.is_empty());
            }
            other => panic!("expected PaneList, got: {other:?}"),
        }

        shutdown.cancel();
    }

    #[tokio::test]
    async fn test_socket_ping_pong() {
        let (socket_path, shutdown, _dir) = start_test_socket_server().await;
        let mut framed = connect_socket(&socket_path).await;

        // Consume auto PaneList
        let _ = recv_frame(&mut framed).await;

        send_frame(&mut framed, &WsFrame::Ping).await;
        let response = recv_frame(&mut framed).await;
        assert_eq!(response, WsFrame::Ping);

        shutdown.cancel();
    }

    #[tokio::test]
    async fn test_socket_client_disconnect_cleanup() {
        let (socket_path, shutdown, _dir) = start_test_socket_server().await;

        // Connect and disconnect
        {
            let framed = connect_socket(&socket_path).await;
            drop(framed);
        }

        // Give server time to clean up
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // New client should still work
        let mut framed = connect_socket(&socket_path).await;
        let response = recv_frame(&mut framed).await;
        match response {
            WsFrame::Control {
                payload: ControlMsg::PaneList { .. },
            } => {}
            other => panic!("expected PaneList, got: {other:?}"),
        }

        shutdown.cancel();
    }

    #[tokio::test]
    async fn test_socket_cleanup_on_shutdown() {
        let (socket_path, shutdown, _dir) = start_test_socket_server().await;
        assert!(socket_path.exists(), "socket file should exist");

        shutdown.cancel();
        // Give time for cleanup
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(!socket_path.exists(), "socket file should be removed on shutdown");
    }
}
