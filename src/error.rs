use serde::{Deserialize, Serialize};

/// Represents the API error response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl std::fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} (type: {})",
            self.code, self.message, self.error_type
        )
    }
}

/// SDK error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API error: {body} (http_status: {status})")]
    Api { status: u16, body: ErrorBody },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),
}

/// Reports whether the error is a 404 Not Found response.
pub fn is_not_found(err: &Error) -> bool {
    matches!(err, Error::Api { status: 404, .. })
}

/// Reports whether the error is a 429 Too Many Requests response.
pub fn is_rate_limited(err: &Error) -> bool {
    matches!(err, Error::Api { status: 429, .. })
}

/// Reports whether the error is an authentication/authorization error (401 or 403).
pub fn is_auth_error(err: &Error) -> bool {
    matches!(
        err,
        Error::Api {
            status: 401 | 403,
            ..
        }
    )
}
