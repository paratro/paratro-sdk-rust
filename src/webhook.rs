//! Webhook signature signing and verification (Stripe-style).
//!
//! # Signature Algorithm
//!
//! Canonical string: `{unix_timestamp}.{raw_request_body}`
//!
//! Signature: `v1=` + hex(HMAC-SHA256(secret, canonical))
//!
//! # Example
//!
//! ```rust
//! use paratro_sdk::webhook;
//!
//! let secret = "whsec_test_secret";
//! let payload = br#"{"event_id":"evt_123","chain":"ethereum"}"#;
//!
//! // Sign
//! let (timestamp, signature) = webhook::sign_payload(secret, payload);
//!
//! // Verify
//! let result = webhook::verify_payload(
//!     secret,
//!     &timestamp,
//!     payload,
//!     &signature,
//!     webhook::DEFAULT_TOLERANCE,
//! );
//! assert!(result.is_ok());
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

// Webhook event type constants.

/// Transaction is confirming (waiting for block confirmations).
pub const EVENT_TRANSACTION_CONFIRMING: &str = "transaction.confirming";

/// Transaction has been confirmed on-chain.
pub const EVENT_TRANSACTION_CONFIRMED: &str = "transaction.confirmed";

/// Transaction has failed.
pub const EVENT_TRANSACTION_FAILED: &str = "transaction.failed";

/// An internal (Paratro-to-Paratro) transfer was credited to the receiving
/// account. Deliberately distinct from `transaction.confirmed`; the payload's
/// `transaction_type` is `INTERNAL`.
pub const EVENT_TRANSFER_CREDITED: &str = "transfer.credited";

/// An x402 settlement paid to one of this client's addresses was confirmed and
/// credited (the seller-side notification of a facilitator settlement). The
/// payload's `transaction_type` is `INBOUND` and `status` is `CONFIRMED`.
pub const EVENT_X402_SETTLEMENT_CONFIRMED: &str = "x402.settlement.confirmed";

/// Every event type the message service emits (`paratro-mpc-message`:
/// `consumer/confirming.go`, `consumer/compliance.go`,
/// `dispatcher/outbound_confirmation_dispatcher.go`,
/// `dispatcher/inbound_notify.go`, `dispatcher/internal_inbound_receipt.go`,
/// `dispatcher/x402_credit.go`).
pub const EVENT_TYPES: &[&str] = &[
    EVENT_TRANSACTION_CONFIRMING,
    EVENT_TRANSACTION_CONFIRMED,
    EVENT_TRANSACTION_FAILED,
    EVENT_TRANSFER_CREDITED,
    EVENT_X402_SETTLEMENT_CONFIRMED,
];

/// A parsed webhook event payload (v2 schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_id: String,
    pub event_type: String,
    pub event_time: String,
    pub source_id: String,
    pub wallet_id: String,
    pub account_id: String,
    pub status: String,
    pub transaction_type: String,
    pub chain: String,
    pub network: String,
    pub txhash: String,
    pub block_number: u64,
    pub from: String,
    pub to: String,
    pub symbol: String,
    pub contract_address: String,
    pub amount: String,
    pub decimals: i32,
    pub confirmations: u64,
    pub required_confirmations: u64,
    pub created_at: String,
    pub confirmed_at: Option<String>,
    pub risk_checked: bool,
    pub risk_score: f64,
    pub risk_level: String,
    pub data: String,
    /// Operation of the underlying transaction, present on every event:
    /// `TRANSFER` / `PROGRAM_CALL` / `CONTRACT_CALL` for `OUTBOUND`, `DEPOSIT`
    /// for deposits, `X402` for `x402.settlement.confirmed`, `TRANSFER` for
    /// `transfer.credited`. Branch on it together with `transaction_type`.
    /// Empty only when the payload predates the field.
    #[serde(default)]
    pub operation: String,
    /// Counter-asset leg of a `PROGRAM_CALL` / `CONTRACT_CALL` swap. `Some` only
    /// on `transaction.confirmed` / `transaction.failed` of those operations;
    /// `None` on every other event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_incoming: Option<SwapIncoming>,
}

/// `operation` value of an on-chain deposit (`INBOUND` `transaction.*`).
pub const OPERATION_DEPOSIT: &str = "DEPOSIT";
/// `operation` value of an x402 facilitator settlement (`x402.settlement.confirmed`).
pub const OPERATION_X402: &str = "X402";

