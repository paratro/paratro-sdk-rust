//! x402 facilitator API.
//!
//! `POST /api/v1/x402/sign` was retired by the gateway (HTTP 410) and is no longer
//! part of this SDK. The facilitator endpoints remain:
//!
//! | method | path | SDK |
//! |--------|------|-----|
//! | `POST` | `/api/v1/x402/verify`          | [`MpcClient::x402_verify`] |
//! | `POST` | `/api/v1/x402/settle`          | [`MpcClient::x402_settle`] |
//! | `GET`  | `/api/v1/x402/settle/{tx_id}`  | [`MpcClient::x402_settle_status`] |
//! | `GET`  | `/api/v1/x402/settlements`     | [`MpcClient::x402_list_settlements`] |
//!
//! The verify / settle request and reply bodies follow the Coinbase facilitator
//! wire format (camelCase): `dto.X402FacilitatorRequest`, `dto.X402VerifyResponse`,
//! `dto.X402SettleResponse`, `dto.X402SettleStatusResponse` on the gateway.

use serde::{Deserialize, Serialize};

use crate::client::{MpcClient, HEADER_IDEMPOTENCY_KEY};
use crate::error::Error;

/// Body of `POST /x402/verify` and `POST /x402/settle`
/// (`dto.X402FacilitatorRequest`). `payment_requirements` is v1 only; v2 embeds
/// the requirements in `payment_payload.accepted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402FacilitatorRequest {
    #[serde(rename = "x402Version")]
    pub x402_version: i32,
    #[serde(rename = "paymentPayload")]
    pub payment_payload: serde_json::Value,
    #[serde(
        rename = "paymentRequirements",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub payment_requirements: Option<serde_json::Value>,
}

/// Reply of `POST /x402/verify` (`dto.X402VerifyResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402VerifyResponse {
    #[serde(rename = "isValid")]
    pub is_valid: bool,
    #[serde(
        rename = "invalidReason",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub invalid_reason: Option<String>,
    #[serde(default)]
    pub payer: String,
}

/// Reply of `POST /x402/settle` (`dto.X402SettleResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402SettleResponse {
    pub success: bool,
    /// Settlement record id; poll [`MpcClient::x402_settle_status`] with it.
    #[serde(rename = "txId", default)]
    pub tx_id: String,
    /// On-chain transaction hash (empty until broadcast).
    #[serde(default)]
    pub transaction: String,
    #[serde(
        rename = "errorReason",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub error_reason: Option<String>,
    #[serde(default)]
    pub payer: String,
    #[serde(default)]
    pub network: String,
}

/// Reply of `GET /x402/settle/{tx_id}` (`dto.X402SettleStatusResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402SettleStatusResponse {
    pub success: bool,
    #[serde(rename = "txId")]
    pub tx_id: String,
    pub status: String,
    #[serde(rename = "txHash", default)]
    pub tx_hash: String,
    #[serde(default)]
    pub network: String,
}

/// One item of `GET /x402/settlements` (`dto.X402SettlementResponse`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402Settlement {
    pub tx_id: String,
    pub chain: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub status: String,
    /// Unix seconds.
    pub valid_before: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_v: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_r: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_s: Option<String>,
    pub created_at: String,
}

/// Query of `GET /x402/settlements` (`dto.X402SettlementListRequest`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListX402SettlementsRequest {
    /// One of `PENDING`, `PROCESSING`, `X402_SIGNED`, `SETTLED`, `CANCELLED`, `FAILED`, `EXPIRED`.
    pub status: Option<String>,
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

/// Paginated list of x402 settlements (`common.PagedBody`).
#[derive(Debug, Clone, Deserialize)]
pub struct ListX402SettlementsResponse {
    #[serde(rename = "data")]
    pub items: Vec<X402Settlement>,
    pub total: i64,
    pub has_more: bool,
}

impl MpcClient {
    /// `POST /api/v1/x402/verify` — verifies a payment payload. Accepts
    /// [`X402FacilitatorRequest`] or any `Serialize` value with the same shape
    /// (e.g. a `serde_json::Value`).
    pub async fn x402_verify<B: Serialize + ?Sized>(
        &self,
        payload: &B,
    ) -> Result<X402VerifyResponse, Error> {
        self.post("/api/v1/x402/verify", payload).await
    }

