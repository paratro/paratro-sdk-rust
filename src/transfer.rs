use serde::{Deserialize, Serialize};

use crate::client::MpcClient;
use crate::error::Error;

/// Request to create a transfer.
#[derive(Debug, Serialize)]
pub struct CreateTransferRequest {
    pub from_address: String,
    pub to_address: String,
    pub chain: String,
    pub token_symbol: String,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

/// Response of a transfer creation.
#[derive(Debug, Deserialize)]
pub struct TransferResponse {
    pub tx_id: String,
    pub status: String,
    pub message: String,
}

impl MpcClient {
    /// Creates a transfer to send funds to an external address.
    pub async fn create_transfer(
        &self,
        req: &CreateTransferRequest,
    ) -> Result<TransferResponse, Error> {
        self.post("/api/v1/transfer", req).await
    }
}
