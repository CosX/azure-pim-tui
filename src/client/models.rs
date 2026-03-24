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

// --- Role definition (full, for permissions) ---

#[derive(Debug, Deserialize)]
pub struct RoleDefinitionResponse {
    pub properties: RoleDefinitionProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDefinitionProperties {
    pub permissions: Vec<RolePermission>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePermission {
    pub actions: Vec<String>,
    pub not_actions: Vec<String>,
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

// --- Graph API models for PIM for Groups ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphListResponse<T> {
    pub value: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupEligibilityScheduleInstance {
    pub id: String,
    pub group_id: String,
    pub principal_id: String,
    pub access_id: String, // "member" or "owner"
    pub member_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupAssignmentScheduleInstance {
    pub id: String,
    pub group_id: String,
    pub principal_id: String,
    pub access_id: String,
    pub assignment_type: Option<String>,
    pub end_date_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupAssignmentRequest {
    pub access_id: String,
    pub principal_id: String,
    pub group_id: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_info: Option<GroupScheduleInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupScheduleInfo {
    pub expiration: GroupExpirationInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupExpirationInfo {
    #[serde(rename = "type")]
    pub expiration_type: String,
    pub duration: String,
}

// --- Graph group display name response ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphGroup {
    pub id: String,
    pub display_name: Option<String>,
}

// --- App-level enriched model ---

#[derive(Debug, Clone, PartialEq)]
pub enum RoleType {
    Resource,
    GroupMember,
    GroupOwner,
}

impl RoleType {
    pub fn label(&self) -> &str {
        match self {
            RoleType::Resource => "Resource",
            RoleType::GroupMember => "Group",
            RoleType::GroupOwner => "Group",
        }
    }

    pub fn access_label(&self) -> &str {
        match self {
            RoleType::Resource => "",
            RoleType::GroupMember => "Member",
            RoleType::GroupOwner => "Owner",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PimRole {
    pub eligibility_id: String,
    pub role_definition_id: String,
    pub principal_id: String,
    pub scope: String,
    pub role_name: String,
    pub scope_display_name: String,
    pub role_type: RoleType,
    pub group_id: Option<String>,
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
