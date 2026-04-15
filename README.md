# Paratro MPC Wallet Gateway Rust SDK

[![Crates.io](https://img.shields.io/crates/v/paratro-sdk.svg)](https://crates.io/crates/paratro-sdk)
[![docs.rs](https://docs.rs/paratro-sdk/badge.svg)](https://docs.rs/paratro-sdk)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Official Rust SDK for Paratro MPC Wallet Gateway - A comprehensive Multi-Party Computation wallet management platform.

## Features

- MPC Wallets - Create and manage MPC wallets with enhanced security
- Multi-Chain Support - Ethereum, BSC, Polygon, Avalanche, Arbitrum, Optimism, Tron, Bitcoin, Solana
- Account Management - Create and manage multiple accounts per wallet
- Asset Management - Support for native tokens and ERC20/TRC20 tokens
- Transfer - Send funds to external addresses with automatic asset resolution
- Transaction Tracking - Complete transaction history and status tracking
- x402 Payments - ERC-3009/Permit2 authorization signing, verification, and settlement
- Secure - Built-in JWT authentication with automatic token management
- Webhook - HMAC-SHA256 signed webhook notifications for incoming transactions

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
paratro-sdk = "1.1.0"
tokio = { version = "1", features = ["full"] }
```

Or install via cargo:

```bash
cargo add paratro-sdk
```

**Requirements**: Rust 1.70 or higher

## Quick Start

```rust
use paratro_sdk::{
    Config, MpcClient,
    CreateWalletRequest, CreateAccountRequest, CreateAssetRequest,
    CreateTransferRequest, ListTransactionsRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = MpcClient::new(
        "your-api-key",
        "your-api-secret",
        Config::sandbox(),
    )?;

    // 1. Create wallet
    let wallet = client.create_wallet(&CreateWalletRequest {
        wallet_name: "My Wallet".to_string(),
        description: Some("Primary wallet".to_string()),
    }).await?;
    println!("Wallet ID: {}", wallet.wallet_id);

    // 2. Create account
    let account = client.create_account(&CreateAccountRequest {
        wallet_id: wallet.wallet_id.clone(),
        chain: "ethereum".to_string(),
        network: "mainnet".to_string(),
        label: Some("Deposit Account".to_string()),
    }).await?;
    println!("Account: {} ({})", account.account_id, account.address);

    // 3. Add asset
    let asset = client.create_asset(&CreateAssetRequest {
        account_id: account.account_id.clone(),
        symbol: "USDT".to_string(),
        chain: Some("ethereum".to_string()),
    }).await?;
    println!("Asset: {} ({})", asset.asset_id, asset.symbol);

    // 4. Create transfer
    let transfer = client.create_transfer(&CreateTransferRequest {
        from_address: account.address.clone(),
        to_address: "0xbbbb...".to_string(),
        chain: "ethereum".to_string(),
        token_symbol: "USDT".to_string(),
        amount: "10.5".to_string(),
        memo: None,
    }).await?;
    println!("Transfer: {} ({})", transfer.tx_id, transfer.status);

    // 5. x402 Sign (ERC-3009 authorization for agent payments)
    let sign_resp = client.x402_sign(&paratro_sdk::X402SignRequest {
        from_address: account.address.clone(),
        to_address: "0xcccc...".to_string(),
        chain: "base".to_string(),
        amount: "0.10".to_string(),
        valid_after: None,
        valid_before: 1800000000,
    }).await?;
    println!("x402 Signed: {} (status: {})", sign_resp.auth_id, sign_resp.status);

    // 6. x402 Verify (Facilitator: verify a payment signature)
    let verify_resp = client.x402_verify(serde_json::json!({
        "x402Version": 2,
        "paymentPayload": { "payload": {}, "accepted": {} }
    })).await?;
    println!("Valid: {}, Payer: {}", verify_resp.is_valid, verify_resp.payer.unwrap_or_default());

    // 7. x402 Settle (Facilitator: execute on-chain settlement)
    let settle_resp = client.x402_settle(settle_payload).await?;
    println!("Settled: {}, TxHash: {}", settle_resp.success, settle_resp.transaction.unwrap_or_default());

    // 8. List transactions
    let tx_list = client.list_transactions(&ListTransactionsRequest {
        wallet_id: Some(wallet.wallet_id.clone()),
        page: Some(1),
        page_size: Some(20),
        ..Default::default()
    }).await?;
    for tx in &tx_list.items {
        println!("TX: {} {} {} ({})", tx.tx_hash, tx.amount, tx.token_symbol, tx.status);
    }

    Ok(())
}
```

## Webhooks

Verify and parse incoming webhook events from Paratro:

```rust
use paratro_sdk::webhook;

fn handle_webhook(body: &[u8], timestamp: &str, signature: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Verify signature
    webhook::verify_payload(
        "whsec_your_webhook_secret",
        timestamp,
        body,
        signature,
        webhook::DEFAULT_TOLERANCE,
    )?;

    // Parse event
    let event = webhook::parse_event(body)?;

    match event.event_type.as_str() {
        webhook::EVENT_TRANSACTION_CONFIRMED => {
            println!("Confirmed: {} {} {}", event.txhash, event.amount, event.symbol);
        }
        webhook::EVENT_TRANSACTION_CONFIRMING => {
            println!("Confirming: {} ({}/{})", event.txhash, event.confirmations, event.required_confirmations);
        }
        webhook::EVENT_TRANSACTION_FAILED => {
            println!("Failed: {}", event.txhash);
        }
        _ => {}
    }

    Ok(())
}
```

## Configuration

```rust
use paratro_sdk::{Config, MpcClient};

// Sandbox (for testing)
let client = MpcClient::new(api_key, api_secret, Config::sandbox())?;

// Production
let client = MpcClient::new(api_key, api_secret, Config::production())?;

// Custom environment
let client = MpcClient::new(api_key, api_secret, Config::custom("https://your-api.example.com"))?;
```

## Error Handling

The SDK returns `paratro_sdk::Error` for failures, with convenience helpers:

```rust
use paratro_sdk::{is_not_found, is_auth_error, is_rate_limited};

match client.get_wallet("wallet-id").await {
    Ok(wallet) => println!("Found: {}", wallet.wallet_name),
    Err(ref e) if is_not_found(e) => println!("Wallet not found"),
    Err(ref e) if is_auth_error(e) => println!("Authentication failed"),
    Err(ref e) if is_rate_limited(e) => println!("Rate limited, retry later"),
    Err(e) => println!("Error: {e}"),
}
```

See [Error Handling Guide](docs/error-handling.md) for detailed usage.

## Documentation

- [API Reference](docs/api-reference.md) - Complete endpoint documentation with request/response formats
- [Webhook Reference](docs/webhook-reference.md) - Webhook signature verification and integration guide
- [Error Handling](docs/error-handling.md) - Error types and handling patterns

## Development

### Project Structure

```
paratro-sdk-rust/
├── Cargo.toml              # Package manifest
├── src/
│   ├── lib.rs              # Public API exports, version
│   ├── client.rs           # HTTP client, response handling
│   ├── config.rs           # Environment configuration
│   ├── error.rs            # Error types and helpers
│   ├── token.rs            # JWT token management
│   ├── wallet.rs           # Wallet API
│   ├── account.rs          # Account API
│   ├── asset.rs            # Asset API
│   ├── transaction.rs      # Transaction API
│   ├── transfer.rs         # Transfer API
│   ├── x402.rs             # x402 Payment API (Sign, Verify, Settle)
│   └── webhook.rs          # Webhook verification & event parsing
├── tests/                  # Integration tests
└── docs/                   # Documentation
```

### Build & Test

```bash
cargo build
cargo clippy -- -D warnings
cargo test
```

## Support

- Email: hello@paratro.com
- Documentation: https://docs.paratro.com
- Issues: https://github.com/paratro/paratro-sdk-rust/issues

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
