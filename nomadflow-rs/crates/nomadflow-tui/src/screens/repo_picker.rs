use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Paragraph},
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.loading {
        let text = Paragraph::new("Loading repos...");
        frame.render_widget(text, area);
        return;
    }

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

    if app.repos.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        let text = Paragraph::new("No repositories found.");
        frame.render_widget(text, chunks[0]);
        let hint = Paragraph::new("Clone a repo via the mobile app or server first.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[1]);
        return;
    }

    let server_name = app.server.as_ref().map(|s| s.name.as_str()).unwrap_or("");
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let title = Paragraph::new(format!("Select a repo ({server_name}):"))
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let last_repo = app.cli_state.last_repo.as_deref().and_then(|r| {
        std::path::Path::new(r)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
    });

    let items: Vec<ListItem> = app
        .repos
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let is_last = last_repo.as_deref() == Some(&r.name);
            let suffix = if is_last { "  (last used)" } else { "" };
            let label = format!("{}  {}{suffix}", r.name, r.branch);

            let item = ListItem::new(label);
            if i == app.selected_index {
                item.style(Style::default().fg(Color::Cyan).bold())
            } else {
                item
            }
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);
}
