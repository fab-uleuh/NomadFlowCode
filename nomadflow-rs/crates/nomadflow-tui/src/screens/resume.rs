use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Paragraph},
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let label = [
        app.cli_state.last_repo.as_deref().and_then(|r| {
            std::path::Path::new(r)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
        }),
        app.cli_state.last_feature.clone(),
    ]
    .iter()
    .filter_map(|x| x.clone())
    .collect::<Vec<_>>()
    .join(":");

    let server_name = app.cli_state.last_server.as_deref().unwrap_or("localhost");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    let title = Paragraph::new("Resume previous session?")
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let info = Paragraph::new(Line::from(vec![
        Span::raw("Last session: "),
        Span::styled(&label, Style::default().fg(Color::Yellow)),
        Span::raw(" on "),
        Span::raw(server_name),
    ]))
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(info, chunks[1]);

    let options = [
        "Yes, attach tmux session",
        "No, choose another session",
    ];

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let marker = if i == app.selected_index { "> " } else { "  " };
            let text = format!("{marker}{label}");
            let item = ListItem::new(text);
            if i == app.selected_index {
                item.style(Style::default().fg(Color::Cyan).bold())
            } else {
                item
            }
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, chunks[2]);
}
