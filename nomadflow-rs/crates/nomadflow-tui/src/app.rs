use std::io;
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::prelude::*;

use nomadflow_core::agent_state::AgentStateService;
use nomadflow_core::config::Settings;
use nomadflow_core::models::{AgentStateKind, Feature, Repository};
use nomadflow_core::services::tmux::parse_session_window as core_parse_session_window;

use crate::api_client;
use crate::event::{poll_event, AppEvent};
use crate::screens;
use crate::state::{self, CliState, ServerConfig};
use crate::tmux_local;
use crate::widgets;

/// What the TUI should do when it exits.
pub enum AppResult {
    Attach(String),               // tmux session name
    AttachWindow(String, String), // (tmux session name, window name)
    Quit,
}

/// Wizard screens.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    SessionPicker,
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

    // Session picker state
    pub session_items: Vec<screens::session_picker::SelectableItem>,

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

    // Delete confirmation state
    pub show_delete_confirm: bool,
    pub delete_window_name: Option<String>,

    // Agent state refresh
    pub last_state_refresh: Instant,

    // Result
    pub should_quit: bool,
    pub attach_session: Option<String>,
    pub attach_window: Option<String>,
}

/// Feature enriched with local tmux pane command.
#[derive(Debug, Clone)]
pub struct CliFeature {
    pub feature: Feature,
    pub pane_command: Option<String>,
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

        // Check for active managed sessions (3-part window names)
        let session_items = if !needs_setup {
            screens::session_picker::build_selectable_items_from_settings(&settings)
        } else {
            Vec::new()
        };
        // session_items always has at least "Browse repos..." when sessions exist
        let has_sessions = session_items.len() > 1; // more than just "Browse repos..."