/// `accounting_status`: the incoming leg was credited to the asset balance.
pub const SWAP_ACCOUNTING_APPLIED: &str = "APPLIED";
/// `accounting_status`: the credit was refused fail-closed; Paratro operations
/// reconcile it by hand (`booked == false`).
pub const SWAP_ACCOUNTING_REVIEW_REQUIRED: &str = "REVIEW_REQUIRED";
/// `accounting_status`: the swap reverted on-chain (`transaction.failed`); there
/// is no incoming leg to book.
pub const SWAP_ACCOUNTING_NOT_APPLICABLE: &str = "NOT_APPLICABLE";

/// The incoming (counter-asset) leg the counterparty paid to your
/// `receive_address` in a `PROGRAM_CALL` / `CONTRACT_CALL` swap, reported in
/// the top-level `swap_incoming` object of `transaction.confirmed` /
/// `transaction.failed` (`paratro-mpc-message` `webhook.SwapIncomingPayload`).
///
/// Credit your customer's target asset only when `booked` is `true`; a
/// confirmed outgoing leg does not by itself prove the counter-asset arrived.
/// The leg is never reported a second time as an `INBOUND` deposit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapIncoming {
    /// Target asset: ERC-20 contract on EVM chains, SPL mint on Solana.
    pub token_address: String,
    /// Target token symbol as registered on Paratro; empty when unregistered.
    pub symbol: String,
    /// Smallest-unit amount that actually arrived on-chain, as an integer
    /// string; `"0"` when nothing arrived (swap reverted, or no credit to
    /// `receive_address` could be derived). It can be non-zero while `booked`
    /// is `false`: the funds arrived but were not credited — see `reason`.
    pub amount: String,
    /// Target token decimals; `0` when unregistered.
    pub decimals: i32,
    /// `true` when the incoming leg was credited to the asset balance.
    pub booked: bool,
    /// One of [`SWAP_ACCOUNTING_APPLIED`], [`SWAP_ACCOUNTING_REVIEW_REQUIRED`],
    /// [`SWAP_ACCOUNTING_NOT_APPLICABLE`].
    pub accounting_status: String,
    /// Only when `booked` is `false`: `onchain_execution_failed` on
    /// `transaction.failed`, otherwise the machine-readable refusal cause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Only when `booked` is `false` and the refusal left an operations audit
    /// record (`SWAP_INCOMING_*`); quote it when contacting support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_type: Option<String>,
}

/// Parse a raw JSON webhook body into a [`WebhookEvent`].
pub fn parse_event(body: &[u8]) -> Result<WebhookEvent, serde_json::Error> {
    serde_json::from_slice(body)
}

/// Header name for the Unix timestamp (seconds).
pub const HEADER_TIMESTAMP: &str = "X-Paratro-Timestamp";

/// Header name for the signature (`v1=<hex>`).
pub const HEADER_SIGNATURE: &str = "X-Paratro-Signature";

/// Signature version prefix.
const SIGNATURE_VERSION: &str = "v1";

/// Default tolerance window for timestamp validation (5 minutes).
pub const DEFAULT_TOLERANCE: Duration = Duration::from_secs(5 * 60);

type HmacSha256 = Hmac<Sha256>;

/// Webhook verification error.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("webhook: invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("webhook: timestamp too old (age: {age_secs}s, tolerance: {tolerance_secs}s)")]
    TimestampExpired { age_secs: u64, tolerance_secs: u64 },

    #[error("webhook: signature mismatch")]
    SignatureMismatch,
}

/// Sign a webhook payload.
///
/// Returns `(timestamp, signature)` where signature is `"v1=<hex>"`.
pub fn sign_payload(secret: &str, payload: &[u8]) -> (String, String) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs()
        .to_string();
    let sig = compute_signature(secret, &ts, payload);
    (ts, sig)
}

/// Sign a webhook payload with a specific timestamp (useful for testing).
pub fn sign_payload_with_timestamp(secret: &str, payload: &[u8], timestamp: &str) -> String {
    compute_signature(secret, timestamp, payload)
}

/// Verify a webhook payload signature.
///
/// - `secret`:    shared webhook secret
/// - `timestamp`: `X-Paratro-Timestamp` header value
/// - `payload`:   raw request body bytes
/// - `signature`: `X-Paratro-Signature` header value (e.g. `"v1=abcdef..."`)
/// - `tolerance`: max allowed time drift; `Duration::ZERO` skips time check
pub fn verify_payload(
    secret: &str,
    timestamp: &str,
    payload: &[u8],
    signature: &str,
    tolerance: Duration,
) -> Result<(), WebhookError> {
    // 1. Validate timestamp format
    let ts: i64 = timestamp
        .parse()
        .map_err(|_| WebhookError::InvalidTimestamp(timestamp.to_string()))?;

    // 2. Anti-replay: validate freshness
    if !tolerance.is_zero() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_secs() as i64;
        let diff = (now - ts).unsigned_abs();
        if diff > tolerance.as_secs() {
            return Err(WebhookError::TimestampExpired {
                age_secs: diff,
                tolerance_secs: tolerance.as_secs(),
            });
        }
    }

    // 3. Compute expected signature
    let expected = compute_signature(secret, timestamp, payload);

    // 4. Constant-time comparison (anti timing attack)
    if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
        return Err(WebhookError::SignatureMismatch);
    }

    Ok(())
}

