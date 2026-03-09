# API Reference

**Base URL:** `https://{host}/api/v1`

**Content-Type:** `application/json`

## Authentication

All protected endpoints require a JWT token in the `Authorization` header:

```
Authorization: Bearer <token>
```

Obtain a token via `POST /api/v1/auth/token` using API Key and API Secret. The SDK handles this automatically.

## Response Format

**Success (HTTP 200) - Single resource:**

```json
{
  "wallet_id": "a1b2c3d4-...",
  "wallet_name": "My Wallet",
  "status": "active",
  "created_at": "2025-01-15T10:30:45+08:00"
}
```

**Success (HTTP 200) - Paginated list:**

```json
{
  "data": [
    { "wallet_id": "a1b2c3d4-...", "wallet_name": "My Wallet" }
  ],
  "total": 42,
  "has_more": true
}
```

| Field     | Type    | Description                    |
|-----------|---------|--------------------------------|
| `data`    | array   | Resource list for current page |
| `total`   | integer | Total count across all pages   |
| `has_more`| boolean | Whether more pages exist       |

**Error (HTTP 4xx / 5xx):**

```json
{
  "code": "not_found",
  "type": "not_found_error",
  "message": "Wallet not found"
}
```

| Field     | Type   | Description                   |
|-----------|--------|-------------------------------|
| `code`    | string | Machine-readable error code   |
| `type`    | string | Error category classification |
| `message` | string | Human-readable description    |

## Error Code Reference

### Client Errors (4xx)

| HTTP | code | type | Description |
|------|------|------|-------------|
| 400 | `bad_request` | `invalid_request_error` | Malformed request |
| 400 | `invalid_parameter` | `invalid_request_error` | Missing or invalid parameter |
| 400 | `validation_failed` | `invalid_request_error` | Field validation failed |
| 401 | `unauthorized` | `authentication_error` | Missing or invalid credentials |
| 401 | `invalid_token` | `authentication_error` | JWT token is invalid |
| 401 | `token_expired` | `authentication_error` | JWT token has expired |
| 403 | `forbidden` | `permission_error` | Access denied |
| 404 | `not_found` | `not_found_error` | Resource not found |
| 409 | `conflict` | `conflict_error` | Resource conflict |
| 429 | `too_many_requests` | `rate_limit_error` | Rate limit exceeded |

### Business Errors (400)

| HTTP | code | type | Description |
|------|------|------|-------------|
| 400 | `wallet_limit_reached` | `business_error` | Maximum wallet count exceeded |
| 400 | `insufficient_balance` | `business_error` | Insufficient balance for operation |
| 400 | `invalid_address` | `business_error` | Invalid blockchain address |
| 400 | `transaction_failed` | `business_error` | Transaction execution failed |
| 400 | `asset_already_exists` | `business_error` | Asset already added to account |
| 400 | `wallet_not_active` | `business_error` | Wallet is not in active state |
| 400 | `account_not_active` | `business_error` | Account is not in active state |
| 400 | `concurrency_error` | `business_error` | Concurrent operation conflict |

### Server Errors (5xx)

| HTTP | code | type | Description |
|------|------|------|-------------|
| 500 | `internal_error` | `api_error` | Internal server error |
| 503 | `service_unavailable` | `api_error` | Service temporarily unavailable |

---

## Endpoints

### Auth

#### POST /auth/token

Issue a JWT token. **No authentication required.**

**Request Headers:**

| Header | Required | Description |
|--------|----------|-------------|
| `X-API-Key` | Yes | Your API key |
| `X-API-Secret` | Yes | Your API secret |

**Request Body:** None

**Success Response (200):**

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_in": 1800,
  "token_type": "Bearer",
  "client": {
    "client_id": "c_01HXYZ...",
    "client_name": "Acme Corp",
    "status": "active",
    "max_wallets": 10
  }
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 401 | `unauthorized` | Invalid API Key or API Secret |
| 403 | `forbidden` | Client inactive or IP not whitelisted |
| 500 | `internal_error` | Service configuration error |

#### GET /auth/me

Get authenticated client information.

**Success Response (200):**

```json
{
  "client_id": "c_01HXYZ..."
}
```

#### POST /auth/refresh

Refresh an existing JWT token.

**Request Body:**

