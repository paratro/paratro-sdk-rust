use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::{Error, ErrorBody};

/// Header carrying the API key on `POST /api/v1/auth/token`.
pub const HEADER_API_KEY: &str = "X-API-Key";
/// Header carrying the API secret on `POST /api/v1/auth/token`.
pub const HEADER_API_SECRET: &str = "X-API-Secret";

/// The SDK refreshes this many seconds before the `expires_in` the gateway reported.
const REFRESH_BUFFER_SECS: i64 = 120;

/// `dto.TokenResponse` on the gateway.
#[derive(Deserialize)]
struct TokenResponse {
    token: String,
    /// Lifetime in seconds as reported by the gateway. The gateway caps its
    /// configured `access-token-expire` at 900 s (`internal/conf/config.go`
    /// `GetJWTExpireSeconds`), so today this is 15 minutes on every environment;
    /// never assume a fixed lifetime — use this field.
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
    #[serde(default)]
    subscription_tier: String,
    #[allow(dead_code)]
    #[serde(default)]
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
    pub fn new(
        api_key: String,
        api_secret: String,
        base_url: String,
        timeout: std::time::Duration,
    ) -> Result<Self, Error> {
        Ok(Self {
            api_key,
            api_secret,
            base_url,
            state: RwLock::new(None),
            http_client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .map_err(Error::Http)?,
        })
    }

    /// Returns a cached token, refreshing proactively when it is within
    /// [`REFRESH_BUFFER_SECS`] of the lifetime the gateway reported.
    pub async fn get_token(&self) -> Result<String, Error> {
        {
            let state = self.state.read().await;
            if let Some(s) = state.as_ref() {
                if tokio::time::Instant::now() < s.expires_at {
                    return Ok(s.token.clone());
                }
            }
        }
        self.refresh(None).await
    }

    /// Called after the gateway answered `401 token_expired` for `stale`: drops the
    /// cached token if it is still the stale one and fetches a new one. If another
    /// task already replaced it, that token is returned without a second round-trip.
    pub async fn refresh_after_expiry(&self, stale: &str) -> Result<String, Error> {
        self.refresh(Some(stale)).await
    }

    async fn refresh(&self, stale: Option<&str>) -> Result<String, Error> {
        let mut state = self.state.write().await;

        // Double-check after acquiring the write lock.
        if let Some(s) = state.as_ref() {
            let still_fresh = tokio::time::Instant::now() < s.expires_at;
            let replaced = stale.is_some_and(|stale| stale != s.token);
            if (stale.is_none() && still_fresh) || replaced {
                return Ok(s.token.clone());
            }
        }

        let url = format!("{}/api/v1/auth/token", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .header(HEADER_API_KEY, &self.api_key)
            .header(HEADER_API_SECRET, &self.api_secret)
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
            *state = None;
            return Err(Error::Api { status, body });
        }

        let bytes = resp.bytes().await.map_err(Error::Http)?;
        let tok_resp: TokenResponse =
            serde_json::from_slice(&bytes).map_err(|source| Error::Decode { status, source })?;
        let expires_in_secs = (tok_resp.expires_in - REFRESH_BUFFER_SECS).max(0) as u64;

        let token = tok_resp.token.clone();
        *state = Some(TokenState {
            token: tok_resp.token,
            expires_at: tokio::time::Instant::now()
                + std::time::Duration::from_secs(expires_in_secs),
        });

        Ok(token)
    }
}
