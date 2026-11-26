use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::app::{ActiveModal, App};
use crate::ui::{layout, modals, role_list};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);
const DETAIL_SCROLL_STEP: usize = 3;

pub fn handle_mouse(app: &mut App, mouse: MouseEvent, size: Rect) {
    let pos = Position::new(mouse.column, mouse.row);

    if app.modal != ActiveModal::None {
        handle_modal_mouse(app, mouse.kind, pos, size);
        return;
    }

    let areas = layout::areas(size);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(pane) = layout::pane_tab_at(areas.tabs, pos) {
                if pane != app.active_pane {
                    app.switch_pane();
                    app.last_click = None;
                }
                return;
            }

            let Some(row) = role_list::row_at(areas.role_list, pos) else {
                return;
            };
            if row >= app.filtered_indices.len() {
                return;
            }

            app.select(row);

            if role_list::in_selector_column(areas.role_list, pos) {
                app.toggle_selected();
                app.last_click = None;
                return;
            }

            let now = Instant::now();
            let is_double = matches!(
                app.last_click,
                Some((at, r)) if r == row && now.duration_since(at) <= DOUBLE_CLICK
            );
            if is_double {
                app.last_click = None;
                app.open_activate_modal();
            } else {
                app.last_click = Some((now, row));
            }
        }
        MouseEventKind::ScrollDown => {
            if areas.role_list.contains(pos) {
                app.select(app.selected.saturating_add(1));
            } else if areas.detail.contains(pos) {
                app.detail_scroll = app.detail_scroll.saturating_add(DETAIL_SCROLL_STEP);
            }
        }
        MouseEventKind::ScrollUp => {
            if areas.role_list.contains(pos) {
                app.select(app.selected.saturating_sub(1));
            } else if areas.detail.contains(pos) {
                app.detail_scroll = app.detail_scroll.saturating_sub(DETAIL_SCROLL_STEP);
            }
        }
        _ => {}
    }
}

fn handle_modal_mouse(app: &mut App, kind: MouseEventKind, pos: Position, size: Rect) {
    if !matches!(kind, MouseEventKind::Down(MouseButton::Left)) {
        return;
    }

    match &mut app.modal {
        ActiveModal::Activate { focused_field, .. } => {
            if let Some(field) = modals::activate_field_at(size, false, pos) {
                *focused_field = field;
            }
        }
        ActiveModal::BulkActivate { focused_field, .. } => {
            if let Some(field) = modals::activate_field_at(size, true, pos) {
                *focused_field = field;
            }
        }
        ActiveModal::Help => app.modal = ActiveModal::None,
        ActiveModal::DeactivateConfirm { .. } | ActiveModal::None => {}
    }
}
