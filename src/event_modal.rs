use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{ActiveModal, App, ModalField};

pub fn handle_modal_key(app: &mut App, key: KeyEvent) -> Option<ModalAction> {
    match &mut app.modal {
        ActiveModal::Activate {
            justification,
            duration,
            focused_field,
            ..
        }
        | ActiveModal::BulkActivate {
            justification,
            duration,
            focused_field,
            ..
        } => match key.code {
            KeyCode::Esc => {
                app.modal = ActiveModal::None;
                None
            }
            KeyCode::Tab | KeyCode::BackTab => {
                *focused_field = match focused_field {
                    ModalField::Justification => ModalField::Duration,
                    ModalField::Duration => ModalField::Justification,
                };
                None
            }
            KeyCode::Enter => {
                let justification = justification.clone();
                let duration = duration.clone();
                let dur_hours: u32 = duration.parse().unwrap_or(app.config.default_duration_hours);

                match &app.modal {
                    ActiveModal::Activate { role_index, .. } => {
                        let idx = *role_index;
                        app.modal = ActiveModal::None;
                        Some(ModalAction::Activate {
                            indices: vec![idx],
                            justification,
                            duration_hours: dur_hours,
                        })
                    }
                    ActiveModal::BulkActivate { indices, .. } => {
                        let indices = indices.clone();
                        app.modal = ActiveModal::None;
                        Some(ModalAction::Activate {
                            indices,
                            justification,
                            duration_hours: dur_hours,
                        })
                    }
                    _ => None,
                }
            }
            KeyCode::Char(c) => {
                match focused_field {
                    ModalField::Justification => justification.push(c),
                    ModalField::Duration => {
                        if c.is_ascii_digit() {
                            duration.push(c);
                        }
                    }
                }
                None
            }
            KeyCode::Backspace => {
                match focused_field {
                    ModalField::Justification => {
                        justification.pop();
                    }
                    ModalField::Duration => {
                        duration.pop();
                    }
                }
                None
            }
            _ => None,
        },
        ActiveModal::DeactivateConfirm { role_index } => match key.code {
            KeyCode::Enter => {
                let idx = *role_index;
                app.modal = ActiveModal::None;
                Some(ModalAction::Deactivate { index: idx })
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => {
                app.modal = ActiveModal::None;
                None
            }
            _ => None,
        },
        ActiveModal::Help => {
            app.modal = ActiveModal::None;
            None
        }
        ActiveModal::None => None,
    }
}

pub enum ModalAction {
    Activate {
        indices: Vec<usize>,
        justification: String,
        duration_hours: u32,
    },
    Deactivate {
        index: usize,
    },
}
