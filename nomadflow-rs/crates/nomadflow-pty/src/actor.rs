use std::path::PathBuf;
use alacritty_terminal::event::Event;
use alacritty_terminal::term::Config;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::Processor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

use crate::error::PtyError;
use crate::snapshot;
use crate::types::{
    PaneEvent, PaneEventProxy, PaneHandle, PaneId, PaneInfo, PaneLabel, PaneMsg, TermSize,
};

/// Channel capacity for the PaneActor mailbox.
const MAILBOX_CAPACITY: usize = 50;

/// Channel capacity for the output broadcast.
const OUTPUT_BROADCAST_CAPACITY: usize = 64;

/// Read buffer size for PTY output.
const PTY_READ_BUF_SIZE: usize = 4096;

/// EIO errno value (PTY read returns this when the child process exits).
const EIO_ERRNO: i32 = 5;

/// Configuration for spawning a PaneActor.
pub struct PaneSpawnConfig {
    pub id: PaneId,
    pub label: PaneLabel,
    pub repo: String,
    pub worktree: String,
    pub agent_type: String,
    pub agent_number: u16,
    pub cols: u16,
    pub rows: u16,
    pub cwd: PathBuf,
    pub shell: Option<String>,
}

/// A PaneActor owns a PTY process and a headless terminal emulator.
/// It processes terminal I/O in isolation, communicating through typed channels.
pub struct PaneActor {
    id: PaneId,
    term: Term<PaneEventProxy>,
    processor: Processor,
    write_half: pty_process::OwnedWritePty,
    event_tx: broadcast::Sender<PaneEvent>,
    _child: tokio::process::Child,
}

