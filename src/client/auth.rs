use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use serde::Deserialize;
use std::process::Command;

use super::error::PimError;

#[derive(Debug, Deserialize)]
struct AzToken {
    #[serde(alias = "accessToken")]
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    oid: Option<String>,
    unique_name: Option<String>,
    upn: Option<String>,
}

pub struct AuthInfo {
    pub token: String,
    pub principal_id: String,
    pub user_display: String,
    pub subscriptions: Vec<SubscriptionInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionInfo {
    pub id: String,
    #[serde(alias = "name")]
    pub name: String,
}

pub async fn get_auth_info() -> Result<AuthInfo> {
    // Run all three az commands in parallel
    let (token_out, user_out, subs_out) = tokio::try_join!(
        tokio::task::spawn_blocking(|| {
            Command::new("az")
                .args([
                    "account", "get-access-token",
                    "--resource", "https://management.azure.com",
                    "--output", "json",
                ])
                .output()
        }),
        tokio::task::spawn_blocking(|| {
            Command::new("az")
                .args(["ad", "signed-in-user", "show", "--query", "id", "-o", "tsv"])
                .output()
        }),
        tokio::task::spawn_blocking(|| {
            Command::new("az")
                .args(["account", "list", "--query", "[?state=='Enabled']", "-o", "json"])
                .output()
        }),
    )?;

    // Parse token
    let token_output = token_out.context("Failed to run az account get-access-token")?;
    if !token_output.status.success() {
        let stderr = String::from_utf8_lossy(&token_output.stderr);
        return Err(PimError::Auth(format!(
            "az CLI failed. Run `az login` first. Error: {stderr}"
        ))
        .into());
    }
    let token_response: AzToken =
        serde_json::from_slice(&token_output.stdout).context("Failed to parse az token response")?;

    // Parse principal ID
    let user_output = user_out.context("Failed to run az ad signed-in-user show")?;
    if !user_output.status.success() {
        let stderr = String::from_utf8_lossy(&user_output.stderr);
        return Err(PimError::Auth(format!("Failed to get user info: {stderr}")).into());
    }
    let principal_id = String::from_utf8_lossy(&user_output.stdout).trim().to_string();

    // Parse subscriptions
    let subs_output = subs_out.context("Failed to run az account list")?;
    let subscriptions: Vec<SubscriptionInfo> = if subs_output.status.success() {
        serde_json::from_slice(&subs_output.stdout).unwrap_or_default()
    } else {
        vec![]
    };

    // Get display name from JWT (lightweight, no extra az call)
    let claims = decode_jwt_claims(&token_response.access_token)?;
    let user_display = claims
        .upn
        .or(claims.unique_name)
        .unwrap_or_else(|| "unknown".to_string());

    Ok(AuthInfo {
        token: token_response.access_token,
        principal_id,
        user_display,
        subscriptions,
    })
}

fn decode_jwt_claims(token: &str) -> Result<JwtClaims> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(PimError::Parse("Invalid JWT format".to_string()).into());
    }

    // JWT base64url uses no padding; the base64 crate's STANDARD_NO_PAD handles
    // standard base64 without padding. We need to convert base64url to standard first.
    let payload = parts[1].replace('-', "+").replace('_', "/");
    let decoded = STANDARD_NO_PAD
        .decode(&payload)
        .or_else(|_| {
            // Try with padding
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
