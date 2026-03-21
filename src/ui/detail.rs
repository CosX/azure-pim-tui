use chrono::Utc;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::client::models::RoleStatus;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let content = if let Some(role) = app.selected_role() {
        let status_line = match &role.status {
            RoleStatus::Active {
                expires_at: Some(exp),
            } => {
                let remaining = *exp - Utc::now();
                let hours = remaining.num_hours();
                let mins = remaining.num_minutes() % 60;
                format!("Active (expires in {hours}h {mins:02}m)")
            }
            _ => role.status.display().to_string(),
        };

        let error_line = if let RoleStatus::Failed(msg) = &role.status {
            vec![Line::from(vec![
                Span::styled("Error: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(msg.clone(), Style::default().fg(Color::Red)),
            ])]
        } else {
            vec![]
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Role: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&role.role_name),
            ]),
            Line::from(vec![
                Span::styled("Scope: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&role.scope),
            ]),
            Line::from(vec![
                Span::styled("Display: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&role.scope_display_name),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    status_line,
                    match &role.status {
                        RoleStatus::Active { .. } => Style::default().fg(Color::Green),
                        RoleStatus::Failed(_) => Style::default().fg(Color::Red),
                        RoleStatus::Activating => Style::default().fg(Color::Yellow),
                        _ => Style::default().fg(Color::Gray),
                    },
                ),
            ]),
        ];
        lines.extend(error_line);
        lines
    } else {
        vec![Line::from(Span::styled(
            "No role selected",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let paragraph = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Details ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(paragraph, area);
}
