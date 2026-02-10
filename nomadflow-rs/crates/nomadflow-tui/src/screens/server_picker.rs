use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Paragraph},
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.health_checking {
        let spinner = Paragraph::new("Checking servers...");
        frame.render_widget(spinner, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let title = Paragraph::new("Select a server:")
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    let mut items: Vec<ListItem> = app
        .servers
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let health = app
                .health_map
                .get(&s.id)
                .map(|ok| if *ok { " ✓" } else { " ✗" })
                .unwrap_or("");
            let label = format!(
                "{} ({}){health}",
                s.name,
                s.api_url.as_deref().unwrap_or("no url")
            );

            let item = ListItem::new(label);
            if i == app.selected_index {
                item.style(Style::default().fg(Color::Cyan).bold())
            } else {
                item
            }
        })
        .collect();

    // "+ Add server" option
    let add_item = ListItem::new("+ Add server");
    let add_idx = app.servers.len();
    items.push(if add_idx == app.selected_index {
        add_item.style(Style::default().fg(Color::Green).bold())
    } else {
        add_item.style(Style::default().fg(Color::Green))
    });

    let list = List::new(items).highlight_symbol("> ");
    frame.render_widget(list, chunks[1]);
}
