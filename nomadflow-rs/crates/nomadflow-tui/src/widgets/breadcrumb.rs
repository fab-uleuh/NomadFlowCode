use ratatui::{
    prelude::*,
    widgets::Paragraph,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    server: Option<&str>,
    repo: Option<&str>,
    feature: Option<&str>,
) {
    let parts: Vec<&str> = [server, repo, feature]
        .iter()
        .filter_map(|x| *x)
        .collect();

    if parts.is_empty() {
        return;
    }

    let mut spans = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" > ", Style::default().fg(Color::DarkGray)));
        }
        let color = if i == parts.len() - 1 {
            Color::Yellow
        } else {
            Color::White
        };
        spans.push(Span::styled(*part, Style::default().fg(color)));
    }

    let breadcrumb = Paragraph::new(Line::from(spans));
    frame.render_widget(breadcrumb, area);
}
