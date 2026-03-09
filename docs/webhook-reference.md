# Webhook Reference

## Overview

The Paratro MPC Message service sends HTTP POST requests to the configured Webhook URL when qualifying transfer transactions are detected, notifying the client of new transaction events. All Webhook requests are authenticated using HMAC-SHA256 signatures to ensure authenticity and integrity.

## Trigger Conditions

Webhook notifications are triggered when all of the following conditions are met:

1. **Transaction type match**: The transaction type is one of:
   - `transfer`: Native token transfer (ETH, BNB, TRX, etc.)
   - `erc20_transfer`: ERC20 token transfer (Ethereum, BSC, Polygon, and other EVM chains)
   - `trc20_transfer`: TRC20 token transfer (Tron chain)

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
| `X-Paratro-Signature` | Base64-encoded HMAC-SHA256 signature | `abc123...` |
| `X-Paratro-Api-Key` | Client API Key | `your-api-key` |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Event ID |
| `chain` | string | Yes | Blockchain name (e.g., `ethereum`, `bsc`, `polygon`, `tron`) |
| `txhash` | string | Yes | Transaction hash (unique identifier) |
| `type` | string | Yes | Transaction type: `transfer`, `erc20_transfer`, or `trc20_transfer` |
| `from` | string | Yes | Sender address |
| `to` | string | Yes | Recipient address |
| `symbol` | string | Yes | Token symbol |
| `amount` | string | Yes | Amount (in smallest unit, string format) |
| `decimals` | number | Yes | Token decimal places |
| `data` | string | Yes | Hex-encoded transaction input data (with `0x` prefix) |

### Native Token Transfer Example

```json
{
  "chain": "ethereum",
  "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
  "type": "transfer",
  "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "to": "0x8ba1f109551bD432803012645Hac136c22C92900",
  "amount": "1000000000000000000",
  "token_decimals": 18,
  "data": "0x"
}
```

### ERC20 Token Transfer Example

```json
{
  "chain": "ethereum",
  "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
  "type": "erc20_transfer",
  "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "to": "0x8ba1f109551bD432803012645Hac136c22C92900",
  "token_amount": "1000000",
  "token_decimals": 6,
  "data": "0xa9059cbb0000000000000000000000008ba1f109551bd432803012645hac136c22c92900000000000000000000000000000000000000000000000000000000000f4240"
}
```

### TRC20 Token Transfer Example

```json
{
  "chain": "tron",
  "hash": "0x9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba",
  "type": "trc20_transfer",
  "from": "TXYZabcdefghijklmnopqrstuvwxyz123456",
  "to": "TXYZ1234567890abcdefghijklmnopqrstuv",
  "token_amount": "50000000",
  "token_decimals": 6,
  "data": "0xa9059cbb000000000000000000000000..."
}
```

## Signature Verification

To ensure the authenticity and integrity of Webhook requests, all requests include an HMAC-SHA256 signature. The receiver must verify the signature before processing the request.

**Signature Algorithm:** HMAC-SHA256 with the client-configured `WebhookSecret` as the key.

**Signature Computation Steps:**

**Step 1: Compute the request body hash**

```rust
use sha2::{Sha256, Digest};

let body_hash = hex::encode(Sha256::digest(&body_bytes));
```

**Step 2: Extract the request path**

```rust
use url::Url;

let parsed = Url::parse(&webhook_url).expect("invalid URL");
let request_path = parsed.path();
// e.g., https://example.com/webhook/notify -> /webhook/notify
```

**Step 3: Build the canonical string**

```rust
let canonical_string = format!("POST\n{}\n{}\n{}", request_path, x_timestamp, body_hash);
```

**Step 4: Compute the HMAC signature**

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

type HmacSha256 = Hmac<Sha256>;

let mut mac = HmacSha256::new_from_slice(webhook_secret.as_bytes())
    .expect("HMAC can take key of any size");
mac.update(canonical_string.as_bytes());
let result = mac.finalize().into_bytes();

let signature = STANDARD.encode(&result);
```

### Signature Verification Example (axum)

```rust
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use hmac::{Hmac, Mac};
use sha2::{Sha256, Digest};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct AppState {
    webhook_secret: String,
    webhook_url: String,
}

