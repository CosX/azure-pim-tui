use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{ActiveModal, App, ModalField};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    match &app.modal {
        ActiveModal::Activate {
            role_index,
            justification,
            duration,
            focused_field,
        } => {
            let role = &app.roles[*role_index];
            render_activate_modal(
                f,
                area,
                &role.role_name,
                &role.scope_display_name,
                justification,
                duration,
                focused_field,
                false,
                1,
            );
        }
        ActiveModal::BulkActivate {
            indices,
            justification,
            duration,
            focused_field,
        } => {
            render_activate_modal(
                f,
                area,
                "Multiple Roles",
                "",
                justification,
                duration,
                focused_field,
                true,
                indices.len(),
            );
        }
        ActiveModal::DeactivateConfirm { role_index } => {
            let role = &app.roles[*role_index];
            render_deactivate_modal(f, area, &role.role_name, &role.scope_display_name);
        }
        ActiveModal::Help => {
            crate::ui::help::render(f, area);
        }
        ActiveModal::None => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn render_activate_modal(
    f: &mut Frame,
    area: Rect,
    role_name: &str,
    scope_name: &str,
    justification: &str,
    duration: &str,
    focused: &ModalField,
    is_bulk: bool,
    count: usize,
) {
    let modal_area = centered_rect(50, 40, area);
    f.render_widget(Clear, modal_area);

    let title = if is_bulk {
        format!(" Activate {count} Roles ")
    } else {
        " Activate Role ".to_string()
    };

    let just_style = if *focused == ModalField::Justification {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let dur_style = if *focused == ModalField::Duration {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let mut lines = vec![];

    if !is_bulk {
        lines.push(Line::from(vec![
            Span::styled("Role: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(role_name),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Scope: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(scope_name),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            format!("Activating {count} selected roles"),
            Style::default().add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "Justification: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "[{justification}{}]",
                if *focused == ModalField::Justification {
                    "▏"
                } else {
                    ""
                }
            ),
            just_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            "Duration (hrs): ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "[{duration}{}]",
                if *focused == ModalField::Duration {
                    "▏"
                } else {
                    ""
                }
            ),
            dur_style,
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Tab: next field  Enter: activate  Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(paragraph, modal_area);
}

fn render_deactivate_modal(f: &mut Frame, area: Rect, role_name: &str, scope_name: &str) {
    let modal_area = centered_rect(45, 25, area);
    f.render_widget(Clear, modal_area);

    let lines = vec![
        Line::from(vec![
            Span::styled("Role: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(role_name),
        ]),
        Line::from(vec![
            Span::styled("Scope: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(scope_name),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Deactivate this role?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter: confirm  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Deactivate Role ")
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(paragraph, modal_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let x = area.width.saturating_sub(area.width * percent_x / 100) / 2;
    let y = area.height.saturating_sub(area.height * percent_y / 100) / 2;
    let w = area.width * percent_x / 100;
    let h = area.height * percent_y / 100;
    Rect::new(area.x + x, area.y + y, w, h)
}
