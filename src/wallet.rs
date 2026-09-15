use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// Request to create a new MPC wallet.
#[derive(Debug, Serialize)]
pub struct CreateWalletRequest {
    pub wallet_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// An MPC wallet.
#[derive(Debug, Deserialize)]
pub struct Wallet {
    pub wallet_id: String,
    pub client_id: String,
    pub wallet_name: String,
    pub description: String,
    pub status: String,
    pub key_status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Request to list wallets.
#[derive(Debug, Default)]
pub struct ListWalletsRequest {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

/// Paginated list of wallets.
#[derive(Debug, Deserialize)]
pub struct ListWalletsResponse {
    #[serde(rename = "data")]
    pub items: Vec<Wallet>,
    pub total: i64,
    pub has_more: bool,
}

impl MpcClient {
    /// Creates a new MPC wallet.
    pub async fn create_wallet(&self, req: &CreateWalletRequest) -> Result<Wallet, Error> {
        self.post("/api/v1/wallets", req).await
    }

    /// Retrieves a wallet by ID.
    pub async fn get_wallet(&self, wallet_id: &str) -> Result<Wallet, Error> {
        self.get(&format!("/api/v1/wallets/{wallet_id}")).await
    }

    /// Retrieves a paginated list of wallets.
    pub async fn list_wallets(
        &self,
        req: &ListWalletsRequest,
    ) -> Result<ListWalletsResponse, Error> {
        let mut params = Vec::new();
        if let Some(page) = req.page {
            params.push(("page".to_string(), page.to_string()));
        }
        if let Some(page_size) = req.page_size {
            params.push(("page_size".to_string(), page_size.to_string()));
        }
        self.get_with_query("/api/v1/wallets", &params).await
    }
}
