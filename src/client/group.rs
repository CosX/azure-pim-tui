use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use reqwest::Client;
use tracing::{debug, warn};

use super::error::PimError;
use super::graph_credential::GraphCredential;
use super::models::*;

const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";

pub struct GroupPimClient {
    client: Client,
    credential: Arc<GraphCredential>,
    pub principal_id: String,
}

impl GroupPimClient {
    pub fn new(credential: Arc<GraphCredential>, principal_id: String) -> Self {
        Self {
            client: Client::new(),
            credential,
            principal_id,
        }
    }

    async fn get_token(&self) -> Result<String> {
        self.credential.get_token().await
    }

    pub async fn fetch_group_roles(&self) -> Result<Vec<PimRole>> {
        let (eligible, active) =
            tokio::join!(self.list_eligible(), self.list_active());

        let eligible = match eligible {
            Ok(e) => {
                debug!("Found {} eligible group roles", e.len());
                e
            }
            Err(e) => {
                warn!("Failed to fetch eligible group roles: {e}");
                return Ok(vec![]);
            }
        };
        let active = match active {
            Ok(a) => a,
            Err(e) => {
                warn!("Failed to fetch active group roles: {e}");
                vec![]
            }
        };

        if eligible.is_empty() {
            return Ok(vec![]);
        }

        // Collect unique group IDs to resolve display names
        let group_ids: Vec<String> = eligible
            .iter()
            .map(|e| e.group_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let group_names = self.resolve_group_names(&group_ids).await.unwrap_or_default();

        let mut roles: Vec<PimRole> = eligible
            .into_iter()
            .map(|e| {
                let group_name = group_names
                    .get(&e.group_id)
                    .cloned()
                    .unwrap_or_else(|| e.group_id.clone());

                let role_type = match e.access_id.as_str() {
                    "owner" => RoleType::GroupOwner,
                    _ => RoleType::GroupMember,
                };

                PimRole {
                    eligibility_id: e.id.clone(),
                    role_definition_id: String::new(),
                    principal_id: e.principal_id.clone(),
                    scope: format!("Group: {group_name}"),
                    role_name: group_name.clone(),
                    scope_display_name: role_type.access_label().to_string(),
                    role_type,
                    group_id: Some(e.group_id),
                    status: RoleStatus::Eligible,
                    selected: false,
                }
            })
            .collect();

        // Mark active
        for assignment in &active {
            for role in &mut roles {
                if role.group_id.as_deref() == Some(&assignment.group_id)
                    && matches!(
                        (&role.role_type, assignment.access_id.as_str()),
                        (RoleType::GroupMember, "member") | (RoleType::GroupOwner, "owner")
                    )
                {
                    role.status = RoleStatus::Active {
                        expires_at: assignment.end_date_time,
                    };
                }
            }
        }

        roles.sort_by(|a, b| {
            a.role_name
                .cmp(&b.role_name)
                .then_with(|| a.scope_display_name.cmp(&b.scope_display_name))
        });

        Ok(roles)
    }

    async fn list_eligible(&self) -> Result<Vec<GroupEligibilityScheduleInstance>> {
        let token = self.get_token().await?;
        let url = format!(
            "{GRAPH_BASE}/identityGovernance/privilegedAccess/group/eligibilityScheduleInstances"
        );
        let filter = format!("principalId eq '{}'", self.principal_id);

        let resp = self
            .client
            .get(&url)
            .query(&[("$filter", &filter)])
            .bearer_auth(&token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            warn!("Group eligibility API returned {status}: {body}");
            // Return empty if the tenant doesn't support PIM for Groups
            if status == 400 || status == 403 || status == 404 {
                return Ok(vec![]);
            }
            return Err(PimError::Api {
                status,
                message: body,
            }
            .into());
        }

        let text = resp.text().await?;
        debug!("Group eligibility response: {text}");
        let body: GraphListResponse<GroupEligibilityScheduleInstance> =
            serde_json::from_str(&text)?;
        Ok(body.value)
    }

    async fn list_active(&self) -> Result<Vec<GroupAssignmentScheduleInstance>> {
        let token = self.get_token().await?;
        let url = format!(
            "{GRAPH_BASE}/identityGovernance/privilegedAccess/group/assignmentScheduleInstances"
        );
        let filter = format!("principalId eq '{}'", self.principal_id);

        let resp = self
            .client
            .get(&url)
            .query(&[("$filter", &filter)])
            .bearer_auth(&token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let body: GraphListResponse<GroupAssignmentScheduleInstance> = resp.json().await?;
        Ok(body.value)
    }

    async fn resolve_group_names(&self, group_ids: &[String]) -> Result<HashMap<String, String>> {
        let token = self.get_token().await?;
        let mut names = HashMap::new();

        // Batch with $filter using 'in' operator (up to 15 per request)
        for chunk in group_ids.chunks(15) {
            let ids: Vec<String> = chunk.iter().map(|id| format!("'{id}'")).collect();
            let filter = format!("id in ({})", ids.join(","));

            let resp = self
                .client
                .get(&format!("{GRAPH_BASE}/groups"))
                .query(&[("$filter", &filter), ("$select", &"id,displayName".to_string())])
                .bearer_auth(&token)
                .send()
                .await?;

            if resp.status().is_success() {
                let body: GraphListResponse<GraphGroup> = resp.json().await?;
                for g in body.value {
                    if let Some(name) = g.display_name {
                        names.insert(g.id, name);
                    }
                }
            }
        }

        Ok(names)
    }

    pub async fn activate_group(
        &self,
        role: &PimRole,
        justification: &str,
        duration_hours: u32,
    ) -> Result<()> {
        let token = self.get_token().await?;
        let url = format!(
            "{GRAPH_BASE}/identityGovernance/privilegedAccess/group/assignmentScheduleRequests"
        );

        let access_id = match role.role_type {
            RoleType::GroupOwner => "owner",
            _ => "member",
        };

        let body = GroupAssignmentRequest {
            access_id: access_id.to_string(),
            principal_id: self.principal_id.clone(),
            group_id: role.group_id.clone().unwrap_or_default(),
            action: "selfActivate".to_string(),
            justification: Some(justification.to_string()),
            schedule_info: Some(GroupScheduleInfo {
                expiration: GroupExpirationInfo {
                    expiration_type: "afterDuration".to_string(),
                    duration: format!("PT{duration_hours}H"),
                },
            }),
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();

            if body_text.contains("RoleAssignmentExists")
                || body_text.contains("has already been activated")
            {
                return Err(PimError::RoleAssignmentExists.into());
            }

            return Err(PimError::Api {
                status,
                message: body_text,
            }
            .into());
        }

        Ok(())
    }

    pub async fn deactivate_group(&self, role: &PimRole) -> Result<()> {
        let token = self.get_token().await?;
        let url = format!(
            "{GRAPH_BASE}/identityGovernance/privilegedAccess/group/assignmentScheduleRequests"
        );

        let access_id = match role.role_type {
            RoleType::GroupOwner => "owner",
            _ => "member",
        };

        let body = GroupAssignmentRequest {
            access_id: access_id.to_string(),
            principal_id: self.principal_id.clone(),
            group_id: role.group_id.clone().unwrap_or_default(),
            action: "selfDeactivate".to_string(),
            justification: None,
            schedule_info: None,
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();

            return Err(PimError::Api {
                status,
                message: body_text,
            }
            .into());
        }

        Ok(())
    }
}
