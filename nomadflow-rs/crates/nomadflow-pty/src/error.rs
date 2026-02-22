use crate::types::PaneId;

/// Errors from PTY operations.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("failed to spawn PTY process: {0}")]
    Spawn(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("failed to send message: {0}")]
    SendFailed(String),

    #[error("channel closed")]
    ChannelClosed,

    #[error("pane not found: {0}")]
    PaneNotFound(PaneId),

    #[error("pane ID space exhausted (max {})", u16::MAX)]
    PaneIdExhausted,

    #[error("invalid frame type: 0x{0:02x}")]
    InvalidFrameType(u8),

    #[error("frame too short")]
    FrameTooShort,

    #[error("invalid control message: {0}")]
    InvalidControlMessage(String),
}
