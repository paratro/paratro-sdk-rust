use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// Request to create a new account.
#[derive(Debug, Serialize)]
pub struct CreateAccountRequest {
    pub wallet_id: String,
    pub chain: String,
    pub network: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// An account in a wallet.
#[derive(Debug, Deserialize)]
pub struct Account {
    pub account_id: String,
    pub wallet_id: String,
    pub client_id: String,
    pub address: String,
    pub chain: String,
    pub network: String,
    pub address_type: String,
    pub label: String,
    pub status: String,
    pub created_at: String,
}

/// Request to list accounts.
#[derive(Debug, Default)]
pub struct ListAccountsRequest {
    pub wallet_id: Option<String>,
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

/// Paginated list of accounts.
#[derive(Debug, Deserialize)]
pub struct ListAccountsResponse {
    #[serde(rename = "data")]
    pub items: Vec<Account>,
    pub total: i64,
    pub has_more: bool,
}

impl MpcClient {
    /// Creates a new account in a wallet.
    pub async fn create_account(&self, req: &CreateAccountRequest) -> Result<Account, Error> {
        self.post("/api/v1/accounts", req).await
    }

    /// Retrieves an account by ID.
    pub async fn get_account(&self, account_id: &str) -> Result<Account, Error> {
        self.get(&format!("/api/v1/accounts/{account_id}")).await
    }

    /// Retrieves a paginated list of accounts.
    pub async fn list_accounts(
        &self,
        req: &ListAccountsRequest,
    ) -> Result<ListAccountsResponse, Error> {
        let mut params = Vec::new();
        if let Some(ref wallet_id) = req.wallet_id {
            params.push(("wallet_id".to_string(), wallet_id.clone()));
        }
        if let Some(page) = req.page {
            params.push(("page".to_string(), page.to_string()));
        }
        if let Some(page_size) = req.page_size {
            params.push(("page_size".to_string(), page_size.to_string()));
        }
        self.get_with_query("/api/v1/accounts", &params).await
    }
}