#[derive(Deserialize)]
struct WebhookPayload {
    chain: String,
    hash: String,
    #[serde(rename = "type")]
    tx_type: String,
    from: String,
    to: String,
    amount: Option<String>,
    token_amount: Option<String>,
    token_decimals: i32,
    data: String,
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

fn verify_signature(
    secret: &str,
    webhook_url: &str,
    timestamp: &str,
    signature: &str,
    body: &[u8],
) -> bool {
    // 1. Compute body hash
    let body_hash = hex::encode(Sha256::digest(body));

    // 2. Extract request path
    let parsed = url::Url::parse(webhook_url).unwrap();
    let request_path = parsed.path();

    // 3. Build canonical string
    let canonical = format!("POST\n{}\n{}\n{}", request_path, timestamp, body_hash);

    // 4. Compute HMAC
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(canonical.as_bytes());
    let expected = STANDARD.encode(mac.finalize().into_bytes());

    // 5. Constant-time comparison
    expected.as_bytes() == signature.as_bytes()
}

async fn webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Json<Response>) {
    // 1. Extract headers
    let timestamp = match headers.get("X-Paratro-Timestamp").and_then(|v| v.to_str().ok()) {
        Some(ts) => ts,
        None => return (StatusCode::BAD_REQUEST, Json(Response {
            success: false, message: None,
            error: Some("Missing timestamp".into()), code: None,
        })),
    };

    let signature = match headers.get("X-Paratro-Signature").and_then(|v| v.to_str().ok()) {
        Some(sig) => sig,
        None => return (StatusCode::BAD_REQUEST, Json(Response {
            success: false, message: None,
            error: Some("Missing signature".into()), code: None,
        })),
    };

    // 2. Verify signature
    if !verify_signature(&state.webhook_secret, &state.webhook_url, timestamp, signature, &body) {
        return (StatusCode::UNAUTHORIZED, Json(Response {
            success: false, message: None,
            error: Some("Invalid signature".into()),
            code: Some("INVALID_SIGNATURE".into()),
        }));
    }

    // 3. Validate timestamp (15 min tolerance)
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        if (now - ts).abs() > 900 {
            return (StatusCode::UNAUTHORIZED, Json(Response {
                success: false, message: None,
                error: Some("Request expired".into()),
                code: Some("REQUEST_EXPIRED".into()),
            }));
        }
    }

    // 4. Parse payload
    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(Response {
            success: false, message: None,
            error: Some("Invalid request body".into()), code: None,
        })),
    };

    // 5. Process asynchronously (use tx hash for idempotency)
    let tx_hash = payload.hash.clone();
    tokio::spawn(async move {
        println!("Processing: chain={}, hash={}, type={}", payload.chain, tx_hash, payload.tx_type);
        // Your business logic here
    });

    // 6. Return success immediately
    (StatusCode::OK, Json(Response {
        success: true,
        message: Some("Webhook received successfully".into()),
        error: None, code: None,
    }))
}

#[tokio::main]
async fn main() {
    let state = AppState {
        webhook_secret: std::env::var("WEBHOOK_SECRET").expect("WEBHOOK_SECRET required"),
        webhook_url: std::env::var("WEBHOOK_URL").expect("WEBHOOK_URL required"),
    };

    let app = Router::new()
        .route("/webhook/notify", post(webhook_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Webhook server listening on :8080");
    axum::serve(listener, app).await.unwrap();
}
```

## Signature Verification Notes

1. **Timestamp validation**: It is recommended to validate the timestamp to prevent replay attacks. A typical tolerance window is 5-15 minutes.

2. **Signature comparison**: Always use a constant-time comparison function to prevent timing attacks.

3. **Path matching**: Ensure the path used for signature verification exactly matches the path configured in the Webhook URL.

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

### Server-Side Error Handling

When a Webhook delivery fails, the server logs detailed error information including:

- HTTP status code
- Client ID
- Transaction hash
- API Key
- Signature information
- Timestamp
- Body hash
- Canonical string
- Request path
- Request body
- Webhook URL
- Response body

### Client-Side Error Handling Recommendations

1. **Idempotency**: Use the transaction hash as a unique identifier to ensure duplicate notifications do not cause duplicate processing.

2. **Retry mechanism**: If processing fails, return a 5xx status code. The server may retry (the specific retry policy depends on the server implementation).

3. **Logging**: Log all received Webhook requests, including headers, body, and responses, to facilitate troubleshooting.

4. **Asynchronous processing**: It is recommended to process Webhook requests asynchronously - return a 200 status code immediately, then handle the business logic in the background.

## Security Considerations

1. **HTTPS transport**: It is strongly recommended to configure the Webhook URL with HTTPS to ensure transport security.

2. **Secret protection**: The `WebhookSecret` must be kept secure. Do not expose it or commit it to a code repository.

3. **IP allowlisting**: If possible, configure an IP allowlist to only accept requests from Paratro services.

4. **Signature verification**: You **must** verify the signature of every request and reject any request that fails verification.

5. **Timestamp validation**: Validate the timestamp to prevent replay attacks.

6. **Request body size limit**: It is recommended to set a request body size limit to prevent malicious requests.

## FAQ

**Q: How do I test Webhooks?**
A: You can use tools like [ngrok](https://ngrok.com/) or [localtunnel](https://localtunnel.github.io/www/) to expose your local service to the internet, then configure the Webhook URL accordingly.

**Q: What if Webhook processing takes too long?**
A: It is recommended to use an asynchronous processing pattern: return a 200 status code immediately, then handle the business logic in the background.

**Q: How do I prevent duplicate processing?**
A: Use the transaction hash (`hash` field) as a unique identifier and check whether the transaction has already been processed before handling it.

**Q: What should I do if signature verification fails?**
A: Check the following:
1. Whether the `WebhookSecret` is correct
2. Whether the request path matches the path configured in the Webhook URL
3. Whether the timestamp is within the valid range
4. Whether the request body has been modified (middleware may have altered the request body)

**Q: What transaction types are supported?**
A: The following three transaction types are currently supported:
- `transfer`: Native token transfer
- `erc20_transfer`: ERC20 token transfer
- `trc20_transfer`: TRC20 token transfer

**Q: What Rust crates are needed for webhook verification?**
A: Add these to your `Cargo.toml`:
```toml
[dependencies]
hmac = "0.12"
sha2 = "0.10"
hex = "0.4"
base64 = "0.22"
url = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```
