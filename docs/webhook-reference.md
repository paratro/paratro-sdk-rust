# Webhook Reference

## Overview

The Paratro MPC Message service sends HTTP POST requests to the configured Webhook URL when qualifying transfer transactions are detected, notifying the client of new transaction events. All Webhook requests are authenticated using HMAC-SHA256 signatures (Stripe-style) to ensure authenticity and integrity.

## Trigger Conditions

Webhook notifications are triggered when all of the following conditions are met:

1. **Transaction type match**: The transaction type is one of:
   - `transfer`: Native token transfer (ETH, BNB, TRX, SOL, etc.)
   - `erc20_transfer`: ERC20 token transfer (Ethereum, BSC, Polygon, and other EVM chains)
   - `trc20_transfer`: TRC20 token transfer (Tron chain)
   - `spl_transfer`: SPL token transfer (Solana chain)

2. **Wallet account exists**: The wallet account corresponding to the recipient address (`to`) already exists

3. **Client status**: The wallet client status is `ACTIVE`

4. **Configuration complete**: The client has configured both `WebhookUrl` and `WebhookSecret`

## Request Format

**HTTP Method:** `POST`

**Content-Type:** `application/json`

**Request Headers:**

| Header Name | Description | Example |
|------------|-------------|---------|
| `Content-Type` | Content type | `application/json` |
| `X-Paratro-Timestamp` | Unix timestamp (seconds) | `1704067200` |
| `X-Paratro-Signature` | `v1=` + hex-encoded HMAC-SHA256 signature | `v1=5257a869e7eceb...` |
| `X-Paratro-Api-Key` | Client API Key | `your-api-key` |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Event ID (UUID) |
| `chain` | string | Yes | Blockchain name (e.g., `ethereum`, `bsc`, `polygon`, `tron`, `solana`) |
| `txhash` | string | Yes | Transaction hash (unique identifier) |
| `type` | string | Yes | Transaction type: `transfer`, `erc20_transfer`, `trc20_transfer`, or `spl_transfer` |
| `from` | string | Yes | Sender address |
| `to` | string | Yes | Recipient address |
| `symbol` | string | Yes | Token symbol (e.g., `ETH`, `USDT`, `SOL`) |
| `amount` | string | Yes | Amount (in smallest unit, string format) |
| `decimals` | number | Yes | Token decimal places |
| `data` | string | Yes | Hex-encoded transaction input data |
| `risk_score` | number | Yes | Risk advisory score (0-3 low risk, 4-6 medium risk, 7-10 high risk) |

### Native Token Transfer Example

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "chain": "ethereum",
  "txhash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
  "type": "transfer",
  "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "to": "0x8ba1f109551bD432803012645Hac136c22C92900",
  "symbol": "ETH",
  "amount": "1000000000000000000",
  "decimals": 18,
  "data": "",
  "risk_score": 0
}
```

### ERC20 Token Transfer Example

```json
{
  "id": "660e8400-e29b-41d4-a716-446655440001",
  "chain": "ethereum",
  "txhash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
  "type": "erc20_transfer",
  "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "to": "0x8ba1f109551bD432803012645Hac136c22C92900",
  "symbol": "USDT",
  "amount": "1000000",
  "decimals": 6,
  "data": "0xa9059cbb...",
  "risk_score": 0
}
```

### SPL Token Transfer Example (Solana)

```json
{
  "id": "770e8400-e29b-41d4-a716-446655440002",
  "chain": "solana",
  "txhash": "5xyzSig123abc456def789ghi012jkl345mno678pqr901stu234vwx567yz",
  "type": "spl_transfer",
  "from": "7EYnhQoR9YM3N7UoaKRoA44Uy8JeaZV3qyouov87awMs",
  "to": "8oBNNgSfF9TCh8hN6NfffZUhUo2sGWEm6a5SADVhGtGe",
  "symbol": "USDC",
  "amount": "5000000",
  "decimals": 6,
  "data": "",
  "risk_score": 0
}
```

## Signature Verification

All Webhook requests include an HMAC-SHA256 signature following the Stripe-style signing scheme. The receiver must verify the signature before processing the request.

### Signature Algorithm

**Canonical String:**

```
{unix_timestamp}.{raw_request_body}
```

- `unix_timestamp`: UTC second-level timestamp (from `X-Paratro-Timestamp` header)
- `.`: fixed separator
- `raw_request_body`: raw JSON body (UTF-8 bytes, no preprocessing)

**Signature Computation:**

```
signature = "v1=" + hex(HMAC-SHA256(webhook_secret, canonical_string))
```

- `v1=` prefix: version tag for future algorithm upgrades (e.g., `v2=` for Ed25519)
- hex encoding: lowercase hexadecimal
- HMAC-SHA256: industry standard, used by Stripe / Slack / GitHub

### Verification Steps

```
1. Extract timestamp and signature from headers
2. Validate timestamp format (must be a valid integer)
3. Validate timestamp freshness (|now - timestamp| <= tolerance), anti-replay
4. Read raw request body
5. Build canonical = "{timestamp}.{body}"
6. Compute expected = "v1=" + hex(HMAC-SHA256(secret, canonical))
7. Constant-time comparison of expected vs signature
```

### Rust Verification Code (Using SDK)

The `paratro-sdk` crate provides built-in webhook verification:

```rust
use paratro_sdk::webhook;

