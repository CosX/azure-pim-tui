use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- API Response wrappers ---

#[derive(Debug, Deserialize)]
pub struct ApiListResponse<T> {
    pub value: Vec<T>,
}

// --- Role Eligibility (what roles the user CAN activate) ---

#[derive(Debug, Deserialize)]
pub struct RoleEligibilityScheduleInstance {
    pub id: String,
    pub properties: EligibilityProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EligibilityProperties {
    pub role_definition_id: String,
    pub scope: String,
    pub principal_id: String,
    pub expanded_properties: Option<ExpandedProperties>,
}

// --- Role Assignment (currently active roles) ---

#[derive(Debug, Deserialize)]
pub struct RoleAssignmentScheduleInstance {
    pub id: String,
    pub properties: AssignmentProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentProperties {
    pub role_definition_id: String,
    pub scope: String,
    pub principal_id: String,
    pub assignment_type: Option<String>,
    pub end_date_time: Option<DateTime<Utc>>,
    pub expanded_properties: Option<ExpandedProperties>,
}

// --- Shared expanded properties ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandedProperties {
    pub role_definition: Option<RoleDefinitionInfo>,
    pub scope: Option<ScopeInfo>,
    pub principal: Option<PrincipalInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDefinitionInfo {
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeInfo {
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrincipalInfo {
    pub display_name: Option<String>,
}

// --- Activation request body ---

#[derive(Debug, Serialize)]
pub struct ActivationRequestBody {
    pub properties: ActivationProperties,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationProperties {
    pub role_definition_id: String,
    pub principal_id: String,
    pub request_type: String,
    pub linked_role_eligibility_schedule_id: Option<String>,
    pub justification: Option<String>,
    pub schedule_info: Option<ScheduleInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInfo {
    pub expiration: ExpirationInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpirationInfo {
    #[serde(rename = "type")]
    pub expiration_type: String,
    pub duration: String,
}

// --- App-level enriched model ---

#[derive(Debug, Clone)]
pub struct PimRole {
    pub eligibility_id: String,
    pub role_definition_id: String,
    pub principal_id: String,
    pub scope: String,
    pub role_name: String,
    pub scope_display_name: String,
    pub status: RoleStatus,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub enum RoleStatus {
    Eligible,
    Active { expires_at: Option<DateTime<Utc>> },
    Activating,
    Failed(String),
}

impl RoleStatus {
    pub fn display(&self) -> &str {
        match self {
            RoleStatus::Eligible => "Eligible",
            RoleStatus::Active { .. } => "Active",
            RoleStatus::Activating => "Activating",
            RoleStatus::Failed(_) => "Failed",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, RoleStatus::Active { .. })
    }

    pub fn is_eligible(&self) -> bool {
        matches!(self, RoleStatus::Eligible)
    }
}
