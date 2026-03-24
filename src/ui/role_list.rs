use chrono::Utc;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table},
    Frame,
};

use crate::app::{App, Pane};
use crate::client::models::{RoleStatus, RoleType};

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

    let rows: Vec<Row> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &role_idx)| {
            let role = &roles[role_idx];

            let selector = if role.selected { "●" } else { " " };

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
                RoleStatus::Eligible => {
                    Span::styled("Eligible", Style::default().fg(Color::Gray))
                }
                RoleStatus::Active { .. } => {
                    Span::styled("Active", Style::default().fg(Color::Green))
                }
                RoleStatus::Activating => {
                    Span::styled("Activating", Style::default().fg(Color::Yellow))
                }
                RoleStatus::Failed(_) => {
                    Span::styled("Failed", Style::default().fg(Color::Red))
                }
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
                Line::from(col2),
                Line::from(status_span),
                Line::from(expires),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Length(2),
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
