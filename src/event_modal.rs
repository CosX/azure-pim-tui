use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{ActiveModal, App, ModalField, Pane};
use crate::client::models::PimRole;

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
                let dur_hours: u32 = duration
                    .parse()
                    .unwrap_or(app.config.default_duration_hours);

                let keys = match &app.modal {
                    ActiveModal::Activate { role, .. } => vec![role.clone()],
                    ActiveModal::BulkActivate { roles, .. } => roles.clone(),
                    _ => return None,
                };
                let pane = app.active_pane;
                let roles: Vec<PimRole> = keys
                    .iter()
                    .filter_map(|k| app.find_role(pane, k))
                    .filter(|r| r.status.can_activate())
                    .cloned()
                    .collect();
                app.modal = ActiveModal::None;
                (!roles.is_empty()).then_some(ModalAction::Activate {
                    pane,
                    roles,
                    justification,
                    duration_hours: dur_hours,
                })
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
        ActiveModal::DeactivateConfirm { role } => match key.code {
            KeyCode::Enter => {
                let key = role.clone();
                let pane = app.active_pane;
                let role = app.find_role(pane, &key).cloned();
                app.modal = ActiveModal::None;
                role.map(|role| ModalAction::Deactivate { pane, role })
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
        pane: Pane,
        roles: Vec<PimRole>,
        justification: String,
        duration_hours: u32,
    },
    Deactivate {
        pane: Pane,
        role: PimRole,
    },
}
