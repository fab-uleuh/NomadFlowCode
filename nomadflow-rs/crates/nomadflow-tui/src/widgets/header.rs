use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub fn render(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title_alignment(Alignment::Left);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("NomadFlow", Style::default().fg(Color::Cyan).bold()),
        Span::raw("  "),
        Span::styled("v0.1", Style::default().fg(Color::DarkGray)),
    ]))
    .block(block);

    frame.render_widget(header, area);
}
