use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(40),
    ])
    .split(area);

    let is_loading = app.active_loading();
    let status_msg = app.active_status();

    // Status message
    let status_color = if is_loading {
        Color::Yellow
    } else if status_msg.starts_with("Failed") || status_msg.starts_with("Auth failed") {
        Color::Red
    } else {
        Color::Green
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            if is_loading { "⟳ " } else { "● " },
            Style::default().fg(status_color),
        ),
        Span::styled(status_msg, Style::default().fg(status_color)),
    ]));
    f.render_widget(status, chunks[0]);

    // Counts
    let counts = Paragraph::new(Line::from(vec![
        Span::raw(format!(
            "{} eligible, {} active",
            app.eligible_count(),
            app.active_count()
        )),
        Span::styled(
            format!(" [{}]", app.view_filter.label()),
            Style::default().fg(Color::Cyan),
        ),
    ]));
    f.render_widget(counts, chunks[1]);

    // Key hints
    let hints = Paragraph::new(Line::from(vec![
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":pane "),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":activate "),
        Span::styled(
            "d",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":deactivate "),
        Span::styled(
            "r",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":refresh "),
        Span::styled(
            "?",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(":help"),
    ]))
    .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(hints, chunks[2]);
}
