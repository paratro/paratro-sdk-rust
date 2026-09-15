//! Unified transaction entry: `POST /api/v1/transactions`, plus
//! `GET /api/v1/transactions/{id}` and `GET /api/v1/transactions`.
//!
//! One endpoint, three operations selected by the `operation` field:
//!
//! | operation       | what it does                                             | success reply |
//! |-----------------|----------------------------------------------------------|---------------|
//! | `TRANSFER`      | send `amount` of `token_symbol` to `to_address` (async signing) | `200 {tx_id,status:"PENDING",message}` |
//! | `PROGRAM_CALL`  | co-sign & broadcast a partially signed Solana transaction | `200 {tx_id,status:"BROADCAST",message,tx_hash}` or `202` |
//! | `CONTRACT_CALL` | sign & broadcast an EVM `executeSwap` (xChange) quote     | `200 {tx_id,status:"BROADCAST",message,tx_hash}` or `202` |
//!
//! A `202 Accepted` (`status:"PENDING"`) means the signing engine did not answer in
//! time: the outcome is unknown. Poll [`MpcClient::get_transaction`] with the returned
//! `tx_id`; **do not** resubmit with the same `reference_id` — that hits the
//! uniqueness key and returns `400 Duplicate reference_id`.

use serde::{Deserialize, Serialize};

use crate::client::{MpcClient, HEADER_IDEMPOTENCY_KEY};
use crate::error::Error;

/// Path of the unified entry.
pub const TRANSACTIONS_PATH: &str = "/api/v1/transactions";

/// `operation` value for a token transfer (also the gateway default when omitted).
pub const OPERATION_TRANSFER: &str = "TRANSFER";
/// `operation` value for a Solana program call.
pub const OPERATION_PROGRAM_CALL: &str = "PROGRAM_CALL";
/// `operation` value for an EVM contract call.
pub const OPERATION_CONTRACT_CALL: &str = "CONTRACT_CALL";

/// `status` in a create reply: queued for asynchronous signing (`TRANSFER`, HTTP 200)
/// or outcome unknown (`PROGRAM_CALL` / `CONTRACT_CALL`, HTTP 202).
pub const STATUS_PENDING: &str = "PENDING";
/// `status` in a create reply: transaction signed and broadcast (HTTP 200).
pub const STATUS_BROADCAST: &str = "BROADCAST";

// ───────────────────────────── requests ─────────────────────────────

/// `operation = "TRANSFER"`: move `amount` of `token_symbol` from `from_address`
/// to `to_address`. `amount` is a human-readable decimal string (e.g. `"10.5"`,
/// at most 18 decimal places). Signing is asynchronous: the reply is `PENDING`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferRequest {
    pub from_address: String,
    pub to_address: String,
    pub chain: String,
    pub token_symbol: String,
    pub amount: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    /// Caller's business reference (≤ 100 chars). Reusing it → `400 Duplicate reference_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
}

impl TransferRequest {
    pub fn new(
        from_address: impl Into<String>,
        to_address: impl Into<String>,
        chain: impl Into<String>,
        token_symbol: impl Into<String>,
        amount: impl Into<String>,
    ) -> Self {
        Self {
            from_address: from_address.into(),
            to_address: to_address.into(),
            chain: chain.into(),
            token_symbol: token_symbol.into(),
            amount: amount.into(),
            memo: None,
            reference_id: None,
        }
    }

    pub fn with_memo(mut self, memo: impl Into<String>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    pub fn with_reference_id(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }
}

/// `operation = "PROGRAM_CALL"` (Solana): the counterparty has partially signed a
/// fixed-shape transaction; the engine fills our fee-payer signature slot and
/// broadcasts it.
///
/// * `signed_transaction` — the partially signed transaction, base64 or `0x`-hex.
/// * `receive_address` — our receiving wallet (the incoming leg's owner); defaults
///   to `from_address` when omitted. Both must be accounts of this client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramCallRequest {
    pub from_address: String,
    pub chain: String,
    pub signed_transaction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

impl ProgramCallRequest {
    pub fn new(
        from_address: impl Into<String>,
        chain: impl Into<String>,
        signed_transaction: impl Into<String>,
    ) -> Self {
        Self {
            from_address: from_address.into(),
            chain: chain.into(),
            signed_transaction: signed_transaction.into(),
            receive_address: None,
            reference_id: None,
            memo: None,
        }
    }

