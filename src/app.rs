use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use azure_core::credentials::TokenCredential;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use crate::client::auth::SubscriptionInfo;
use crate::client::graph_credential::GraphCredential;
use crate::client::models::{
    sort_group_roles, sort_resource_roles, PimRole, RoleFetch, RoleKey, RoleStatus,
};
use crate::config::Config;

pub const MAX_REFRESH_BACKOFF: u32 = 8;

pub struct AuthData {
    pub http: reqwest::Client,
    pub credential: Arc<dyn TokenCredential>,
    pub graph_credential: Arc<GraphCredential>,
    pub principal_id: String,
    pub user_display: String,
    pub subscriptions: Vec<SubscriptionInfo>,
}

pub enum BgEvent {
    RolesLoaded {
        generation: u64,
        result: Result<RoleFetch, String>,
    },
    GroupRolesLoaded {
        generation: u64,
        result: Result<RoleFetch, String>,
    },
    ActivationResult {
        pane: Pane,
        key: RoleKey,
        result: Result<(), String>,
    },
    DeactivationResult {
        pane: Pane,
        key: RoleKey,
        result: Result<(), String>,
    },
    AuthReady(Result<AuthData, String>),
    RolePermissionsLoaded(Result<HashMap<String, Vec<String>>, String>),
    GraphStatus(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        role: RoleKey,
        justification: String,
        duration: String,
        focused_field: ModalField,
    },
    BulkActivate {
        roles: Vec<RoleKey>,
        justification: String,
        duration: String,
        focused_field: ModalField,
    },
    DeactivateConfirm {
        role: RoleKey,
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
    pub group_roles: Vec<PimRole>,
    pub groups_loaded: bool,
    pub groups_loading: bool,
    pub group_status_message: String,

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
    pub last_click: Option<(Instant, usize)>,
    pub list_offset: Cell<usize>,
    pub resource_generation: u64,
    pub group_generation: u64,
    pub refresh_backoff: u32,
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
            last_click: None,
            list_offset: Cell::new(0),
            resource_generation: 0,
            group_generation: 0,
            refresh_backoff: 1,
        }
    }

    pub fn roles_for(&self, pane: Pane) -> &[PimRole] {
        match pane {
            Pane::Resources => &self.roles,
            Pane::Groups => &self.group_roles,
        }
    }

    fn roles_for_mut(&mut self, pane: Pane) -> &mut Vec<PimRole> {
        match pane {
            Pane::Resources => &mut self.roles,
            Pane::Groups => &mut self.group_roles,
        }
    }

    pub fn active_roles(&self) -> &[PimRole] {
        self.roles_for(self.active_pane)
    }

    pub fn active_roles_mut(&mut self) -> &mut Vec<PimRole> {
        self.roles_for_mut(self.active_pane)
    }

    pub fn find_role(&self, pane: Pane, key: &RoleKey) -> Option<&PimRole> {
        self.roles_for(pane).iter().find(|r| &r.key() == key)
    }

    fn find_role_mut(&mut self, pane: Pane, key: &RoleKey) -> Option<&mut PimRole> {
        self.roles_for_mut(pane)
            .iter_mut()
            .find(|r| &r.key() == key)
    }

    fn status_for_mut(&mut self, pane: Pane) -> &mut String {
        match pane {
            Pane::Resources => &mut self.status_message,
            Pane::Groups => &mut self.group_status_message,
        }
    }

    pub fn begin_fetch(&mut self, pane: Pane) -> u64 {
        match pane {
            Pane::Resources => {
                self.loading = true;
                self.resource_generation += 1;
                self.resource_generation
            }
            Pane::Groups => {
                self.groups_loading = true;
                self.groups_loaded = false;
                self.group_generation += 1;
                self.group_generation
            }
        }
    }

    pub fn active_status(&self) -> &str {
        match self.active_pane {
            Pane::Resources => &self.status_message,
            Pane::Groups => &self.group_status_message,
        }
    }

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

    pub fn select(&mut self, display_idx: usize) {
        if display_idx < self.filtered_indices.len() && display_idx != self.selected {
            self.selected = display_idx;
            self.detail_scroll = 0;
        }
    }

    pub fn open_activate_modal(&mut self) {
        let Some(role) = self.selected_role() else {
            return;
        };
        if role.status.can_activate() {
            self.modal = ActiveModal::Activate {
                role: role.key(),
                justification: self.config.default_justification.clone(),
                duration: self.config.default_duration_hours.to_string(),
                focused_field: ModalField::Justification,
            };
        }
    }

    pub fn open_deactivate_modal(&mut self) {
        let Some(role) = self.selected_role() else {
            return;
        };
        if role.status.is_active() {
            self.modal = ActiveModal::DeactivateConfirm { role: role.key() };
        }
    }

    pub fn open_bulk_activate_modal(&mut self) {
        let roles = self.active_roles();
        let keys: Vec<RoleKey> = self
            .filtered_indices
            .iter()
            .filter_map(|&i| roles.get(i))
            .filter(|r| r.selected && r.status.can_activate())
            .map(PimRole::key)
            .collect();
        if !keys.is_empty() {
            self.modal = ActiveModal::BulkActivate {
                roles: keys,
                justification: self.config.default_justification.clone(),
                duration: self.config.default_duration_hours.to_string(),
                focused_field: ModalField::Justification,
            };
        }
    }

    fn drop_stale_modal(&mut self) {
        let pane = self.active_pane;
        let closed = match &mut self.modal {
            ActiveModal::Activate { role, .. } | ActiveModal::DeactivateConfirm { role } => {
                let role = role.clone();
                self.find_role(pane, &role).is_none()
            }
            ActiveModal::BulkActivate { roles, .. } => {
                let mut kept = std::mem::take(roles);
                kept.retain(|k| self.find_role(pane, k).is_some());
                let empty = kept.is_empty();
                if let ActiveModal::BulkActivate { roles, .. } = &mut self.modal {
                    *roles = kept;
                }
                empty
            }
            ActiveModal::Help | ActiveModal::None => false,
        };
        if closed {
            self.modal = ActiveModal::None;
            *self.status_for_mut(pane) = "Role list changed - dialog closed".to_string();
        }
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

    fn selected_key(&self) -> Option<RoleKey> {
        self.selected_role().map(PimRole::key)
    }

    fn merge_fetch(old: &[PimRole], fetch: RoleFetch, pane: Pane) -> Vec<PimRole> {
        let under = |role: &PimRole, scopes: &[String]| {
            let scope = role.scope.to_ascii_lowercase();
            scopes
                .iter()
                .any(|prefix| scope.starts_with(&prefix.to_ascii_lowercase()))
        };

        let mut roles = fetch.roles;

        let marked: HashSet<RoleKey> = old
            .iter()
            .filter(|r| r.selected)
            .map(PimRole::key)
            .collect();
        let previous: HashMap<RoleKey, &PimRole> = old.iter().map(|r| (r.key(), r)).collect();

        for role in roles.iter_mut() {
            let key = role.key();
            role.selected = marked.contains(&key);
            if under(role, &fetch.stale_status_scopes) {
                if let Some(prev) = previous.get(&key) {
                    role.status = prev.status.clone();
                }
            }
        }

        let fetched: HashSet<RoleKey> = roles.iter().map(PimRole::key).collect();
        let carried: Vec<PimRole> = old
            .iter()
            .filter(|r| under(r, &fetch.missing_scopes) && !fetched.contains(&r.key()))
            .cloned()
            .collect();
        roles.extend(carried);

        match pane {
            Pane::Resources => sort_resource_roles(&mut roles),
            Pane::Groups => sort_group_roles(&mut roles),
        }
        roles
    }

    fn partial_status(fetch: &RoleFetch) -> String {
        let missing = fetch.missing_scopes.len();
        let stale = fetch.stale_status_scopes.len();
        if missing > 0 {
            format!("Ready - {missing} scope(s) unavailable, showing last known roles")
        } else {
            format!("Ready - {stale} scope(s) unavailable, active status may be stale")
        }
    }

    fn refilter_keeping_cursor(&mut self, key: Option<RoleKey>) {
        self.update_filtered_indices();
        let Some(key) = key else {
            return;
        };
        let roles = self.active_roles();
        let pos = self
            .filtered_indices
            .iter()
            .position(|&i| roles.get(i).is_some_and(|r| r.key() == key));
        if let Some(pos) = pos {
            self.selected = pos;
        }
    }

    fn apply_fetch(&mut self, pane: Pane, result: Result<RoleFetch, String>) {
        let key = (self.active_pane == pane)
            .then(|| self.selected_key())
            .flatten();
        match result {
            Ok(fetch) => {
                let status = if fetch.is_partial() {
                    Self::partial_status(&fetch)
                } else {
                    "Ready".to_string()
                };
                if pane == Pane::Resources {
                    self.refresh_backoff = if fetch.is_partial() {
                        (self.refresh_backoff * 2).min(MAX_REFRESH_BACKOFF)
                    } else {
                        1
                    };
                }
                let merged = Self::merge_fetch(self.roles_for(pane), fetch, pane);
                *self.roles_for_mut(pane) = merged;
                *self.status_for_mut(pane) = status;
            }
            Err(e) => {
                if pane == Pane::Resources {
                    self.refresh_backoff = (self.refresh_backoff * 2).min(MAX_REFRESH_BACKOFF);
                }
                *self.status_for_mut(pane) = format!("Failed to load roles: {e}");
            }
        }
        if self.active_pane == pane {
            self.refilter_keeping_cursor(key);
            self.drop_stale_modal();
        }
    }

    fn apply_action_result(
        &mut self,
        pane: Pane,
        key: &RoleKey,
        result: Result<(), String>,
        ok_status: RoleStatus,
        verb: &str,
        mark_failed: bool,
    ) {
        let cursor = (self.active_pane == pane)
            .then(|| self.selected_key())
            .flatten();
        let message = match result {
            Ok(()) => {
                if let Some(role) = self.find_role_mut(pane, key) {
                    role.status = ok_status;
                    role.selected = false;
                }
                format!("{verb} requested - refreshing...")
            }
            Err(e) => {
                let message = format!("{verb} failed: {e}");
                if mark_failed {
                    if let Some(role) = self.find_role_mut(pane, key) {
                        role.status = RoleStatus::Failed(e);
                    }
                }
                message
            }
        };
        *self.status_for_mut(pane) = message;
        if self.active_pane == pane {
            self.refilter_keeping_cursor(cursor);
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
            BgEvent::RolesLoaded { generation, result } => {
                if generation != self.resource_generation {
                    return;
                }
                self.loading = false;
                if result.is_ok() {
                    self.last_refresh = Some(Utc::now());
                }
                self.apply_fetch(Pane::Resources, result);
            }
            BgEvent::GroupRolesLoaded { generation, result } => {
                if generation != self.group_generation {
                    return;
                }
                self.groups_loading = false;
                self.groups_loaded = true;
                self.apply_fetch(Pane::Groups, result);
            }
            BgEvent::ActivationResult { pane, key, result } => {
                self.apply_action_result(
                    pane,
                    &key,
                    result,
                    RoleStatus::Activating,
                    "Activation",
                    true,
                );
            }
            BgEvent::DeactivationResult { pane, key, result } => {
                self.apply_action_result(
                    pane,
                    &key,
                    result,
                    RoleStatus::Eligible,
                    "Deactivation",
                    false,
                );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::models::RoleType;

    fn role(name: &str, scope: &str, status: RoleStatus) -> PimRole {
        PimRole {
            eligibility_id: format!("{scope}/{name}"),
            role_definition_id: format!("/roleDefinitions/{name}"),
            principal_id: "me".to_string(),
            scope: scope.to_string(),
            role_name: name.to_string(),
            scope_display_name: scope.to_string(),
            role_type: RoleType::Resource,
            group_id: None,
            status,
            selected: false,
        }
    }

    fn group(name: &str, role_type: RoleType) -> PimRole {
        PimRole {
            eligibility_id: format!("{name}-{role_type:?}"),
            role_definition_id: String::new(),
            principal_id: "me".to_string(),
            scope: format!("Group: {name}"),
            role_name: name.to_string(),
            scope_display_name: role_type.access_label().to_string(),
            role_type,
            group_id: Some(format!("{name}-id")),
            status: RoleStatus::Eligible,
            selected: false,
        }
    }

    fn app_with(roles: Vec<PimRole>) -> App {
        let mut app = App::new(Config::default());
        app.roles = roles;
        app.update_filtered_indices();
        app
    }

    fn load(app: &mut App, roles: Vec<PimRole>) {
        let generation = app.begin_fetch(Pane::Resources);
        app.handle_bg_event(BgEvent::RolesLoaded {
            generation,
            result: Ok(RoleFetch::complete(roles)),
        });
    }

    #[test]
    fn activate_dialog_keeps_its_role_when_refresh_reorders_list() {
        let mut app = app_with(vec![
            role("Reader", "/s/a", RoleStatus::Eligible),
            role("Writer", "/s/b", RoleStatus::Eligible),
        ]);
        app.select(1);
        app.open_activate_modal();

        load(
            &mut app,
            vec![
                role("Admin", "/s/0", RoleStatus::Eligible),
                role("Reader", "/s/a", RoleStatus::Eligible),
                role("Writer", "/s/b", RoleStatus::Eligible),
            ],
        );

        let ActiveModal::Activate { role: key, .. } = &app.modal else {
            panic!("dialog should stay open");
        };
        assert_eq!(
            app.find_role(Pane::Resources, key).unwrap().role_name,
            "Writer"
        );
    }

    #[test]
    fn activate_dialog_closes_when_its_role_disappears() {
        let mut app = app_with(vec![
            role("Reader", "/s/a", RoleStatus::Eligible),
            role("Writer", "/s/b", RoleStatus::Eligible),
        ]);
        app.select(1);
        app.open_activate_modal();

        load(&mut app, vec![role("Reader", "/s/a", RoleStatus::Eligible)]);

        assert_eq!(app.modal, ActiveModal::None);
    }

    #[test]
    fn superseded_fetch_result_is_ignored() {
        let mut app = app_with(vec![]);
        let old = app.begin_fetch(Pane::Resources);
        let new = app.begin_fetch(Pane::Resources);

        app.handle_bg_event(BgEvent::RolesLoaded {
            generation: old,
            result: Ok(RoleFetch::complete(vec![role(
                "Old",
                "/s/a",
                RoleStatus::Eligible,
            )])),
        });
        assert!(app.roles.is_empty());
        assert!(app.loading);

        app.handle_bg_event(BgEvent::RolesLoaded {
            generation: new,
            result: Ok(RoleFetch::complete(vec![role(
                "New",
                "/s/a",
                RoleStatus::Eligible,
            )])),
        });
        assert_eq!(app.roles[0].role_name, "New");
        assert!(!app.loading);
    }

    #[test]
    fn activation_result_goes_to_requesting_pane_after_tab() {
        let mut app = app_with(vec![role("Reader", "/s/a", RoleStatus::Eligible)]);
        app.group_roles = vec![group("Ops", RoleType::GroupMember)];
        let key = app.roles[0].key();

        app.switch_pane();
        app.handle_bg_event(BgEvent::ActivationResult {
            pane: Pane::Resources,
            key,
            result: Ok(()),
        });

        assert!(matches!(app.roles[0].status, RoleStatus::Activating));
        assert!(matches!(app.group_roles[0].status, RoleStatus::Eligible));
    }

    #[test]
    fn group_member_and_owner_have_distinct_keys() {
        let member = group("Ops", RoleType::GroupMember);
        let owner = group("Ops", RoleType::GroupOwner);
        assert_ne!(member.key(), owner.key());
    }

    #[test]
    fn failed_activation_can_be_retried() {
        let mut app = app_with(vec![role(
            "Reader",
            "/s/a",
            RoleStatus::Failed("boom".to_string()),
        )]);
        app.open_activate_modal();
        assert!(matches!(app.modal, ActiveModal::Activate { .. }));
    }

    #[test]
    fn bulk_activate_ignores_selected_roles_hidden_by_filter() {
        let mut app = app_with(vec![
            role("Reader", "/s/a", RoleStatus::Eligible),
            role("Writer", "/s/b", RoleStatus::Eligible),
        ]);
        app.roles[0].selected = true;
        app.roles[1].selected = true;
        app.filter_text = "writer".to_string();
        app.update_filtered_indices();

        app.open_bulk_activate_modal();

        let ActiveModal::BulkActivate { roles, .. } = &app.modal else {
            panic!("bulk dialog should open");
        };
        assert_eq!(roles.len(), 1);
        assert_eq!(
            app.find_role(Pane::Resources, &roles[0]).unwrap().role_name,
            "Writer"
        );
    }

    #[test]
    fn partial_fetch_backs_off_and_full_fetch_resets() {
        let mut app = app_with(vec![]);
        for _ in 0..5 {
            let generation = app.begin_fetch(Pane::Resources);
            app.handle_bg_event(BgEvent::RolesLoaded {
                generation,
                result: Err("offline".to_string()),
            });
        }
        assert_eq!(app.refresh_backoff, MAX_REFRESH_BACKOFF);
        load(&mut app, vec![]);
        assert_eq!(app.refresh_backoff, 1);
    }
}
