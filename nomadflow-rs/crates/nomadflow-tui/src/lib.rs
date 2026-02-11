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

// ── Inline mini-TUI pickers ──────────────────────────────────────────

use crossterm::event::{self as ct_event, KeyCode};
use ratatui::widgets::Paragraph;
use std::time::Duration;

/// Item for the list picker: label shown on the left, detail on the right.
pub struct PickItem {
    pub label: String,
    pub detail: String,
}

/// Show a ratatui list picker. Returns the selected index, or None if cancelled.
pub fn pick_from_list(title: &str, items: &[PickItem]) -> Result<Option<usize>> {
    if items.is_empty() {
        return Ok(None);
    }

    let mut terminal = init_terminal()?;
    let mut selected: usize = 0;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(area);

            let title_w = Paragraph::new(title)
                .style(Style::default().fg(Color::Cyan).bold());
            f.render_widget(title_w, chunks[0]);

            let visible_height = chunks[1].height as usize;
            let scroll = if selected >= visible_height {
                selected - visible_height + 1
            } else {
                0
            };

            let list_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    (0..visible_height)
                        .map(|_| Constraint::Length(1))
                        .collect::<Vec<_>>(),
                )
                .split(chunks[1]);

            for (vi, row_area) in list_chunks.iter().enumerate() {
                let i = vi + scroll;
                if i >= items.len() {
                    break;
                }
                let marker = if i == selected { "> " } else { "  " };
                let style = if i == selected {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default()
                };
                let line = Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(&items[i].label, style),
                    Span::raw("  "),
                    Span::styled(&items[i].detail, Style::default().fg(Color::DarkGray)),
                ]);
                f.render_widget(Paragraph::new(line), *row_area);
            }

            let footer = Paragraph::new("Up/Down: navigate  Enter: select  Esc: cancel")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(footer, chunks[2]);
        })?;

        if ct_event::poll(Duration::from_millis(50))? {
            if let ct_event::Event::Key(key) = ct_event::read()? {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected + 1 < items.len() {
                            selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        restore_terminal(&mut terminal)?;
                        return Ok(Some(selected));
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        restore_terminal(&mut terminal)?;
                        return Ok(None);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Show a ratatui y/n confirmation. Returns true if confirmed.
pub fn confirm(message: &str) -> Result<bool> {
    let mut terminal = init_terminal()?;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Length(1)])
                .split(area);

            let msg = Paragraph::new(message).style(Style::default().bold());
            f.render_widget(msg, chunks[0]);

            let hint = Paragraph::new("y: confirm  n/Esc: cancel")
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(hint, chunks[1]);
        })?;

        if ct_event::poll(Duration::from_millis(50))? {
            if let ct_event::Event::Key(key) = ct_event::read()? {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        restore_terminal(&mut terminal)?;
                        return Ok(true);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        restore_terminal(&mut terminal)?;
                        return Ok(false);
                    }
                    _ => {}
                }
            }
        }
    }
}