    pub fn with_receive_address(mut self, receive_address: impl Into<String>) -> Self {
        self.receive_address = Some(receive_address.into());
        self
    }

    pub fn with_reference_id(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }

    pub fn with_memo(mut self, memo: impl Into<String>) -> Self {
        self.memo = Some(memo.into());
        self
    }
}

/// Our side's outgoing leg of a `CONTRACT_CALL` (what we pay the counterparty).
/// `from` is filled in by the gateway (`= from_address`) and is not sent.
/// `amount` is a smallest-unit decimal integer string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCallIncomingLeg {
    pub to: String,
    pub token: String,
    pub amount: String,
}

/// The counterparty's leg of a `CONTRACT_CALL` (what they pay us).
/// `amount` is a smallest-unit decimal integer string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCallOutgoingLeg {
    pub from: String,
    pub to: String,
    pub token: String,
    pub amount: String,
}

/// The `contract_call` body of a `CONTRACT_CALL`: the counterparty's `executeSwap`
/// quote, forwarded as-is. The contract address is **not** part of it — the
/// gateway takes it from the policy (`allowed_contracts[chain]`); native value is
/// always 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCall {
    /// bytes32 hex, `0x` prefix optional.
    pub quote_id: String,
    /// Quote expiry, unix seconds. Must be in the future and within the policy's
    /// `max_execution_timeout_seconds`.
    pub expiration: i64,
    pub incoming: ContractCallIncomingLeg,
    pub outgoing: ContractCallOutgoingLeg,
    /// Counterparty's EIP-712 signature over the quote (hex). Verified by the contract.
    pub counterparty_signature: String,
    /// Optional permit deadline (unix seconds). Default:
    /// `now + min(fee_limits.max_permit_lifetime_seconds, 300)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permit_deadline: Option<i64>,
}

impl ContractCall {
    pub fn new(
        quote_id: impl Into<String>,
        expiration: i64,
        incoming: ContractCallIncomingLeg,
        outgoing: ContractCallOutgoingLeg,
        counterparty_signature: impl Into<String>,
    ) -> Self {
        Self {
            quote_id: quote_id.into(),
            expiration,
            incoming,
            outgoing,
            counterparty_signature: counterparty_signature.into(),
            permit_deadline: None,
        }
    }

    pub fn with_permit_deadline(mut self, permit_deadline: i64) -> Self {
        self.permit_deadline = Some(permit_deadline);
        self
    }
}

/// `operation = "CONTRACT_CALL"` (EVM): sign and broadcast an xChange `executeSwap`.
///
/// * `from_address` — our paying wallet = `incoming` leg's `from` = permit owner =
///   transaction sender. The quote must have been generated for this address.
/// * `receive_address` — our receiving wallet = `outgoing.to`; defaults to `from_address`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCallRequest {
    pub from_address: String,
    pub chain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_address: Option<String>,
    pub contract_call: ContractCall,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
}

impl ContractCallRequest {
    pub fn new(
        from_address: impl Into<String>,
        chain: impl Into<String>,
        contract_call: ContractCall,
    ) -> Self {
        Self {
            from_address: from_address.into(),
            chain: chain.into(),
            receive_address: None,
            contract_call,
            reference_id: None,
            memo: None,
        }
    }

    pub fn with_receive_address(mut self, receive_address: impl Into<String>) -> Self {
        self.receive_address = Some(receive_address.into());
        self
    }

    pub fn with_reference_id(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }

    pub fn with_memo(mut self, memo: impl Into<String>) -> Self {
        self.memo = Some(memo.into());
        self
    }
}

