use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{ActiveModal, App};

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(3),  // Title bar
        Constraint::Min(8),    // Role list
        Constraint::Length(7), // Detail panel
        Constraint::Length(1), // Status bar
    ])
    .split(size);

    // Title bar
    render_title_bar(f, chunks[0], app);

    // Role list
    crate::ui::role_list::render(f, chunks[1], app);

    // Detail panel
    crate::ui::detail::render(f, chunks[2], app);

    // Status bar
    crate::ui::status_bar::render(f, chunks[3], app);

    // Modal overlay
    if app.modal != ActiveModal::None {
        crate::ui::modals::render(f, size, app);
    }
}

fn render_title_bar(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let title_chunks = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " Azure PIM TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  ·  "),
        Span::styled(
            &app.user_display,
            Style::default().fg(Color::White),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));

    let loading_text = if app.loading {
        "Loading…"
    } else {
        ""
    };
    let status = Paragraph::new(Line::from(Span::styled(
        loading_text,
        Style::default().fg(Color::Yellow),
    )))
    .alignment(ratatui::layout::Alignment::Right)
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));

    f.render_widget(title, title_chunks[0]);
    f.render_widget(status, title_chunks[1]);
}
