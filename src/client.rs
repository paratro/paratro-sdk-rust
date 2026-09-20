use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

use crate::config::{self, Config};
use crate::error::{code, Error, ErrorBody};
use crate::token::TokenManager;

/// Name of the optional idempotency header honoured by `POST /api/v1/transactions`
/// and `POST /api/v1/x402/settle` (gateway `middleware/idempotency.go`). The gateway
/// caches a 2xx body for 24 h under `<client_id>:<key>` and replays it with HTTP
/// **200** — a first reply of `202 PENDING` comes back as `200 PENDING`.
pub const HEADER_IDEMPOTENCY_KEY: &str = "Idempotency-Key";

/// The main MPC SDK client.
pub struct MpcClient {
    config: Config,
    token_manager: Arc<TokenManager>,
    http_client: reqwest::Client,
}

/// A decoded gateway reply: HTTP status plus the deserialized body.
pub(crate) struct Reply<R> {
    pub status: u16,
    pub body: R,
}

impl MpcClient {
    /// Creates a new MPC SDK client.
    ///
    /// Fails with [`Error::InvalidConfig`] when `api_key` or `api_secret` is
    /// empty, or when `config.base_url` is not an absolute `http://` /
    /// `https://` URL. Trailing slashes are stripped from the base URL;
    /// [`MpcClient::config`] returns the normalized value. The SDK has no
    /// built-in gateway address — see [`Config::new`].
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        mut config: Config,
    ) -> Result<Self, Error> {
        let api_key = api_key.into();
        let api_secret = api_secret.into();

        if api_key.is_empty() {
            return Err(Error::InvalidConfig("apiKey is required".to_string()));
        }
        if api_secret.is_empty() {
            return Err(Error::InvalidConfig("apiSecret is required".to_string()));
        }

        // The gateway address is never built in: it must be an absolute http(s)
        // URL (Paratro cloud or a private deployment). Trailing slashes are
        // stripped so `url()` can append `/api/v1/...` verbatim.
        config.base_url = config::normalize_base_url(&config.base_url)?;

        // One timeout for every exchange, the auth call included: a shorter
        // auth timeout would cut a PROGRAM_CALL / CONTRACT_CALL short at the
        // token step. See `config::DEFAULT_TIMEOUT` for why 200 s.
        let token_manager = Arc::new(TokenManager::new(
            api_key,
            api_secret,
            config.base_url.clone(),
            config.timeout,
        )?);

        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(Error::Http)?;

        Ok(Self {
            config,
            token_manager,
            http_client,
        })
    }

    /// Returns the client configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    pub(crate) async fn post<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, Error> {
        Ok(self.post_with_status(path, body).await?.body)
    }

    /// POST with extra request headers (e.g. [`HEADER_IDEMPOTENCY_KEY`]).
    pub(crate) async fn post_with_headers<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        headers: &[(&str, &str)],
    ) -> Result<R, Error> {
        Ok(self
            .post_with_status_and_headers(path, body, headers)
            .await?
            .body)
    }

    /// POST that also reports the HTTP status — `POST /api/v1/transactions`
    /// distinguishes `200` (done / broadcast) from `202` (outcome unknown).
    pub(crate) async fn post_with_status<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<Reply<R>, Error> {
        self.post_with_status_and_headers(path, body, &[]).await
    }

    /// [`MpcClient::post_with_status`] plus extra request headers. The headers
    /// are sent on the `token_expired` retry as well.
    pub(crate) async fn post_with_status_and_headers<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        headers: &[(&str, &str)],
    ) -> Result<Reply<R>, Error> {
        let url = self.url(path);
        let payload =
            serde_json::to_vec(body).map_err(|source| Error::Decode { status: 0, source })?;
        self.send(|token| {
            let mut builder = self
                .http_client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {token}"));
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            builder.body(payload.clone())
        })
        .await
    }

    pub(crate) async fn get<R: DeserializeOwned>(&self, path: &str) -> Result<R, Error> {
        let url = self.url(path);
        Ok(self
            .send(|token| {
                self.http_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {token}"))
            })
            .await?
            .body)
    }

    pub(crate) async fn get_with_query<R: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
    ) -> Result<R, Error> {
        let url = self.url(path);
        Ok(self
            .send(|token| {
                self.http_client
                    .get(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .query(params)
            })
            .await?
            .body)
    }

    /// Sends a request built by `build(token)`. If the gateway answers
    /// `401 token_expired`, the token is refreshed and the request is retried
    /// exactly once; any other error is returned as-is.
    async fn send<F, R>(&self, build: F) -> Result<Reply<R>, Error>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
        R: DeserializeOwned,
    {
        let token = self.token_manager.get_token().await?;
        let (status, bytes) = Self::execute(build(&token)).await?;
        let (status, bytes) = if status == 401 && is_token_expired(&bytes) {
            let token = self.token_manager.refresh_after_expiry(&token).await?;
            Self::execute(build(&token)).await?
        } else {
            (status, bytes)
        };
        decode(status, &bytes)
    }

    async fn execute(request: reqwest::RequestBuilder) -> Result<(u16, Vec<u8>), Error> {
        let resp = request.send().await.map_err(Error::Http)?;
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await.map_err(Error::Http)?;
        Ok((status, bytes.to_vec()))
    }
}

fn is_token_expired(bytes: &[u8]) -> bool {
    serde_json::from_slice::<ErrorBody>(bytes)
        .map(|b| b.code == code::TOKEN_EXPIRED)
        .unwrap_or(false)
}

fn decode<R: DeserializeOwned>(status: u16, bytes: &[u8]) -> Result<Reply<R>, Error> {
    if status >= 400 {
        let body: ErrorBody = serde_json::from_slice(bytes).unwrap_or(ErrorBody {
            code: "unknown".to_string(),
            error_type: "unknown".to_string(),
            message: "failed to decode error response".to_string(),
        });
        return Err(Error::Api { status, body });
    }
    let body = serde_json::from_slice(bytes).map_err(|source| Error::Decode { status, source })?;
    Ok(Reply { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BASE_URL_ERROR;

    fn client_with(base_url: &str) -> Result<MpcClient, Error> {
        MpcClient::new("key", "secret", Config::new(base_url))
    }

    #[test]
    fn new_normalizes_the_base_url() {
        let client = client_with("https://gateway.example///").unwrap();
        assert_eq!(client.config().base_url, "https://gateway.example");
        assert_eq!(
            client.url("/api/v1/wallets"),
            "https://gateway.example/api/v1/wallets"
        );
        assert_eq!(
            client_with("http://127.0.0.1:8080/")
                .unwrap()
                .config()
                .base_url,
            "http://127.0.0.1:8080"
        );
    }

    #[test]
    fn new_rejects_a_base_url_that_is_not_an_absolute_http_url() {
        for given in ["", "gateway.example", "ftp://gateway.example", "https://"] {
            match client_with(given) {
                Err(Error::InvalidConfig(msg)) => assert_eq!(msg, BASE_URL_ERROR, "{given:?}"),
                Err(other) => panic!("{given:?}: expected InvalidConfig, got {other:?}"),
                Ok(_) => panic!("{given:?}: expected InvalidConfig"),
            }
        }
    }

    #[test]
    fn new_still_requires_key_and_secret() {
        let cfg = || Config::new("https://gateway.example");
        assert!(matches!(
            MpcClient::new("", "secret", cfg()),
            Err(Error::InvalidConfig(_))
        ));
        assert!(matches!(
            MpcClient::new("key", "", cfg()),
            Err(Error::InvalidConfig(_))
        ));
    }
}