/// Body of `POST /api/v1/transactions`. Serializes as the variant's fields plus
/// `"operation": "<TRANSFER|PROGRAM_CALL|CONTRACT_CALL>"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum CreateTransactionRequest {
    #[serde(rename = "TRANSFER")]
    Transfer(TransferRequest),
    #[serde(rename = "PROGRAM_CALL")]
    ProgramCall(ProgramCallRequest),
    #[serde(rename = "CONTRACT_CALL")]
    ContractCall(ContractCallRequest),
}

impl CreateTransactionRequest {
    pub fn transfer(req: TransferRequest) -> Self {
        Self::Transfer(req)
    }

    pub fn program_call(req: ProgramCallRequest) -> Self {
        Self::ProgramCall(req)
    }

    pub fn contract_call(req: ContractCallRequest) -> Self {
        Self::ContractCall(req)
    }

    /// The `operation` string this request carries.
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Transfer(_) => OPERATION_TRANSFER,
            Self::ProgramCall(_) => OPERATION_PROGRAM_CALL,
            Self::ContractCall(_) => OPERATION_CONTRACT_CALL,
        }
    }

    /// The caller's `reference_id`, if set.
    pub fn reference_id(&self) -> Option<&str> {
        match self {
            Self::Transfer(r) => r.reference_id.as_deref(),
            Self::ProgramCall(r) => r.reference_id.as_deref(),
            Self::ContractCall(r) => r.reference_id.as_deref(),
        }
    }
}

impl From<TransferRequest> for CreateTransactionRequest {
    fn from(req: TransferRequest) -> Self {
        Self::Transfer(req)
    }
}

impl From<ProgramCallRequest> for CreateTransactionRequest {
    fn from(req: ProgramCallRequest) -> Self {
        Self::ProgramCall(req)
    }
}

impl From<ContractCallRequest> for CreateTransactionRequest {
    fn from(req: ContractCallRequest) -> Self {
        Self::ContractCall(req)
    }
}

// ───────────────────────────── responses ─────────────────────────────

/// Reply of `POST /api/v1/transactions` (`dto.TransferResponse` on the gateway).
///
/// `http_status` is `200` when the request completed (`TRANSFER` queued, or
/// `PROGRAM_CALL` / `CONTRACT_CALL` broadcast) and `202` when the engine's outcome is
/// unknown — see [`CreateTransactionResponse::is_accepted`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateTransactionResponse {
    pub tx_id: String,
    /// `PENDING` or `BROADCAST`.
    pub status: String,
    pub message: String,
    /// Only present for synchronously broadcast operations (`PROGRAM_CALL` / `CONTRACT_CALL`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    /// HTTP status the gateway answered with (`200` or `202`). Not part of the JSON body.
    #[serde(skip)]
    pub http_status: u16,
    /// `operation` of the request this reply answers ([`OPERATION_TRANSFER`],
    /// [`OPERATION_PROGRAM_CALL`] or [`OPERATION_CONTRACT_CALL`]). The gateway does
    /// not echo it; [`MpcClient::create_transaction`] fills it in. Empty when the
    /// value was deserialized by hand.
    #[serde(skip)]
    pub operation: String,
}

impl CreateTransactionResponse {
    /// `true` when the outcome of a `PROGRAM_CALL` / `CONTRACT_CALL` is unknown: the
    /// transaction row was created but the signing engine did not answer in time. It
    /// may still be signing or may already be broadcast. Poll
    /// [`MpcClient::get_transaction`] with `tx_id`; do **not** resubmit with the same
    /// `reference_id` (it would be rejected as a duplicate), and do not resubmit with
    /// a new one either or you may pay twice.
    ///
    /// The gateway signals this with HTTP `202`. An `Idempotency-Key` replay of such
    /// an answer comes back as HTTP `200` with the same body (`status = "PENDING"`,
    /// no `tx_hash`), so this also treats "`PROGRAM_CALL` / `CONTRACT_CALL` with
    /// status `PENDING`" as accepted: those operations never answer `PENDING`
    /// otherwise. A `TRANSFER`'s normal `200 PENDING` is not "accepted" in this
    /// sense. Same rule as `Accepted()` in the Go SDK.
    pub fn is_accepted(&self) -> bool {
        if self.http_status == 202 {
            return true;
        }
        !self.operation.is_empty()
            && self.operation != OPERATION_TRANSFER
            && self.status == STATUS_PENDING
    }

