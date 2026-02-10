use std::io;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::prelude::*;

use nomadflow_core::config::Settings;
use nomadflow_core::models::{Feature, Repository};

use crate::api_client;
use crate::event::{poll_event, AppEvent};
use crate::screens;
use crate::state::{self, CliState, ServerConfig};
use crate::tmux_local;
use crate::widgets;

/// What the TUI should do when it exits.
pub enum AppResult {
    Attach(String), // tmux session name
    Quit,
}

/// Wizard screens.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Resume,
    ServerPicker,
    ServerAdd,
    RepoPicker,
    FeaturePicker,
    FeatureCreate,
    Attaching,
}

/// Main application state.
pub struct App {
    pub settings: Settings,
    pub screen: Screen,
    pub servers: Vec<ServerConfig>,
    pub cli_state: CliState,

    // Selection state
    pub server: Option<ServerConfig>,
    pub repos: Vec<Repository>,
    pub features: Vec<CliFeature>,
    pub repo: Option<Repository>,
    pub feature: Option<Feature>,

    // Server health
    pub health_map: std::collections::HashMap<String, bool>,
    pub health_checking: bool,

    // UI state
    pub selected_index: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub input_text: String,
    pub input_cursor: usize,
    pub confirm_step: bool,

    // Server add state
    pub server_add_step: u8,
    pub server_add_name: String,
    pub server_add_url: String,

    // Result
    pub should_quit: bool,
    pub attach_session: Option<String>,
}

/// Feature enriched with local tmux pane command.
#[derive(Debug, Clone)]
pub struct CliFeature {
    pub feature: Feature,
    pub pane_command: Option<String>,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        let servers = state::load_servers(&settings);
        let cli_state = state::load_state(&settings);

        let has_last = cli_state.last_server.is_some()
            && cli_state.last_repo.is_some()
            && cli_state.last_feature.is_some();

        let initial_screen = if has_last {
            Screen::Resume
        } else {
            Screen::ServerPicker
        };

        // Auto-select server if only one
        let server = if servers.len() == 1 {
            Some(servers[0].clone())
        } else {
            None
        };

