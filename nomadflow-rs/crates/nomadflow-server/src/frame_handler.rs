use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::warn;

use nomadflow_pty::protocol::{ControlMsg, PaneInfoDto, WsFrame};
use nomadflow_pty::{ClientId, PaneEvent, PaneId};

use crate::state::AppState;

/// Tracks active forwarding tasks per pane for a single client.
pub struct ForwardingTasks {
    tasks: HashMap<u16, tokio::task::JoinHandle<()>>,
}

impl ForwardingTasks {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn insert(&mut self, pane_id: u16, handle: tokio::task::JoinHandle<()>) {
        if let Some(old) = self.tasks.insert(pane_id, handle) {
            old.abort();
        }
    }

    pub fn remove(&mut self, pane_id: u16) {
        if let Some(handle) = self.tasks.remove(&pane_id) {
            handle.abort();
        }
    }

    pub fn cleanup_finished(&mut self) {
        self.tasks.retain(|_, handle| !handle.is_finished());
    }

    pub fn abort_all(&mut self) {
        for (_, handle) in self.tasks.drain() {
            handle.abort();
        }
    }
}

impl Drop for ForwardingTasks {
    fn drop(&mut self) {
        self.abort_all();
    }
}

/// Process a single decoded frame.
pub async fn handle_frame(
    frame: WsFrame,
    state: &Arc<AppState>,
    client_id: ClientId,
    write_tx: &mpsc::Sender<Vec<u8>>,
    forwarding: &mut ForwardingTasks,
) {
    match frame {
        WsFrame::PtyData { pane_id, data } => {
            if let Err(e) = state
                .pane_manager
                .lock()
                .await
                .route_input(pane_id, data)
                .await
            {
                warn!(%client_id, %pane_id, "route_input failed: {e}");
            }
        }

        WsFrame::Resize {
            pane_id,
            cols,
            rows,
        } => {
            if let Err(e) = state
                .pane_manager
                .lock()
                .await
                .resize_pane(pane_id, cols, rows)
                .await
            {
                warn!(%client_id, %pane_id, "resize failed: {e}");
            }
        }

        WsFrame::Control { payload } => {
            handle_control(payload, state, client_id, write_tx, forwarding).await;
        }

        WsFrame::BufferSnapshot { .. } => {
            // Server-to-client only, ignore from client.
        }

        WsFrame::Ping => {
            let _ = write_tx.send(WsFrame::Ping.encode()).await;
        }
    }
}

