use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Paragraph},
};

use crate::app::App;
use crate::tmux_local;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.loading {
        let text = Paragraph::new("Loading features...");
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

    let repo_name = app.repo.as_ref().map(|r| r.name.as_str()).unwrap_or("");
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let title = Paragraph::new(format!("Select a feature ({repo_name}):"))
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let mut items: Vec<ListItem> = app
        .features
        .iter()
        .enumerate()
        .map(|(i, cf)| {
            let f = &cf.feature;
            let is_idle = tmux_local::is_shell_idle_str(cf.pane_command.as_deref());
            let process_info = match &cf.pane_command {
                Some(_) if is_idle => "  idle".to_string(),
                Some(cmd) => format!("  ● {cmd} running"),
                None => String::new(),
            };
            let prefix = if f.is_main { "⌂ " } else { "" };
            let suffix = if f.is_main { "  [source]" } else { "" };
            let label = format!("{prefix}{}  {}{process_info}{suffix}", f.name, f.branch);

            let item = ListItem::new(label);
            if i == app.selected_index {
                item.style(Style::default().fg(Color::Cyan).bold())
            } else {
                item
            }
        })
        .collect();

    // Add "Create" option
    let create_idx = app.features.len();
    let create_item = ListItem::new("+ Create a feature");
    let create_item = if app.selected_index == create_idx {
        create_item.style(Style::default().fg(Color::Cyan).bold())
    } else {
        create_item
    };
    items.push(create_item);

    let list = List::new(items);
    frame.render_widget(list, chunks[1]);
}
