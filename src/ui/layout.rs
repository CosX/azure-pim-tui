use ratatui::{
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{ActiveModal, App, Pane};

/// Screen regions of the main layout. Shared by rendering and mouse hit-testing
/// so both always agree on where things are.
pub struct Areas {
    pub title: Rect,
    pub tabs: Rect,
    pub title_status: Rect,
    pub role_list: Rect,
    pub detail: Rect,
    pub status_bar: Rect,
}

pub fn areas(size: Rect) -> Areas {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Title bar
        Constraint::Min(8),    // Content area
        Constraint::Length(1), // Status bar
    ])
    .split(size);

    let title_chunks = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
    ])
    .split(chunks[0]);

    // Content area: role list (left) | detail panel (right)
    let content = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);

    Areas {
        title: title_chunks[0],
        tabs: title_chunks[1],
        title_status: title_chunks[2],
        role_list: content[0],
        detail: content[1],
        status_bar: chunks[2],
    }
}

/// Leading padding before the first tab label inside the tabs block.
const TAB_LEAD: &str = " ";
/// Gap between tab labels.
const TAB_GAP: &str = "  ";

/// Returns the pane whose tab label is at `pos`, if any.
pub fn pane_tab_at(tabs: Rect, pos: Position) -> Option<Pane> {
    // Labels sit on the single inner row, just inside the left border.
    if pos.y != tabs.y + 1 || pos.x <= tabs.x || pos.x >= tabs.right().saturating_sub(1) {
        return None;
    }
    let col = (pos.x - tabs.x - 1) as usize;
    let res_start = TAB_LEAD.len();
    let res_end = res_start + Pane::Resources.label().len();
    let grp_start = res_end + TAB_GAP.len();
    let grp_end = grp_start + Pane::Groups.label().len();
    if (res_start..res_end).contains(&col) {
        Some(Pane::Resources)
    } else if (grp_start..grp_end).contains(&col) {
        Some(Pane::Groups)
    } else {
        None
    }
}

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();
    let areas = areas(size);

    render_title_bar(f, &areas, app);
    crate::ui::role_list::render(f, areas.role_list, app);
    crate::ui::detail::render(f, areas.detail, app);
    crate::ui::status_bar::render(f, areas.status_bar, app);

    // Modal overlay
    if app.modal != ActiveModal::None {
        crate::ui::modals::render(f, size, app);
    }
}

fn render_title_bar(f: &mut Frame, areas: &Areas, app: &App) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " Azure PIM TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            concat!(" v", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  ·  "),
        Span::styled(&app.user_display, Style::default().fg(Color::White)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    // Pane tabs
    let resources_style = if app.active_pane == Pane::Resources {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let groups_style = if app.active_pane == Pane::Groups {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tabs = Paragraph::new(Line::from(vec![
        Span::raw(TAB_LEAD),
        Span::styled(Pane::Resources.label(), resources_style),
        Span::raw(TAB_GAP),
        Span::styled(Pane::Groups.label(), groups_style),
        Span::styled("  (Tab)", Style::default().fg(Color::DarkGray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    let loading_text = if app.active_loading() {
        "Loading…"
    } else {
        ""
    };
    let status = Paragraph::new(Line::from(Span::styled(
        loading_text,
        Style::default().fg(Color::Yellow),
    )))
    .alignment(ratatui::layout::Alignment::Right)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(title, areas.title);
    f.render_widget(tabs, areas.tabs);
    f.render_widget(status, areas.title_status);
}
