use std::collections::HashMap;

use chrono::Utc;
use ratatui::{
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table},
    Frame,
};

use crate::app::{App, Pane};
use crate::client::models::{PimRole, RoleStatus, RoleType};

const SELECTOR_WIDTH: u16 = 2;

pub fn row_at(area: Rect, pos: Position) -> Option<usize> {
    let first_row = area.y + 2;
    let inside_x = pos.x > area.x && pos.x < area.right().saturating_sub(1);
    let inside_y = pos.y >= first_row && pos.y < area.bottom().saturating_sub(1);
    (inside_x && inside_y).then(|| (pos.y - first_row) as usize)
}

pub fn in_selector_column(area: Rect, pos: Position) -> bool {
    pos.x > area.x && pos.x <= area.x + SELECTOR_WIDTH
}

const SCOPE_PALETTE: [Color; 8] = [
    Color::Indexed(109),
    Color::Indexed(144),
    Color::Indexed(139),
    Color::Indexed(108),
    Color::Indexed(180),
    Color::Indexed(146),
    Color::Indexed(138),
    Color::Indexed(152),
];

fn scope_colors(roles: &[PimRole]) -> HashMap<&str, Color> {
    let mut colors = HashMap::new();
    for role in roles {
        let next = SCOPE_PALETTE[colors.len() % SCOPE_PALETTE.len()];
        colors.entry(role.scope.as_str()).or_insert(next);
    }
    colors
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let is_groups = app.active_pane == Pane::Groups;

    let header_cols = if is_groups {
        vec!["", "Group", "Access", "Status", "Expires"]
    } else {
        vec!["", "Role", "Scope", "Status", "Expires"]
    };

    let header = Row::new(header_cols)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(0);

    let roles = app.active_roles();
    let scope_colors = if is_groups {
        HashMap::new()
    } else {
        scope_colors(roles)
    };

    let rows: Vec<Row> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &role_idx)| {
            let role = &roles[role_idx];

            let selector = if role.selected { "●" } else { " " };

            let col2_style = scope_colors
                .get(role.scope.as_str())
                .map(|&c| Style::default().fg(c))
                .unwrap_or_default();

            let (col1, col2) = if is_groups {
                let access = match role.role_type {
                    RoleType::GroupOwner => "Owner",
                    RoleType::GroupMember => "Member",
                    _ => "",
                };
                (role.role_name.clone(), access.to_string())
            } else {
                (role.role_name.clone(), role.scope_display_name.clone())
            };

            let status_span = match &role.status {
                RoleStatus::Eligible => Span::styled("Eligible", Style::default().fg(Color::Gray)),
                RoleStatus::Active { .. } => {
                    Span::styled("Active", Style::default().fg(Color::Green))
                }
                RoleStatus::Activating => {
                    Span::styled("Activating", Style::default().fg(Color::Yellow))
                }
                RoleStatus::Failed(_) => Span::styled("Failed", Style::default().fg(Color::Red)),
            };

            let expires = match &role.status {
                RoleStatus::Active {
                    expires_at: Some(exp),
                } => {
                    let remaining = *exp - Utc::now();
                    if remaining.num_seconds() > 0 {
                        let hours = remaining.num_hours();
                        let mins = remaining.num_minutes() % 60;
                        format!("{hours}h {mins:02}m")
                    } else {
                        "Expired".to_string()
                    }
                }
                _ => String::new(),
            };

            let style = if display_idx == app.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Line::from(selector),
                Line::from(col1),
                Line::from(Span::styled(col2, col2_style)),
                Line::from(status_span),
                Line::from(expires),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Length(SELECTOR_WIDTH),
        ratatui::layout::Constraint::Percentage(35),
        ratatui::layout::Constraint::Percentage(25),
        ratatui::layout::Constraint::Length(12),
        ratatui::layout::Constraint::Length(10),
    ];

    let pane_label = app.active_pane.label();
    let title = if app.filter_text.is_empty() {
        format!(" {pane_label} ")
    } else {
        format!(" {pane_label} [/{}] ", app.filter_text)
    };

    let border_color = if is_groups {
        Color::Magenta
    } else {
        Color::Cyan
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(border_color)),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(table, area);
}
