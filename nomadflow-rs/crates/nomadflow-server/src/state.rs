use std::sync::Arc;

use nomadflow_core::agent_state::AgentStateService;
use nomadflow_core::config::Settings;
use nomadflow_core::services::git::GitService;
use nomadflow_core::services::git_diff::GitDiffService;
use nomadflow_pty::PaneManager;

pub struct AppState {
    pub settings: Settings,
    pub git: GitService,
    pub http_client: reqwest::Client,
    pub agent_state: AgentStateService,
    pub git_diff: GitDiffService,
    pub pane_manager: Arc<tokio::sync::Mutex<PaneManager>>,
    pub agent_state_broadcast: tokio::sync::broadcast::Sender<(u16, nomadflow_pty::types::AgentStateKind)>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let git = GitService::new(&settings);
        let agent_state = AgentStateService::new(&settings);
        let (agent_state_broadcast, _) = tokio::sync::broadcast::channel(100);
        Self {
            settings,
            git,
            http_client: reqwest::Client::new(),
            agent_state,
            git_diff: GitDiffService::new(),
            pane_manager: Arc::new(tokio::sync::Mutex::new(PaneManager::new())),
            agent_state_broadcast,
        }
    }
}