fn verify_webhook(
    secret: &str,
    timestamp: &str,  // from X-Paratro-Timestamp header
    body: &[u8],      // raw request body
    signature: &str,  // from X-Paratro-Signature header
) -> Result<(), webhook::WebhookError> {
    webhook::verify_payload(secret, timestamp, body, signature, webhook::DEFAULT_TOLERANCE)
}
```

### Rust Verification Code (Standalone)

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const SIGNATURE_VERSION: &str = "v1";

fn compute_signature(secret: &str, timestamp: &str, payload: &[u8]) -> String {
    let mut canonical = Vec::with_capacity(timestamp.len() + 1 + payload.len());
    canonical.extend_from_slice(timestamp.as_bytes());
    canonical.push(b'.');
    canonical.extend_from_slice(payload);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(&canonical);

    format!("{}={}", SIGNATURE_VERSION, hex::encode(mac.finalize().into_bytes()))
}

fn verify_payload(
    secret: &str,
    timestamp: &str,
    payload: &[u8],
    signature: &str,
    tolerance_secs: u64,
) -> Result<(), String> {
    // 1. Validate timestamp format
    let ts: i64 = timestamp.parse().map_err(|_| "invalid timestamp")?;

    // 2. Anti-replay: validate freshness
    if tolerance_secs > 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let diff = (now - ts).unsigned_abs();
        if diff > tolerance_secs {
            return Err(format!("timestamp too old (age: {}s, tolerance: {}s)", diff, tolerance_secs));
        }
    }

    // 3. Compute expected signature
    let expected = compute_signature(secret, timestamp, payload);

    // 4. Constant-time comparison
    if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
        return Err("signature mismatch".into());
    }

    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
```

## Signature Verification Notes

1. **Timestamp validation**: It is recommended to validate the timestamp to prevent replay attacks. A typical tolerance window is 5 minutes (default). Use up to 15 minutes for cross-region / high-latency scenarios.

2. **Signature comparison**: Always use a constant-time comparison function to prevent timing attacks.

3. **Raw body**: Use the raw request body bytes for verification. Do not re-serialize or modify the body before computing the signature.

## Response Requirements

**Success Response** - Return HTTP 200:

```json
{
  "success": true,
  "message": "Webhook received successfully"
}
```

**Error Response** - Return appropriate HTTP error status code (4xx or 5xx):

```json
{
  "success": false,
  "error": "Invalid signature",
  "code": "INVALID_SIGNATURE"
}
```

**Response Timeout:** 10 seconds. If the receiver does not respond within 10 seconds, the request is considered failed.

## Error Handling

### Retry Policy

When a Webhook delivery fails (network error or non-2xx response), the server retries with exponential backoff:

- Max attempts: 8
- Backoff sequence: 30s, 1m, 2m, 4m, 8m, 16m, 32m, 2h (capped)

### Client-Side Error Handling Recommendations

1. **Idempotency**: Use the `txhash` field as a unique identifier to ensure duplicate notifications do not cause duplicate processing.

2. **Retry mechanism**: If processing fails, return a 5xx status code. The server will retry according to the retry policy.

3. **Logging**: Log all received Webhook requests, including headers, body, and responses, to facilitate troubleshooting.

4. **Asynchronous processing**: It is recommended to process Webhook requests asynchronously - return a 200 status code immediately, then handle the business logic in the background.

## Security Considerations

1. **HTTPS transport**: It is strongly recommended to configure the Webhook URL with HTTPS to ensure transport security.

2. **Secret protection**: The `WebhookSecret` must be kept secure. Do not expose it or commit it to a code repository.

3. **IP allowlisting**: If possible, configure an IP allowlist to only accept requests from Paratro services.

4. **Signature verification**: You **must** verify the signature of every request and reject any request that fails verification.

5. **Timestamp validation**: Validate the timestamp to prevent replay attacks.

