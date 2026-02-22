use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::PtyError;
use crate::types::PaneId;

// Frame type constants.
pub const PTY_DATA: u8 = 0x01;
pub const RESIZE: u8 = 0x02;
pub const CONTROL: u8 = 0x03;
pub const BUFFER_SNAPSHOT: u8 = 0x04;
pub const PING: u8 = 0x05;

/// Minimum frame size: type (1B) + pane_id (2B).
const MIN_FRAME_SIZE: usize = 3;

/// Control channel pane ID (broadcast / system messages).
pub const CONTROL_PANE_ID: u16 = 0x0000;

/// A WebSocket frame in the multiplexed binary protocol.
///
/// Wire format: `[Type 1B][PaneID 2B big-endian][Payload variable]`
#[derive(Debug, Clone, PartialEq)]
pub enum WsFrame {
    /// PTY data (input from client, output from server).
    PtyData { pane_id: PaneId, data: Vec<u8> },
    /// Resize a pane's terminal.
    Resize {
        pane_id: PaneId,
        cols: u16,
        rows: u16,
    },
    /// JSON-encoded control message on the control channel.
    Control { payload: ControlMsg },
    /// Buffer snapshot (raw ANSI bytes for terminal.write()).
    BufferSnapshot { pane_id: PaneId, data: Vec<u8> },
    /// Keepalive ping/pong.
    Ping,
}

/// DTO for pane info sent over the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneInfoDto {
    pub id: u16,
    pub label: String,
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub agent_number: u16,
    pub cols: u16,
    pub rows: u16,
    pub cwd: String,
    pub agent_state: crate::types::AgentStateKind,
}

impl From<&crate::types::PaneInfo> for PaneInfoDto {
    fn from(info: &crate::types::PaneInfo) -> Self {
        Self {
            id: info.id.0,
            label: info.label.0.clone(),
            repo: info.repo.clone(),
            worktree: info.worktree.clone(),
            agent_type: info.agent_type.clone(),
            agent_number: info.agent_number,
            cols: info.cols,
            rows: info.rows,
            cwd: info.cwd.clone(),
            agent_state: info.agent_state,
        }
    }
}

/// Control messages sent on the control channel (pane_id = 0x0000).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ControlMsg {
    /// Client requests the list of panes.
    List,
    /// Client requests creation of a new pane.
    Create(CreatePaneWsRequest),
    /// Client requests destruction of a pane.
    Destroy {
        #[serde(rename = "paneId")]
        pane_id: u16,
    },
    /// Client subscribes to output from specific panes.
    Subscribe {
        #[serde(rename = "paneIds")]
        pane_ids: Vec<u16>,
    },
    /// Client unsubscribes from specific panes.
    Unsubscribe {
        #[serde(rename = "paneIds")]
        pane_ids: Vec<u16>,
    },
    /// Server responds with the list of panes.
    PaneList { panes: Vec<PaneInfoDto> },
    /// Server reports an error.
    Error { message: String },
    /// Server confirms a pane was created.
    PaneCreated(PaneInfoDto),
    /// Server confirms a pane was destroyed (or its shell process exited).
    PaneDestroyed {
        #[serde(rename = "paneId")]
        pane_id: u16,
        /// Exit code of the shell process, if the pane exited naturally.
        /// `None` when the pane was explicitly destroyed by a client.
        #[serde(default, rename = "exitCode", skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },
    /// Server reports that a pane's state (e.g. agent state) has changed.
    PaneStateUpdated {
        #[serde(rename = "paneId")]
        pane_id: u16,
        #[serde(rename = "agentState")]
        agent_state: crate::types::AgentStateKind,
    },
}

/// Wire request to create a pane (subset of CreatePaneRequest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePaneWsRequest {
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub cwd: String,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub shell: Option<String>,
}