impl PaneActor {
    /// Spawn a new PaneActor with a PTY process and headless terminal.
    ///
    /// Returns a `PaneHandle` for communication, a broadcast sender for events,
    /// and a `JoinHandle` to await actor completion.
    /// Callers can create receivers via `sender.subscribe()`.
    pub fn spawn(
        config: PaneSpawnConfig,
    ) -> Result<(PaneHandle, broadcast::Sender<PaneEvent>, tokio::task::JoinHandle<()>), PtyError> {
        let (pty, pts) = pty_process::open().map_err(|e| PtyError::Spawn(e.to_string()))?;

        // Resize PTY before spawning
        pty.resize(pty_process::Size::new(config.rows, config.cols))
            .map_err(|e| PtyError::Spawn(format!("resize: {e}")))?;

        // Determine shell
        let shell = config
            .shell
            .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()));

        // Spawn child process
        let child = pty_process::Command::new(&shell)
            .env("TERM", "xterm-256color")
            .current_dir(&config.cwd)
            .kill_on_drop(true)
            .spawn(pts)
            .map_err(|e| PtyError::Spawn(format!("spawn {shell}: {e}")))?;

        // Split PTY for concurrent I/O
        let (read_half, write_half) = pty.into_split();

        // Create alacritty_terminal event proxy
        let (proxy_tx, proxy_rx) = mpsc::unbounded_channel::<Event>();
        let event_proxy = PaneEventProxy { tx: proxy_tx };

        // Create headless terminal
        let term_size = TermSize {
            cols: config.cols as usize,
            lines: config.rows as usize,
        };
        let term_config = Config::default();
        let term = Term::new(term_config, &term_size, event_proxy);

        // Create channels
        let (msg_tx, msg_rx) = mpsc::channel::<PaneMsg>(MAILBOX_CAPACITY);
        let (event_tx, _) = broadcast::channel::<PaneEvent>(OUTPUT_BROADCAST_CAPACITY);
        let event_tx_clone = event_tx.clone();

        let pane_info = PaneInfo {
            id: config.id,
            label: PaneLabel(config.label.0.clone()),
            repo: config.repo.clone(),
            worktree: config.worktree.clone(),
            agent_type: config.agent_type.clone(),
            agent_number: config.agent_number,
            cols: config.cols,
            rows: config.rows,
            cwd: config.cwd.to_string_lossy().into_owned(),
            agent_state: crate::types::AgentStateKind::Unknown,
        };

        let handle = PaneHandle {
            tx: msg_tx,
            info: pane_info,
        };

        let processor = Processor::new();

        let actor = PaneActor {
            id: config.id,
            term,
            processor,
            write_half,
            event_tx,
            _child: child,
        };

        // Spawn the actor loop as a Tokio task
        let join_handle = tokio::spawn(actor.run(read_half, msg_rx, proxy_rx));

        Ok((handle, event_tx_clone, join_handle))
    }

    /// Main actor loop: select! on PTY read, mailbox recv, event proxy recv.
    async fn run(
        mut self,
        mut read_half: pty_process::OwnedReadPty,
        mut msg_rx: mpsc::Receiver<PaneMsg>,
        mut proxy_rx: mpsc::UnboundedReceiver<Event>,
    ) {
        let mut read_buf = vec![0u8; PTY_READ_BUF_SIZE];

        info!("PaneActor {} started", self.id);

        loop {
            tokio::select! {
                // PTY output
                result = read_half.read(&mut read_buf) => {
                    match result {
                        Ok(0) => {
                            info!("PaneActor {}: PTY EOF", self.id);
                            let _ = self.event_tx.send(PaneEvent::Exited {
                                pane_id: self.id,
                                code: 0,
                            });
                            break;
                        }
                        Ok(n) => {
                            let data = read_buf[..n].to_vec();

                            // Feed bytes through VTE processor into Term
                            self.processor.advance(&mut self.term, &data);

                            // Broadcast raw output
                            let _ = self.event_tx.send(PaneEvent::Output {
                                pane_id: self.id,
                                data,
                            });
                        }
                        Err(e) => {
                            // EIO is expected when the child exits
                            if e.raw_os_error() == Some(EIO_ERRNO) {
                                debug!("PaneActor {}: PTY read EIO (child exited)", self.id);
                            } else {
                                error!("PaneActor {}: PTY read error: {}", self.id, e);
                            }
                            let _ = self.event_tx.send(PaneEvent::Exited {
                                pane_id: self.id,
                                code: -1,
                            });
                            break;
                        }
                    }
                }

                // Mailbox messages
                msg = msg_rx.recv() => {
                    match msg {
                        Some(PaneMsg::Input(data)) => {
                            if let Err(e) = self.write_half.write_all(&data).await {
                                warn!("PaneActor {}: write error: {}", self.id, e);
                            }
                        }
                        Some(PaneMsg::Resize { cols, rows }) => {
                            debug!("PaneActor {}: resize to {}x{}", self.id, cols, rows);
                            if let Err(e) = self.write_half.resize(
                                pty_process::Size::new(rows, cols),
                            ) {
                                warn!("PaneActor {}: resize error: {}", self.id, e);
                            }
                            let new_size = TermSize {
                                cols: cols as usize,
                                lines: rows as usize,
                            };
                            self.term.resize(new_size);
                        }
                        Some(PaneMsg::Snapshot(reply_tx)) => {
                            let data = snapshot::snapshot(&self.term);
                            let _ = reply_tx.send(data);
                        }
                        Some(PaneMsg::Shutdown) => {
                            info!("PaneActor {}: shutdown requested", self.id);
                            break;
                        }
                        None => {
                            info!("PaneActor {}: mailbox closed", self.id);
                            break;
                        }
                    }
                }

                // alacritty_terminal events
                event = proxy_rx.recv() => {
                    match event {
                        Some(Event::Title(title)) => {
                            let _ = self.event_tx.send(PaneEvent::TitleChanged {
                                pane_id: self.id,
                                title,
                            });
                        }
                        Some(Event::ChildExit(code)) => {
                            info!("PaneActor {}: child exit code {}", self.id, code);
                            let _ = self.event_tx.send(PaneEvent::Exited {
                                pane_id: self.id,
                                code,
                            });
                            break;
                        }
                        Some(_) => {
                            // Ignore Wakeup, ClipboardStore, MouseCursorDirty, etc.
                        }
                        None => {
                            debug!("PaneActor {}: event proxy channel closed", self.id);
                        }
                    }
                }
            }
        }

        info!("PaneActor {} stopped", self.id);
    }
}