    /// `true` when the operation was signed and broadcast (`status == "BROADCAST"`).
    pub fn is_broadcast(&self) -> bool {
        self.status == STATUS_BROADCAST
    }
}

/// A transaction as returned by `GET /api/v1/transactions/{id}` and the list
/// endpoint (`dto.TransactionResponse` on the gateway).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Omitted by the gateway when the transaction was not risk-scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<String>,
    /// Omitted by the gateway when the transaction was not risk-scored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    pub created_at: String,
}

/// Query of `GET /api/v1/transactions` (`dto.TransactionRequest` on the gateway).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListTransactionsRequest {
    pub wallet_id: Option<String>,
    pub account_id: Option<String>,
    pub chain: Option<String>,
    /// 1-based; gateway default 1.
    pub page: Option<i32>,
    /// 1..=100; gateway default 20.
    pub page_size: Option<i32>,
}

/// Paginated list of transactions (`common.PagedBody`).
#[derive(Debug, Clone, Deserialize)]
pub struct ListTransactionsResponse {
    #[serde(rename = "data")]
    pub items: Vec<Transaction>,
    pub total: i64,
    pub has_more: bool,
}

impl MpcClient {
    /// `POST /api/v1/transactions` — creates a `TRANSFER`, `PROGRAM_CALL` or
    /// `CONTRACT_CALL`.
    ///
    /// * `Ok(resp)` with `resp.http_status == 200`: done (`TRANSFER` → `PENDING`,
    ///   asynchronous signing; `PROGRAM_CALL` / `CONTRACT_CALL` → `BROADCAST` with `tx_hash`).
    /// * `Ok(resp)` with `resp.is_accepted()` (HTTP 202 `PENDING`, or its
    ///   `Idempotency-Key` replay as HTTP 200 `PENDING`): outcome unknown —
    ///   poll [`MpcClient::get_transaction`]; never resubmit with the same `reference_id`.
    /// * `Err(e)` — inspect [`Error::kind`] / [`Error::reason_tag`]:
    ///   `400 "Rejected: <tag>: …"` policy / verifier rejection (normally pre-row; the
    ///   `CONTRACT_CALL` post-sign re-verification rejects **after** the row was inserted and
    ///   the permit signed — that row is held `PENDING` until the permit deadline and keeps
    ///   its `reference_id`), `400 Duplicate reference_id`, `400 insufficient_balance`,
    ///   `400 transaction_failed` (engine failed after the row was created; the row is
    ///   `FAILED`, or — for a `CONTRACT_CALL` whose EIP-2612 permit was already signed —
    ///   held `PENDING` with its reservation locked until the permit deadline, so a retry
    ///   can see `insufficient_balance` meanwhile; either way the `reference_id` is
    ///   consumed), `403` no policy authorises the operation, `404` address / asset not
    ///   this client's or token not credited yet, `503` — two different cases:
    ///   [`ApiErrorKind::ChainRpcUnavailable`](crate::ApiErrorKind::ChainRpcUnavailable)
    ///   (`"Chain RPC unavailable; cannot verify request"`) is raised before any row exists,
    ///   so the same `reference_id` can be retried later;
    ///   [`ApiErrorKind::EngineBusy`](crate::ApiErrorKind::EngineBusy)
    ///   (`"Signing service is busy, retry later"`) is raised **after** the row was created —
    ///   it is `FAILED` (or held `PENDING` like the `transaction_failed` case above when a
    ///   live permit exists) and the `reference_id` is consumed, so a fresh attempt needs a
    ///   **new** `reference_id` (a retry with the old one returns `400 Duplicate reference_id`).
    ///
    /// The gateway offers no lookup by `reference_id` (`GET /api/v1/transactions` filters
    /// only `wallet_id` / `account_id` / `chain`), so persist `reference_id → tx_id` from the
    /// first reply yourself; after a `400 Duplicate reference_id` that stored `tx_id` is the
    /// only way back to the original transaction. To be able to recover the original reply
    /// after a transport failure, send an `Idempotency-Key` with
    /// [`MpcClient::create_transaction_with_idempotency_key`].
    pub async fn create_transaction(
        &self,
        req: &CreateTransactionRequest,
    ) -> Result<CreateTransactionResponse, Error> {
        self.create_transaction_with_headers(req, &[]).await
    }

