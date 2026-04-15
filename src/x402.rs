use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// Request to create an ERC-3009 authorization signature.
#[derive(Debug, Serialize)]
pub struct X402SignRequest {
    pub from_address: String,
    pub to_address: String,
    pub chain: String,
    pub amount: String,
    pub valid_before: String,
}

/// Response from signing an x402 authorization.
#[derive(Debug, Deserialize)]
pub struct X402SignResponse {
    pub auth_id: String,
    pub tx_id: String,
    pub status: String,
    pub nonce: String,
    pub eip712_hash: String,
    pub signature_v: String,
    pub signature_r: String,
    pub signature_s: String,
    #[serde(default)]
    pub error: String,
}

/// An x402 authorization record.
#[derive(Debug, Deserialize)]
pub struct X402Authorization {
    pub auth_id: String,
    pub tx_id: String,
    pub from_address: String,
    pub to_address: String,
    pub chain: String,
    pub amount: String,
    pub status: String,
    pub nonce: String,
    #[serde(default)]
    pub eip712_hash: String,
    #[serde(default)]
    pub signature_v: String,
    #[serde(default)]
    pub signature_r: String,
    #[serde(default)]
    pub signature_s: String,
    #[serde(default)]
    pub valid_before: String,
    #[serde(default)]
    pub created_at: String,
}

/// Request to list x402 authorizations.
#[derive(Debug, Default)]
pub struct ListX402AuthorizationsRequest {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

/// Paginated list of x402 authorizations.
#[derive(Debug, Deserialize)]
pub struct ListX402AuthorizationsResponse {
    #[serde(rename = "data")]
    pub items: Vec<X402Authorization>,
    pub total: i64,
    pub has_more: bool,
}

/// Response from verifying an x402 payment signature.
#[derive(Debug, Deserialize)]
pub struct X402VerifyResponse {
    pub is_valid: bool,
    #[serde(default)]
    pub invalid_reason: String,
    #[serde(default)]
    pub payer: String,
}

/// Response from executing an x402 on-chain settlement.
#[derive(Debug, Deserialize)]
pub struct X402SettleResponse {
    pub success: bool,
    #[serde(default)]
    pub transaction: String,
    #[serde(default)]
    pub network: String,
    #[serde(default)]
    pub payer: String,
    #[serde(default)]
    pub tx_id: String,
    #[serde(default)]
    pub error: String,
}

/// Response for querying a settle transaction status.
#[derive(Debug, Deserialize)]
pub struct X402SettleStatusResponse {
    pub tx_id: String,
    pub status: String,
    #[serde(default)]
    pub tx_hash: String,
    #[serde(default)]
    pub network: String,
}

impl MpcClient {
    /// Creates an ERC-3009 authorization signature.
    pub async fn x402_sign(&self, req: &X402SignRequest) -> Result<X402SignResponse, Error> {
        self.post("/api/v1/x402/sign", req).await
    }

    /// Retrieves a paginated list of x402 authorization records.
    pub async fn x402_list_authorizations(
        &self,
        req: &ListX402AuthorizationsRequest,
    ) -> Result<ListX402AuthorizationsResponse, Error> {
        let mut params = Vec::new();
        if let Some(page) = req.page {
            params.push(("page".to_string(), page.to_string()));
        }
        if let Some(page_size) = req.page_size {
            params.push(("page_size".to_string(), page_size.to_string()));
        }
        self.get_with_query("/api/v1/x402/transactions", &params)
            .await
    }

    /// Verifies a payment signature.
    pub async fn x402_verify(
        &self,
        payload: &serde_json::Value,
    ) -> Result<X402VerifyResponse, Error> {
        self.post("/api/v1/x402/verify", payload).await
    }

    /// Executes on-chain settlement.
    pub async fn x402_settle(
        &self,
        payload: &serde_json::Value,
    ) -> Result<X402SettleResponse, Error> {
        self.post("/api/v1/x402/settle", payload).await
    }

    /// Retrieves the status of a settle transaction.
    pub async fn x402_settle_status(&self, tx_id: &str) -> Result<X402SettleStatusResponse, Error> {
        self.get(&format!("/api/v1/x402/settle/{tx_id}")).await
    }
}
