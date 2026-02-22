use std::io::Write;
use std::path::Path;

use color_eyre::{eyre::eyre, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use nomadflow_pty::protocol::{ControlMsg, PaneInfoDto, WsFrame};
use nomadflow_pty::PaneId;

/// RAII guard to restore terminal mode on drop.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

/// Connect to the server via Unix socket, list panes, select one, and bridge
/// the user's terminal stdin/stdout to the pane's PTY.
pub async fn run(socket_path: &Path, pane_arg: Option<u16>) -> Result<()> {
    if !socket_path.exists() {
        return Err(eyre!(
            "Server not running (socket not found at {}).\nStart with `nomadflow serve`.",
            socket_path.display()
        ));
    }

    // Connect to Unix socket (subtask 3.2)
    let stream = UnixStream::connect(socket_path).await.map_err(|e| {
        eyre!(
            "Cannot connect to server socket at {}: {e}\nIs the server running?",
            socket_path.display()
        )
    })?;

    let framed = Framed::new(stream, LengthDelimitedCodec::new());
    let (mut writer, mut reader) = framed.split();

    // Receive auto PaneList on connect (subtask 3.3)
    let panes = match recv_frame(&mut reader).await? {
        WsFrame::Control {
            payload: ControlMsg::PaneList { panes },
        } => panes,
        other => return Err(eyre!("Expected PaneList on connect, got: {other:?}")),
    };

    if panes.is_empty() {
        println!("No active panes. Create one first (e.g. via web dashboard or API).");
        return Ok(());
    }

    // Select pane (subtask 3.4)
    let selected_pane_id = match pane_arg {
        Some(id) => {
            if !panes.iter().any(|p| p.id == id) {
                return Err(eyre!(
                    "Pane {id} not found. Available: {}",
                    panes.iter().map(|p| p.id.to_string()).collect::<Vec<_>>().join(", ")
                ));
            }
            id
        }
        None => select_pane(&panes)?,
    };

    // Subscribe to the selected pane (subtask 3.5)
    let subscribe = WsFrame::Control {
        payload: ControlMsg::Subscribe {
            pane_ids: vec![selected_pane_id],
        },
    };
    send_frame(&mut writer, &subscribe).await?;

    // Receive buffer snapshot and write to stdout
    match recv_frame(&mut reader).await? {
        WsFrame::BufferSnapshot { data, .. } => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&data)?;
            stdout.flush()?;
        }
        other => {
            // Might be PtyData if no snapshot yet, write it
            if let WsFrame::PtyData { data, .. } = &other {
                let mut stdout = std::io::stdout().lock();
                stdout.write_all(data)?;
                stdout.flush()?;
            }
        }
    }

    // Enter raw mode (subtask 3.6)
    let _guard = RawModeGuard::enable()?;

    // Send initial terminal size
    let (cols, rows) = crossterm::terminal::size()?;
    let resize_frame = WsFrame::Resize {
        pane_id: PaneId(selected_pane_id),
        cols,
        rows,
    };
    send_frame(&mut writer, &resize_frame).await?;

    // Main loop (subtask 3.7)
    let mut stdin = tokio::io::stdin();
    let mut stdin_buf = [0u8; 4096];
    let mut ctrl_b_pressed = false;

    let mut sigwinch =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())?;

    loop {
        tokio::select! {
            // Read from stdin → send to pane (subtask 3.7)
            n = stdin.read(&mut stdin_buf) => {
                let n = n?;
                if n == 0 {
                    break; // EOF
                }

                // Check for Ctrl+B d detach sequence (subtask 3.8)
                // Ctrl+B = 0x02, 'd' = 0x64
                let mut i = 0;
                while i < n {
                    let byte = stdin_buf[i];
                    if ctrl_b_pressed {
                        ctrl_b_pressed = false;
                        if byte == b'd' {
                            // Detach!
                            drop(_guard);
                            println!("\r\nDetached from pane {selected_pane_id}.");
                            return Ok(());
                        }
                        // Not 'd' after Ctrl+B — send the buffered Ctrl+B + current byte
                        let data = vec![0x02, byte];
                        let frame = WsFrame::PtyData {
                            pane_id: PaneId(selected_pane_id),
                            data,
                        };
                        send_frame(&mut writer, &frame).await?;
                        i += 1;
                        continue;
                    }
                    if byte == 0x02 {
                        ctrl_b_pressed = true;
                        i += 1;
                        continue;
                    }
                    // Find the next Ctrl+B or end of buffer
                    let start = i;
                    i += 1;
                    while i < n && stdin_buf[i] != 0x02 {
                        i += 1;
                    }
                    let frame = WsFrame::PtyData {
                        pane_id: PaneId(selected_pane_id),
                        data: stdin_buf[start..i].to_vec(),
                    };
                    send_frame(&mut writer, &frame).await?;
                }
            }

            // Read from socket → write to stdout (subtask 3.7)
            msg = reader.next() => {
                let data = match msg {
                    Some(Ok(d)) => d,
                    Some(Err(e)) => {
                        drop(_guard);
                        return Err(eyre!("Socket read error: {e}"));
                    }
                    None => {
                        // Server disconnected
                        drop(_guard);
                        eprintln!("\r\nServer disconnected.");
                        return Ok(());
                    }
                };

                match WsFrame::decode(&data) {
                    Ok(WsFrame::PtyData { data, .. }) => {
                        let mut stdout = std::io::stdout().lock();
                        stdout.write_all(&data)?;
                        stdout.flush()?;
                    }
                    Ok(WsFrame::BufferSnapshot { data, .. }) => {
                        let mut stdout = std::io::stdout().lock();
                        stdout.write_all(&data)?;
                        stdout.flush()?;
                    }
                    Ok(WsFrame::Control { payload: ControlMsg::PaneDestroyed { pane_id, exit_code } })
                        if pane_id == selected_pane_id =>
                    {
                        // Pane exited (subtask 3.10)
                        drop(_guard);
                        match exit_code {
                            Some(code) => eprintln!("\r\nPane exited (code {code})."),
                            None => eprintln!("\r\nPane destroyed."),
                        }
                        return Ok(());
                    }
                    Ok(_) => {} // Ignore other frames
                    Err(e) => {
                        eprintln!("\r\nInvalid frame from server: {e}");
                    }
                }
            }

            // Handle SIGWINCH (subtask 3.9)
            _ = sigwinch.recv() => {
                let (cols, rows) = crossterm::terminal::size()?;
                let frame = WsFrame::Resize {
                    pane_id: PaneId(selected_pane_id),
                    cols,
                    rows,
                };
                send_frame(&mut writer, &frame).await?;
            }
        }
    }

    Ok(())
}