        Self {
            settings,
            screen: initial_screen,
            servers,
            cli_state,
            server,
            repos: Vec::new(),
            features: Vec::new(),
            repo: None,
            feature: None,
            health_map: std::collections::HashMap::new(),
            health_checking: false,
            selected_index: 0,
            loading: false,
            error: None,
            input_text: String::new(),
            input_cursor: 0,
            confirm_step: false,
            server_add_step: 0,
            server_add_name: String::new(),
            server_add_url: String::new(),
            should_quit: false,
            attach_session: None,
        }
    }

    /// Main event loop.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<AppResult> {
        // Async event channel for API results
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();

        // Auto-skip to repo picker if single server
        if self.screen == Screen::ServerPicker && self.servers.len() == 1 {
            self.server = Some(self.servers[0].clone());
            self.screen = Screen::RepoPicker;
            self.loading = true;
            self.trigger_load_repos(tx.clone());
        }

        loop {
            // Draw
            terminal.draw(|f| self.draw(f))?;

            // Check for async events
            while let Ok(event) = rx.try_recv() {
                self.handle_async_event(event, tx.clone());
            }

            // Poll terminal events (50ms tick)
            if let Some(event) = poll_event(Duration::from_millis(50)) {
                match event {
                    AppEvent::Key(key) => {
                        self.handle_key(key.code, key.modifiers, tx.clone());
                    }
                    _ => {}
                }
            }

            if self.should_quit {
                return Ok(match self.attach_session.take() {
                    Some(session) => AppResult::Attach(session),
                    None => AppResult::Quit,
                });
            }
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(1), // Breadcrumb
                Constraint::Min(1),   // Content
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Header
        widgets::header::render(frame, chunks[0]);

        // Breadcrumb
        widgets::breadcrumb::render(
            frame,
            chunks[1],
            self.server.as_ref().map(|s| s.name.as_str()),
            self.repo.as_ref().map(|r| r.name.as_str()),
            self.feature.as_ref().map(|f| f.name.as_str()),
        );

        // Content
        match self.screen {
            Screen::Resume => screens::resume::render(frame, chunks[2], self),
            Screen::ServerPicker => screens::server_picker::render(frame, chunks[2], self),
            Screen::ServerAdd => screens::server_add::render(frame, chunks[2], self),
            Screen::RepoPicker => screens::repo_picker::render(frame, chunks[2], self),
            Screen::FeaturePicker => screens::feature_picker::render(frame, chunks[2], self),
            Screen::FeatureCreate => screens::feature_create::render(frame, chunks[2], self),
            Screen::Attaching => screens::attaching::render(frame, chunks[2], self),
        }

        // Footer
        let footer_text = if self.screen == Screen::Attaching {
            ""
        } else {
            "Escape: back  q: quit"
        };
        let footer = ratatui::widgets::Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(footer, chunks[3]);
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        // Global keys
        if code == KeyCode::Char('q')
            && self.screen != Screen::FeatureCreate
            && self.screen != Screen::ServerAdd
        {
            self.should_quit = true;
            return;
        }
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        if code == KeyCode::Esc {
            self.go_back();
            return;
        }

        // Screen-specific keys
        match self.screen {
            Screen::Resume => self.handle_resume_key(code, tx),
            Screen::ServerPicker => self.handle_server_picker_key(code, tx),
            Screen::ServerAdd => self.handle_server_add_key(code),
            Screen::RepoPicker => self.handle_repo_picker_key(code, tx),
            Screen::FeaturePicker => self.handle_feature_picker_key(code, tx),
            Screen::FeatureCreate => self.handle_feature_create_key(code, tx),
            Screen::Attaching => {} // No keys during attaching
        }
    }

    fn go_back(&mut self) {
        self.error = None;
        match self.screen {
            Screen::RepoPicker => {
                self.screen = Screen::ServerPicker;
                self.server = None;
                self.repos.clear();
                self.selected_index = 0;
            }
            Screen::FeaturePicker => {
                self.screen = Screen::RepoPicker;
                self.repo = None;
                self.features.clear();
                self.selected_index = 0;
            }
            Screen::ServerAdd => {
                self.screen = Screen::ServerPicker;
                self.input_text.clear();
                self.input_cursor = 0;
                self.server_add_step = 0;
                self.server_add_name.clear();
                self.server_add_url.clear();
                self.selected_index = 0;
            }
            Screen::FeatureCreate => {
                self.screen = Screen::FeaturePicker;
                self.input_text.clear();
                self.input_cursor = 0;
                self.confirm_step = false;
                self.selected_index = 0;
            }
            Screen::Resume => {
                self.screen = Screen::ServerPicker;
                self.selected_index = 0;
            }
            _ => {}
        }
    }

    fn handle_resume_key(
        &mut self,
        code: KeyCode,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < 1 {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter => {
                if self.selected_index == 0 {
                    // Resume
                    self.do_resume(tx);
                } else {
                    // Skip to server picker
                    self.screen = Screen::ServerPicker;
                    self.selected_index = 0;
                }
            }
            _ => {}
        }
    }

    fn handle_server_picker_key(
        &mut self,
        code: KeyCode,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        // Servers + 1 for "Add server" option
        let count = self.servers.len() + 1;
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index + 1 < count {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter => {
                if self.selected_index == self.servers.len() {
                    // "+ Add server"
                    self.screen = Screen::ServerAdd;
                    self.selected_index = 0;
                    self.input_text.clear();
                    self.input_cursor = 0;
                    self.server_add_step = 0;
                    self.server_add_name.clear();
                    self.server_add_url.clear();
                } else if self.selected_index < self.servers.len() {
                    self.server = Some(self.servers[self.selected_index].clone());
                    self.screen = Screen::RepoPicker;
                    self.selected_index = 0;
                    self.loading = true;
                    self.trigger_load_repos(tx);
                }
            }
            _ => {}
        }
    }

    fn handle_server_add_key(&mut self, code: KeyCode) {
        if self.server_add_step == 3 {
            // Confirmation step: y/n
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let token = self.input_text.trim().to_string();
                    let new_server = ServerConfig {
                        id: format!(
                            "{}-{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis(),
                            std::process::id()
                        ),
                        name: self.server_add_name.clone(),
                        api_url: Some(self.server_add_url.clone()),
                        auth_token: if token.is_empty() { None } else { Some(token) },
                    };
                    self.servers.push(new_server);
                    state::save_servers(&self.settings, &self.servers);

                    // Reset and go back to picker
                    self.screen = Screen::ServerPicker;
                    self.input_text.clear();
                    self.input_cursor = 0;
                    self.server_add_step = 0;
                    self.server_add_name.clear();
                    self.server_add_url.clear();
                    self.selected_index = 0;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.server_add_step = 0;
                    self.input_text.clear();
                    self.input_cursor = 0;
                    self.server_add_name.clear();
                    self.server_add_url.clear();
                }
                _ => {}
            }
            return;
        }

        // Text input for steps 0-2
        match code {
            KeyCode::Char(c) => {
                self.input_text.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_text.remove(self.input_cursor);
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input_text.len() {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Enter => {
                let trimmed = self.input_text.trim().to_string();
                match self.server_add_step {
                    0 => {
                        if trimmed.is_empty() {
                            return;
                        }
                        self.server_add_name = trimmed;
                        self.input_text.clear();
                        self.input_cursor = 0;
                        self.server_add_step = 1;
                    }
                    1 => {
                        if trimmed.is_empty() {
                            return;
                        }
                        self.server_add_url = trimmed;
                        self.input_text.clear();
                        self.input_cursor = 0;
                        self.server_add_step = 2;
                    }
                    2 => {
                        // Token is optional, so empty is fine; move to confirm
                        self.server_add_step = 3;
                    }
                    _ => {}
                }
            }
            KeyCode::Esc => {
                self.go_back();
            }
            _ => {}
        }
    }

    fn handle_repo_picker_key(
        &mut self,
        code: KeyCode,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        let count = self.repos.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index + 1 < count {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter => {
                if self.selected_index < count {
                    self.repo = Some(self.repos[self.selected_index].clone());
                    self.screen = Screen::FeaturePicker;
                    self.selected_index = 0;
                    self.loading = true;
                    self.trigger_load_features(tx);
                }
            }
            _ => {}
        }
    }

    fn handle_feature_picker_key(
        &mut self,
        code: KeyCode,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        // Features + 1 for "Create" option
        let count = self.features.len() + 1;
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index + 1 < count {
                    self.selected_index += 1;
                }
            }
            KeyCode::Enter => {
                if self.selected_index == self.features.len() {
                    // Create
                    self.screen = Screen::FeatureCreate;
                    self.selected_index = 0;
                    self.input_text.clear();
                    self.input_cursor = 0;
                    self.confirm_step = false;
                } else if self.selected_index < self.features.len() {
                    let f = &self.features[self.selected_index];
                    self.feature = Some(f.feature.clone());
                    self.do_attach(tx);
                }
            }
            _ => {}
        }
    }

    fn handle_feature_create_key(
        &mut self,
        code: KeyCode,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        if self.confirm_step {
            // y/n confirmation
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.do_create_feature(tx);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_step = false;
                }
                _ => {}
            }
            return;
        }

        // Text input mode
        match code {
            KeyCode::Char(c) => {
                self.input_text.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_text.remove(self.input_cursor);
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input_text.len() {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Enter => {
                let trimmed = self.input_text.trim().to_string();
                if !trimmed.is_empty() {
                    self.input_text = trimmed;
                    self.confirm_step = true;
                }
            }
            KeyCode::Esc => {
                self.go_back();
            }
            _ => {}
        }
    }

    fn handle_async_event(
        &mut self,
        event: AppEvent,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match event {
            AppEvent::ReposLoaded(Ok(repos)) => {
                self.repos = repos;
                self.loading = false;
                self.error = None;
            }
            AppEvent::ReposLoaded(Err(e)) => {
                self.loading = false;
                self.error = Some(e);
            }
            AppEvent::FeaturesLoaded(Ok(features)) => {
                let session = &self.settings.tmux.session;
                self.features = features
                    .into_iter()
                    .map(|f| {
                        let repo_name = self.repo.as_ref().map(|r| r.name.as_str()).unwrap_or("");
                        let win_name = format!("{repo_name}:{}", f.name);
                        let pane_cmd = tmux_local::get_pane_command(session, &win_name);
                        CliFeature {
                            feature: f,
                            pane_command: pane_cmd,
                        }
                    })
                    .collect();
                self.loading = false;
                self.error = None;
            }
            AppEvent::FeaturesLoaded(Err(e)) => {
                self.loading = false;
                self.error = Some(e);
            }
            AppEvent::FeatureCreated(Ok(name)) => {
                // After creation, switch to the feature
                self.feature = Some(Feature {
                    name: name.clone(),
                    worktree_path: String::new(),
                    branch: format!("feature/{name}"),
                    is_active: false,
                    is_main: false,
                });
                self.do_attach(tx);
            }
            AppEvent::FeatureCreated(Err(e)) => {
                self.loading = false;
                self.error = Some(e);
            }
            AppEvent::SwitchDone(Ok(_)) => {
                // Save state and prepare to attach
                let new_state = CliState {
                    last_server: self.server.as_ref().map(|s| s.id.clone()),
                    last_repo: self.repo.as_ref().map(|r| r.path.clone()),
                    last_feature: self.feature.as_ref().map(|f| f.name.clone()),
                    last_attached: Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    ),
                };
                state::save_state(&self.settings, &new_state);

                self.attach_session = Some(self.settings.tmux.session.clone());
                self.should_quit = true;
            }
            AppEvent::SwitchDone(Err(e)) => {
                self.loading = false;
                self.error = Some(e);
                self.screen = Screen::FeaturePicker;
            }
            AppEvent::HealthResult(id, ok) => {
                self.health_map.insert(id, ok);
            }
            _ => {}
        }
    }

    fn trigger_load_repos(&self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        if let Some(ref server) = self.server {
            let server = server.clone();
            tokio::spawn(async move {
                let result = api_client::list_repos(&server).await;
                tx.send(AppEvent::ReposLoaded(result)).ok();
            });
        }
    }

    fn trigger_load_features(&self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        if let Some(ref server) = self.server {
            if let Some(ref repo) = self.repo {
                let server = server.clone();
                let repo_path = repo.path.clone();
                tokio::spawn(async move {
                    let result = api_client::list_features(&server, &repo_path).await;
                    tx.send(AppEvent::FeaturesLoaded(result)).ok();
                });
            }
        }
    }

    fn do_attach(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        self.screen = Screen::Attaching;
        self.loading = true;
        self.error = None;

        if let (Some(server), Some(repo), Some(feature)) =
            (self.server.clone(), self.repo.clone(), self.feature.clone())
        {
            tokio::spawn(async move {
                let result =
                    api_client::switch_feature(&server, &repo.path, &feature.name).await;
                tx.send(AppEvent::SwitchDone(result)).ok();
            });
        }
    }

    fn do_resume(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        let server_id = self.cli_state.last_server.clone();
        let repo_path = self.cli_state.last_repo.clone();
        let feature_name = self.cli_state.last_feature.clone();

        if let (Some(sid), Some(rp), Some(fn_)) = (server_id, repo_path, feature_name) {
            let srv = self.servers.iter().find(|s| s.id == sid).cloned();
            if let Some(srv) = srv {
                self.server = Some(srv.clone());
                self.repo = Some(Repository {
                    name: std::path::Path::new(&rp)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    path: rp.clone(),
                    branch: String::new(),
                });
                self.feature = Some(Feature {
                    name: fn_.clone(),
                    worktree_path: String::new(),
                    branch: String::new(),
                    is_active: false,
                    is_main: false,
                });
                self.do_attach(tx);
            }
        }
    }

    fn do_create_feature(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        self.screen = Screen::Attaching;
        self.loading = true;

        if let (Some(server), Some(repo)) = (self.server.clone(), self.repo.clone()) {
            let name = self.input_text.clone();
            tokio::spawn(async move {
                let result = api_client::create_feature(&server, &repo.path, &name).await;
                tx.send(AppEvent::FeatureCreated(result)).ok();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn test_initial_screen_with_last_session() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();

        // Save a state with last session
        let state = CliState {
            last_server: Some("localhost".to_string()),
            last_repo: Some("/tmp/repo".to_string()),
            last_feature: Some("feat".to_string()),
            last_attached: None,
        };
        state::save_state(&settings, &state);

        let app = App::new(settings);
        assert_eq!(app.screen, Screen::Resume);
    }

    #[test]
    fn test_initial_screen_without_last_session() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();

        let app = App::new(settings);
        assert_eq!(app.screen, Screen::ServerPicker);
    }

    #[test]
    fn test_go_back_from_repo_picker() {
        let mut app = App::new(test_settings());
        app.screen = Screen::RepoPicker;
        app.go_back();
        assert_eq!(app.screen, Screen::ServerPicker);
    }

    #[test]
    fn test_go_back_from_feature_picker() {
        let mut app = App::new(test_settings());
        app.screen = Screen::FeaturePicker;
        app.go_back();
        assert_eq!(app.screen, Screen::RepoPicker);
    }

    #[test]
    fn test_go_back_from_feature_create() {
        let mut app = App::new(test_settings());
        app.screen = Screen::FeatureCreate;
        app.go_back();
        assert_eq!(app.screen, Screen::FeaturePicker);
    }

    #[test]
    fn test_go_back_from_server_add() {
        let mut app = App::new(test_settings());
        app.screen = Screen::ServerAdd;
        app.go_back();
        assert_eq!(app.screen, Screen::ServerPicker);
    }
}
