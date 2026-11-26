use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use reqwest::Client;
use tracing::{debug, warn};

use super::error::PimError;
use super::graph_credential::GraphCredential;
use super::http::send_with_retry;
use super::models::*;

const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";
const MAX_PAGES: usize = 100;

pub struct GroupPimClient {
    client: Client,
    credential: Arc<GraphCredential>,
    pub principal_id: String,
}

impl GroupPimClient {
    pub fn new(client: Client, credential: Arc<GraphCredential>, principal_id: String) -> Self {
        Self {
            client,
            credential,
            principal_id,
        }
    }

    async fn get_token(&self) -> Result<String> {
        self.credential.get_token().await
    }

    pub async fn fetch_group_roles(&self) -> Result<RoleFetch> {
        let (eligible, active) = tokio::join!(self.list_eligible(), self.list_active());

        let eligible = eligible?;
        debug!("Found {} eligible group roles", eligible.len());

        let mut stale_status_scopes = Vec::new();
        let active = match active {
            Ok(a) => a,
            Err(e) => {
                warn!("Failed to fetch active group roles: {e}");
                stale_status_scopes.push(String::new());
                vec![]
            }
        };

        if eligible.is_empty() {
            return Ok(RoleFetch::complete(vec![]));
        }

        let group_ids: Vec<String> = eligible
            .iter()
            .map(|e| e.group_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let group_names = self
            .resolve_group_names(&group_ids)
            .await
            .unwrap_or_default();

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

        sort_group_roles(&mut roles);

        Ok(RoleFetch {
            roles,
            missing_scopes: Vec::new(),
            stale_status_scopes,
        })
    }

    async fn list_paged<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<Vec<T>> {
        let token = self.get_token().await?;
        let filter = format!("principalId eq '{}'", self.principal_id);
        let mut out = Vec::new();
        let mut next: Option<String> = None;

        for page in 0..MAX_PAGES {
            let req = match &next {
                Some(link) => self.client.get(link),
                None => self.client.get(url).query(&[("$filter", &filter)]),
            };

            let resp = send_with_retry(req.bearer_auth(&token)).await?;

            if !resp.status().is_success() {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                let err = PimError::api(status, &body);
                warn!("Graph {url} failed: {err}");
                if page == 0 && (status == 400 || status == 403 || status == 404) {
                    return Ok(out);
                }
                return Err(err.into());
            }

            let text = resp.text().await?;
            let body: GraphListResponse<T> = serde_json::from_str(&text)?;
            out.extend(body.value);

            match body.next_link {
                Some(link) => next = Some(link),
                None => return Ok(out),
            }
        }

        Err(PimError::Other(format!("Result exceeded {MAX_PAGES} pages for {url}")).into())
    }

    async fn list_eligible(&self) -> Result<Vec<GroupEligibilityScheduleInstance>> {
        self.list_paged(&format!(
            "{GRAPH_BASE}/identityGovernance/privilegedAccess/group/eligibilityScheduleInstances"
        ))
        .await
    }

    async fn list_active(&self) -> Result<Vec<GroupAssignmentScheduleInstance>> {
        self.list_paged(&format!(
            "{GRAPH_BASE}/identityGovernance/privilegedAccess/group/assignmentScheduleInstances"
        ))
        .await
    }

    async fn resolve_group_names(&self, group_ids: &[String]) -> Result<HashMap<String, String>> {
        let token = self.get_token().await?;
        let mut names = HashMap::new();

        for chunk in group_ids.chunks(15) {
            let ids: Vec<String> = chunk.iter().map(|id| format!("'{id}'")).collect();
            let filter = format!("id in ({})", ids.join(","));

            let resp = send_with_retry(
                self.client
                    .get(format!("{GRAPH_BASE}/groups"))
                    .query(&[
                        ("$filter", &filter),
                        ("$select", &"id,displayName".to_string()),
                    ])
                    .bearer_auth(&token),
            )
            .await?;

            if !resp.status().is_success() {
                warn!("Group name lookup returned {}", resp.status());
            } else {
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

        let resp = send_with_retry(self.client.post(&url).bearer_auth(&token).json(&body)).await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();

            if body_text.contains("RoleAssignmentExists")
                || body_text.contains("has already been activated")
            {
                return Err(PimError::RoleAssignmentExists.into());
            }

            return Err(PimError::api(status, &body_text).into());
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

        let resp = send_with_retry(self.client.post(&url).bearer_auth(&token).json(&body)).await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();

            if body_text.contains("ActiveDurationTooShort") {
                return Err(PimError::Other(
                    "Role was activated too recently to deactivate".to_string(),
                )
                .into());
            }

            return Err(PimError::api(status, &body_text).into());
        }

        Ok(())
    }
}
