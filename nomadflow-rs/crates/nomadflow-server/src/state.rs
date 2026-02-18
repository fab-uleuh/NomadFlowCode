use std::sync::Arc;

use nomadflow_core::agent_state::AgentStateService;
use nomadflow_core::config::Settings;
use nomadflow_core::services::git::GitService;
use nomadflow_core::services::git_diff::GitDiffService;
use nomadflow_core::services::tmux::TmuxService;

pub struct AppState {
    pub settings: Settings,
    pub git: GitService,
    pub tmux: TmuxService,
    pub http_client: reqwest::Client,
    pub agent_state: AgentStateService,
    pub git_diff: GitDiffService,
    /// Serializes linked session discovery to prevent race conditions
    /// when multiple WS connections arrive simultaneously.
    pub session_discovery_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let git = GitService::new(&settings);
        let tmux = TmuxService::new(&settings.tmux.session);
        let agent_state = AgentStateService::new(&settings);
        Self {
            settings,
            git,
            tmux,
            http_client: reqwest::Client::new(),
            agent_state,
            git_diff: GitDiffService::new(),
            session_discovery_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}
