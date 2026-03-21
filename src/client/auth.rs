use std::sync::Arc;

use anyhow::{Context, Result};
use azure_core::credentials::TokenCredential;
use azure_identity::DeveloperToolsCredential;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use serde::Deserialize;

use super::error::PimError;

const MANAGEMENT_SCOPE: &str = "https://management.azure.com/.default";

#[derive(Debug, Deserialize)]
struct JwtClaims {
    oid: Option<String>,
    unique_name: Option<String>,
    upn: Option<String>,
}

pub struct AuthInfo {
    pub credential: Arc<dyn TokenCredential>,
    pub principal_id: String,
    pub user_display: String,
    pub subscriptions: Vec<SubscriptionInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionInfo {
    #[serde(alias = "subscriptionId")]
    pub id: String,
    #[serde(alias = "displayName")]
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct ArmSubscription {
    #[serde(rename = "subscriptionId")]
    id: String,
    #[serde(rename = "displayName")]
    name: String,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscriptionListResponse {
    value: Vec<ArmSubscription>,
}

pub async fn get_auth_info() -> Result<AuthInfo> {
    let credential: Arc<dyn TokenCredential> =
        DeveloperToolsCredential::new(None).context("Failed to create Azure credential")?;

    // Get a token to extract principal ID and user info from JWT claims
    let token_response = credential
        .get_token(&[MANAGEMENT_SCOPE], None)
        .await
        .map_err(|e| PimError::Auth(format!("Failed to get token: {e}")))?;

    let token_str = token_response.token.secret();

    // Extract principal ID and display name from JWT
    let claims = decode_jwt_claims(token_str)?;
    let principal_id = claims
        .oid
        .ok_or_else(|| PimError::Parse("No 'oid' claim in token".to_string()))?;
    let user_display = claims
        .upn
        .or(claims.unique_name)
        .unwrap_or_else(|| "unknown".to_string());

    // Fetch subscriptions via REST (replaces `az account list`)
    let subscriptions = fetch_subscriptions(token_str)
        .await
        .context("Failed to fetch subscriptions")?;

    Ok(AuthInfo {
        credential,
        principal_id,
        user_display,
        subscriptions,
    })
}

async fn fetch_subscriptions(token: &str) -> Result<Vec<SubscriptionInfo>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://management.azure.com/subscriptions")
        .query(&[("api-version", "2022-12-01")])
        .bearer_auth(token)
        .send()
        .await
        .context("Failed to list subscriptions")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PimError::Api {
            status: status.as_u16(),
            message: body,
        }
        .into());
    }

    let body: SubscriptionListResponse = resp.json().await.context("Failed to parse subscriptions")?;
    Ok(body
        .value
        .into_iter()
        .filter(|s| s.state.as_deref() == Some("Enabled"))
        .map(|s| SubscriptionInfo {
            id: s.id,
            name: s.name,
        })
        .collect())
}

fn decode_jwt_claims(token: &str) -> Result<JwtClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(PimError::Parse("Invalid JWT format".to_string()).into());
    }

    let payload = parts[1].replace('-', "+").replace('_', "/");
    let decoded = STANDARD_NO_PAD
        .decode(&payload)
        .or_else(|_| {
            let padded = match payload.len() % 4 {
                2 => format!("{payload}=="),
                3 => format!("{payload}="),
                _ => payload.clone(),
            };
            STANDARD_NO_PAD.decode(&padded)
        })
        .context("Failed to decode JWT payload")?;

    let claims: JwtClaims =
        serde_json::from_slice(&decoded).context("Failed to parse JWT claims")?;

    Ok(claims)
}