6. **Request body size limit**: It is recommended to set a request body size limit to prevent malicious requests.

## Complete Examples

### Rust Example (Using axum)

```rust
use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use paratro_sdk::webhook;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct WebhookPayload {
    id: String,
    chain: String,
    txhash: String,
    #[serde(rename = "type")]
    tx_type: String,
    from: String,
    to: String,
    symbol: String,
    amount: String,
    decimals: i32,
    data: String,
    risk_score: i32,
}

#[derive(Serialize)]
struct Response {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

async fn webhook_handler(headers: HeaderMap, body: Bytes) -> (StatusCode, Json<Response>) {
    let webhook_secret = std::env::var("WEBHOOK_SECRET").expect("WEBHOOK_SECRET required");

    // 1. Extract headers
    let timestamp = match headers.get("X-Paratro-Timestamp").and_then(|v| v.to_str().ok()) {
        Some(ts) => ts.to_string(),
        None => return (StatusCode::BAD_REQUEST, Json(Response {
            success: false, message: None,
            error: Some("Missing timestamp".into()), code: None,
        })),
    };

    let signature = match headers.get("X-Paratro-Signature").and_then(|v| v.to_str().ok()) {
        Some(sig) => sig.to_string(),
        None => return (StatusCode::BAD_REQUEST, Json(Response {
            success: false, message: None,
            error: Some("Missing signature".into()), code: None,
        })),
    };

    // 2. Verify signature
    if let Err(_) = webhook::verify_payload(
        &webhook_secret,
        &timestamp,
        &body,
        &signature,
        webhook::DEFAULT_TOLERANCE,
    ) {
        return (StatusCode::UNAUTHORIZED, Json(Response {
            success: false, message: None,
            error: Some("Invalid signature".into()),
            code: Some("INVALID_SIGNATURE".into()),
        }));
    }

    // 3. Parse payload
    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(Response {
            success: false, message: None,
            error: Some("Invalid request body".into()), code: None,
        })),
    };

    // 4. Async processing (use txhash for idempotency)
    let txhash = payload.txhash.clone();
    tokio::spawn(async move {
        println!("Processing: chain={}, txhash={}, type={}, amount={} {}",
            payload.chain, txhash, payload.tx_type, payload.amount, payload.symbol);
        // Your business logic here
    });

    // 5. Return success immediately
    (StatusCode::OK, Json(Response {
        success: true,
        message: Some("Webhook received successfully".into()),
        error: None, code: None,
    }))
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/webhook/notify", post(webhook_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Webhook server listening on :8080");
    axum::serve(listener, app).await.unwrap();
}
```

## Security Guarantees

| Threat | Mitigation |
|--------|-----------|
| Body tampering | HMAC-SHA256 guarantees integrity |
| Signature forgery | Requires webhook_secret, cannot forge |
| Replay attack | Timestamp window validation (default 5 min) |
| Timing attack | Constant-time comparison |
| Future algorithm upgrade | `v1=` prefix, add `v2=` without breaking |

## FAQ

**Q: How do I test Webhooks?**
A: You can use tools like [ngrok](https://ngrok.com/) or [localtunnel](https://localtunnel.github.io/www/) to expose your local service to the internet, then configure the Webhook URL accordingly.

**Q: What if Webhook processing takes too long?**
A: It is recommended to use an asynchronous processing pattern: return a 200 status code immediately, then handle the business logic in the background.

**Q: How do I prevent duplicate processing?**
A: Use the transaction hash (`txhash` field) as a unique identifier and check whether the transaction has already been processed before handling it.

**Q: What should I do if signature verification fails?**
A: Check the following:
1. Whether the `WebhookSecret` is correct
2. Whether the raw request body is being used (not re-serialized or modified by middleware)
3. Whether the timestamp is within the valid range (default 5 minutes)
4. Whether the signature has the `v1=` prefix

**Q: What transaction types are supported?**
A: The following transaction types are currently supported:
- `transfer`: Native token transfer (ETH, BNB, TRX, SOL, BTC, etc.)
- `erc20_transfer`: ERC20 token transfer (Ethereum, BSC, Polygon)
- `trc20_transfer`: TRC20 token transfer (Tron)
- `spl_transfer`: SPL token transfer (Solana)

**Q: What tolerance window should I use?**

| Scenario | Window |
|----------|--------|
| Default | 5 minutes |
| High security | 2 minutes |
| Cross-region / high latency | 15 minutes |
| Testing / debugging | 0 (skip check) |

**Q: What Rust crates are needed for webhook verification?**
A: If using the `paratro-sdk` crate, everything is included. For standalone use, add:
```toml
[dependencies]
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
```
