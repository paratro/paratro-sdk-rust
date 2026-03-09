# Error Handling

The Paratro SDK provides structured error types and convenience functions for handling API errors.

## Error Types

### Error

All SDK errors are represented by the `paratro_sdk::Error` enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API error: {body} (http_status: {status})")]
    Api { status: u16, body: ErrorBody },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),
}
```

### ErrorBody

The `ErrorBody` struct contains the API error details:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub error_type: String,
    pub message: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `code` | `String` | Machine-readable error code (e.g., `"not_found"`) |
| `error_type` | `String` | Error category (e.g., `"not_found_error"`) |
| `message` | `String` | Human-readable description |

### Error Helper Functions

The SDK provides convenience functions for common error checks:

```rust
// Reports whether the error is a 404 Not Found response.
paratro_sdk::is_not_found(err: &Error) -> bool

// Reports whether the error is a 429 Too Many Requests response.
paratro_sdk::is_rate_limited(err: &Error) -> bool

// Reports whether the error is an authentication/authorization error (401 or 403).
paratro_sdk::is_auth_error(err: &Error) -> bool
```

## Usage Examples

### Basic Error Handling

```rust
use paratro_sdk::is_not_found;

let result = client.get_wallet(wallet_id).await;
match result {
    Ok(wallet) => println!("Found wallet: {}", wallet.wallet_name),
    Err(ref e) if is_not_found(e) => {
        println!("Wallet {} not found", wallet_id);
    }
    Err(e) => {
        println!("Unexpected error: {e}");
    }
}
```

### Detailed Error Inspection

```rust
use paratro_sdk::{Error, CreateAssetRequest};

let result = client.create_asset(&CreateAssetRequest {
    account_id: account_id.to_string(),
    symbol: "USDT".to_string(),
    chain: Some("ethereum".to_string()),
}).await;

match result {
    Ok(asset) => println!("Asset created: {}", asset.asset_id),
    Err(Error::Api { status, ref body }) => {
        match body.code.as_str() {
            "asset_already_exists" => println!("Asset already added to this account"),
            "account_not_active" => println!("Account is not active"),
            "invalid_parameter" => println!("Invalid parameter: {}", body.message),
            _ => println!("API error [{}]: {} - {}", status, body.code, body.message),
        }
    }
    Err(Error::Http(e)) => {
        // Network error, timeout, JSON decode failure, etc.
        println!("Request failed: {e}");
    }
    Err(Error::InvalidConfig(msg)) => {
        println!("Invalid configuration: {msg}");
    }
}
```

### Rate Limiting

```rust
use paratro_sdk::{is_rate_limited, ListWalletsRequest};
use tokio::time::{sleep, Duration};

let result = client.list_wallets(&ListWalletsRequest {
    page: Some(1),
    page_size: Some(100),
}).await;

match result {
    Ok(resp) => { /* handle response */ }
    Err(ref e) if is_rate_limited(e) => {
        println!("Rate limited, retrying after delay...");
        sleep(Duration::from_secs(5)).await;
        // retry
    }
    Err(e) => println!("Error: {e}"),
}
```

### Authentication Errors

```rust
use paratro_sdk::{is_auth_error, CreateWalletRequest};

let result = client.create_wallet(&CreateWalletRequest {
    wallet_name: "New Wallet".to_string(),
    description: None,
}).await;

if let Err(ref e) = result {
    if is_auth_error(e) {
        println!("Authentication failed - check API key and secret");
    }
}
```

## Error Code Reference

See [API Reference - Error Codes](api-reference.md#error-code-reference) for the complete list of error codes.