```json
{
  "refresh_token": "rt_abc123..."
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `refresh_token` | string | Yes | Refresh token |

**Success Response (200):**

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_in": 1800,
  "token_type": "Bearer"
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Missing refresh_token |
| 401 | `unauthorized` | Invalid or expired refresh token |

#### POST /auth/logout

Logout and revoke the current token.

**Request Body:** None

**Success Response (200):**

```json
{
  "message": "Logged out successfully"
}
```

---

### Wallets

#### POST /wallets

Create a new MPC wallet.

**Request Body:**

```json
{
  "wallet_name": "My Wallet",
  "description": "Optional description"
}
```

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| `wallet_name` | string | Yes | 1-100 chars | Wallet display name |
| `description` | string | No | max 500 chars | Description |

**Success Response (200):**

```json
{
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "wallet_name": "My Wallet",
  "description": "Optional description",
  "status": "active",
  "key_status": "generating",
  "created_at": "2025-01-15T10:30:45+08:00",
  "updated_at": "2025-01-15T10:30:45+08:00"
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Invalid request body |
| 400 | `wallet_limit_reached` | Client wallet limit exceeded |
| 500 | `internal_error` | Creation failed |

#### GET /wallets/{id}

Get wallet details by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Wallet ID (UUID) |

**Success Response (200):**

```json
{
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "wallet_name": "My Wallet",
  "description": "",
  "status": "active",
  "key_status": "ready",
  "created_at": "2025-01-15T10:30:45+08:00",
  "updated_at": "2025-01-15T10:30:45+08:00"
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Missing wallet ID |
| 404 | `not_found` | Wallet not found |

#### GET /wallets

List wallets with pagination.

**Query Parameters:**

| Parameter | Type | Default | Constraints | Description |
|-----------|------|---------|-------------|-------------|
| `page` | integer | 1 | min 1 | Page number |
| `page_size` | integer | 10 | 1-100 | Items per page |

**Success Response (200):**

```json
{
  "data": [
    {
      "wallet_id": "w_01HXYZ...",
      "client_id": "c_01HXYZ...",
      "wallet_name": "My Wallet",
      "description": "",
      "status": "active",
      "key_status": "ready",
      "created_at": "2025-01-15T10:30:45+08:00",
      "updated_at": "2025-01-15T10:30:45+08:00"
    }
  ],
  "total": 5,
  "has_more": false
}
```

---

### Accounts

#### POST /accounts

Create a new blockchain account under a wallet.

**Request Body:**

```json
{
  "wallet_id": "w_01HXYZ...",
  "chain": "ethereum",
  "network": "mainnet",
  "label": "Deposit Account"
}
```

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| `wallet_id` | string | Yes | max 36 chars | Parent wallet ID |
| `chain` | string | Yes | enum | Blockchain (see below) |
| `network` | string | Yes | `mainnet` \| `testnet` | Target network |
| `label` | string | No | max 100 chars | Account label |

**Supported chains:** `ethereum`, `bsc`, `polygon`, `avalanche`, `arbitrum`, `optimism`, `tron`, `bitcoin`, `solana`

> **Note:** EVM-compatible chains (`ethereum`, `bsc`, `polygon`, `avalanche`, `arbitrum`, `optimism`) share the same key derivation and produce the same address. The system handles this internally.

**Success Response (200):**

```json
{
  "account_id": "acc_01HXYZ...",
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "address": "0x1a2b3c4d5e6f...",
  "chain": "ethereum",
  "network": "mainnet",
  "address_type": "DEPOSIT",
  "label": "Deposit Account",
  "status": "ACTIVE",
  "created_at": "2025-01-15T10:30:45+08:00"
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Invalid parameters or unsupported chain |
| 404 | `not_found` | Wallet not found |
| 400 | `wallet_not_active` | Wallet is not active |

#### GET /accounts/{id}

Get account details by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Account ID (UUID) |

**Success Response (200):**

```json
{
  "account_id": "acc_01HXYZ...",
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "address": "0x1a2b3c4d5e6f...",
  "chain": "evm",
  "network": "mainnet",
  "address_type": "DEPOSIT",
  "label": "Deposit Account",
  "status": "ACTIVE",
  "created_at": "2025-01-15T10:30:45+08:00"
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Missing account ID |
| 404 | `not_found` | Account not found |

#### GET /accounts

List accounts with optional wallet filter and pagination.

**Query Parameters:**

| Parameter | Type | Default | Constraints | Description |
|-----------|------|---------|-------------|-------------|
| `wallet_id` | string | - | max 36 chars | Filter by wallet (omit for all) |
| `page` | integer | 1 | min 1 | Page number |
| `page_size` | integer | 10 | 1-100 | Items per page |

**Success Response (200):**

```json
{
  "data": [
    {
      "account_id": "acc_01HXYZ...",
      "wallet_id": "w_01HXYZ...",
      "client_id": "c_01HXYZ...",
      "address": "0x1a2b3c4d5e6f...",
      "chain": "evm",
      "network": "mainnet",
      "address_type": "DEPOSIT",
      "label": "",
      "status": "ACTIVE",
      "created_at": "2025-01-15T10:30:45+08:00"
    }
  ],
  "total": 12,
  "has_more": true
}
```

---

### Assets

#### POST /assets

Add an asset (token) to an account. Asset configuration (contract address, decimals, etc.) is resolved automatically.

**Request Body:**

```json
{
  "account_id": "acc_01HXYZ...",
  "symbol": "USDT",
  "chain": "ethereum"
}
```

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| `account_id` | string | Yes | max 36 chars | Target account ID |
| `symbol` | string | Yes | enum | Token symbol (see below) |
| `chain` | string | Conditional | enum | Required for EVM accounts to specify target chain |

**Supported symbols:** `ETH`, `TRX`, `USDT`, `USDC`, `BNB`, `MATIC`, `BTC`, `SOL`

**Supported chains (for `chain` field):** `ethereum`, `bsc`, `polygon`, `avalanche`, `arbitrum`, `optimism`, `solana`

> **When is `chain` required?** EVM accounts can operate across multiple EVM chains. Provide `chain` to specify which chain's asset configuration to use (e.g., USDT on Ethereum vs. USDT on BSC). For Tron accounts, `chain` can be omitted.

**Success Response (200):**

```json
{
  "asset_id": "ast_01HXYZ...",
  "account_id": "acc_01HXYZ...",
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "chain": "ethereum",
  "network": "mainnet",
  "symbol": "USDT",
  "name": "Tether USD",
  "contract_address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
  "decimals": 6,
  "asset_type": "ERC20",
  "balance": "0",
  "locked_balance": "0",
  "is_active": true,
  "created_at": "2025-01-15T10:30:45+08:00"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `asset_type` | string | `NATIVE`, `ERC20`, `BEP20`, `TRC20` |
| `balance` | string | Available balance (decimal string, use BigDecimal to parse) |
| `locked_balance` | string | Locked balance (decimal string) |
| `contract_address` | string | Token contract address (`0x` for native assets) |

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Invalid parameters or unsupported symbol |
| 404 | `not_found` | Account not found |
| 400 | `asset_already_exists` | Asset already added to this account |
| 400 | `account_not_active` | Account is not active |

#### GET /assets/{id}

Get asset details by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Asset ID (UUID) |

**Success Response (200):**

```json
{
  "asset_id": "ast_01HXYZ...",
  "account_id": "acc_01HXYZ...",
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "chain": "ethereum",
  "network": "mainnet",
  "symbol": "ETH",
  "name": "Ethereum",
  "contract_address": "0x",
  "decimals": 18,
  "asset_type": "NATIVE",
  "balance": "1.5",
  "locked_balance": "0",
  "is_active": true,
  "created_at": "2025-01-15T10:30:45+08:00"
}
```

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Missing asset ID |
| 404 | `not_found` | Asset not found |

#### GET /assets

List assets with optional account filter and pagination.

**Query Parameters:**

| Parameter | Type | Default | Constraints | Description |
|-----------|------|---------|-------------|-------------|
| `account_id` | string | - | max 36 chars | Filter by account (omit for all) |
| `page` | integer | 1 | min 1 | Page number |
| `page_size` | integer | 10 | 1-100 | Items per page |

**Success Response (200):**

```json
{
  "data": [
    {
      "asset_id": "ast_01HXYZ...",
      "account_id": "acc_01HXYZ...",
      "wallet_id": "w_01HXYZ...",
      "client_id": "c_01HXYZ...",
      "chain": "ethereum",
      "network": "mainnet",
      "symbol": "ETH",
      "name": "Ethereum",
      "contract_address": "0x",
      "decimals": 18,
      "asset_type": "NATIVE",
      "balance": "1.5",
      "locked_balance": "0",
      "is_active": true,
      "created_at": "2025-01-15T10:30:45+08:00"
    }
  ],
  "total": 3,
  "has_more": false
}
```

---

### Transactions

#### POST /transfer

Create a transfer transaction. Atomically creates a transaction record and a sweep task, locks the transfer amount, and returns a `tx_id` for tracking.

**Request Body:**

```json
{
  "from_address": "0xaaaa...",
  "to_address": "0xbbbb...",
  "chain": "ethereum",
  "token_symbol": "USDT",
  "amount": "100.50",
  "memo": "Optional memo"
}
```

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| `from_address` | string | Yes | max 255 chars | Source blockchain address (must belong to the authenticated client) |
| `to_address` | string | Yes | max 255 chars | Destination blockchain address |
| `chain` | string | Yes | enum | Blockchain network (see supported chains below) |
| `token_symbol` | string | Yes | max 20 chars | Token symbol (e.g. `USDT`, `ETH`, `BTC`) |
| `amount` | string | Yes | > 0 | Transfer amount (decimal string, in user-facing units e.g. "1.5" BTC) |
| `memo` | string | No | max 100 chars | Optional memo or tag |

**Supported chains:** `ethereum`, `bsc`, `polygon`, `avalanche`, `arbitrum`, `optimism`, `tron`, `bitcoin`, `solana`

> **Asset resolution:** The system automatically locates the source asset by matching `from_address` + `chain` + `token_symbol` under the authenticated client. This eliminates the need for clients to store internal asset IDs.
>
> **Address validation:** The destination address format is validated against the specified chain before processing. EVM chains require `0x`-prefixed 42-character hex, Tron requires base58check-encoded `T`-prefix, Bitcoin requires valid mainnet/testnet format, Solana requires valid base58 Ed25519 public key.

**Success Response (200):**

```json
{
  "tx_id": "tx_01HXYZ...",
  "status": "PENDING",
  "message": "Transfer task created"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `tx_id` | string | Transaction ID for tracking transfer status |
| `status` | string | Always `PENDING` on creation |
| `message` | string | Confirmation message |

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Invalid request body or amount <= 0 |
| 400 | `insufficient_balance` | Insufficient available balance |
| 400 | `invalid_address` | Address format invalid for the asset's chain |
| 400 | `wallet_not_active` | Wallet is not in active state |
| 400 | `business_error` | Transaction amount exceeds wallet limit |
| 404 | `not_found` | No asset found for the given address/chain/token_symbol combination |
| 500 | `internal_error` | Internal processing failure |

#### GET /transactions/{id}

Get transaction details by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Transaction ID |

**Success Response (200):**

```json
{
  "tx_id": "tx_01HXYZ...",
  "wallet_id": "w_01HXYZ...",
  "client_id": "c_01HXYZ...",
  "chain": "ethereum",
  "transaction_type": "TRANSFER",
  "from_address": "0xaaaa...",
  "to_address": "0xbbbb...",
  "token_symbol": "USDT",
  "amount": "100.50",
  "status": "CONFIRMED",
  "tx_hash": "0xabcdef...",
  "created_at": "2025-01-15T10:30:45+08:00"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `amount` | string | Transaction amount (decimal string) |
| `status` | string | `PENDING`, `CONFIRMED`, `FAILED` |
| `tx_hash` | string | On-chain transaction hash (empty if not yet broadcast) |

**Error Responses:**

| HTTP | code | Condition |
|------|------|-----------|
| 400 | `invalid_parameter` | Missing transaction ID |
| 404 | `not_found` | Transaction not found |

#### GET /transactions

List transactions with optional filters and pagination.

**Query Parameters:**

| Parameter | Type | Default | Constraints | Description |
|-----------|------|---------|-------------|-------------|
| `wallet_id` | string | - | max 36 chars | Filter by wallet |
| `account_id` | string | - | max 36 chars | Filter by account |
| `chain` | string | - | enum | Filter by chain |
| `network` | string | - | `mainnet` \| `testnet` | Filter by network |
| `page` | integer | 1 | min 1 | Page number |
| `page_size` | integer | 10 | 1-100 | Items per page |

**Supported chains for filter:** `ethereum`, `tron`, `bsc`, `polygon`, `avalanche`, `arbitrum`, `optimism`, `bitcoin`, `solana`

**Success Response (200):**

```json
{
  "data": [
    {
      "tx_id": "tx_01HXYZ...",
      "wallet_id": "w_01HXYZ...",
      "client_id": "c_01HXYZ...",
      "chain": "ethereum",
      "transaction_type": "TRANSFER",
      "from_address": "0xaaaa...",
      "to_address": "0xbbbb...",
      "token_symbol": "USDT",
      "amount": "100.50",
      "status": "CONFIRMED",
      "tx_hash": "0xabcdef...",
      "created_at": "2025-01-15T10:30:45+08:00"
    }
  ],
  "total": 42,
  "has_more": true
}
```

---

## Quick Start (Rust)

```rust
use paratro_sdk::{Config, MpcClient, CreateWalletRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create client
    let client = MpcClient::new("your-api-key", "your-api-secret", Config::sandbox())?;

    // 2. Create wallet
    let wallet = client.create_wallet(&CreateWalletRequest {
        wallet_name: "My Wallet".to_string(),
        description: None,
    }).await?;
    println!("Wallet: {}", wallet.wallet_id);

    // 3. Logout when done
    client.logout().await?;

    Ok(())
}
```