/// Display panes and prompt user to select one using an interactive TUI.
fn select_pane(panes: &[PaneInfoDto]) -> Result<u16> {
    if panes.is_empty() {
        return Err(eyre!("No active panes"));
    }

    if panes.len() == 1 {
        return Ok(panes[0].id);
    }

    let items: Vec<nomadflow_tui::PickItem> = panes
        .iter()
        .map(|p| nomadflow_tui::PickItem {
            label: p.label.clone(),
            detail: format!("id={}, {}x{}, cwd={}", p.id, p.cols, p.rows, p.cwd),
        })
        .collect();

    match nomadflow_tui::pick_from_list("Select a session to attach:", &items)? {
        Some(index) => Ok(panes[index].id),
        None => Err(eyre!("Selection cancelled")),
    }
}

/// Send a WsFrame over the length-delimited framed transport.
async fn send_frame(
    writer: &mut futures_util::stream::SplitSink<
        Framed<UnixStream, LengthDelimitedCodec>,
        tokio_util::bytes::Bytes,
    >,
    frame: &WsFrame,
) -> Result<()> {
    writer
        .send(tokio_util::bytes::Bytes::from(frame.encode()))
        .await
        .map_err(|e| eyre!("Failed to send frame: {e}"))
}

/// Receive and decode a WsFrame from the length-delimited framed transport.
async fn recv_frame(
    reader: &mut futures_util::stream::SplitStream<Framed<UnixStream, LengthDelimitedCodec>>,
) -> Result<WsFrame> {
    let data = reader
        .next()
        .await
        .ok_or_else(|| eyre!("Server closed connection"))?
        .map_err(|e| eyre!("Socket read error: {e}"))?;

    WsFrame::decode(&data).map_err(|e| eyre!("Invalid frame: {e}"))
}