    /// [`MpcClient::create_transaction`] with an `Idempotency-Key` header
    /// ([`crate::HEADER_IDEMPOTENCY_KEY`]).
    ///
    /// The gateway caches the 2xx body for 24 h under `<client_id>:<key>` and replays
    /// it on a repeat — always with HTTP **200**, even when the original answer was
    /// `202`. Because of that, [`CreateTransactionResponse::is_accepted`] looks at
    /// `status` / `operation` and not only at `http_status`, and stays `true` on a
    /// replayed `202`. Replaying the identical request (same `reference_id`, same key)
    /// after a timeout or connection reset is therefore safe: you either get the
    /// original body back or — when the cache was never written — the normal
    /// `400 Duplicate reference_id` / first answer.
    pub async fn create_transaction_with_idempotency_key(
        &self,
        req: &CreateTransactionRequest,
        idempotency_key: &str,
    ) -> Result<CreateTransactionResponse, Error> {
        self.create_transaction_with_headers(req, &[(HEADER_IDEMPOTENCY_KEY, idempotency_key)])
            .await
    }

    async fn create_transaction_with_headers(
        &self,
        req: &CreateTransactionRequest,
        headers: &[(&str, &str)],
    ) -> Result<CreateTransactionResponse, Error> {
        let reply = self
            .post_with_status_and_headers::<_, CreateTransactionResponse>(
                TRANSACTIONS_PATH,
                req,
                headers,
            )
            .await?;
        let mut resp = reply.body;
        resp.http_status = reply.status;
        resp.operation = req.operation().to_string();
        Ok(resp)
    }

    /// `GET /api/v1/transactions/{tx_id}`.
    pub async fn get_transaction(&self, tx_id: &str) -> Result<Transaction, Error> {
        self.get(&format!("{TRANSACTIONS_PATH}/{tx_id}")).await
    }

