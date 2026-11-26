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

fn visible_rows(area: Rect) -> usize {
    usize::from(area.height.saturating_sub(3)).max(1)
}

pub fn scroll_offset(previous: usize, selected: usize, len: usize, visible: usize) -> usize {
    let offset = if selected < previous {
        selected
    } else if selected >= previous + visible {
        selected + 1 - visible
    } else {
        previous
    };
    offset.min(len.saturating_sub(visible))
}

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

    let visible = visible_rows(area);
    let offset = scroll_offset(
        app.list_offset.get(),
        app.selected,
        app.filtered_indices.len(),
        visible,
    );
    app.list_offset.set(offset);

    let rows: Vec<Row> = app
        .filtered_indices
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_follows_selection_down_and_up() {
        assert_eq!(scroll_offset(0, 4, 20, 5), 0);
        assert_eq!(scroll_offset(0, 5, 20, 5), 1);
        assert_eq!(scroll_offset(10, 3, 20, 5), 3);
        assert_eq!(scroll_offset(2, 4, 20, 5), 2);
    }

    #[test]
    fn offset_clamps_when_list_shrinks() {
        assert_eq!(scroll_offset(15, 2, 4, 5), 0);
        assert_eq!(scroll_offset(15, 9, 10, 5), 5);
    }

    #[test]
    fn row_at_maps_below_header_inside_borders() {
        let area = Rect::new(0, 3, 40, 10);
        assert_eq!(row_at(area, Position::new(5, 4)), None);
        assert_eq!(row_at(area, Position::new(5, 5)), Some(0));
        assert_eq!(row_at(area, Position::new(5, 11)), Some(6));
        assert_eq!(row_at(area, Position::new(5, 12)), None);
        assert_eq!(row_at(area, Position::new(0, 6)), None);
    }

    #[test]
    fn scope_colors_differ_for_adjacent_scopes() {
        let role = |scope: &str| PimRole {
            eligibility_id: String::new(),
            role_definition_id: String::new(),
            principal_id: String::new(),
            scope: scope.to_string(),
            role_name: String::new(),
            scope_display_name: String::new(),
            role_type: RoleType::Resource,
            group_id: None,
            status: RoleStatus::Eligible,
            selected: false,
        };
        let roles = vec![role("/a"), role("/a"), role("/b"), role("/c")];
        let colors = scope_colors(&roles);
        assert_eq!(colors.len(), 3);
        assert_ne!(colors["/a"], colors["/b"]);
        assert_ne!(colors["/b"], colors["/c"]);
    }
}
