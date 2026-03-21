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
