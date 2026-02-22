use std::collections::{HashMap, HashSet};

use tokio::sync::{broadcast, oneshot};

use crate::actor::{PaneActor, PaneSpawnConfig};
use crate::error::PtyError;
use crate::types::{
    ClientId, CreatePaneRequest, PaneEvent, PaneId, PaneInfo, PaneLabel, PaneMsg,
};

/// Internal entry storing a pane's handle, event sender, and actor task handle.
struct PaneEntry {
    handle: crate::types::PaneHandle,
    event_sender: broadcast::Sender<PaneEvent>,
    _join_handle: tokio::task::JoinHandle<()>,
}

/// Central registry that creates, destroys, and routes messages to PaneActors.
pub struct PaneManager {
    panes: HashMap<PaneId, PaneEntry>,
    clients: HashMap<ClientId, HashSet<PaneId>>,
    next_pane_id: u16,
    next_agent_numbers: HashMap<String, u16>,
}

impl Default for PaneManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PaneManager {
    /// Create a new PaneManager with empty state.
    pub fn new() -> Self {
        Self {
            panes: HashMap::new(),
            clients: HashMap::new(),
            next_pane_id: 1,
            next_agent_numbers: HashMap::new(),
        }
    }

    /// Create a new pane from the given request.
    ///
    /// Allocates a PaneId, computes an agent label, spawns a PaneActor,
    /// and registers it in the registry.
    pub fn create_pane(&mut self, req: CreatePaneRequest) -> Result<PaneInfo, PtyError> {
        let pane_id = PaneId(self.next_pane_id);
        self.next_pane_id = self
            .next_pane_id
            .checked_add(1)
            .ok_or(PtyError::PaneIdExhausted)?;

        let worktree_key = format!("{}:{}", req.repo, req.worktree);
        let agent_num = self.next_agent_numbers.entry(worktree_key).or_insert(0);
        *agent_num += 1;
        let label = format!(
            "{}:{}:{}-{}",
            req.repo, req.worktree, req.agent_type, agent_num
        );

        let config = PaneSpawnConfig {
            id: pane_id,
            label: PaneLabel(label),
            repo: req.repo,
            worktree: req.worktree,
            agent_type: req.agent_type,
            agent_number: *agent_num,
            cols: req.cols.unwrap_or(80),
            rows: req.rows.unwrap_or(24),
            cwd: req.cwd,
            shell: req.shell,
        };

        let (handle, event_sender, join_handle) = PaneActor::spawn(config)?;
        let info = handle.info.clone();

        self.panes.insert(
            pane_id,
            PaneEntry {
                handle,
                event_sender,
                _join_handle: join_handle,
            },
        );
        Ok(info)
    }

    /// Update the agent state of a pane.
    pub fn update_pane_state(
        &mut self,
        pane_id: PaneId,
        state: crate::types::AgentStateKind,
    ) -> Result<(), PtyError> {
        let entry = self
            .panes
            .get_mut(&pane_id)
            .ok_or(PtyError::PaneNotFound(pane_id))?;
        entry.handle.info.agent_state = state;
        Ok(())
    }

    /// Destroy a pane by sending Shutdown and removing it from the registry.
    pub fn destroy_pane(&mut self, pane_id: PaneId) -> Result<(), PtyError> {
        let entry = self
            .panes
            .remove(&pane_id)
            .ok_or(PtyError::PaneNotFound(pane_id))?;

        let _ = entry.handle.tx.try_send(PaneMsg::Shutdown);

        // Remove from all client subscriptions and clean up empty entries
        self.clients.retain(|_, subs| {
            subs.remove(&pane_id);
            !subs.is_empty()
        });

        Ok(())
    }

    /// Get info for a specific pane by ID.
    pub fn get_pane_info(&self, pane_id: PaneId) -> Option<PaneInfo> {
        self.panes.get(&pane_id).map(|e| e.handle.info.clone())
    }

    /// List all active panes (excludes dead panes whose actor has stopped).
    pub fn list_panes(&self) -> Vec<PaneInfo> {
        self.panes
            .values()
            .filter(|entry| !entry.handle.tx.is_closed())
            .map(|entry| entry.handle.info.clone())
            .collect()
    }

    /// Remove panes whose actor has stopped (channel closed).
    pub fn cleanup_dead_panes(&mut self) -> Vec<PaneId> {
        let dead: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|(_, entry)| entry.handle.tx.is_closed())
            .map(|(id, _)| *id)
            .collect();
        for id in &dead {
            self.panes.remove(id);
            self.clients.retain(|_, subs| {
                subs.remove(id);
                !subs.is_empty()
            });
        }
        dead
    }

    /// Resize a specific pane's terminal.
    pub async fn resize_pane(
        &self,
        pane_id: PaneId,
        cols: u16,
        rows: u16,
    ) -> Result<(), PtyError> {
        let entry = self
            .panes
            .get(&pane_id)
            .ok_or(PtyError::PaneNotFound(pane_id))?;

        entry
            .handle
            .tx
            .send(PaneMsg::Resize { cols, rows })
            .await
            .map_err(|e| PtyError::SendFailed(format!("pane {}: {}", pane_id, e)))
    }

    /// Route input bytes to a specific pane's actor.
    pub async fn route_input(&self, pane_id: PaneId, data: Vec<u8>) -> Result<(), PtyError> {
        let entry = self
            .panes
            .get(&pane_id)
            .ok_or(PtyError::PaneNotFound(pane_id))?;

        entry
            .handle
            .tx
            .send(PaneMsg::Input(data))
            .await
            .map_err(|e| PtyError::SendFailed(format!("pane {}: {}", pane_id, e)))
    }

    /// Request a buffer snapshot from a pane's actor.
    pub async fn get_buffer_snapshot(&self, pane_id: PaneId) -> Result<Vec<u8>, PtyError> {
        let entry = self
            .panes
            .get(&pane_id)
            .ok_or(PtyError::PaneNotFound(pane_id))?;

        let (tx, rx) = oneshot::channel();
        entry
            .handle
            .tx
            .send(PaneMsg::Snapshot(tx))
            .await
            .map_err(|e| PtyError::SendFailed(format!("pane {}: {}", pane_id, e)))?;

        rx.await.map_err(|_| PtyError::ChannelClosed)
    }

    /// Subscribe a client to a pane's event stream.
    ///
    /// Returns a broadcast receiver for the pane's events.
    pub fn subscribe_client(
        &mut self,
        client_id: ClientId,
        pane_id: PaneId,
    ) -> Result<broadcast::Receiver<PaneEvent>, PtyError> {
        let entry = self
            .panes
            .get(&pane_id)
            .ok_or(PtyError::PaneNotFound(pane_id))?;

        self.clients
            .entry(client_id)
            .or_default()
            .insert(pane_id);

        Ok(entry.event_sender.subscribe())
    }

    /// Unsubscribe a client from a specific pane.
    pub fn unsubscribe_client(&mut self, client_id: ClientId, pane_id: PaneId) {
        if let Some(subs) = self.clients.get_mut(&client_id) {
            subs.remove(&pane_id);
        }
    }

    /// Remove a client from all subscriptions (for disconnect cleanup).
    pub fn remove_client(&mut self, client_id: ClientId) {
        self.clients.remove(&client_id);
    }

    /// Get the set of panes a client is subscribed to.
    pub fn get_client_subscriptions(&self, client_id: ClientId) -> HashSet<PaneId> {
        self.clients
            .get(&client_id)
            .cloned()
            .unwrap_or_default()
    }
}
