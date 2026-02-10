use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};
use tracing::info;

/// Bridge bidirectional messages between an axum WebSocket and a tungstenite WebSocket.
pub async fn bridge(
    client: WebSocket,
    upstream: WebSocketStream<MaybeTlsStream<TcpStream>>,
) {
    let (mut client_tx, mut client_rx) = client.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let client_to_upstream = async {
        while let Some(msg) = client_rx.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if upstream_tx
                        .send(tungstenite::Message::Text(text.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    if upstream_tx
                        .send(tungstenite::Message::Binary(data.to_vec().into()))
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

    let upstream_to_client = async {
        while let Some(msg) = upstream_rx.next().await {
            match msg {
                Ok(tungstenite::Message::Text(text)) => {
                    if client_tx
                        .send(Message::Text(text.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(tungstenite::Message::Binary(data)) => {
                    if client_tx
                        .send(Message::Binary(data.to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(tungstenite::Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = client_to_upstream => {},
        _ = upstream_to_client => {},
    }

    info!("WebSocket bridge session ended");
}
