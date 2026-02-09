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
        let hint = Paragraph::new("Press q to quit")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[1]);
        return;
    }

    let text = Paragraph::new("Preparing tmux session...");
    frame.render_widget(text, area);
}
