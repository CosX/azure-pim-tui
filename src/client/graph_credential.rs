use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use super::error::PimError;

/// Microsoft Graph PowerShell app ID — first-party Microsoft app with broad
/// Graph permissions pre-consented, including PIM for Groups.
const CLIENT_ID: &str = "14d82eec-204b-4c2f-b7e8-296a70dab67e";

const GRAPH_SCOPES: &str = "https://graph.microsoft.com/PrivilegedAccess.ReadWrite.AzureADGroup https://graph.microsoft.com/PrivilegedEligibilitySchedule.Read.AzureADGroup https://graph.microsoft.com/GroupMember.Read.All offline_access";

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: u64, // unix timestamp
}

pub struct GraphCredential {
    client: reqwest::Client,
    tenant_id: String,
    cached: Mutex<Option<CachedToken>>,
    status_tx: Option<mpsc::UnboundedSender<String>>,
}

impl GraphCredential {
    pub fn new(tenant_id: String, status_tx: Option<mpsc::UnboundedSender<String>>) -> Self {
        let cached = Self::load_cached_token();
        Self {
            client: reqwest::Client::new(),
            tenant_id,
            cached: Mutex::new(cached),
            status_tx,
        }
    }

    fn cache_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("azure-pim-tui")
            .join("graph_token.json")
    }

    fn load_cached_token() -> Option<CachedToken> {
        let path = Self::cache_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).ok()?;
            let token: CachedToken = serde_json::from_str(&content).ok()?;
            debug!("Loaded cached Graph token from {}", path.display());
            Some(token)
        } else {
            None
        }
    }

    fn save_cached_token(token: &CachedToken) {
        let path = Self::cache_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(token) {
            let _ = std::fs::write(&path, content);
            debug!("Saved Graph token cache to {}", path.display());
        }
    }

    fn send_status(&self, msg: String) {
        if let Some(tx) = &self.status_tx {
            let _ = tx.send(msg);
        }
    }

    pub async fn get_token(&self) -> Result<String> {
        let mut cached = self.cached.lock().await;

        // Check if we have a valid cached token
        if let Some(ref token) = *cached {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Token still valid (with 5 min buffer)
            if now + 300 < token.expires_at {
                debug!("Using cached Graph token (expires in {}s)", token.expires_at - now);
                return Ok(token.access_token.clone());
            }

            // Try refresh
            if let Some(ref refresh_token) = token.refresh_token {
                debug!("Attempting token refresh");
                match self.refresh_token(refresh_token).await {
                    Ok(new_token) => {
                        let result = new_token.access_token.clone();
                        *cached = Some(new_token);
                        Self::save_cached_token(cached.as_ref().unwrap());
                        return Ok(result);
                    }
                    Err(e) => {
                        warn!("Token refresh failed: {e}, falling back to device code flow");
                    }
                }
            }
        }

        // Need fresh auth via device code flow
        let new_token = self.device_code_flow().await?;
        let result = new_token.access_token.clone();
        *cached = Some(new_token);
        Self::save_cached_token(cached.as_ref().unwrap());
        Ok(result)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<CachedToken> {
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        );

        let resp = self
            .client
            .post(&token_url)
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("scope", GRAPH_SCOPES),
            ])
            .send()
            .await
            .context("Failed to refresh token")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(PimError::Auth(format!("Token refresh failed: {body}")).into());
        }

        let token_resp: TokenResponse = resp.json().await.context("Failed to parse token response")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(CachedToken {
            access_token: token_resp.access_token,
            refresh_token: token_resp.refresh_token.or_else(|| Some(refresh_token.to_string())),
            expires_at: now + token_resp.expires_in,
        })
    }

    async fn device_code_flow(&self) -> Result<CachedToken> {
        let device_code_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/devicecode",
            self.tenant_id
        );

        info!("Starting device code flow for Graph API permissions");

        let resp = self
            .client
            .post(&device_code_url)
            .form(&[
                ("client_id", CLIENT_ID),
                ("scope", GRAPH_SCOPES),
            ])
            .send()
            .await
            .context("Failed to initiate device code flow")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(PimError::Auth(format!("Device code request failed: {body}")).into());
        }

        let dc_resp: DeviceCodeResponse = resp.json().await.context("Failed to parse device code response")?;

        // Notify the TUI to display the code
        self.send_status(dc_resp.message.clone());
        info!("{}", dc_resp.message);

        // Poll for token
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        );

        let interval = std::time::Duration::from_secs(dc_resp.interval.max(5));
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(dc_resp.expires_in);

        loop {
            if std::time::Instant::now() > deadline {
                return Err(PimError::Auth("Device code flow timed out".to_string()).into());
            }

            tokio::time::sleep(interval).await;

            let resp = self
                .client
                .post(&token_url)
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", &dc_resp.device_code),
                ])
                .send()
                .await?;

            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if status.is_success() {
                let token_resp: TokenResponse =
                    serde_json::from_str(&body).context("Failed to parse token response")?;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                self.send_status("Graph API authenticated".to_string());
                return Ok(CachedToken {
                    access_token: token_resp.access_token,
                    refresh_token: token_resp.refresh_token,
                    expires_at: now + token_resp.expires_in,
                });
            }

            // Check if still waiting for user
            if let Ok(err_resp) = serde_json::from_str::<TokenErrorResponse>(&body) {
                match err_resp.error.as_str() {
                    "authorization_pending" => {
                        debug!("Waiting for user to authenticate...");
                        continue;
                    }
                    "slow_down" => {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    "expired_token" => {
                        return Err(
                            PimError::Auth("Device code expired. Try again.".to_string()).into()
                        );
                    }
                    _ => {
                        return Err(PimError::Auth(format!(
                            "Device code flow failed: {} - {}",
                            err_resp.error,
                            err_resp.error_description.unwrap_or_default()
                        ))
                        .into());
                    }
                }
            }

            return Err(PimError::Auth(format!("Unexpected response: {body}")).into());
        }
    }
}
