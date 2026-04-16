use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

use crate::config::Config;
use crate::error::{Error, ErrorBody};
use crate::token::TokenManager;

/// The main MPC SDK client.
pub struct MpcClient {
    config: Config,
    token_manager: Arc<TokenManager>,
    http_client: reqwest::Client,
}

impl MpcClient {
    /// Creates a new MPC SDK client.
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        config: Config,
    ) -> Result<Self, Error> {
        let api_key = api_key.into();
        let api_secret = api_secret.into();

        if api_key.is_empty() {
            return Err(Error::InvalidConfig("apiKey is required".to_string()));
        }
        if api_secret.is_empty() {
            return Err(Error::InvalidConfig("apiSecret is required".to_string()));
        }

        let token_manager = Arc::new(TokenManager::new(
            api_key,
            api_secret,
            config.base_url.clone(),
        ));

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
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

    pub(crate) async fn post<B: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, Error> {
        let token = self.token_manager.get_token().await?;
        let url = format!("{}{}", self.config.base_url, path);

        let resp = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .json(body)
            .send()
            .await
            .map_err(Error::Http)?;

        self.handle_response(resp).await
    }

    pub(crate) async fn get<R: DeserializeOwned>(&self, path: &str) -> Result<R, Error> {
        let token = self.token_manager.get_token().await?;
        let url = format!("{}{}", self.config.base_url, path);

        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(Error::Http)?;

        self.handle_response(resp).await
    }

    pub(crate) async fn get_with_query<R: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(String, String)],
    ) -> Result<R, Error> {
        let token = self.token_manager.get_token().await?;
        let url = format!("{}{}", self.config.base_url, path);

        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .query(params)
            .send()
            .await
            .map_err(Error::Http)?;

        self.handle_response(resp).await
    }

    pub(crate) async fn delete_with_body<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<(), Error> {
        let token = self.token_manager.get_token().await?;
        let url = format!("{}{}", self.config.base_url, path);

        let resp = self
            .http_client
            .delete(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .json(body)
            .send()
            .await
            .map_err(Error::Http)?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body: ErrorBody = resp.json().await.unwrap_or(ErrorBody {
                code: "unknown".to_string(),
                error_type: "unknown".to_string(),
                message: "failed to decode error response".to_string(),
            });
            return Err(Error::Api { status, body });
        }

        Ok(())
    }

    async fn handle_response<R: DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<R, Error> {
        let status = resp.status().as_u16();

        if status >= 400 {
            let body: ErrorBody = resp.json().await.unwrap_or(ErrorBody {
                code: "unknown".to_string(),
                error_type: "unknown".to_string(),
                message: "failed to decode error response".to_string(),
            });
            return Err(Error::Api { status, body });
        }

        resp.json().await.map_err(Error::Http)
    }
}
