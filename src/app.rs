use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use crate::client::auth::SubscriptionInfo;
use crate::client::models::{PimRole, RoleStatus};
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct AuthData {
    pub token: String,
    pub principal_id: String,
    pub user_display: String,
    pub subscriptions: Vec<SubscriptionInfo>,
}

#[derive(Debug)]
pub enum BgEvent {
    RolesLoaded(Result<Vec<PimRole>, String>),
    ActivationResult {
        index: usize,
        result: Result<(), String>,
    },
    DeactivationResult {
        index: usize,
        result: Result<(), String>,
    },
    AuthReady(Result<AuthData, String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewFilter {
    All,
    Eligible,
    Active,
}

impl ViewFilter {
    pub fn label(&self) -> &str {
        match self {
            ViewFilter::All => "All",
            ViewFilter::Eligible => "Eligible",
            ViewFilter::Active => "Active",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            ViewFilter::All => ViewFilter::Eligible,
            ViewFilter::Eligible => ViewFilter::Active,
            ViewFilter::Active => ViewFilter::All,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveModal {
    None,
    Activate {
        role_index: usize,
        justification: String,
        duration: String,
        focused_field: ModalField,
    },
    BulkActivate {
        indices: Vec<usize>,
        justification: String,
        duration: String,
        focused_field: ModalField,
    },
    DeactivateConfirm {
        role_index: usize,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalField {
    Justification,
    Duration,
}

pub struct App {
    pub roles: Vec<PimRole>,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
    pub view_filter: ViewFilter,
    pub filter_text: String,
    pub filtering: bool,
    pub modal: ActiveModal,
    pub status_message: String,
    pub user_display: String,
    pub loading: bool,
    pub should_quit: bool,
    pub bg_tx: mpsc::UnboundedSender<BgEvent>,
    pub bg_rx: mpsc::UnboundedReceiver<BgEvent>,
    pub config: Config,
    pub auth: Option<Arc<AuthData>>,
    pub last_refresh: Option<DateTime<Utc>>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let (bg_tx, bg_rx) = mpsc::unbounded_channel();
        Self {
            roles: Vec::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            view_filter: ViewFilter::All,
            filter_text: String::new(),
            filtering: false,
            modal: ActiveModal::None,
            status_message: "Authenticating...".to_string(),
            user_display: String::new(),
            loading: true,
            should_quit: false,
            bg_tx,
            bg_rx,
            config,
            auth: None,
            last_refresh: None,
        }
    }

    pub fn update_filtered_indices(&mut self) {
        self.filtered_indices = self
            .roles
            .iter()
            .enumerate()
            .filter(|(_, role)| {
                let matches_filter = match self.view_filter {
                    ViewFilter::All => true,
                    ViewFilter::Eligible => role.status.is_eligible(),
                    ViewFilter::Active => role.status.is_active(),
                };
                let matches_search = if self.filter_text.is_empty() {
                    true
                } else {
                    let lower = self.filter_text.to_lowercase();
                    role.role_name.to_lowercase().contains(&lower)
                        || role.scope_display_name.to_lowercase().contains(&lower)
                };
                matches_filter && matches_search
            })
            .map(|(i, _)| i)
            .collect();

        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }

    pub fn selected_role(&self) -> Option<&PimRole> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.roles.get(i))
    }

    pub fn selected_role_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected).copied()
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let len = self.filtered_indices.len() as i32;
        let new = (self.selected as i32 + delta).rem_euclid(len);
        self.selected = new as usize;
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = self.filtered_indices.len() - 1;
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(&i) = self.filtered_indices.get(self.selected) {
            self.roles[i].selected = !self.roles[i].selected;
        }
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.roles
            .iter()
            .enumerate()
            .filter(|(_, r)| r.selected)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn eligible_count(&self) -> usize {
        self.roles.iter().filter(|r| r.status.is_eligible()).count()
    }

    pub fn active_count(&self) -> usize {
        self.roles.iter().filter(|r| r.status.is_active()).count()
    }

    pub fn handle_bg_event(&mut self, event: BgEvent) {
        match event {
            BgEvent::AuthReady(Ok(auth_data)) => {
                self.user_display = auth_data.user_display.clone();
                self.auth = Some(Arc::new(auth_data));
                self.status_message = "Fetching roles...".to_string();
                self.spawn_fetch_roles();
            }
            BgEvent::AuthReady(Err(e)) => {
                self.loading = false;
                self.status_message = format!("Auth failed: {e}");
            }
            BgEvent::RolesLoaded(Ok(roles)) => {
                self.roles = roles;
                self.update_filtered_indices();
                self.loading = false;
                self.last_refresh = Some(Utc::now());
                self.status_message = "Ready".to_string();
            }
            BgEvent::RolesLoaded(Err(e)) => {
                self.loading = false;
                self.status_message = format!("Failed to load roles: {e}");
            }
            BgEvent::ActivationResult { index, result } => {
                match result {
                    Ok(()) => {
                        if let Some(role) = self.roles.get_mut(index) {
                            role.status = RoleStatus::Activating;
                            role.selected = false;
                        }
                        self.status_message = "Activation requested - refreshing...".to_string();
                        self.spawn_fetch_roles();
                    }
                    Err(e) => {
                        if let Some(role) = self.roles.get_mut(index) {
                            role.status = RoleStatus::Failed(e.clone());
                        }
                        self.status_message = format!("Activation failed: {e}");
                    }
                }
                self.update_filtered_indices();
            }
            BgEvent::DeactivationResult { index, result } => {
                match result {
                    Ok(()) => {
                        if let Some(role) = self.roles.get_mut(index) {
                            role.status = RoleStatus::Eligible;
                            role.selected = false;
                        }
                        self.status_message = "Deactivated - refreshing...".to_string();
                        self.spawn_fetch_roles();
                    }
                    Err(e) => {
                        self.status_message = format!("Deactivation failed: {e}");
                    }
                }
                self.update_filtered_indices();
            }
        }
    }

    pub fn spawn_fetch_roles(&self) {
        // The actual spawning happens in main.rs using the client
        // This is a signal method - main loop checks this
    }
}
