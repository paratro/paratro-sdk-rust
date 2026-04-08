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
//! let payload = br#"{"id":"evt_123","chain":"ethereum"}"#;
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
use serde::Deserialize;
use sha2::Sha256;

// Webhook event type constants.

/// Transaction is confirming (waiting for block confirmations).
pub const EVENT_TRANSACTION_CONFIRMING: &str = "transaction.confirming";

/// Transaction has been confirmed on-chain.
pub const EVENT_TRANSACTION_CONFIRMED: &str = "transaction.confirmed";

/// Transaction has failed.
pub const EVENT_TRANSACTION_FAILED: &str = "transaction.failed";

/// A parsed webhook event payload.
#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    pub event_type: String,
    pub chain: String,
    pub txhash: String,
    pub transaction_type: String,
    pub direction: String,
    pub status: String,
    pub from: String,
    pub to: String,
    pub symbol: String,
    pub amount: String,
    #[serde(default)]
    pub decimals: i32,
    #[serde(default)]
    pub block_number: i64,
    #[serde(default)]
    pub confirmations: i32,
    #[serde(default)]
    pub required_confirmations: i32,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub risk_score: i32,
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
    const TEST_PAYLOAD: &[u8] = br#"{"id":"evt_123","chain":"ethereum","txhash":"0xabc"}"#;

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
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }
}
