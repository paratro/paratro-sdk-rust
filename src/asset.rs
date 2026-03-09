use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// Request to add a new asset.
#[derive(Debug, Serialize)]
pub struct CreateAssetRequest {
    pub account_id: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
}

/// An asset (token) in an account.
#[derive(Debug, Deserialize)]
pub struct Asset {
    pub asset_id: String,
    pub account_id: String,
    pub wallet_id: String,
    pub client_id: String,
    pub chain: String,
    pub network: String,
    pub symbol: String,
    pub name: String,
    pub contract_address: String,
    pub decimals: i32,
    pub asset_type: String,
    pub balance: String,
    pub locked_balance: String,
    pub is_active: bool,
    pub created_at: String,
}

/// Request to list assets.
#[derive(Debug, Default)]
pub struct ListAssetsRequest {
    pub account_id: Option<String>,
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

/// Paginated list of assets.
#[derive(Debug, Deserialize)]
pub struct ListAssetsResponse {
    #[serde(rename = "data")]
    pub items: Vec<Asset>,
    pub total: i64,
    pub has_more: bool,
}

impl MpcClient {
    /// Creates a new asset for an account.
    pub async fn create_asset(&self, req: &CreateAssetRequest) -> Result<Asset, Error> {
        self.post("/api/v1/assets", req).await
    }

    /// Retrieves an asset by ID.
    pub async fn get_asset(&self, asset_id: &str) -> Result<Asset, Error> {
        self.get(&format!("/api/v1/assets/{asset_id}")).await
    }

    /// Retrieves a paginated list of assets.
    pub async fn list_assets(
        &self,
        req: &ListAssetsRequest,
    ) -> Result<ListAssetsResponse, Error> {
        let mut params = Vec::new();
        if let Some(ref account_id) = req.account_id {
            params.push(("account_id".to_string(), account_id.clone()));
        }
        if let Some(page) = req.page {
            params.push(("page".to_string(), page.to_string()));
        }
        if let Some(page_size) = req.page_size {
            params.push(("page_size".to_string(), page_size.to_string()));
        }
        self.get_with_query("/api/v1/assets", &params).await
    }
}
