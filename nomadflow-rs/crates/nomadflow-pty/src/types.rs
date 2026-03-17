use std::fmt;
use std::path::PathBuf;

use alacritty_terminal::event::{Event, EventListener};
use tokio::sync::{mpsc, oneshot};

/// Re-export AgentStateKind from nomadflow-core (single source of truth).
pub use nomadflow_core::models::AgentStateKind;

/// Unique identifier for a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub u16);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pane-{}", self.0)
    }
}

/// Human-readable label for a pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneLabel(pub String);

/// Metadata about a pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: PaneId,
    pub label: PaneLabel,
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub agent_number: u16,
    pub cols: u16,
    pub rows: u16,
    pub cwd: String,
    pub agent_state: AgentStateKind,
}

/// Messages sent TO a PaneActor.
pub enum PaneMsg {
    /// Write input bytes to the PTY.
    Input(Vec<u8>),
    /// Resize the terminal.
    Resize { cols: u16, rows: u16 },
    /// Request a snapshot of the current screen state.
    Snapshot(oneshot::Sender<Vec<u8>>),
    /// Shut down the actor cleanly.
    Shutdown,
}

/// Events emitted FROM a PaneActor.
#[derive(Debug, Clone)]
pub enum PaneEvent {
    /// Raw output bytes from the PTY.
    Output { pane_id: PaneId, data: Vec<u8> },
    /// The PTY process exited.
    Exited { pane_id: PaneId, code: i32 },
    /// The terminal title changed.
    TitleChanged { pane_id: PaneId, title: String },
}

/// Handle to communicate with a running PaneActor.
pub struct PaneHandle {
    pub tx: mpsc::Sender<PaneMsg>,
    pub info: PaneInfo,
}

/// Unique identifier for a connected client (e.g., WebSocket connection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "client-{}", self.0)
    }
}

/// Request to create a new pane.
pub struct CreatePaneRequest {
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub cwd: PathBuf,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub shell: Option<String>,
}

/// Proxy that forwards alacritty_terminal events to a channel.
#[derive(Clone)]
pub struct PaneEventProxy {
    pub tx: mpsc::UnboundedSender<Event>,
}

impl EventListener for PaneEventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}

/// Dimensions for headless Term.
pub struct TermSize {
    pub cols: usize,
    pub lines: usize,
}

impl alacritty_terminal::grid::Dimensions for TermSize {
    fn columns(&self) -> usize {
        self.cols
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn total_lines(&self) -> usize {
        self.lines
    }
}