    /// `POST /api/v1/x402/settle` — executes an on-chain settlement. Same body
    /// as [`MpcClient::x402_verify`]. A failed settlement is still HTTP 200 with
    /// `success == false` and `error_reason` set.
    pub async fn x402_settle<B: Serialize + ?Sized>(
        &self,
        payload: &B,
    ) -> Result<X402SettleResponse, Error> {
        self.post("/api/v1/x402/settle", payload).await
    }

    /// [`MpcClient::x402_settle`] with an `Idempotency-Key` header
    /// ([`crate::HEADER_IDEMPOTENCY_KEY`]): the gateway replays the cached 2xx body
    /// (24 h) for a repeat under the same key instead of settling twice.
    pub async fn x402_settle_with_idempotency_key<B: Serialize + ?Sized>(
        &self,
        payload: &B,
        idempotency_key: &str,
    ) -> Result<X402SettleResponse, Error> {
        self.post_with_headers(
            "/api/v1/x402/settle",
            payload,
            &[(HEADER_IDEMPOTENCY_KEY, idempotency_key)],
        )
        .await
    }

    /// `GET /api/v1/x402/settle/{tx_id}` — status of a settlement.
    pub async fn x402_settle_status(&self, tx_id: &str) -> Result<X402SettleStatusResponse, Error> {
        self.get(&format!("/api/v1/x402/settle/{tx_id}")).await
    }

    /// `GET /api/v1/x402/settlements` — paginated settlement records, optionally
    /// filtered by `status`.
    pub async fn x402_list_settlements(
        &self,
        req: &ListX402SettlementsRequest,
    ) -> Result<ListX402SettlementsResponse, Error> {
        let mut params = Vec::new();
        if let Some(ref status) = req.status {
            params.push(("status".to_string(), status.clone()));
        }
        if let Some(page) = req.page {
            params.push(("page".to_string(), page.to_string()));
        }
        if let Some(page_size) = req.page_size {
            params.push(("page_size".to_string(), page_size.to_string()));
        }
        self.get_with_query("/api/v1/x402/settlements", &params)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn facilitator_request_is_camel_case() {
        let req = X402FacilitatorRequest {
            x402_version: 2,
            payment_payload: json!({"payload": {}, "accepted": {}}),
            payment_requirements: None,
        };
        assert_eq!(
            serde_json::to_value(&req).unwrap(),
            json!({"x402Version": 2, "paymentPayload": {"payload": {}, "accepted": {}}})
        );
    }

    #[test]
    fn verify_and_settle_replies_parse() {
        let v: X402VerifyResponse =
            serde_json::from_value(json!({"isValid": false, "invalidReason": "expired"})).unwrap();
        assert!(!v.is_valid);
        assert_eq!(v.invalid_reason.as_deref(), Some("expired"));
        assert_eq!(v.payer, "");

        let s: X402SettleResponse = serde_json::from_value(json!({
            "success": true, "txId": "st-1", "transaction": "0xhash", "payer": "0xp", "network": "base-sepolia"
        }))
        .unwrap();
        assert_eq!(s.tx_id, "st-1");
        assert_eq!(s.error_reason, None);

        let st: X402SettleStatusResponse = serde_json::from_value(json!({
            "success": true, "txId": "st-1", "status": "SETTLED", "txHash": "0xhash", "network": "base-sepolia"
        }))
        .unwrap();
        assert_eq!(st.tx_hash, "0xhash");
    }

    #[test]
    fn settlement_item_parses() {
        let item: X402Settlement = serde_json::from_value(json!({
            "tx_id": "st-1", "chain": "base", "from_address": "a", "to_address": "b",
            "amount": "1000000", "status": "SETTLED", "valid_before": 1800000000,
            "signature_v": 27, "signature_r": "0xr", "signature_s": "0xs",
            "created_at": "2026-09-15T00:00:00Z"
        }))
        .unwrap();
        assert_eq!(item.signature_v, Some(27));
        let minimal: X402Settlement = serde_json::from_value(json!({
            "tx_id": "st-2", "chain": "base", "from_address": "a", "to_address": "b",
            "amount": "1", "status": "PENDING", "valid_before": 0,
            "created_at": "2026-09-15T00:00:00Z"
        }))
        .unwrap();
        assert_eq!(minimal.signature_r, None);
    }
}