        let initial_screen = if needs_setup {
            Screen::Setup
        } else if has_sessions {
            Screen::SessionPicker
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
            session_items,
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
            show_delete_confirm: false,
            delete_window_name: None,
            last_state_refresh: Instant::now(),
            should_quit: false,
            attach_session: None,
            attach_window: None,
        }
    }

    /// Reload the session list from tmux and clamp selection.
    pub fn reload_sessions(&mut self) {
        self.session_items =
            screens::session_picker::build_selectable_items_from_settings(&self.settings);
        if self.selected_index >= self.session_items.len() {
            self.selected_index = self.session_items.len().saturating_sub(1);
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

        // Trigger initial agent state refresh for session picker
        if self.screen == Screen::SessionPicker && !self.session_items.is_empty() {
            self.trigger_state_refresh(tx.clone());
            self.last_state_refresh = Instant::now();
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

            // Periodic agent state refresh (every 3s) on session picker
            if self.screen == Screen::SessionPicker
                && !self.session_items.is_empty()
                && self.last_state_refresh.elapsed() >= Duration::from_secs(3)
            {
                self.trigger_state_refresh(tx.clone());
                self.last_state_refresh = Instant::now();
            }

            if self.should_quit {
                return Ok(
                    match (self.attach_session.take(), self.attach_window.take()) {
                        (Some(session), Some(window)) => AppResult::AttachWindow(session, window),
                        (Some(session), None) => AppResult::Attach(session),
                        _ => AppResult::Quit,
                    },
                );
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
            Screen::SessionPicker => screens::session_picker::render(frame, chunks[2], self),
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
            Screen::SessionPicker => {
                if self.show_delete_confirm {
                    "" // Footer replaced by confirmation bar in session_picker render
                } else {
                    "q: quit  \u{2191}\u{2193}: navigate  Enter: attach  d: close  r: reload  | nomadflow run: create session"
                }
            }
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
            && !self.show_delete_confirm
        {
            self.should_quit = true;
            return;
        }
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        if code == KeyCode::Esc {
            // During delete confirmation, Esc is handled by session_picker_key
            if self.screen == Screen::SessionPicker && self.show_delete_confirm {
                // Fall through to screen-specific handler
            } else {
                self.go_back();
                return;
            }
        }

        // Screen-specific keys
        match self.screen {
            Screen::SessionPicker => self.handle_session_picker_key(code, tx),
            Screen::Setup => self.handle_setup_key(code),
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
            Screen::SessionPicker => {
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

    fn handle_session_picker_key(
        &mut self,
        code: KeyCode,
        _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) {
        // Handle delete confirmation mode
        if self.show_delete_confirm {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    if let Some(window) = self.delete_window_name.take() {
                        let killed = tmux_local::kill_window(&self.settings.tmux.session, &window);
                        if killed {
                            self.error = None;
                        } else {
                            self.error =
                                Some("Failed to close session (may already be gone)".to_string());
                        }
                        self.show_delete_confirm = false;
                        self.reload_sessions();

                        // If no sessions left, transition away
                        let has_sessions = self.session_items.len() > 1;
                        if !has_sessions {
                            let has_last = self.cli_state.last_server.is_some()
                                && self.cli_state.last_repo.is_some()
                                && self.cli_state.last_feature.is_some();
                            self.screen = if has_last {
                                Screen::Resume
                            } else {
                                Screen::ServerPicker
                            };
                            self.selected_index = 0;
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.show_delete_confirm = false;
                    self.delete_window_name = None;
                }
                _ => {} // Block all other keys during confirmation
            }
            return;
        }

        let count = self.session_items.len();
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
                    let item = &self.session_items[self.selected_index];
                    match &item.window_name {
                        Some(window) => {
                            // Verify window still exists before attaching
                            if tmux_local::window_exists(&self.settings.tmux.session, window) {
                                self.attach_window = Some(window.clone());
                                self.attach_session = Some(self.settings.tmux.session.clone());
                                self.should_quit = true;
                            } else {
                                self.error =
                                    Some("Session no longer exists — reloading...".to_string());
                                self.reload_sessions();
                            }
                        }
                        None => {
                            // "Browse repos..." — enter existing flow
                            let has_last = self.cli_state.last_server.is_some()
                                && self.cli_state.last_repo.is_some()
                                && self.cli_state.last_feature.is_some();
                            self.screen = if has_last {
                                Screen::Resume
                            } else {
                                Screen::ServerPicker
                            };
                            self.selected_index = 0;
                        }
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('x') => {
                // Only allow delete on session items (not "Browse repos...")
                if self.selected_index < count {
                    let item = &self.session_items[self.selected_index];
                    if let Some(ref window) = item.window_name {
                        self.show_delete_confirm = true;
                        self.delete_window_name = Some(window.clone());
                    }
                }
            }
            KeyCode::Char('r') => {
                self.reload_sessions();
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
                        ttyd_url: Some(state::derive_ttyd_url(&self.server_add_url)),
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
            AppEvent::AgentStatesRefreshed(states) => {
                for (window, state) in states {
                    if let Some(item) = self
                        .session_items
                        .iter_mut()
                        .find(|i| i.window_name.as_deref() == Some(&window))
                    {
                        item.agent_state = Some(state);
                    }
                }
            }
            _ => {}
        }
    }

    fn trigger_state_refresh(&self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) {
        let agent_svc = AgentStateService::new(&self.settings);
        let tmux_session = self.settings.tmux.session.clone();
        let window_names: Vec<String> = self
            .session_items
            .iter()
            .filter_map(|i| i.window_name.clone())
            .collect();

        tokio::spawn(async move {
            let mut states = Vec::new();
            for window_name in &window_names {
                if let Some(session) = core_parse_session_window(window_name) {
                    let state =
                        read_session_state(&agent_svc, &session, &tmux_session).await;
                    states.push((window_name.clone(), state));
                }
            }
            tx.send(AppEvent::AgentStatesRefreshed(states)).ok();
        });
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
                let result = api_client::switch_feature(&server, &repo.path, &feature.name).await;
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

/// Read agent state for a session from disk (claude-code) or tmux process detection (generic).
///
/// Delegates to `AgentStateService::read_state()` for claude-code sessions.
/// Uses local tmux commands for generic sessions (TUI-specific tmux_local module).
async fn read_session_state(
    svc: &AgentStateService,
    session: &nomadflow_core::models::Session,
    tmux_session: &str,
) -> AgentStateKind {
    if session.agent_type == "claude-code" {
        svc.read_state(&session.session_id)
            .await
            .map(|s| s.state)
            .unwrap_or(AgentStateKind::Unknown)
    } else if !tmux_local::window_exists(tmux_session, &session.window_name) {
        AgentStateKind::Done
    } else {
        match tmux_local::get_pane_command(tmux_session, &session.window_name) {
            Some(cmd) if tmux_local::is_shell_idle_str(Some(&cmd)) => {
                AgentStateKind::WaitingForInput
            }
            Some(_) => AgentStateKind::Generating,
            None => AgentStateKind::Idle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> Settings {
        let mut settings = Settings::default();
        settings.tmux.session = format!("nf-test-default-{}", std::process::id());
        settings
    }

    /// Create a temp settings with a config.toml so setup wizard doesn't trigger.
    fn tmp_settings_with_config() -> (tempfile::TempDir, Settings) {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.tmux.session = format!("nf-test-cfg-{}", std::process::id());
        settings.ensure_directories().unwrap();
        // Create config.toml so setup screen is skipped
        std::fs::write(settings.config_file(), "").unwrap();
        (tmp, settings)
    }

    #[test]
    fn test_initial_screen_setup_when_no_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        settings.ensure_directories().unwrap();
        // No config.toml → setup screen
        let app = App::new(settings);
        assert_eq!(app.screen, Screen::Setup);
        assert!(!app.setup_secret.is_empty());
    }

    #[test]
    fn test_initial_screen_with_last_session() {
        let (_tmp, settings) = tmp_settings_with_config();

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
        let (_tmp, settings) = tmp_settings_with_config();

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

    #[test]
    fn test_go_back_from_session_picker() {
        let mut app = App::new(test_settings());
        app.screen = Screen::SessionPicker;
        app.go_back();
        assert!(app.should_quit); // SessionPicker is root — Esc quits
    }

    #[test]
    fn test_initial_screen_session_picker_with_tmux_sessions() {
        if !tmux_local::is_tmux_installed() {
            eprintln!("Skipping: tmux not installed");
            return;
        }

        let pid = std::process::id();
        let session_name = format!("nf-test-app-{pid}");

        // Create a tmux session with a session-format window
        let created = std::process::Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &session_name,
                "-n",
                "myapp:feat:agent-1",
            ])
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !created {
            eprintln!("Skipping: could not create tmux session");
            return;
        }

        let (_tmp, mut settings) = tmp_settings_with_config();
        settings.tmux.session = session_name.clone();

        let app = App::new(settings);

        // Cleanup
        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", &session_name])
            .output();

        assert_eq!(app.screen, Screen::SessionPicker);
        assert!(app.session_items.len() > 1); // sessions + "Browse repos..."
    }

    #[test]
    fn test_initial_screen_no_sessions_falls_through() {
        if !tmux_local::is_tmux_installed() {
            eprintln!("Skipping: tmux not installed");
            return;
        }

        let pid = std::process::id();
        let session_name = format!("nf-test-nosess-{pid}");

        // Create a tmux session with only a single-part window (not a session)
        let created = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", &session_name, "-n", "zsh"])
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !created {
            eprintln!("Skipping: could not create tmux session");
            return;
        }

        let (_tmp, mut settings) = tmp_settings_with_config();
        settings.tmux.session = session_name.clone();

        let app = App::new(settings);

        // Cleanup
        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", &session_name])
            .output();

        // No session-format windows → falls through to ServerPicker (no last session)
        assert_eq!(app.screen, Screen::ServerPicker);
        assert!(app.session_items.is_empty());
    }

    // ── delete confirmation state tests ──

    fn app_with_session_items() -> App {
        let mut app = App::new(test_settings());
        app.screen = Screen::SessionPicker;
        app.session_items = vec![
            screens::session_picker::SelectableItem {
                label: "agent-1".to_string(),
                window_name: Some("myapp:feat:agent-1".to_string()),
                group_header: Some("myapp / feat".to_string()),
                agent_state: None,
            },
            screens::session_picker::SelectableItem {
                label: "agent-2".to_string(),
                window_name: Some("myapp:feat:agent-2".to_string()),
                group_header: None,
                agent_state: None,
            },
            screens::session_picker::SelectableItem {
                label: "Browse repos...".to_string(),
                window_name: None,
                group_header: None,
                agent_state: None,
            },
        ];
        app.selected_index = 0;
        app
    }

    #[test]
    fn test_delete_confirm_d_key_on_session_item() {
        let mut app = app_with_session_items();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        app.handle_session_picker_key(KeyCode::Char('d'), tx);

        assert!(app.show_delete_confirm);
        assert_eq!(
            app.delete_window_name,
            Some("myapp:feat:agent-1".to_string())
        );
    }

    #[test]
    fn test_delete_confirm_x_key_on_session_item() {
        let mut app = app_with_session_items();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        app.handle_session_picker_key(KeyCode::Char('x'), tx);

        assert!(app.show_delete_confirm);
        assert_eq!(
            app.delete_window_name,
            Some("myapp:feat:agent-1".to_string())
        );
    }

    #[test]
    fn test_delete_confirm_d_key_on_browse_repos_ignored() {
        let mut app = app_with_session_items();
        app.selected_index = 2; // "Browse repos..."
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        app.handle_session_picker_key(KeyCode::Char('d'), tx);

        assert!(!app.show_delete_confirm);
        assert!(app.delete_window_name.is_none());
    }

    #[test]
    fn test_delete_confirm_cancel_with_n() {
        let mut app = app_with_session_items();
        app.show_delete_confirm = true;
        app.delete_window_name = Some("myapp:feat:agent-1".to_string());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        app.handle_session_picker_key(KeyCode::Char('n'), tx);

        assert!(!app.show_delete_confirm);
        assert!(app.delete_window_name.is_none());
    }

    #[test]
    fn test_delete_confirm_cancel_with_esc() {
        let mut app = app_with_session_items();
        app.show_delete_confirm = true;
        app.delete_window_name = Some("myapp:feat:agent-1".to_string());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        app.handle_session_picker_key(KeyCode::Esc, tx);

        assert!(!app.show_delete_confirm);
        assert!(app.delete_window_name.is_none());
    }

    #[test]
    fn test_delete_confirm_y_on_nonexistent_window_sets_error() {
        let mut app = app_with_session_items();
        app.show_delete_confirm = true;
        // Set a window name that doesn't actually exist in tmux
        app.delete_window_name = Some("nonexistent:window:agent-99".to_string());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        app.handle_session_picker_key(KeyCode::Char('y'), tx);

        assert!(!app.show_delete_confirm);
        assert!(app.delete_window_name.is_none());
        assert!(app.error.is_some());
        assert!(app
            .error
            .as_ref()
            .unwrap()
            .contains("Failed to close session"));
    }

    #[test]
    fn test_delete_confirm_blocks_navigation() {
        let mut app = app_with_session_items();
        app.show_delete_confirm = true;
        app.delete_window_name = Some("myapp:feat:agent-1".to_string());
        let original_index = app.selected_index;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        // Arrow keys should be blocked during confirmation
        app.handle_session_picker_key(KeyCode::Down, tx.clone());
        assert_eq!(app.selected_index, original_index);
        assert!(app.show_delete_confirm); // Still in confirm mode

        app.handle_session_picker_key(KeyCode::Up, tx);
        assert_eq!(app.selected_index, original_index);
        assert!(app.show_delete_confirm);
    }

    #[test]
    fn test_delete_confirm_y_kills_and_reloads() {
        if !tmux_local::is_tmux_installed() {
            eprintln!("Skipping: tmux not installed");
            return;
        }

        let pid = std::process::id();
        let session_name = format!("nf-test-delconf-{pid}");
        let window1 = "myapp:feat:agent-1";
        let window2 = "myapp:feat:agent-2";

        // Create session with 2 windows
        let created = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", &session_name, "-n", window1])
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !created {
            eprintln!("Skipping: could not create tmux session");
            return;
        }

        let _ = std::process::Command::new("tmux")
            .args(["new-window", "-t", &session_name, "-n", window2])
            .output();

        let (_tmp, mut settings) = tmp_settings_with_config();
        settings.tmux.session = session_name.clone();

        let mut app = App::new(settings);
        assert_eq!(app.screen, Screen::SessionPicker);
        // session_items should have agent-1, agent-2, and "Browse repos..."
        let initial_session_count = app
            .session_items
            .iter()
            .filter(|i| i.window_name.is_some())
            .count();
        assert_eq!(initial_session_count, 2);

        // Press 'd' on the first session
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        app.handle_session_picker_key(KeyCode::Char('d'), tx.clone());
        assert!(app.show_delete_confirm);

        // Confirm with 'y'
        app.handle_session_picker_key(KeyCode::Char('y'), tx);
        assert!(!app.show_delete_confirm);
        assert!(app.delete_window_name.is_none());

        // Session list should now have 1 session
        let remaining = app
            .session_items
            .iter()
            .filter(|i| i.window_name.is_some())
            .count();
        assert_eq!(remaining, 1);

        // The remaining window should be agent-2
        let remaining_window = app
            .session_items
            .iter()
            .find(|i| i.window_name.is_some())
            .unwrap();
        assert_eq!(remaining_window.window_name.as_deref(), Some(window2));

        // Cleanup
        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", &session_name])
            .output();
    }

    // ── read_session_state tests ──

    fn svc_from_tmp(tmp: &tempfile::TempDir) -> AgentStateService {
        let settings = Settings {
            paths: nomadflow_core::config::PathsConfig {
                base_dir: tmp.path().to_string_lossy().to_string(),
            },
            ..Default::default()
        };
        AgentStateService::new(&settings)
    }

    #[tokio::test]
    async fn test_read_session_state_claude_code_from_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let svc = svc_from_tmp(&tmp);
        let session_dir = svc.sessions_dir().join("myapp-feat-claude-code-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("agent-state.json"),
            r#"{"sessionId":"myapp-feat-claude-code-1","agentType":"claude-code","state":"waiting_for_input","timestamp":"2026-02-15T10:30:00Z","lastEvent":"Stop"}"#,
        )
        .unwrap();

        let session = nomadflow_core::models::Session {
            session_id: "myapp-feat-claude-code-1".to_string(),
            window_name: "myapp:feat:claude-code-1".to_string(),
            repo: "myapp".to_string(),
            worktree: "feat".to_string(),
            agent_type: "claude-code".to_string(),
            agent_number: 1,
        };

        let state = read_session_state(&svc, &session, "unused").await;
        assert_eq!(state, AgentStateKind::WaitingForInput);
    }

    #[tokio::test]
    async fn test_read_session_state_claude_code_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let svc = svc_from_tmp(&tmp);

        let session = nomadflow_core::models::Session {
            session_id: "nonexistent-session".to_string(),
            window_name: "myapp:feat:claude-code-1".to_string(),
            repo: "myapp".to_string(),
            worktree: "feat".to_string(),
            agent_type: "claude-code".to_string(),
            agent_number: 1,
        };

        let state = read_session_state(&svc, &session, "unused").await;
        assert_eq!(state, AgentStateKind::Unknown);
    }

    #[tokio::test]
    async fn test_read_session_state_claude_code_corrupt_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let svc = svc_from_tmp(&tmp);
        let session_dir = svc.sessions_dir().join("corrupt-session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("agent-state.json"), "not json").unwrap();

        let session = nomadflow_core::models::Session {
            session_id: "corrupt-session".to_string(),
            window_name: "myapp:feat:claude-code-1".to_string(),
            repo: "myapp".to_string(),
            worktree: "feat".to_string(),
            agent_type: "claude-code".to_string(),
            agent_number: 1,
        };

        let state = read_session_state(&svc, &session, "unused").await;
        assert_eq!(state, AgentStateKind::Unknown);
    }

    #[tokio::test]
    async fn test_agent_states_refreshed_event_updates_items() {
        let mut app = app_with_session_items();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        // Simulate receiving state refresh results
        app.handle_async_event(
            AppEvent::AgentStatesRefreshed(vec![
                (
                    "myapp:feat:agent-1".to_string(),
                    AgentStateKind::WaitingForInput,
                ),
                ("myapp:feat:agent-2".to_string(), AgentStateKind::Generating),
            ]),
            tx,
        );

        assert_eq!(
            app.session_items[0].agent_state,
            Some(AgentStateKind::WaitingForInput)
        );
        assert_eq!(
            app.session_items[1].agent_state,
            Some(AgentStateKind::Generating)
        );
        // "Browse repos..." should remain None
        assert_eq!(app.session_items[2].agent_state, None);
    }
}
