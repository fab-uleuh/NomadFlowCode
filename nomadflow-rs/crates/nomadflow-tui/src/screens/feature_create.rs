use ratatui::{
    prelude::*,
    widgets::Paragraph,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let repo_name = app.repo.as_ref().map(|r| r.name.as_str()).unwrap_or("");

    if let Some(ref err) = app.error {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        let error = Paragraph::new(format!("Error: {err}"))
            .style(Style::default().fg(Color::Red));
        frame.render_widget(error, chunks[0]);
        let hint = Paragraph::new("Press Escape to go back")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[1]);
        return;
    }

    if app.confirm_step {
        // Confirmation step
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(1)])
            .split(area);

        let confirm = Paragraph::new(Line::from(vec![
            Span::raw("Create feature "),
            Span::styled(&app.input_text, Style::default().fg(Color::Cyan).bold()),
            Span::raw(" in "),
            Span::styled(repo_name, Style::default().bold()),
            Span::raw("? (y/n)"),
        ]));
        frame.render_widget(confirm, chunks[0]);
        return;
    }

    // Input step
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(area);

    let title = Paragraph::new(format!("Create a new feature ({repo_name}):"))
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    // Render input field with cursor
    let input_display = if app.input_text.is_empty() {
        Line::from(vec![
            Span::raw("Feature name: "),
            Span::styled("my-feature", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::raw("Feature name: "),
            Span::raw(&app.input_text),
        ])
    };

    let input = Paragraph::new(input_display);
    frame.render_widget(input, chunks[1]);

    // Show cursor position
    let cursor_x = chunks[1].x + 14 + app.input_cursor as u16; // "Feature name: " is 14 chars
    let cursor_y = chunks[1].y;
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}
