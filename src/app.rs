use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use azure_core::credentials::TokenCredential;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use crate::client::auth::SubscriptionInfo;
use crate::client::graph_credential::GraphCredential;
use crate::client::models::{
    sort_group_roles, sort_resource_roles, PimRole, RoleFetch, RoleStatus,
};
use crate::config::Config;

pub struct AuthData {
    pub credential: Arc<dyn TokenCredential>,
    pub graph_credential: Arc<GraphCredential>,
    pub principal_id: String,
    pub user_display: String,
    pub subscriptions: Vec<SubscriptionInfo>,
}

pub enum BgEvent {
    RolesLoaded(Result<RoleFetch, String>),
    GroupRolesLoaded(Result<RoleFetch, String>),
    ActivationResult {
        index: usize,
        result: Result<(), String>,
    },
    DeactivationResult {
        index: usize,
        result: Result<(), String>,
    },
    AuthReady(Result<AuthData, String>),
    RolePermissionsLoaded(Result<HashMap<String, Vec<String>>, String>),
    /// Status message from graph credential (device code flow)
    GraphStatus(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane {
    Resources,
    Groups,
}

impl Pane {
    pub fn label(&self) -> &str {
        match self {
            Pane::Resources => "Resources",
            Pane::Groups => "Groups",
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Pane::Resources => Pane::Groups,
            Pane::Groups => Pane::Resources,
        }
    }
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
    // Resource roles
    pub roles: Vec<PimRole>,
    // Group roles
    pub group_roles: Vec<PimRole>,
    pub groups_loaded: bool,
    pub groups_loading: bool,
    pub group_status_message: String,

    // Shared UI state
    pub active_pane: Pane,
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
    pub role_permissions: HashMap<String, Vec<String>>,
    pub detail_scroll: usize,
}

impl App {
    pub fn new(config: Config) -> Self {
        let (bg_tx, bg_rx) = mpsc::unbounded_channel();
        Self {
            roles: Vec::new(),
            group_roles: Vec::new(),
            groups_loaded: false,
            groups_loading: false,
            group_status_message: "Press Tab to load groups".to_string(),
            active_pane: Pane::Resources,
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
            role_permissions: HashMap::new(),
            detail_scroll: 0,
        }
    }

    /// Returns the role list for the active pane
    pub fn active_roles(&self) -> &[PimRole] {
        match self.active_pane {
            Pane::Resources => &self.roles,
            Pane::Groups => &self.group_roles,
        }
    }

    /// Returns a mutable reference to the role list for the active pane
    pub fn active_roles_mut(&mut self) -> &mut Vec<PimRole> {
        match self.active_pane {
            Pane::Resources => &mut self.roles,
            Pane::Groups => &mut self.group_roles,
        }
    }

    /// Returns the status message for the active pane
    pub fn active_status(&self) -> &str {
        match self.active_pane {
            Pane::Resources => &self.status_message,
            Pane::Groups => &self.group_status_message,
        }
    }

    /// Is the active pane loading?
    pub fn active_loading(&self) -> bool {
        match self.active_pane {
            Pane::Resources => self.loading,
            Pane::Groups => self.groups_loading,
        }
    }

    pub fn switch_pane(&mut self) {
        self.active_pane = self.active_pane.toggle();
        self.selected = 0;
        self.detail_scroll = 0;
        self.filter_text.clear();
        self.update_filtered_indices();
    }

    /// Returns true if groups need to be fetched (first visit)
    pub fn needs_group_fetch(&self) -> bool {
        self.active_pane == Pane::Groups && !self.groups_loaded && !self.groups_loading
    }

    pub fn update_filtered_indices(&mut self) {
        let roles = self.active_roles();
        self.filtered_indices = roles
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
            .and_then(|&i| self.active_roles().get(i))
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
        self.detail_scroll = 0;
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
        self.detail_scroll = 0;
    }

    pub fn select_last(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = self.filtered_indices.len() - 1;
        }
        self.detail_scroll = 0;
    }

    /// Toggles selection on the current role and returns `true` if the role is now selected.
    pub fn toggle_selected(&mut self) -> bool {
        if let Some(&i) = self.filtered_indices.get(self.selected) {
            let new_val = !self.active_roles_mut()[i].selected;
            self.active_roles_mut()[i].selected = new_val;
            return new_val;
        }
        false
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.active_roles()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.selected)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn eligible_count(&self) -> usize {
        self.active_roles()
            .iter()
            .filter(|r| r.status.is_eligible())
            .count()
    }

    pub fn active_count(&self) -> usize {
        self.active_roles()
            .iter()
            .filter(|r| r.status.is_active())
            .count()
    }

    /// Owned copy of the highlighted role's identity, for restoring the cursor
    /// after the role list is replaced by a refresh.
    fn selected_identity(&self) -> Option<(String, String, String)> {
        self.selected_role().map(|r| {
            let (rd, scope, group) = r.identity();
            (rd.to_string(), scope.to_string(), group.to_string())
        })
    }

    /// Folds a fetch result into the previous list.
    ///
    /// A refresh is not authoritative about scopes it failed to query, so roles under a
    /// missing scope are carried over rather than dropped, and roles whose active-status
    /// query failed keep the status they already had. Multi-select marks survive too.
    fn merge_fetch(old: &[PimRole], fetch: RoleFetch, pane: Pane) -> Vec<PimRole> {
        let under = |role: &PimRole, scopes: &[String]| {
            scopes.iter().any(|prefix| role.scope.starts_with(prefix))
        };

        let mut roles = fetch.roles;

        let marked: HashSet<(&str, &str, &str)> = old
            .iter()
            .filter(|r| r.selected)
            .map(|r| r.identity())
            .collect();
        let previous: HashMap<(&str, &str, &str), &PimRole> =
            old.iter().map(|r| (r.identity(), r)).collect();

        for role in roles.iter_mut() {
            role.selected = marked.contains(&role.identity());
            if under(role, &fetch.stale_status_scopes) {
                if let Some(prev) = previous.get(&role.identity()) {
                    role.status = prev.status.clone();
                }
            }
        }

        let carried: Vec<PimRole> = {
            let fetched: HashSet<(&str, &str, &str)> = roles.iter().map(|r| r.identity()).collect();
            old.iter()
                .filter(|r| under(r, &fetch.missing_scopes) && !fetched.contains(&r.identity()))
                .cloned()
                .collect()
        };
        roles.extend(carried);

        match pane {
            Pane::Resources => sort_resource_roles(&mut roles),
            Pane::Groups => sort_group_roles(&mut roles),
        }
        roles
    }

    /// Status line for a fetch that only partly succeeded.
    fn partial_status(fetch: &RoleFetch) -> String {
        let missing = fetch.missing_scopes.len();
        let stale = fetch.stale_status_scopes.len();
        if missing > 0 {
            format!("Ready - {missing} scope(s) unavailable, showing last known roles")
        } else {
            format!("Ready - {stale} scope(s) unavailable, active status may be stale")
        }
    }

    /// Rebuilds the filtered view and puts the cursor back on `ident` if it is
    /// still visible, so a refresh does not jump the highlight to another role.
    fn refilter_keeping_cursor(&mut self, ident: Option<(String, String, String)>) {
        self.update_filtered_indices();
        let Some((rd, scope, group)) = ident else {
            return;
        };
        let pos = self.filtered_indices.iter().position(|&i| {
            self.active_roles()
                .get(i)
                .is_some_and(|r| r.identity() == (rd.as_str(), scope.as_str(), group.as_str()))
        });
        if let Some(pos) = pos {
            self.selected = pos;
        }
    }

    pub fn handle_bg_event(&mut self, event: BgEvent) {
        match event {
            BgEvent::AuthReady(Ok(auth_data)) => {
                self.user_display = auth_data.user_display.clone();
                self.auth = Some(Arc::new(auth_data));
                self.status_message = "Fetching roles...".to_string();
            }
            BgEvent::AuthReady(Err(e)) => {
                self.loading = false;
                self.status_message = format!("Auth failed: {e}");
            }
            BgEvent::RolesLoaded(Ok(fetch)) => {
                let ident = (self.active_pane == Pane::Resources)
                    .then(|| self.selected_identity())
                    .flatten();
                self.status_message = if fetch.is_partial() {
                    Self::partial_status(&fetch)
                } else {
                    "Ready".to_string()
                };
                self.roles = Self::merge_fetch(&self.roles, fetch, Pane::Resources);
                if self.active_pane == Pane::Resources {
                    self.refilter_keeping_cursor(ident);
                }
                self.loading = false;
                self.last_refresh = Some(Utc::now());
            }
            BgEvent::RolesLoaded(Err(e)) => {
                self.loading = false;
                self.status_message = format!("Failed to load roles: {e}");
            }
            BgEvent::GroupRolesLoaded(Ok(fetch)) => {
                let ident = (self.active_pane == Pane::Groups)
                    .then(|| self.selected_identity())
                    .flatten();
                self.group_status_message = if fetch.is_partial() {
                    Self::partial_status(&fetch)
                } else {
                    "Ready".to_string()
                };
                self.group_roles = Self::merge_fetch(&self.group_roles, fetch, Pane::Groups);
                self.groups_loaded = true;
                self.groups_loading = false;
                if self.active_pane == Pane::Groups {
                    self.refilter_keeping_cursor(ident);
                }
            }
            BgEvent::GroupRolesLoaded(Err(e)) => {
                self.groups_loading = false;
                self.group_status_message = format!("Failed: {e}");
            }
            BgEvent::ActivationResult { index, result } => {
                let roles = self.active_roles_mut();
                match result {
                    Ok(()) => {
                        if let Some(role) = roles.get_mut(index) {
                            role.status = RoleStatus::Activating;
                            role.selected = false;
                        }
                        match self.active_pane {
                            Pane::Resources => {
                                self.status_message =
                                    "Activation requested - refreshing...".to_string();
                            }
                            Pane::Groups => {
                                self.group_status_message =
                                    "Activation requested - refreshing...".to_string();
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(role) = roles.get_mut(index) {
                            role.status = RoleStatus::Failed(e.clone());
                        }
                        match self.active_pane {
                            Pane::Resources => {
                                self.status_message = format!("Activation failed: {e}");
                            }
                            Pane::Groups => {
                                self.group_status_message = format!("Activation failed: {e}");
                            }
                        }
                    }
                }
                self.update_filtered_indices();
            }
            BgEvent::DeactivationResult { index, result } => {
                let roles = self.active_roles_mut();
                match result {
                    Ok(()) => {
                        if let Some(role) = roles.get_mut(index) {
                            role.status = RoleStatus::Eligible;
                            role.selected = false;
                        }
                        match self.active_pane {
                            Pane::Resources => {
                                self.status_message = "Deactivated - refreshing...".to_string();
                            }
                            Pane::Groups => {
                                self.group_status_message =
                                    "Deactivated - refreshing...".to_string();
                            }
                        }
                    }
                    Err(e) => match self.active_pane {
                        Pane::Resources => {
                            self.status_message = format!("Deactivation failed: {e}");
                        }
                        Pane::Groups => {
                            self.group_status_message = format!("Deactivation failed: {e}");
                        }
                    },
                }
                self.update_filtered_indices();
            }
            BgEvent::RolePermissionsLoaded(Ok(perms)) => {
                self.role_permissions.extend(perms);
            }
            BgEvent::RolePermissionsLoaded(Err(_)) => {}
            BgEvent::GraphStatus(msg) => {
                self.group_status_message = msg;
            }
        }
    }
}
