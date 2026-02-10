use nomadflow_core::config::Settings;
use nomadflow_core::services::git::GitService;
use nomadflow_core::services::tmux::TmuxService;

pub struct AppState {
    pub settings: Settings,
    pub git: GitService,
    pub tmux: TmuxService,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let git = GitService::new(&settings);
        let tmux = TmuxService::new(&settings.tmux.session);
        Self {
            settings,
            git,
            tmux,
            http_client: reqwest::Client::new(),
        }
    }
}
