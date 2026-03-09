/// Configuration for the MPC SDK.
pub struct Config {
    pub base_url: String,
}

impl Config {
    /// Returns configuration for the sandbox environment.
    pub fn sandbox() -> Self {
        Self {
            base_url: "https://api-sandbox.paratro.com".to_string(),
        }
    }

    /// Returns configuration for the production environment.
    pub fn production() -> Self {
        Self {
            base_url: "https://api.paratro.com".to_string(),
        }
    }

    /// Returns a custom configuration with the specified base URL.
    pub fn custom(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}