    /// `GET /api/v1/transactions` — paginated, optionally filtered by
    /// `wallet_id` / `account_id` / `chain`.
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
        self.get_with_query(TRANSACTIONS_PATH, &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transfer_serializes_with_operation_tag_first() {
        let req = CreateTransactionRequest::transfer(
            TransferRequest::new("0xfrom", "0xto", "ethereum", "USDT", "10.5")
                .with_memo("invoice 1")
                .with_reference_id("order-1"),
        );
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"operation":"TRANSFER","from_address":"0xfrom","to_address":"0xto","chain":"ethereum","token_symbol":"USDT","amount":"10.5","memo":"invoice 1","reference_id":"order-1"}"#
        );
    }

    #[test]
    fn optional_fields_are_omitted_not_nulled() {
        let req: CreateTransactionRequest =
            ProgramCallRequest::new("payer", "solana", "AQID").into();
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            json!({
                "operation": "PROGRAM_CALL",
                "from_address": "payer",
                "chain": "solana",
                "signed_transaction": "AQID",
            })
        );
    }

    #[test]
    fn contract_call_shape() {
        let req = CreateTransactionRequest::contract_call(
            ContractCallRequest::new(
                "0x9658",
                "ethereum",
                ContractCall::new(
                    "0xquote",
                    1789449058,
                    ContractCallIncomingLeg {
                        to: "0xcf8a".into(),
                        token: "0x59fb".into(),
                        amount: "10000000".into(),
                    },
                    ContractCallOutgoingLeg {
                        from: "0xcf8a".into(),
                        to: "0x9658".into(),
                        token: "0xa55a".into(),
                        amount: "42000000000000000".into(),
                    },
                    "0xsig",
                )
                .with_permit_deadline(1789449358),
            )
            .with_receive_address("0x9658")
            .with_reference_id("quote-1"),
        );
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            json!({
                "operation": "CONTRACT_CALL",
                "from_address": "0x9658",
                "chain": "ethereum",
                "receive_address": "0x9658",
                "contract_call": {
                    "quote_id": "0xquote",
                    "expiration": 1789449058,
                    "incoming": {"to": "0xcf8a", "token": "0x59fb", "amount": "10000000"},
                    "outgoing": {"from": "0xcf8a", "to": "0x9658", "token": "0xa55a", "amount": "42000000000000000"},
                    "counterparty_signature": "0xsig",
                    "permit_deadline": 1789449358
                },
                "reference_id": "quote-1"
            })
        );
        assert_eq!(req.operation(), OPERATION_CONTRACT_CALL);
        assert_eq!(req.reference_id(), Some("quote-1"));
    }

    #[test]
    fn response_parses_with_and_without_tx_hash() {
        let broadcast: CreateTransactionResponse = serde_json::from_str(
            r#"{"tx_id":"t1","status":"BROADCAST","message":"CONTRACT_CALL broadcast","tx_hash":"0xabc"}"#,
        )
        .unwrap();
        assert_eq!(broadcast.tx_hash.as_deref(), Some("0xabc"));
        assert!(broadcast.is_broadcast());
        assert_eq!(broadcast.http_status, 0);

        let pending: CreateTransactionResponse = serde_json::from_str(
            r#"{"tx_id":"t2","status":"PENDING","message":"Transfer task created"}"#,
        )
        .unwrap();
        assert_eq!(pending.tx_hash, None);
        assert!(!pending.is_broadcast());
        assert_eq!(pending.operation, "");
        assert!(
            !pending.is_accepted(),
            "no operation known and no 202: cannot be classified as accepted"
        );
    }

    #[test]
    fn is_accepted_follows_status_and_operation_not_only_http_status() {
        let pending = |operation: &str, http_status: u16| CreateTransactionResponse {
            tx_id: "t".into(),
            status: STATUS_PENDING.into(),
            message: String::new(),
            tx_hash: None,
            http_status,
            operation: operation.into(),
        };
        // The gateway's own 202.
        assert!(pending(OPERATION_CONTRACT_CALL, 202).is_accepted());
        // Idempotency-Key replay of that 202: HTTP 200, same PENDING body.
        assert!(pending(OPERATION_CONTRACT_CALL, 200).is_accepted());
        assert!(pending(OPERATION_PROGRAM_CALL, 200).is_accepted());
        // A TRANSFER's normal answer is 200 PENDING and is not "accepted".
        assert!(!pending(OPERATION_TRANSFER, 200).is_accepted());
        // Broadcast is never "accepted".
        let mut broadcast = pending(OPERATION_CONTRACT_CALL, 200);
        broadcast.status = STATUS_BROADCAST.into();
        assert!(!broadcast.is_accepted());
    }

    #[test]
    fn transaction_parses_without_risk_fields() {
        let tx: Transaction = serde_json::from_value(json!({
            "tx_id": "t", "wallet_id": "w", "client_id": "c", "chain": "ethereum",
            "transaction_type": "OUTBOUND", "from_address": "a", "to_address": "b",
            "token_symbol": "USDT", "amount": "1", "status": "CONFIRMED", "tx_hash": "0x",
            "created_at": "2026-09-15T00:00:00Z"
        }))
        .unwrap();
        assert_eq!(tx.risk_score, None);
        assert_eq!(tx.risk_level, None);
    }
}
