use ratatui::{
    prelude::*,
    widgets::Paragraph,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    match app.setup_step {
        0 => render_password_choice(frame, area, app),
        1 => render_password_input(frame, area, app),
        2 => render_public_choice(frame, area, app),
        3 => render_subdomain_input(frame, area, app),
        4 => render_confirm(frame, area, app),
        _ => {}
    }
}

/// Step 0: Choose generate vs custom password
fn render_password_choice(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Welcome to NomadFlow!")
        .style(Style::default().fg(Color::Cyan).bold());
    frame.render_widget(title, chunks[0]);

    let label = Paragraph::new("Set a password to secure your server:")
        .style(Style::default().bold());
    frame.render_widget(label, chunks[1]);

    let options = [
        "Generate a password (recommended)",
        "Enter my own password",
    ];

    for (i, opt) in options.iter().enumerate() {
        let style = if app.selected_index == i {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default()
        };
        let marker = if app.selected_index == i { "> " } else { "  " };
        let line = Paragraph::new(format!("{marker}{opt}")).style(style);
        frame.render_widget(line, chunks[3 + i]);
    }
}

/// Step 1: Custom password text input
fn render_password_input(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Enter your password:").style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let label = "Password: ";
    let input_display = if app.input_text.is_empty() {
        Line::from(vec![
            Span::raw(label),
            Span::styled("my-secret", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::raw(label),
            Span::raw(&app.input_text),
        ])
    };

    let input = Paragraph::new(input_display);
    frame.render_widget(input, chunks[1]);

    let cursor_x = chunks[1].x + label.len() as u16 + app.input_cursor as u16;
    let cursor_y = chunks[1].y;
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}

/// Step 2: Public tunnel mode? (y/n)
fn render_public_choice(frame: &mut Frame, area: Rect, _app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Will you use public tunnel mode? (y/n)")
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let hint = Paragraph::new("This exposes your server over the internet via a tunnel URL.")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[2]);
}

/// Step 3: Fixed subdomain? y/n (only if public=y)
fn render_subdomain_input(frame: &mut Frame, area: Rect, app: &App) {
    let base_domain = app
        .settings
        .tunnel
        .relay_host
        .strip_prefix("relay.")
        .unwrap_or(&app.settings.tunnel.relay_host);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Use a fixed subdomain for a stable public URL? (y/n)")
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let sub_line = Paragraph::new(Line::from(vec![
        Span::raw("  Your subdomain: "),
        Span::styled(&app.setup_subdomain, Style::default().fg(Color::Cyan).bold()),
    ]));
    frame.render_widget(sub_line, chunks[1]);

    let url_preview = format!(
        "  -> https://{}.tunnel.{base_domain}",
        app.setup_subdomain
    );
    let url_line =
        Paragraph::new(url_preview).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(url_line, chunks[2]);

    let hint = Paragraph::new("y: use this fixed subdomain  n: random URL each time")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[4]);
}

/// Step 4: Confirmation summary
fn render_confirm(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Configuration summary:")
        .style(Style::default().fg(Color::Cyan).bold());
    frame.render_widget(title, chunks[0]);

    let password_line = Paragraph::new(Line::from(vec![
        Span::raw("  Password: "),
        Span::styled(&app.setup_secret, Style::default().fg(Color::Yellow)),
    ]));
    frame.render_widget(password_line, chunks[1]);

    let public_str = if app.setup_public { "yes" } else { "no" };
    let public_line = Paragraph::new(Line::from(vec![
        Span::raw("  Public mode: "),
        Span::styled(public_str, Style::default().bold()),
    ]));
    frame.render_widget(public_line, chunks[2]);

    if app.setup_public {
        let sub = if app.setup_subdomain.is_empty() {
            "(random)".to_string()
        } else {
            app.setup_subdomain.clone()
        };
        let sub_line = Paragraph::new(Line::from(vec![
            Span::raw("  Subdomain: "),
            Span::styled(sub, Style::default().bold()),
        ]));
        frame.render_widget(sub_line, chunks[3]);
    }

    let config_path = app.settings.config_file();
    let path_line = Paragraph::new(Line::from(vec![
        Span::raw("  Config: "),
        Span::styled(
            config_path.display().to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    frame.render_widget(path_line, chunks[4]);

    let confirm = Paragraph::new("Save and continue? (y/n)").style(Style::default().bold());
    frame.render_widget(confirm, chunks[5]);
}
