use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{ActiveModal, App};

pub fn handle_key(app: &mut App, key: KeyEvent) -> EventAction {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return EventAction::None;
    }

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

    if app.filtering {
        return handle_filter_key(app, key);
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            EventAction::None
        }

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

        KeyCode::Char('a') | KeyCode::Enter => {
            app.open_activate_modal();
            EventAction::None
        }

        KeyCode::Char('d') => {
            app.open_deactivate_modal();
            EventAction::None
        }

        KeyCode::Char(' ') => {
            if app.toggle_selected() {
                app.move_selection(1);
            }
            EventAction::None
        }

        KeyCode::Char('A') => {
            app.open_bulk_activate_modal();
            EventAction::None
        }

        KeyCode::Char('r') | KeyCode::F(5) => EventAction::Refresh,

        KeyCode::Char('/') => {
            app.filtering = true;
            app.filter_text.clear();
            EventAction::None
        }

        KeyCode::Tab => {
            app.switch_pane();
            EventAction::PaneSwitch
        }

        KeyCode::Char('v') => {
            app.view_filter = app.view_filter.cycle();
            app.update_filtered_indices();
            EventAction::None
        }

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