/// Handle a control message.
pub async fn handle_control(
    msg: ControlMsg,
    state: &Arc<AppState>,
    client_id: ClientId,
    write_tx: &mpsc::Sender<Vec<u8>>,
    forwarding: &mut ForwardingTasks,
) {
    match msg {
        ControlMsg::List => {
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

        ControlMsg::Create(req) => {
            let result = {
                let mut manager = state.pane_manager.lock().await;
                manager.create_pane(req.into())
            };
            match result {
                Ok(info) => {
                    let pane_id = info.id;
                    let dto = PaneInfoDto::from(&info);

                    let rx = {
                        let mut manager = state.pane_manager.lock().await;
                        manager.subscribe_client(client_id, pane_id)
                    };

                    if let Ok(rx) = rx {
                        send_buffer_snapshot(state, pane_id, write_tx).await;
                        let handle =
                            spawn_output_forwarder(pane_id, rx, write_tx.clone(), state.clone());
                        forwarding.insert(pane_id.0, handle);
                    }

                    let resp = WsFrame::Control {
                        payload: ControlMsg::PaneCreated(dto),
                    };
                    let _ = write_tx.send(resp.encode()).await;
                }
                Err(e) => {
                    let resp = WsFrame::Control {
                        payload: ControlMsg::Error {
                            message: e.to_string(),
                        },
                    };
                    let _ = write_tx.send(resp.encode()).await;
                }
            }
        }

        ControlMsg::Destroy { pane_id } => {
            let (result, cwd) = {
                let mut manager = state.pane_manager.lock().await;
                let cwd = manager
                    .get_pane_info(PaneId(pane_id))
                    .map(|info| info.cwd.clone());
                (manager.destroy_pane(PaneId(pane_id)), cwd)
            };
            match result {
                Ok(()) => {
                    if let Some(cwd) = cwd {
                        state.agent_state.delete_state_file(&cwd).await;
                    }
                    forwarding.remove(pane_id);
                    let resp = WsFrame::Control {
                        payload: ControlMsg::PaneDestroyed {
                            pane_id,
                            exit_code: None,
                        },
                    };
                    let _ = write_tx.send(resp.encode()).await;
                }
                Err(e) => {
                    let resp = WsFrame::Control {
                        payload: ControlMsg::Error {
                            message: e.to_string(),
                        },
                    };
                    let _ = write_tx.send(resp.encode()).await;
                }
            }
        }

        ControlMsg::Subscribe { pane_ids } => {
            for pid in pane_ids {
                let pane_id = PaneId(pid);
                let rx = {
                    let mut manager = state.pane_manager.lock().await;
                    manager.subscribe_client(client_id, pane_id)
                };
                match rx {
                    Ok(rx) => {
                        send_buffer_snapshot(state, pane_id, write_tx).await;
                        let handle =
                            spawn_output_forwarder(pane_id, rx, write_tx.clone(), state.clone());
                        forwarding.insert(pid, handle);
                    }
                    Err(e) => {
                        let resp = WsFrame::Control {
                            payload: ControlMsg::Error {
                                message: e.to_string(),
                            },
                        };
                        let _ = write_tx.send(resp.encode()).await;
                    }
                }
            }
        }

        ControlMsg::Unsubscribe { pane_ids } => {
            let mut manager = state.pane_manager.lock().await;
            for pid in pane_ids {
                manager.unsubscribe_client(client_id, PaneId(pid));
                forwarding.remove(pid);
            }
        }

        // Server-to-client only messages — ignore if sent by client.
        ControlMsg::PaneList { .. }
        | ControlMsg::Error { .. }
        | ControlMsg::PaneCreated(_)
        | ControlMsg::PaneDestroyed { .. }
        | ControlMsg::PaneStateUpdated { .. } => {}
    }
}

/// Send a buffer snapshot for a pane to the client.
pub async fn send_buffer_snapshot(
    state: &Arc<AppState>,
    pane_id: PaneId,
    write_tx: &mpsc::Sender<Vec<u8>>,
) {
    let snapshot = state
        .pane_manager
        .lock()
        .await
        .get_buffer_snapshot(pane_id)
        .await;

    match snapshot {
        Ok(data) => {
            let frame = WsFrame::BufferSnapshot { pane_id, data };
            let _ = write_tx.send(frame.encode()).await;
        }
        Err(e) => {
            warn!(%pane_id, "Failed to get buffer snapshot: {e}");
        }
    }
}

/// Spawn a task that forwards PaneEvents from a broadcast receiver to the writer.
pub fn spawn_output_forwarder(
    pane_id: PaneId,
    mut rx: tokio::sync::broadcast::Receiver<PaneEvent>,
    write_tx: mpsc::Sender<Vec<u8>>,
    state: Arc<AppState>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let frame = match event {
                        PaneEvent::Output {
                            pane_id: pid,
                            data,
                        } => WsFrame::PtyData {
                            pane_id: pid,
                            data,
                        },
                        PaneEvent::Exited {
                            pane_id: pid,
                            code,
                        } => {
                            // Auto-cleanup: remove dead pane from manager registry
                            {
                                let mut manager = state.pane_manager.lock().await;
                                let _ = manager.destroy_pane(pid);
                            }
                            let ctrl = WsFrame::Control {
                                payload: ControlMsg::PaneDestroyed {
                                    pane_id: pid.0,
                                    exit_code: Some(code),
                                },
                            };
                            let _ = write_tx.send(ctrl.encode()).await;
                            break;
                        }
                        PaneEvent::TitleChanged { .. } => continue,
                    };
                    if write_tx.send(frame.encode()).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(%pane_id, "Client lagged, missed {n} messages — sending buffer snapshot for recovery");
                    let snapshot = state
                        .pane_manager
                        .lock()
                        .await
                        .get_buffer_snapshot(pane_id)
                        .await;
                    match snapshot {
                        Ok(data) => {
                            let frame = WsFrame::BufferSnapshot { pane_id, data };
                            if write_tx.send(frame.encode()).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(%pane_id, "Failed to get recovery snapshot: {e}");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    let ctrl = WsFrame::Control {
                        payload: ControlMsg::PaneDestroyed {
                            pane_id: pane_id.0,
                            exit_code: None,
                        },
                    };
                    let _ = write_tx.send(ctrl.encode()).await;
                    break;
                }
            }
        }
    })
}
