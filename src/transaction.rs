use serde::Deserialize;

use crate::client::MpcClient;
use crate::error::Error;

/// A blockchain transaction.
#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub tx_id: String,
    pub wallet_id: String,
    pub client_id: String,
    pub chain: String,
    pub transaction_type: String,
    pub from_address: String,
    pub to_address: String,
    pub token_symbol: String,
    pub amount: String,
    pub status: String,
    pub tx_hash: String,
    pub created_at: String,
}

/// Request to list transactions.
#[derive(Debug, Default)]
pub struct ListTransactionsRequest {
    pub wallet_id: Option<String>,
    pub account_id: Option<String>,
    pub chain: Option<String>,
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

/// Paginated list of transactions.
#[derive(Debug, Deserialize)]
pub struct ListTransactionsResponse {
    #[serde(rename = "data")]
    pub items: Vec<Transaction>,
    pub total: i64,
    pub has_more: bool,
}

impl MpcClient {
    /// Retrieves a transaction by ID.
    pub async fn get_transaction(&self, tx_id: &str) -> Result<Transaction, Error> {
        self.get(&format!("/api/v1/transactions/{tx_id}")).await
    }

    /// Retrieves a paginated list of transactions.
    pub async fn list_transactions(
        &self,
        req: &ListTransactionsRequest,
    ) -> Result<ListTransactionsResponse, Error> {
        let mut params = Vec::new();
        if let Some(ref wallet_id) = req.wallet_id {
            params.push(("wallet_id".to_string(), wallet_id.clone()));
        }
        if let Some(ref account_id) = req.account_id {
            params.push(("account_id".to_string(), account_id.clone()));
        }
        if let Some(ref chain) = req.chain {
            params.push(("chain".to_string(), chain.clone()));
        }
        if let Some(page) = req.page {
            params.push(("page".to_string(), page.to_string()));
        }
        if let Some(page_size) = req.page_size {
            params.push(("page_size".to_string(), page_size.to_string()));
        }
        self.get_with_query("/api/v1/transactions", &params).await
    }
}