impl From<CreatePaneWsRequest> for crate::types::CreatePaneRequest {
    fn from(ws_req: CreatePaneWsRequest) -> Self {
        let cwd = if ws_req.cwd.is_empty() {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        } else {
            PathBuf::from(ws_req.cwd)
        };
        Self {
            repo: ws_req.repo,
            worktree: ws_req.worktree,
            agent_type: ws_req.agent_type,
            cwd,
            cols: ws_req.cols,
            rows: ws_req.rows,
            shell: ws_req.shell,
        }
    }
}

impl WsFrame {
    /// Encode a frame into the binary wire format.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            WsFrame::PtyData { pane_id, data } => {
                let mut buf = Vec::with_capacity(3 + data.len());
                buf.push(PTY_DATA);
                buf.extend_from_slice(&pane_id.0.to_be_bytes());
                buf.extend_from_slice(data);
                buf
            }
            WsFrame::Resize {
                pane_id,
                cols,
                rows,
            } => {
                let mut buf = Vec::with_capacity(7);
                buf.push(RESIZE);
                buf.extend_from_slice(&pane_id.0.to_be_bytes());
                buf.extend_from_slice(&cols.to_be_bytes());
                buf.extend_from_slice(&rows.to_be_bytes());
                buf
            }
            WsFrame::Control { payload } => {
                let json = serde_json::to_vec(payload).expect("ControlMsg serialization");
                let mut buf = Vec::with_capacity(3 + json.len());
                buf.push(CONTROL);
                buf.extend_from_slice(&CONTROL_PANE_ID.to_be_bytes());
                buf.extend_from_slice(&json);
                buf
            }
            WsFrame::BufferSnapshot { pane_id, data } => {
                let mut buf = Vec::with_capacity(3 + data.len());
                buf.push(BUFFER_SNAPSHOT);
                buf.extend_from_slice(&pane_id.0.to_be_bytes());
                buf.extend_from_slice(data);
                buf
            }
            WsFrame::Ping => {
                let mut buf = Vec::with_capacity(3);
                buf.push(PING);
                buf.extend_from_slice(&CONTROL_PANE_ID.to_be_bytes());
                buf
            }
        }
    }

    /// Decode a frame from the binary wire format.
    pub fn decode(data: &[u8]) -> Result<WsFrame, PtyError> {
        if data.len() < MIN_FRAME_SIZE {
            return Err(PtyError::FrameTooShort);
        }

        let frame_type = data[0];
        let pane_id = u16::from_be_bytes([data[1], data[2]]);
        let payload = &data[3..];

        match frame_type {
            PTY_DATA => Ok(WsFrame::PtyData {
                pane_id: PaneId(pane_id),
                data: payload.to_vec(),
            }),
            RESIZE => {
                if payload.len() < 4 {
                    return Err(PtyError::FrameTooShort);
                }
                let cols = u16::from_be_bytes([payload[0], payload[1]]);
                let rows = u16::from_be_bytes([payload[2], payload[3]]);
                Ok(WsFrame::Resize {
                    pane_id: PaneId(pane_id),
                    cols,
                    rows,
                })
            }
            CONTROL => {
                let msg: ControlMsg = serde_json::from_slice(payload)
                    .map_err(|e| PtyError::InvalidControlMessage(e.to_string()))?;
                Ok(WsFrame::Control { payload: msg })
            }
            BUFFER_SNAPSHOT => Ok(WsFrame::BufferSnapshot {
                pane_id: PaneId(pane_id),
                data: payload.to_vec(),
            }),
            PING => Ok(WsFrame::Ping),
            other => Err(PtyError::InvalidFrameType(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_pty_data() {
        let frame = WsFrame::PtyData {
            pane_id: PaneId(42),
            data: b"hello".to_vec(),
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_pty_data_empty_payload() {
        let frame = WsFrame::PtyData {
            pane_id: PaneId(1),
            data: vec![],
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_pty_data_max_pane_id() {
        let frame = WsFrame::PtyData {
            pane_id: PaneId(u16::MAX),
            data: b"test".to_vec(),
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_resize() {
        let frame = WsFrame::Resize {
            pane_id: PaneId(7),
            cols: 120,
            rows: 40,
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_list() {
        let frame = WsFrame::Control {
            payload: ControlMsg::List,
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_create() {
        let frame = WsFrame::Control {
            payload: ControlMsg::Create(CreatePaneWsRequest {
                repo: "myrepo".into(),
                worktree: "main".into(),
                agent_type: "claude".into(),
                cwd: "/home/user".into(),
                cols: Some(80),
                rows: Some(24),
                shell: None,
            }),
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_destroy() {
        let frame = WsFrame::Control {
            payload: ControlMsg::Destroy { pane_id: 5 },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_subscribe() {
        let frame = WsFrame::Control {
            payload: ControlMsg::Subscribe {
                pane_ids: vec![1, 2, 3],
            },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_unsubscribe() {
        let frame = WsFrame::Control {
            payload: ControlMsg::Unsubscribe {
                pane_ids: vec![4, 5],
            },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_pane_list() {
        let frame = WsFrame::Control {
            payload: ControlMsg::PaneList {
                panes: vec![PaneInfoDto {
                    id: 1,
                    label: "myrepo:main:claude-1".into(),
                    repo: "myrepo".into(),
                    worktree: "main".into(),
                    agent_type: "claude".into(),
                    agent_number: 1,
                    cols: 80,
                    rows: 24,
                    cwd: "/tmp".into(),
                    agent_state: crate::types::AgentStateKind::Unknown,
                }],
            },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_error() {
        let frame = WsFrame::Control {
            payload: ControlMsg::Error {
                message: "pane not found".into(),
            },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_pane_created() {
        let frame = WsFrame::Control {
            payload: ControlMsg::PaneCreated(PaneInfoDto {
                id: 2,
                label: "test:feat:shell-1".into(),
                repo: "test".into(),
                worktree: "feat".into(),
                agent_type: "shell".into(),
                agent_number: 1,
                cols: 120,
                rows: 40,
                cwd: "/home".into(),
                agent_state: crate::types::AgentStateKind::Unknown,
            }),
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_pane_destroyed() {
        let frame = WsFrame::Control {
            payload: ControlMsg::PaneDestroyed {
                pane_id: 99,
                exit_code: None,
            },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_control_pane_destroyed_with_exit_code() {
        let frame = WsFrame::Control {
            payload: ControlMsg::PaneDestroyed {
                pane_id: 42,
                exit_code: Some(137),
            },
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_buffer_snapshot() {
        let frame = WsFrame::BufferSnapshot {
            pane_id: PaneId(3),
            data: b"\x1b[2J\x1b[Hhello world".to_vec(),
        };
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn round_trip_ping() {
        let frame = WsFrame::Ping;
        let encoded = frame.encode();
        let decoded = WsFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn decode_frame_too_short() {
        assert!(matches!(
            WsFrame::decode(&[0x01]),
            Err(PtyError::FrameTooShort)
        ));
        assert!(matches!(
            WsFrame::decode(&[0x01, 0x00]),
            Err(PtyError::FrameTooShort)
        ));
    }

    #[test]
    fn decode_invalid_frame_type() {
        let data = [0xFF, 0x00, 0x01];
        assert!(matches!(
            WsFrame::decode(&data),
            Err(PtyError::InvalidFrameType(0xFF))
        ));
    }

    #[test]
    fn decode_resize_payload_too_short() {
        // Type=RESIZE, PaneId=1, but only 2 bytes of payload (needs 4)
        let data = [RESIZE, 0x00, 0x01, 0x00, 0x50];
        assert!(matches!(
            WsFrame::decode(&data),
            Err(PtyError::FrameTooShort)
        ));
    }

    #[test]
    fn decode_invalid_control_json() {
        let mut data = vec![CONTROL, 0x00, 0x00];
        data.extend_from_slice(b"not valid json");
        assert!(matches!(
            WsFrame::decode(&data),
            Err(PtyError::InvalidControlMessage(_))
        ));
    }

    #[test]
    fn pane_info_dto_from_pane_info() {
        let info = crate::types::PaneInfo {
            id: PaneId(5),
            label: crate::types::PaneLabel("repo:wt:agent-1".into()),
            repo: "repo".into(),
            worktree: "wt".into(),
            agent_type: "agent".into(),
            agent_number: 1,
            cols: 80,
            rows: 24,
            cwd: "/tmp".into(),
            agent_state: crate::types::AgentStateKind::WaitingForInput,
        };
        let dto = PaneInfoDto::from(&info);
        assert_eq!(dto.id, 5);
        assert_eq!(dto.label, "repo:wt:agent-1");
    }

    #[test]
    fn create_pane_ws_request_to_create_pane_request() {
        let ws_req = CreatePaneWsRequest {
            repo: "repo".into(),
            worktree: "main".into(),
            agent_type: "claude".into(),
            cwd: "/home/user".into(),
            cols: Some(120),
            rows: None,
            shell: Some("/bin/zsh".into()),
        };
        let req: crate::types::CreatePaneRequest = ws_req.into();
        assert_eq!(req.repo, "repo");
        assert_eq!(req.cwd, PathBuf::from("/home/user"));
        assert_eq!(req.cols, Some(120));
        assert_eq!(req.rows, None);
        assert_eq!(req.shell, Some("/bin/zsh".into()));
    }

    /// Round-trip encode/decode of all WsFrame variants through LengthDelimitedCodec framing,
    /// simulating the Unix socket transport layer.
    #[tokio::test]
    async fn round_trip_all_variants_through_length_delimited_framing() {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::UnixListener;
        use tokio_util::codec::{Framed, LengthDelimitedCodec};

        let dir = std::env::temp_dir().join(format!("nomadflow-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");
        let _ = std::fs::remove_file(&sock_path);

        let listener = UnixListener::bind(&sock_path).unwrap();

        let frames = vec![
            WsFrame::PtyData {
                pane_id: PaneId(1),
                data: b"hello world".to_vec(),
            },
            WsFrame::Resize {
                pane_id: PaneId(2),
                cols: 120,
                rows: 40,
            },
            WsFrame::Control {
                payload: ControlMsg::List,
            },
            WsFrame::Control {
                payload: ControlMsg::Subscribe {
                    pane_ids: vec![1, 2],
                },
            },
            WsFrame::Control {
                payload: ControlMsg::PaneList {
                    panes: vec![PaneInfoDto {
                        id: 1,
                        label: "test".into(),
                        repo: "r".into(),
                        worktree: "w".into(),
                        agent_type: "shell".into(),
                        agent_number: 1,
                        cols: 80,
                        rows: 24,
                        cwd: "/tmp".into(),
                        agent_state: crate::types::AgentStateKind::Unknown,
                    }],
                },
            },
            WsFrame::BufferSnapshot {
                pane_id: PaneId(3),
                data: b"\x1b[2Jtest".to_vec(),
            },
            WsFrame::Ping,
        ];

        let frames_clone = frames.clone();
        let sock_path_clone = sock_path.clone();

        // Sender task
        let sender = tokio::spawn(async move {
            let stream = tokio::net::UnixStream::connect(&sock_path_clone)
                .await
                .unwrap();
            let mut framed = Framed::new(stream, LengthDelimitedCodec::new());
            for frame in &frames_clone {
                let encoded = frame.encode();
                framed
                    .send(tokio_util::bytes::Bytes::from(encoded))
                    .await
                    .unwrap();
            }
        });

        // Receiver task
        let (stream, _) = listener.accept().await.unwrap();
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        let mut received = Vec::new();
        while received.len() < frames.len() {
            let data = framed.next().await.unwrap().unwrap();
            let decoded = WsFrame::decode(&data).unwrap();
            received.push(decoded);
        }

        sender.await.unwrap();

        assert_eq!(frames.len(), received.len());
        for (expected, actual) in frames.iter().zip(received.iter()) {
            assert_eq!(expected, actual);
        }

        let _ = std::fs::remove_file(&sock_path);
        let _ = std::fs::remove_dir(&dir);
    }
}
