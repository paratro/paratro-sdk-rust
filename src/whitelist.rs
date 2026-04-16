use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// A transfer whitelist entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferWhitelistItem {
    pub whitelist_id: String,
    pub chain: String,
    pub address: String,
    pub label: String,
    pub added_by: String,
    pub created_at: String,
}

/// Response for listing whitelist entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListWhitelistResponse {
    pub items: Vec<TransferWhitelistItem>,
    pub total: i64,
}

/// Request to add a whitelist entry.
#[derive(Debug, Clone, Serialize)]
pub struct AddWhitelistRequest {
    pub chain: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub mfa_code: String,
}

/// Request to delete a whitelist entry.
#[derive(Debug, Clone, Serialize)]
pub struct DeleteWhitelistRequest {
    pub mfa_code: String,
}

impl MpcClient {
    /// Lists transfer whitelist entries, optionally filtered by chain.
    pub async fn list_whitelist(
        &self,
        chain: Option<&str>,
    ) -> Result<ListWhitelistResponse, Error> {
        let mut params = Vec::new();
        if let Some(c) = chain {
            params.push(("chain".to_string(), c.to_string()));
        }
        if params.is_empty() {
            self.get("/api/v1/whitelist").await
        } else {
            self.get_with_query("/api/v1/whitelist", &params).await
        }
    }

    /// Adds a new address to the transfer whitelist.
    pub async fn add_whitelist(
        &self,
        req: &AddWhitelistRequest,
    ) -> Result<TransferWhitelistItem, Error> {
        self.post("/api/v1/whitelist", req).await
    }

    /// Deletes a whitelist entry by ID.
    pub async fn delete_whitelist(
        &self,
        whitelist_id: &str,
        req: &DeleteWhitelistRequest,
    ) -> Result<(), Error> {
        let path = format!("/api/v1/whitelist/{}", whitelist_id);
        self.delete_with_body(&path, req).await
    }
}
