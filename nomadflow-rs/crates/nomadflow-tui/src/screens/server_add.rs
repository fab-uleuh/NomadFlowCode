use ratatui::{
    prelude::*,
    widgets::Paragraph,
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
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

    match app.server_add_step {
        3 => render_confirm(frame, area, app),
        _ => render_input(frame, area, app),
    }
}

fn render_confirm(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(area);

    let confirm = Paragraph::new(Line::from(vec![
        Span::raw("Add server "),
        Span::styled(&app.server_add_name, Style::default().fg(Color::Cyan).bold()),
        Span::raw(" at "),
        Span::styled(&app.server_add_url, Style::default().bold()),
        Span::raw("? (y/n)"),
    ]));
    frame.render_widget(confirm, chunks[0]);
}

fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let (title, label, placeholder) = match app.server_add_step {
        0 => ("Add a new server:", "Server name: ", "my-server"),
        1 => ("Add a new server:", "API URL: ", "http://host:8080"),
        2 => ("Add a new server:", "Auth token: ", "(optional, Enter to skip)"),
        _ => unreachable!(),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1)])
        .split(area);

    let title_widget = Paragraph::new(title).style(Style::default().bold());
    frame.render_widget(title_widget, chunks[0]);

    let input_display = if app.input_text.is_empty() {
        Line::from(vec![
            Span::raw(label),
            Span::styled(placeholder, Style::default().fg(Color::DarkGray)),
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
