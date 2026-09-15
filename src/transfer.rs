//! Backwards-compatible transfer API.
//!
//! `POST /api/v1/transfer` was retired by the gateway (it now answers
//! `410 endpoint_retired`). [`MpcClient::create_transfer`] is kept as a thin
//! wrapper that sends the same request through the unified entry
//! `POST /api/v1/transactions` with `operation = "TRANSFER"`. New code should call
//! [`MpcClient::create_transaction`] directly.

use crate::client::MpcClient;
use crate::error::Error;
use crate::transaction::{CreateTransactionRequest, CreateTransactionResponse, TransferRequest};

/// Request to create a transfer. Alias of [`TransferRequest`]; kept for pre-1.8 callers.
pub type CreateTransferRequest = TransferRequest;

/// Response of a transfer creation. Alias of [`CreateTransactionResponse`]; kept for
/// pre-1.8 callers (`tx_id`, `status`, `message` are unchanged; `tx_hash`,
/// `http_status` and `operation` are new).
pub type TransferResponse = CreateTransactionResponse;

impl MpcClient {
    /// Creates a transfer to an external address.
    ///
    /// Compatibility wrapper: since 1.8.1 this sends
    /// `POST /api/v1/transactions {"operation":"TRANSFER", ...}` — the old
    /// `POST /api/v1/transfer` route is gone (HTTP 410). Behaviour is otherwise
    /// identical: signing is asynchronous and the reply is `200 {status:"PENDING"}`.
    pub async fn create_transfer(
        &self,
        req: &CreateTransferRequest,
    ) -> Result<TransferResponse, Error> {
        self.create_transaction(&CreateTransactionRequest::Transfer(req.clone()))
            .await
    }
}
