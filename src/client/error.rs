use thiserror::Error;

#[derive(Error, Debug)]
pub enum PimError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("API request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("Role assignment already exists")]
    RoleAssignmentExists,

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("{0}")]
    Other(String),
}

#[derive(serde::Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    code: Option<String>,
    message: Option<String>,
}

impl PimError {
    pub fn api(status: u16, body: &str) -> Self {
        let message = serde_json::from_str::<ErrorEnvelope>(body)
            .ok()
            .and_then(|env| match (env.error.code, env.error.message) {
                (Some(code), Some(msg)) => Some(format!("{code}: {msg}")),
                (None, Some(msg)) => Some(msg),
                (Some(code), None) => Some(code),
                (None, None) => None,
            })
            .unwrap_or_else(|| body.trim().to_string());
        PimError::Api { status, message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_extracts_code_and_message() {
        let body = r#"{"error":{"code":"RoleAssignmentExists","message":"The role assignment already exists."}}"#;
        let err = PimError::api(409, body);
        assert_eq!(
            err.to_string(),
            "API error (409): RoleAssignmentExists: The role assignment already exists."
        );
    }

    #[test]
    fn api_error_falls_back_to_raw_body() {
        let err = PimError::api(500, "  upstream timeout \n");
        assert_eq!(err.to_string(), "API error (500): upstream timeout");
    }
}
