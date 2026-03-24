use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{ActiveModal, App, ModalField};

/// Returns true if a refresh should be triggered
pub fn handle_key(app: &mut App, key: KeyEvent) -> EventAction {
    // Global keys
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return EventAction::None;
    }

    // Detail panel scroll
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => {
                app.detail_scroll = app.detail_scroll.saturating_add(5);
                return EventAction::None;
            }
            KeyCode::Char('u') => {
                app.detail_scroll = app.detail_scroll.saturating_sub(5);
                return EventAction::None;
            }
            _ => {}
        }
    }

    // Filter mode input
    if app.filtering {
        return handle_filter_key(app, key);
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            EventAction::None
        }

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_selection(1);
            EventAction::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_selection(-1);
            EventAction::None
        }
        KeyCode::Char('g') => {
            app.select_first();
            EventAction::None
        }
        KeyCode::Char('G') => {
            app.select_last();
            EventAction::None
        }

        // Activate
        KeyCode::Char('a') | KeyCode::Enter => {
            if let Some(idx) = app.selected_role_index() {
                let role = &app.active_roles()[idx];
                if role.status.is_eligible() {
                    app.modal = ActiveModal::Activate {
                        role_index: idx,
                        justification: app.config.default_justification.clone(),
                        duration: app.config.default_duration_hours.to_string(),
                        focused_field: ModalField::Justification,
                    };
                }
            }
            EventAction::None
        }

        // Deactivate
        KeyCode::Char('d') => {
            if let Some(idx) = app.selected_role_index() {
                let role = &app.active_roles()[idx];
                if role.status.is_active() {
                    app.modal = ActiveModal::DeactivateConfirm { role_index: idx };
                }
            }
            EventAction::None
        }

        // Toggle selection
        KeyCode::Char(' ') => {
            app.toggle_selected();
            app.move_selection(1);
            EventAction::None
        }

        // Bulk activate
        KeyCode::Char('A') => {
            let selected = app.selected_indices();
            let eligible: Vec<usize> = selected
                .into_iter()
                .filter(|&i| app.active_roles()[i].status.is_eligible())
                .collect();
            if !eligible.is_empty() {
                app.modal = ActiveModal::BulkActivate {
                    indices: eligible,
                    justification: app.config.default_justification.clone(),
                    duration: app.config.default_duration_hours.to_string(),
                    focused_field: ModalField::Justification,
                };
            }
            EventAction::None
        }

        // Refresh
        KeyCode::Char('r') | KeyCode::F(5) => EventAction::Refresh,

        // Filter
        KeyCode::Char('/') => {
            app.filtering = true;
            app.filter_text.clear();
            EventAction::None
        }

        // Switch pane
        KeyCode::Tab => {
            app.switch_pane();
            EventAction::PaneSwitch
        }

        // View cycle
        KeyCode::Char('v') => {
            app.view_filter = app.view_filter.cycle();
            app.update_filtered_indices();
            EventAction::None
        }

        // Help
        KeyCode::Char('?') => {
            app.modal = ActiveModal::Help;
            EventAction::None
        }

        _ => EventAction::None,
    }
}

fn handle_filter_key(app: &mut App, key: KeyEvent) -> EventAction {
    match key.code {
        KeyCode::Esc => {
            app.filtering = false;
            app.filter_text.clear();
            app.update_filtered_indices();
            EventAction::None
        }
        KeyCode::Enter => {
            app.filtering = false;
            EventAction::None
        }
        KeyCode::Char(c) => {
            app.filter_text.push(c);
            app.update_filtered_indices();
            EventAction::None
        }
        KeyCode::Backspace => {
            app.filter_text.pop();
            app.update_filtered_indices();
            EventAction::None
        }
        _ => EventAction::None,
    }
}

pub enum EventAction {
    None,
    Refresh,
    PaneSwitch,
}
