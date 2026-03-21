use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect) {
    let help_area = centered_rect(60, 70, area);

    let lines = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("j/↓", "Move down"),
        help_line("k/↑", "Move up"),
        help_line("g", "Go to first"),
        help_line("G", "Go to last"),
        help_line("a / Enter", "Activate role"),
        help_line("d", "Deactivate role"),
        help_line("Space", "Toggle selection"),
        help_line("A (Shift)", "Bulk activate selected"),
        help_line("r / F5", "Refresh roles"),
        help_line("/", "Filter roles"),
        help_line("Esc", "Clear filter / close modal"),
        help_line("v", "Cycle view: all/eligible/active"),
        help_line("?", "Toggle help"),
        help_line("q / Ctrl+C", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    f.render_widget(Clear, help_area);
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(paragraph, help_area);
}

fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{key:>14}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::raw(desc),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let x = area.width.saturating_sub(area.width * percent_x / 100) / 2;
    let y = area.height.saturating_sub(area.height * percent_y / 100) / 2;
    let w = area.width * percent_x / 100;
    let h = area.height * percent_y / 100;
    Rect::new(area.x + x, area.y + y, w, h)
}
