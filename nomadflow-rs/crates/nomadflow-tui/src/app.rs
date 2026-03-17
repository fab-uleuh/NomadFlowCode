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
use crate::widgets;

/// What the TUI should do when it exits.
pub enum AppResult {
    Quit,
}

/// Wizard screens.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Setup,
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

    // Setup wizard state
    pub setup_step: u8,
    pub setup_secret: String,
    pub setup_subdomain: String,
    pub setup_public: bool,

    // Result
    pub should_quit: bool,
}

/// Feature enriched with local state.
#[derive(Debug, Clone)]
pub struct CliFeature {
    pub feature: Feature,
}

impl App {
    /// Generate a random password (16 chars: a-z, 0-9, hyphens).
    fn generate_password() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789-";
        let mut rng = rand::rng();
        (0..16)
            .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
            .collect()
    }

    /// Generate a random subdomain (nf-XXXXXXXX, 8 lowercase alphanumeric chars).
    fn generate_subdomain() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::rng();
        let suffix: String = (0..8)
            .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
            .collect();
        format!("nf-{suffix}")
    }

    pub fn new(settings: Settings) -> Self {
        let servers = state::load_servers(&settings);
        let cli_state = state::load_state(&settings);

        let needs_setup = !settings.config_file().exists();

        let has_last = cli_state.last_server.is_some()
            && cli_state.last_repo.is_some()
            && cli_state.last_feature.is_some();

        let initial_screen = if needs_setup {
            Screen::Setup
        } else if has_last {
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

        // Pre-generate a password and subdomain for the setup wizard
        let setup_secret = if needs_setup {
            Self::generate_password()
        } else {
            String::new()
        };
        let setup_subdomain = if needs_setup {
            Self::generate_subdomain()
        } else {
            String::new()
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
            setup_step: 0,
            setup_secret,
            setup_subdomain,
            setup_public: false,
            should_quit: false,
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
            if let Some(AppEvent::Key(key)) = poll_event(Duration::from_millis(50)) {
                self.handle_key(key.code, key.modifiers, tx.clone());
            }

            if self.should_quit {
                return Ok(AppResult::Quit);
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
                Constraint::Min(1),    // Content
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
            Screen::Setup => screens::setup::render(frame, chunks[2], self),
            Screen::Resume => screens::resume::render(frame, chunks[2], self),
            Screen::ServerPicker => screens::server_picker::render(frame, chunks[2], self),
            Screen::ServerAdd => screens::server_add::render(frame, chunks[2], self),
            Screen::RepoPicker => screens::repo_picker::render(frame, chunks[2], self),
            Screen::FeaturePicker => screens::feature_picker::render(frame, chunks[2], self),
            Screen::FeatureCreate => screens::feature_create::render(frame, chunks[2], self),
            Screen::Attaching => screens::attaching::render(frame, chunks[2], self),
        }

        // Footer
        let footer_text = match self.screen {
            Screen::Attaching => "",
            Screen::Setup => "Escape: back",
            _ => "Escape: back  q: quit",
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
            && self.screen != Screen::Setup
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
            Screen::Setup => self.handle_setup_key(code),
            Screen::Resume => self.handle_resume_key(code),
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
            Screen::ServerPicker => {
                // Root screen — Esc quits
                self.should_quit = true;
            }
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
            Screen::Setup => {
                self.should_quit = true;
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
                    // Resume: go directly to server picker
                    self.screen = Screen::ServerPicker;
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

    fn handle_setup_key(&mut self, code: KeyCode) {
        match self.setup_step {
            0 => {
                // Password choice: up/down + enter
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
                            // Generate — secret already pre-generated, go to public choice
                            self.setup_step = 2;
                            self.selected_index = 0;
                        } else {
                            // Custom password input
                            self.setup_step = 1;
                            self.input_text.clear();
                            self.input_cursor = 0;
                        }
                    }
                    KeyCode::Esc => self.go_back(),
                    _ => {}
                }
            }
            1 => {
                // Custom password text input
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
                            self.setup_secret = trimmed;
                            self.input_text.clear();
                            self.input_cursor = 0;
                            self.setup_step = 2;
                        }
                    }
                    KeyCode::Esc => {
                        self.input_text.clear();
                        self.input_cursor = 0;
                        self.setup_step = 0;
                        self.selected_index = 0;
                    }
                    _ => {}
                }
            }
            2 => {
                // Public mode? y/n
                match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        self.setup_public = true;
                        self.setup_step = 3;
                        self.input_text.clear();
                        self.input_cursor = 0;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        self.setup_public = false;
                        self.setup_step = 4; // skip subdomain, go to confirm
                    }
                    KeyCode::Esc => {
                        self.setup_step = 0;
                        self.selected_index = 0;
                    }
                    _ => {}
                }
            }
            3 => {
                // Fixed subdomain? y/n (subdomain is pre-generated)
                match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        // Keep the pre-generated subdomain
                        self.setup_step = 4;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        // No fixed subdomain → random each time
                        self.setup_subdomain.clear();
                        self.setup_step = 4;
                    }
                    KeyCode::Esc => {
                        self.setup_step = 2;
                    }
                    _ => {}
                }
            }
            4 => {
                // Confirm y/n
                match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                        self.save_setup();
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        // Restart setup
                        self.setup_step = 0;
                        self.selected_index = 0;
                        self.setup_secret = Self::generate_password();
                        self.setup_subdomain = Self::generate_subdomain();
                        self.setup_public = false;
                    }
                    KeyCode::Esc => {
                        if self.setup_public {
                            // Regenerate if user had previously said "no" (cleared it)
                            if self.setup_subdomain.is_empty() {
                                self.setup_subdomain = Self::generate_subdomain();
                            }
                            self.setup_step = 3;
                        } else {
                            self.setup_step = 2;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Run only the setup wizard loop (for `nomadflow serve` first-run).
    /// Returns `true` if setup completed, `false` if cancelled.
    pub fn run_setup_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<bool> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if crossterm::event::poll(Duration::from_millis(50))? {
                if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(false);
                    }
                    self.handle_setup_key(key.code);
                }
            }

            if self.screen != Screen::Setup {
                return Ok(true);
            }
            if self.should_quit {
                return Ok(false);
            }
        }
    }

    fn save_setup(&mut self) {
        self.settings.auth.secret = self.setup_secret.clone();
        self.settings.tunnel.subdomain = self.setup_subdomain.clone();
        if let Err(e) = self.settings.save() {
            self.error = Some(format!("Failed to save config: {e}"));
            return;
        }
        // Transition to normal flow
        self.screen = Screen::ServerPicker;
        self.selected_index = 0;
        self.setup_step = 0;
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
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        match event {
            AppEvent::ReposLoaded(repos) => {
                self.loading = false;
                self.repos = repos.unwrap_or_default();
            }
            AppEvent::FeaturesLoaded(features) => {
                self.loading = false;
                self.features = features
                    .unwrap_or_default()
                    .into_iter()
                    .map(|f| CliFeature { feature: f })
                    .collect();
            }
            AppEvent::FeatureCreated(result) => {
                self.loading = false;
                match result {
                    Ok(feature_name) => {
                        self.feature = Some(Feature {
                            name: feature_name,
                            worktree_path: String::new(),
                            branch: String::new(),
                            is_active: true,
                            is_main: false,
                        });
                        self.screen = Screen::Attaching;
                    }
                    Err(e) => {
                        self.error = Some(e);
                    }
                }
            }
            AppEvent::SwitchDone(result) => {
                self.loading = false;
                match result {
                    Ok(_) => {
                        self.screen = Screen::Attaching;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.screen = Screen::FeaturePicker;
                    }
                }
            }
            AppEvent::Key(_) => {}
        }
    }

    fn trigger_load_repos(&self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        if let Some(server) = &self.server {
            let server = server.clone();
            tokio::spawn(async move {
                match api_client::list_repos(&server).await {
                    Ok(repos) => {
                        let _ = tx.send(AppEvent::ReposLoaded(Ok(repos)));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::ReposLoaded(Ok(Vec::new())));
                        eprintln!("Failed to load repos: {e}");
                    }
                }
            });
        }
    }

    fn trigger_load_features(&self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        if let (Some(server), Some(repo)) = (&self.server, &self.repo) {
            let server = server.clone();
            let repo_path = repo.path.clone();
            tokio::spawn(async move {
                match api_client::list_features(&server, &repo_path).await {
                    Ok(features) => {
                        let _ = tx.send(AppEvent::FeaturesLoaded(Ok(features)));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::FeaturesLoaded(Ok(Vec::new())));
                        eprintln!("Failed to load features: {e}");
                    }
                }
            });
        }
    }

    fn do_create_feature(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        self.loading = true;
        if let (Some(server), Some(repo)) = (&self.server, &self.repo) {
            let server = server.clone();
            let repo_path = repo.path.clone();
            let branch_name = self.input_text.clone();
            tokio::spawn(async move {
                match api_client::create_feature(&server, &repo_path, &branch_name).await {
                    Ok(feature) => {
                        let _ = tx.send(AppEvent::FeatureCreated(Ok(feature)));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::FeatureCreated(Err(e.to_string())));
                    }
                }
            });
        }
        self.input_text.clear();
        self.input_cursor = 0;
        self.confirm_step = false;
    }

    fn do_attach(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        self.loading = true;
        if let (Some(server), Some(repo), Some(feature)) =
            (&self.server, &self.repo, &self.feature)
        {
            let server = server.clone();
            let repo_path = repo.path.clone();
            let feature_name = feature.name.clone();
            tokio::spawn(async move {
                match api_client::switch_feature(&server, &repo_path, &feature_name).await {
                    Ok(_) => {
                        let _ = tx.send(AppEvent::SwitchDone(Ok(String::new())));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::SwitchDone(Err(e.to_string())));
                    }
                }
            });
        }
    }

}
