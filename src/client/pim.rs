use anyhow::Result;
use reqwest::Client;
use uuid::Uuid;

use super::auth::SubscriptionInfo;
use super::error::PimError;
use super::models::*;

const API_VERSION: &str = "2020-10-01";
const BASE_URL: &str = "https://management.azure.com";

#[derive(Debug)]
pub struct PimClient {
    client: Client,
    token: String,
    pub principal_id: String,
    pub subscriptions: Vec<SubscriptionInfo>,
}

impl PimClient {
    pub fn new(token: String, principal_id: String, subscriptions: Vec<SubscriptionInfo>) -> Self {
        Self {
            client: Client::new(),
            token,
            principal_id,
            subscriptions,
        }
    }

    /// List eligible role schedules at a specific scope, filtered to this principal.
    async fn list_eligible_at_scope(
        &self,
        scope: &str,
    ) -> Result<Vec<RoleEligibilityScheduleInstance>> {
        let filter = format!(
            "assignedTo('{}') and atScope()",
            self.principal_id
        );
        let base = format!(
            "{BASE_URL}{scope}/providers/Microsoft.Authorization/roleEligibilitySchedules"
        );

        let resp = self
            .client
            .get(&base)
            .query(&[("$filter", &filter), ("api-version", &API_VERSION.to_string())])
            .bearer_auth(&self.token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let text = resp.text().await?;
        let body: ApiListResponse<RoleEligibilityScheduleInstance> =
            serde_json::from_str(&text)?;
        Ok(body.value)
    }

    /// List active assignment schedule instances at a specific scope, filtered to this principal.
    async fn list_active_at_scope(
        &self,
        scope: &str,
    ) -> Result<Vec<RoleAssignmentScheduleInstance>> {
        let filter = format!(
            "assignedTo('{}') and atScope()",
            self.principal_id
        );
        let base = format!(
            "{BASE_URL}{scope}/providers/Microsoft.Authorization/roleAssignmentScheduleInstances"
        );

        let resp = self
            .client
            .get(&base)
            .query(&[("$filter", &filter), ("api-version", &API_VERSION.to_string())])
            .bearer_auth(&self.token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let text = resp.text().await?;
        let body: ApiListResponse<RoleAssignmentScheduleInstance> =
            serde_json::from_str(&text)?;
        Ok(body.value)
    }

    pub async fn fetch_roles(&self) -> Result<Vec<PimRole>> {
        // Build scope list from subscriptions
        let scopes: Vec<String> = self
            .subscriptions
            .iter()
            .map(|s| format!("/subscriptions/{}", s.id))
            .collect();

        if scopes.is_empty() {
            return Err(PimError::Other("No subscriptions found. Run `az login` first.".to_string()).into());
        }

        // Query eligibility + active assignments for all scopes in parallel
        let mut eligible_futures = Vec::new();
        let mut active_futures = Vec::new();
        for scope in &scopes {
            eligible_futures.push(self.list_eligible_at_scope(scope));
            active_futures.push(self.list_active_at_scope(scope));
        }

        let (eligible_results, active_results) = tokio::join!(
            futures::future::join_all(eligible_futures),
            futures::future::join_all(active_futures),
        );

        let eligible: Vec<RoleEligibilityScheduleInstance> = eligible_results
            .into_iter()
            .filter_map(|r| r.ok())
            .flatten()
            .collect();

        let active: Vec<RoleAssignmentScheduleInstance> = active_results
            .into_iter()
            .filter_map(|r| r.ok())
            .flatten()
            .collect();

        // Build subscription name lookup
        let sub_names: std::collections::HashMap<&str, &str> = self
            .subscriptions
            .iter()
            .map(|s| (s.id.as_str(), s.name.as_str()))
            .collect();

        let mut roles: Vec<PimRole> = eligible
            .into_iter()
            .map(|e| {
                let role_name = e
                    .properties
                    .expanded_properties
                    .as_ref()
                    .and_then(|ep| ep.role_definition.as_ref())
                    .and_then(|rd| rd.display_name.clone())
                    .unwrap_or_else(|| "Unknown Role".to_string());

                let scope_display = e
                    .properties
                    .expanded_properties
                    .as_ref()
                    .and_then(|ep| ep.scope.as_ref())
                    .and_then(|s| s.display_name.clone())
                    .unwrap_or_else(|| {
                        extract_sub_id(&e.properties.scope)
                            .and_then(|sid| sub_names.get(sid).map(|n| n.to_string()))
                            .unwrap_or_else(|| extract_scope_name(&e.properties.scope))
                    });

                PimRole {
                    eligibility_id: e.id.clone(),
                    role_definition_id: e.properties.role_definition_id.clone(),
                    principal_id: e.properties.principal_id.clone(),
                    scope: e.properties.scope.clone(),
                    role_name,
                    scope_display_name: scope_display,
                    status: RoleStatus::Eligible,
                    selected: false,
                }
            })
            .collect();

        // Mark active roles
        for assignment in &active {
            let is_activated = assignment
                .properties
                .assignment_type
                .as_deref()
                .map(|t| t == "Activated")
                .unwrap_or(false);

            if !is_activated {
                continue;
            }

            for role in &mut roles {
                if role.role_definition_id == assignment.properties.role_definition_id
                    && role.scope == assignment.properties.scope
                {
                    role.status = RoleStatus::Active {
                        expires_at: assignment.properties.end_date_time,
                    };
                }
            }
        }

        // Sort: by scope, then role name, then active first
        roles.sort_by(|a, b| {
            a.scope_display_name
                .cmp(&b.scope_display_name)
                .then_with(|| a.role_name.cmp(&b.role_name))
                .then_with(|| {
                    let a_active = a.status.is_active() as u8;
                    let b_active = b.status.is_active() as u8;
                    b_active.cmp(&a_active)
                })
        });

        Ok(roles)
    }

    pub async fn activate_role(
        &self,
        role: &PimRole,
        justification: &str,
        duration_hours: u32,
    ) -> Result<()> {
        let request_id = Uuid::new_v4().to_string();
        let base = format!(
            "{BASE_URL}{}/providers/Microsoft.Authorization/roleAssignmentScheduleRequests/{request_id}",
            role.scope
        );

        let body = ActivationRequestBody {
            properties: ActivationProperties {
                role_definition_id: role.role_definition_id.clone(),
                principal_id: self.principal_id.clone(),
                request_type: "SelfActivate".to_string(),
                linked_role_eligibility_schedule_id: Some(role.eligibility_id.clone()),
                justification: Some(justification.to_string()),
                schedule_info: Some(ScheduleInfo {
                    expiration: ExpirationInfo {
                        expiration_type: "AfterDuration".to_string(),
                        duration: format!("PT{duration_hours}H"),
                    },
                }),
            },
        };

        let resp = self
            .client
            .put(&base)
            .query(&[("api-version", API_VERSION)])
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();

            if body_text.contains("RoleAssignmentExists") {
                return Err(PimError::RoleAssignmentExists.into());
            }

            if body_text.contains("ActiveDurationTooShort") {
                return Err(PimError::Other(
                    "Role was activated too recently to modify".to_string(),
                )
                .into());
            }

            return Err(PimError::Api {
                status,
                message: body_text,
            }
            .into());
        }

        Ok(())
    }

    pub async fn deactivate_role(&self, role: &PimRole) -> Result<()> {
        let request_id = Uuid::new_v4().to_string();
        let base = format!(
            "{BASE_URL}{}/providers/Microsoft.Authorization/roleAssignmentScheduleRequests/{request_id}",
            role.scope
        );

        let body = ActivationRequestBody {
            properties: ActivationProperties {
                role_definition_id: role.role_definition_id.clone(),
                principal_id: self.principal_id.clone(),
                request_type: "SelfDeactivate".to_string(),
                linked_role_eligibility_schedule_id: Some(role.eligibility_id.clone()),
                justification: None,
                schedule_info: None,
            },
        };

        let resp = self
            .client
            .put(&base)
            .query(&[("api-version", API_VERSION)])
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();

            if body_text.contains("ActiveDurationTooShort") {
                return Err(PimError::Other(
                    "Role was activated too recently to deactivate".to_string(),
                )
                .into());
            }

            return Err(PimError::Api {
                status,
                message: body_text,
            }
            .into());
        }

        Ok(())
    }
}

/// Extract subscription ID from a scope string like /subscriptions/{id}/...
fn extract_sub_id(scope: &str) -> Option<&str> {
    let parts: Vec<&str> = scope.split('/').collect();
    parts.iter().position(|&p| p == "subscriptions").and_then(|i| parts.get(i + 1).copied())
}

fn extract_scope_name(scope: &str) -> String {
    let parts: Vec<&str> = scope.split('/').collect();
    if parts.len() >= 3 {
        parts.last().unwrap_or(&"Unknown").to_string()
    } else {
        scope.to_string()
    }
}