/// Build canonical string `"{timestamp}.{payload}"` and compute HMAC-SHA256.
/// Returns `"v1=<hex>"`.
fn compute_signature(secret: &str, timestamp: &str, payload: &[u8]) -> String {
    let mut canonical = Vec::with_capacity(timestamp.len() + 1 + payload.len());
    canonical.extend_from_slice(timestamp.as_bytes());
    canonical.push(b'.');
    canonical.extend_from_slice(payload);

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(&canonical);

    format!(
        "{}={}",
        SIGNATURE_VERSION,
        hex::encode(mac.finalize().into_bytes())
    )
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "whsec_test_secret_key_12345";
    const TEST_PAYLOAD: &[u8] = br#"{"event_id":"evt_123","chain":"ethereum","txhash":"0xabc"}"#;

    #[test]
    fn test_sign_and_verify() {
        let (ts, sig) = sign_payload(TEST_SECRET, TEST_PAYLOAD);
        let result = verify_payload(TEST_SECRET, &ts, TEST_PAYLOAD, &sig, DEFAULT_TOLERANCE);
        assert!(result.is_ok());
    }

    #[test]
    fn test_wrong_secret() {
        let (ts, sig) = sign_payload(TEST_SECRET, TEST_PAYLOAD);
        let result = verify_payload("wrong_secret", &ts, TEST_PAYLOAD, &sig, DEFAULT_TOLERANCE);
        assert!(matches!(result, Err(WebhookError::SignatureMismatch)));
    }

    #[test]
    fn test_tampered_payload() {
        let (ts, sig) = sign_payload(TEST_SECRET, TEST_PAYLOAD);
        let tampered = br#"{"id":"evt_456","chain":"ethereum","txhash":"0xabc"}"#;
        let result = verify_payload(TEST_SECRET, &ts, tampered, &sig, DEFAULT_TOLERANCE);
        assert!(matches!(result, Err(WebhookError::SignatureMismatch)));
    }

    #[test]
    fn test_expired_timestamp() {
        let old_ts = "1000000000"; // year 2001
        let sig = sign_payload_with_timestamp(TEST_SECRET, TEST_PAYLOAD, old_ts);
        let result = verify_payload(TEST_SECRET, old_ts, TEST_PAYLOAD, &sig, DEFAULT_TOLERANCE);
        assert!(matches!(result, Err(WebhookError::TimestampExpired { .. })));
    }

    #[test]
    fn test_future_timestamp() {
        let future_ts = "9999999999"; // year 2286
        let sig = sign_payload_with_timestamp(TEST_SECRET, TEST_PAYLOAD, future_ts);
        let result = verify_payload(
            TEST_SECRET,
            future_ts,
            TEST_PAYLOAD,
            &sig,
            DEFAULT_TOLERANCE,
        );
        assert!(matches!(result, Err(WebhookError::TimestampExpired { .. })));
    }

    #[test]
    fn test_invalid_timestamp() {
        let result = verify_payload(
            TEST_SECRET,
            "not-a-number",
            TEST_PAYLOAD,
            "v1=abc",
            DEFAULT_TOLERANCE,
        );
        assert!(matches!(result, Err(WebhookError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_zero_tolerance_skips_time_check() {
        let old_ts = "1000000000";
        let sig = sign_payload_with_timestamp(TEST_SECRET, TEST_PAYLOAD, old_ts);
        let result = verify_payload(TEST_SECRET, old_ts, TEST_PAYLOAD, &sig, Duration::ZERO);
        assert!(result.is_ok());
    }

    #[test]
    fn test_signature_format() {
        let (_, sig) = sign_payload(TEST_SECRET, TEST_PAYLOAD);
        assert!(sig.starts_with("v1="), "signature must start with v1=");
        // v1= (3 chars) + 64 hex chars = 67 total
        assert_eq!(sig.len(), 67, "signature must be v1= + 64 hex chars");
    }

    #[test]
    fn test_deterministic_signature() {
        let ts = "1704067200";
        let sig1 = sign_payload_with_timestamp(TEST_SECRET, TEST_PAYLOAD, ts);
        let sig2 = sign_payload_with_timestamp(TEST_SECRET, TEST_PAYLOAD, ts);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_different_payloads_different_signatures() {
        let ts = "1704067200";
        let sig1 = sign_payload_with_timestamp(TEST_SECRET, b"payload1", ts);
        let sig2 = sign_payload_with_timestamp(TEST_SECRET, b"payload2", ts);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn event_types_are_complete_and_distinct() {
        assert_eq!(EVENT_TYPES.len(), 5);
        let mut sorted = EVENT_TYPES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), EVENT_TYPES.len(), "duplicate event type");
        for expected in [
            "transaction.confirming",
            "transaction.confirmed",
            "transaction.failed",
            "transfer.credited",
            "x402.settlement.confirmed",
        ] {
            assert!(EVENT_TYPES.contains(&expected), "missing {expected}");
        }
    }

    /// Payload shape of `webhook.BuildEventPayload` in paratro-mpc-message for the
    /// x402 seller-side credit (`dispatcher/x402_credit.go`).
    #[test]
    fn parse_x402_settlement_confirmed_event() {
        let body = br#"{
            "event_id":"evt_x402_1","event_type":"x402.settlement.confirmed",
            "event_time":"2026-09-15T08:00:00Z","source_id":"tx-1","wallet_id":"w-1",
            "account_id":"a-1","status":"CONFIRMED","transaction_type":"INBOUND",
            "chain":"base","network":"mainnet","txhash":"0xabc","block_number":100,
            "from":"0xpayer","to":"0xseller","symbol":"USDC","contract_address":"0xusdc",
            "amount":"1000000","decimals":6,"confirmations":0,"required_confirmations":0,
            "created_at":"2026-09-15T07:59:00Z","confirmed_at":"2026-09-15T08:00:00Z",
            "risk_checked":false,"risk_score":0.0,"risk_level":"UNSCANNED","data":"",
            "operation":"X402"
        }"#;
        let event = parse_event(body).unwrap();
        assert_eq!(event.event_type, EVENT_X402_SETTLEMENT_CONFIRMED);
        assert_eq!(event.transaction_type, "INBOUND");
        assert_eq!(event.status, "CONFIRMED");
        assert_eq!(event.amount, "1000000");
        // x402 settlements are operation X402, not DEPOSIT (paratro-mpc-message
        // webhook.ResolveOperation maps X402_SETTLE -> X402).
        assert_eq!(event.operation, OPERATION_X402);
        assert!(
            event.swap_incoming.is_none(),
            "non-swap events carry no swap_incoming"
        );
    }

    // Shapes pinned by paratro-mpc-message internal/dispatcher/swap_webhook_test.go
    // and documented in paratro-docs features/webhooks.mdx (Swap events).
    fn swap_event_body(
        event_type: &str,
        status: &str,
        operation: &str,
        swap_incoming: &str,
    ) -> Vec<u8> {
        format!(
            r#"{{
            "event_id":"evt_swap_1","event_type":"{event_type}",
            "event_time":"2026-09-16T09:12:40Z","source_id":"tx-swap","wallet_id":"w-1",
            "account_id":"a-1","status":"{status}","transaction_type":"OUTBOUND",
            "operation":"{operation}",
            "chain":"ethereum","network":"testnet","txhash":"0xswap","block_number":10671204,
            "from":"0xpayer","to":"0xcounterparty","symbol":"USDC","contract_address":"0xusdc",
            "amount":"10000000","decimals":6,"confirmations":0,"required_confirmations":6,
            "created_at":"2026-09-16T09:11:58Z","confirmed_at":"2026-09-16T09:12:40Z",
            "risk_checked":false,"risk_score":0.0,"risk_level":"UNSCANNED","data":"",
            "swap_incoming":{swap_incoming}
        }}"#
        )
        .into_bytes()
    }

    #[test]
    fn parse_swap_confirmed_booked_leg() {
        let body = swap_event_body(
            EVENT_TRANSACTION_CONFIRMED,
            "CONFIRMED",
            "CONTRACT_CALL",
            r#"{"token_address":"0xa55a927f2211fe52188526ed7e779b7298646e75","symbol":"AAPLx",
                "amount":"42000000000000000","decimals":18,"booked":true,"accounting_status":"APPLIED"}"#,
        );
        let event = parse_event(&body).unwrap();
        assert_eq!(event.operation, "CONTRACT_CALL");
        // the outgoing leg stays at the top level
        assert_eq!(event.amount, "10000000");
        let leg = event
            .swap_incoming
            .expect("swap_incoming on a CONTRACT_CALL confirmed event");
        assert_eq!(
            leg.token_address,
            "0xa55a927f2211fe52188526ed7e779b7298646e75"
        );
        assert_eq!(leg.symbol, "AAPLx");
        assert_eq!(leg.amount, "42000000000000000");
        assert_eq!(leg.decimals, 18);
        assert!(leg.booked);
        assert_eq!(leg.accounting_status, SWAP_ACCOUNTING_APPLIED);
        assert!(
            leg.reason.is_none() && leg.audit_type.is_none(),
            "a booked leg carries no reason/audit_type"
        );
    }

    #[test]
    fn parse_swap_confirmed_not_booked_carries_reason_and_audit_type() {
        let body = swap_event_body(
            EVENT_TRANSACTION_CONFIRMED,
            "CONFIRMED",
            "PROGRAM_CALL",
            r#"{"token_address":"Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB","symbol":"",
                "amount":"1500","decimals":0,"booked":false,"accounting_status":"REVIEW_REQUIRED",
                "reason":"currency_code_not_found_or_inactive","audit_type":"SWAP_INCOMING_ASSET_UNREGISTERED"}"#,
        );
        let event = parse_event(&body).unwrap();
        assert_eq!(event.operation, "PROGRAM_CALL");
        let leg = event.swap_incoming.unwrap();
        assert!(!leg.booked);
        assert_eq!(leg.accounting_status, SWAP_ACCOUNTING_REVIEW_REQUIRED);
        assert_eq!(
            leg.reason.as_deref(),
            Some("currency_code_not_found_or_inactive")
        );
        assert_eq!(
            leg.audit_type.as_deref(),
            Some("SWAP_INCOMING_ASSET_UNREGISTERED")
        );
        // funds arrived on-chain but were not credited: the amount is the on-chain amount, not "0"
        assert_eq!(leg.amount, "1500");
        assert_eq!(
            (leg.symbol.as_str(), leg.decimals),
            ("", 0),
            "unregistered asset is not invented"
        );
    }

    #[test]
    fn parse_swap_failed_is_not_applicable() {
        let body = swap_event_body(
            EVENT_TRANSACTION_FAILED,
            "FAILED",
            "CONTRACT_CALL",
            r#"{"token_address":"0xa55a927f2211fe52188526ed7e779b7298646e75","symbol":"AAPLx",
                "amount":"0","decimals":18,"booked":false,"accounting_status":"NOT_APPLICABLE",
                "reason":"onchain_execution_failed"}"#,
        );
        let event = parse_event(&body).unwrap();
        assert_eq!(event.event_type, EVENT_TRANSACTION_FAILED);
        let leg = event.swap_incoming.unwrap();
        assert!(!leg.booked);
        assert_eq!(leg.accounting_status, SWAP_ACCOUNTING_NOT_APPLICABLE);
        assert_eq!(leg.reason.as_deref(), Some("onchain_execution_failed"));
        assert!(
            leg.audit_type.is_none(),
            "a reverted swap opens no SWAP_INCOMING_* audit"
        );
        assert_eq!(leg.amount, "0");
    }

    #[test]
    fn parse_legacy_event_without_operation_or_swap_incoming() {
        // A payload from a message service that predates both fields must still parse,
        // and re-serialising it must not invent a swap_incoming key.
        let body = br#"{
            "event_id":"evt_legacy","event_type":"transaction.confirmed",
            "event_time":"2026-09-15T08:00:00Z","source_id":"tx-1","wallet_id":"w-1",
            "account_id":"a-1","status":"CONFIRMED","transaction_type":"OUTBOUND",
            "chain":"base","network":"mainnet","txhash":"0xabc","block_number":100,
            "from":"0xa","to":"0xb","symbol":"USDC","contract_address":"0xusdc",
            "amount":"1000000","decimals":6,"confirmations":0,"required_confirmations":12,
            "created_at":"2026-09-15T07:59:00Z","confirmed_at":"2026-09-15T08:00:00Z",
            "risk_checked":false,"risk_score":0.0,"risk_level":"UNSCANNED","data":""
        }"#;
        let event = parse_event(body).unwrap();
        assert_eq!(event.operation, "");
        assert!(event.swap_incoming.is_none());
        let out: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert!(
            out.get("swap_incoming").is_none(),
            "swap_incoming must be skipped when None"
        );
        assert_eq!(out["operation"], "");
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }
}
