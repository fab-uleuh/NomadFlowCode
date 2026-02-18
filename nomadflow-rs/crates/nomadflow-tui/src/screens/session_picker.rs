use ratatui::{
    prelude::*,
    widgets::{List, ListItem, Paragraph},
};

use nomadflow_core::config::Settings;
use nomadflow_core::models::AgentStateKind;

use crate::app::App;
use crate::tmux_local;

/// A selectable item in the session picker list.
#[derive(Debug, Clone)]
pub struct SelectableItem {
    pub label: String,
    /// Window name to attach to, or None for the "Browse repos..." action.
    pub window_name: Option<String>,
    /// Group header to render above this item (if this is the first in a group).
    pub group_header: Option<String>,
    /// Agent state for this session, if known.
    pub agent_state: Option<AgentStateKind>,
}

/// Build the list of selectable items from settings (queries tmux).
pub fn build_selectable_items_from_settings(settings: &Settings) -> Vec<SelectableItem> {
    let session = &settings.tmux.session;
    let sessions = tmux_local::list_sessions(session);
    if sessions.is_empty() {
        return Vec::new();
    }

    let grouped = tmux_local::grouped_sessions(&sessions);
    let mut items = Vec::new();

    for ((repo, worktree), group_sessions) in &grouped {
        let header = format!("{repo} / {worktree}");
        for (i, s) in group_sessions.iter().enumerate() {
            items.push(SelectableItem {
                label: format!("{}-{}", s.agent_type, s.agent_number),
                window_name: Some(s.window_name.clone()),
                group_header: if i == 0 { Some(header.clone()) } else { None },
                agent_state: None,
            });
        }
    }

    // "Browse repos..." action at the bottom
    items.push(SelectableItem {
        label: "Browse repos...".to_string(),
        window_name: None,
        group_header: None,
        agent_state: None,
    });

    items
}

/// Map an agent state to its display label and ratatui color.
pub fn format_agent_state(state: &AgentStateKind) -> (&'static str, Color) {
    match state {
        AgentStateKind::WaitingForInput => ("waiting", Color::Yellow),
        AgentStateKind::Generating => ("generating", Color::Magenta),
        AgentStateKind::Idle => ("idle", Color::DarkGray),
        AgentStateKind::Done => ("done", Color::Green),
        AgentStateKind::Error => ("error", Color::Red),
        AgentStateKind::Unknown => ("\u{2014}", Color::DarkGray),
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.session_items.is_empty() {
        let text = Paragraph::new("No active sessions \u{2014} use `nomadflow run` to create one");
        frame.render_widget(text, area);
        return;
    }

    // Count actual sessions (all items except "Browse repos...")
    let session_count = app
        .session_items
        .iter()
        .filter(|i| i.window_name.is_some())
        .count();

    let has_confirm = app.show_delete_confirm;
    let has_error = app.error.is_some() && !has_confirm;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),                                            // Title
            Constraint::Min(1),                                               // List
            Constraint::Length(if has_confirm || has_error { 1 } else { 0 }), // Confirmation/error bar
        ])
        .split(area);

    let title = Paragraph::new(format!("Sessions ({session_count} active):"))
        .style(Style::default().bold());
    frame.render_widget(title, chunks[0]);

    // Build visual list items (including non-selectable group headers)
    let mut list_items: Vec<ListItem> = Vec::new();
    let mut selectable_idx = 0;
    let mut selected_visual_idx: usize = 0;

    for item in &app.session_items {
        // Render group header if present
        if let Some(ref header) = item.group_header {
            // Add spacer before group (except first)
            if !list_items.is_empty() {
                list_items.push(ListItem::new(""));
            }
            list_items.push(
                ListItem::new(format!("  {header}"))
                    .style(Style::default().fg(Color::Yellow).bold()),
            );
        }

        let is_selected = selectable_idx == app.selected_index;
        if is_selected {
            selected_visual_idx = list_items.len();
        }

        let marker = if is_selected { "\u{25b8} " } else { "    " };
        let base_style = if is_selected {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default()
        };

        // Add separator before "Browse repos..."
        if item.window_name.is_none() {
            list_items.push(ListItem::new(""));
            list_items.push(
                ListItem::new("  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}")
                    .style(Style::default().fg(Color::DarkGray)),
            );
            // Update selected_visual_idx after separator (selected item comes after it)
            if is_selected {
                selected_visual_idx = list_items.len();
            }
            list_items.push(ListItem::new(format!("{marker}{}", item.label)).style(base_style));
        } else {
            // Session item — show agent state with color
            let state_span = match &item.agent_state {
                Some(state) => {
                    let (label, color) = format_agent_state(state);
                    Span::styled(format!("({label})"), Style::default().fg(color))
                }
                None => Span::styled("(\u{2026})", Style::default().fg(Color::DarkGray)),
            };
            let line = Line::from(vec![
                Span::styled(format!("{marker}{}  ", item.label), base_style),
                state_span,
            ]);
            list_items.push(ListItem::new(line));
        }

        selectable_idx += 1;
    }

    // Scroll to keep selected item visible
    let available_height = chunks[1].height as usize;
    let scroll_offset = if selected_visual_idx >= available_height {
        selected_visual_idx - available_height + 1
    } else {
        0
    };

    let visible_items: Vec<ListItem> = list_items
        .into_iter()
        .skip(scroll_offset)
        .take(available_height)
        .collect();

    let list = List::new(visible_items);
    frame.render_widget(list, chunks[1]);

    // Render delete confirmation bar or error message
    if has_confirm {
        if let Some(ref window) = app.delete_window_name {
            let confirm_text = format!("Close session {window}? [y] confirm  [n/Esc] cancel");
            let confirm =
                Paragraph::new(confirm_text).style(Style::default().fg(Color::Yellow).bold());
            frame.render_widget(confirm, chunks[2]);
        }
    } else if let Some(ref error) = app.error {
        let error_bar = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        frame.render_widget(error_bar, chunks[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_agent_state_all_variants() {
        let cases = vec![
            (AgentStateKind::WaitingForInput, "waiting", Color::Yellow),
            (AgentStateKind::Generating, "generating", Color::Magenta),
            (AgentStateKind::Idle, "idle", Color::DarkGray),
            (AgentStateKind::Done, "done", Color::Green),
            (AgentStateKind::Error, "error", Color::Red),
            (AgentStateKind::Unknown, "\u{2014}", Color::DarkGray),
        ];
        for (state, expected_label, expected_color) in cases {
            let (label, color) = format_agent_state(&state);
            assert_eq!(label, expected_label, "Wrong label for {:?}", state);
            assert_eq!(color, expected_color, "Wrong color for {:?}", state);
        }
    }

    #[test]
    fn test_selectable_item_default_agent_state_none() {
        let item = SelectableItem {
            label: "agent-1".to_string(),
            window_name: Some("myapp:feat:agent-1".to_string()),
            group_header: None,
            agent_state: None,
        };
        assert!(item.agent_state.is_none());
    }

    #[test]
    fn test_selectable_item_with_agent_state() {
        let item = SelectableItem {
            label: "agent-1".to_string(),
            window_name: Some("myapp:feat:agent-1".to_string()),
            group_header: None,
            agent_state: Some(AgentStateKind::WaitingForInput),
        };
        assert_eq!(item.agent_state, Some(AgentStateKind::WaitingForInput));
    }
}
