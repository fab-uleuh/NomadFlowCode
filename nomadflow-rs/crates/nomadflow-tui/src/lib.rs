pub mod api_client;
pub mod app;
pub mod event;
pub mod screens;
pub mod state;
pub mod tmux_local;
pub mod widgets;

use std::io;

use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use nomadflow_core::config::Settings;

use crate::app::{App, AppResult};

/// Initialize the terminal.
fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI wizard. Returns the tmux session name to attach to (if any).
pub async fn run_tui(settings: Settings) -> Result<Option<String>> {
    let mut terminal = init_terminal()?;
    let mut app = App::new(settings);

    let result = app.run(&mut terminal).await;

    restore_terminal(&mut terminal)?;

    match result {
        Ok(AppResult::Attach(session)) => Ok(Some(session)),
        Ok(AppResult::Quit) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Run status mode: print tmux status and exit.
pub fn run_status(settings: &Settings) {
    let session = &settings.tmux.session;

    if !tmux_local::session_exists(session) {
        println!("Session: {session}");
        println!("No active session");
        return;
    }

    let windows = tmux_local::list_windows(session);
    println!("Session: {session}");
    println!("{} window(s)", windows.len());
    println!();

    for w in &windows {
        let cmd = tmux_local::get_pane_command(session, &w.name);
        let idle = tmux_local::is_shell_idle_str(cmd.as_deref());
        let status = match &cmd {
            Some(_) if idle => "idle".to_string(),
            Some(c) => format!("● {c}"),
            None => String::new(),
        };
        let marker = if w.active { ">" } else { " " };
        println!("{marker} {}: {}  {status}", w.index, w.name);
    }
}

/// Exec into tmux attach (replaces the process for --attach mode).
pub fn exec_tmux_attach(session: &str) {
    tmux_local::attach_session(session);
}
