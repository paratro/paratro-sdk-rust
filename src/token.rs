use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::{Error, ErrorBody};

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
    expires_in: i64,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    client: ClientInfo,
}

#[derive(Deserialize)]
struct ClientInfo {
    #[allow(dead_code)]
    client_id: String,
    #[allow(dead_code)]
    client_name: String,
    #[allow(dead_code)]
    status: String,
    #[allow(dead_code)]
    subscription_tier: String,
    #[allow(dead_code)]
    max_wallets: i64,
}

struct TokenState {
    token: String,
    expires_at: tokio::time::Instant,
}

pub(crate) struct TokenManager {
    api_key: String,
    api_secret: String,
    base_url: String,
    state: RwLock<Option<TokenState>>,
    http_client: reqwest::Client,
}

impl TokenManager {
    pub fn new(api_key: String, api_secret: String, base_url: String) -> Self {
        Self {
            api_key,
            api_secret,
            base_url,
            state: RwLock::new(None),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    pub async fn get_token(&self) -> Result<String, Error> {
        {
            let state = self.state.read().await;
            if let Some(s) = state.as_ref() {
                if tokio::time::Instant::now() < s.expires_at {
                    return Ok(s.token.clone());
                }
            }
        }
        self.refresh_token().await
    }

    async fn refresh_token(&self) -> Result<String, Error> {
        let mut state = self.state.write().await;

        // Double-check after acquiring write lock
        if let Some(s) = state.as_ref() {
            if tokio::time::Instant::now() < s.expires_at {
                return Ok(s.token.clone());
            }
        }

        let url = format!("{}/api/v1/auth/token", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .header("X-API-Secret", &self.api_secret)
            .send()
            .await
            .map_err(Error::Http)?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body: ErrorBody = resp.json().await.unwrap_or(ErrorBody {
                code: "unknown".to_string(),
                error_type: "unknown".to_string(),
                message: format!("auth request failed with status {status}"),
            });
            return Err(Error::Api { status, body });
        }

        let tok_resp: TokenResponse = resp.json().await.map_err(Error::Http)?;
        let expires_in_secs = (tok_resp.expires_in - 300).max(0) as u64;

        let token = tok_resp.token.clone();
        *state = Some(TokenState {
            token: tok_resp.token,
            expires_at: tokio::time::Instant::now()
                + std::time::Duration::from_secs(expires_in_secs),
        });

        Ok(token)
    }

}
