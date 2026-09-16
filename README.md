# Paratro MPC Wallet Gateway Rust SDK

[![Crates.io](https://img.shields.io/crates/v/paratro-sdk.svg)](https://crates.io/crates/paratro-sdk)
[![docs.rs](https://docs.rs/paratro-sdk/badge.svg)](https://docs.rs/paratro-sdk)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Official Rust SDK for the Paratro MPC Wallet Gateway.

> **1.8.1 is not a purely additive release.** `POST /api/v1/transfer` and
> `POST /api/v1/x402/sign` were retired by the gateway (HTTP 410); transactions now go
> through one entry, `POST /api/v1/transactions`, with an `operation` field. The crate
> name is unchanged. See the **⚠️ Breaking** section of [CHANGELOG.md](CHANGELOG.md)
> for the 1.6 → 1.8.1 migration.

## Features

- MPC wallets, accounts and assets
- Unified transactions: `TRANSFER`, `PROGRAM_CALL` (Solana) and `CONTRACT_CALL` (EVM xChange `executeSwap`)
- Idempotent submission via `reference_id`, explicit `202 Accepted` handling
- Typed errors: gateway `{code,type,message}` body, error kinds, machine-readable rejection tags
- x402 facilitator: verify, settle, settle status, settlements
- JWT authentication with proactive refresh and one automatic retry on `401 token_expired`
- Webhook signature verification and event parsing

## Installation

```toml
[dependencies]
paratro-sdk = "1.8"
tokio = { version = "1", features = ["full"] }
```

**Requirements**: Rust 1.85 or higher (`rust-version` in `Cargo.toml`; verified with
`cargo +1.85 check --locked --all-targets`. The SDK's own code needs far less — the
bound comes from the dependency tree: `reqwest` → `native-tls` → `getrandom 0.4`,
`icu 2.x` and an edition-2024 dependency do not build on 1.83 or older).

## Authentication

`MpcClient` exchanges the API key / secret for a JWT at `POST /api/v1/auth/token`
(headers `X-API-Key` / `X-API-Secret`) and caches it. The token is valid for the
`expires_in` the gateway returns — currently 15 minutes on every environment (the
gateway caps its configured lifetime at 900 s) — and the SDK refreshes two minutes
early. If the gateway still answers
`401 token_expired`, the SDK refreshes once and retries that request once; a second
`token_expired` is returned to you as `Error` with `kind() == Some(ApiErrorKind::TokenExpired)`.

The caller's IP must be in the client's IP allowlist.

```rust
use paratro_sdk::{Config, MpcClient};

let client = MpcClient::new("your-api-key", "your-api-secret", Config::sandbox())?;
// Config::production(), or Config::custom("https://your-gateway.example.com")
```

### HTTP timeout

`Config::timeout` bounds every HTTP exchange (auth call included) and defaults to
`paratro_sdk::config::DEFAULT_TIMEOUT` = **200 s**. `PROGRAM_CALL` / `CONTRACT_CALL`
are synchronous: the gateway holds the connection while the engine signs and
broadcasts — engine budget 120 s, it waits 150 s (budget + 30 s margin) for the engine
before answering `202`, and its server write timeout closes the connection at 180 s.
The default sits **above the 180 s write timeout** so the gateway, never the SDK, is
the side that gives up: a slow engine ends in a `202` with a `tx_id` instead of a
client-side timeout. If the SDK gave up first you would lose the `tx_id` while the row
already exists under your `reference_id` (a resend answers `400 Duplicate reference_id`,
and the API cannot search by `reference_id`); 150 s is exactly when the gateway answers
`202`, so it is not enough. Keep the default for clients that send those two
operations; `Config::sandbox().with_timeout(Duration::from_secs(10))` is fine for a
client that only reads or sends `TRANSFER`. Same default as the Go
(`paratro.DefaultTimeout`) and Python (`paratro.DEFAULT_TIMEOUT`) SDKs.

## Transactions

All three operations use `client.create_transaction(&CreateTransactionRequest)`.
Field names are exactly the gateway's JSON field names.

| Field | TRANSFER | PROGRAM_CALL | CONTRACT_CALL |
|---|---|---|---|
| `from_address` | required | required (our fee payer) | required (our payer / permit owner) |
| `chain` | required | `solana` | `ethereum` … |
| `reference_id` | optional, ≤100 chars, idempotency key — persist `reference_id → tx_id` from the reply; the API cannot search by `reference_id` | same | same |
| `memo` | optional, ≤100 chars | same | same |
| `to_address`, `token_symbol`, `amount` | required (`amount` human-readable decimal, e.g. `"10.5"`) | — | — |
| `receive_address` | — | optional, default = `from_address` | optional, default = `from_address` |
| `signed_transaction` | — | required (base64 or `0x`-hex) | — |
| `contract_call` | — | — | required, see below |

Reusing a `reference_id` returns `400 Duplicate reference_id`. That is the idempotency
contract of the endpoint: after a `202`, poll — never resubmit under the same reference.
The uniqueness key is status-agnostic: a row that ended `FAILED` (`400 transaction_failed`,
`503 EngineBusy`) still owns its `reference_id`, so a fresh attempt needs a new one.
There is no lookup by `reference_id` (`GET /api/v1/transactions` filters only
`wallet_id` / `account_id` / `chain`), so store the `tx_id` from the first reply.

### TRANSFER (asynchronous signing → `200 PENDING`)

```rust
use paratro_sdk::{CreateTransactionRequest, TransferRequest};

let resp = client
    .create_transaction(&CreateTransactionRequest::transfer(
        TransferRequest::new(
            "0x96586e99CE724F45bAb65cf963533b810147c1F4", // from_address
            "0xbbbb…",                                    // to_address
            "ethereum",
            "USDT",
            "10.5",
        )
        .with_reference_id("order-42")
        .with_memo("invoice 42"),
    ))
    .await?;
println!("{} {}", resp.tx_id, resp.status); // "<tx_id> PENDING"
```

`client.create_transfer(&CreateTransferRequest { .. })` still exists; since 1.8.1 it is a
wrapper that sends the same `TRANSFER` through `POST /api/v1/transactions`.

### PROGRAM_CALL (Solana, synchronous → `200 BROADCAST` or `202`)

The counterparty has partially signed a fixed-shape transaction (Memo + our
`TransferChecked` out + their `TransferChecked` in; no address lookup tables; our
wallet in the fee-payer slot with an empty signature). The engine fills our signature
and broadcasts.

```rust
use paratro_sdk::{CreateTransactionRequest, ProgramCallRequest};

let resp = client
    .create_transaction(&CreateTransactionRequest::program_call(
        ProgramCallRequest::new(
            "<our paying wallet>",
            "solana",
            "<base64 or 0x-hex partially signed transaction>",
        )
        .with_receive_address("<our receiving wallet>") // optional, default = from_address
        .with_reference_id("quote-7"),
    ))
    .await?;
```

### CONTRACT_CALL (EVM xChange `executeSwap`, synchronous → `200 BROADCAST` or `202`)

Amounts are **smallest-unit decimal integer strings**. The contract address is
**not** sent — the gateway takes it from the policy (`allowed_contracts[chain]`) —
and native value is always 0. `incoming` is what we pay (its `from` is filled by the
gateway = `from_address`); `outgoing` is what the counterparty pays us.

```rust
use paratro_sdk::{
    ContractCall, ContractCallIncomingLeg, ContractCallOutgoingLeg, ContractCallRequest,
    CreateTransactionRequest,
};

let our_wallet = "0x96586e99CE724F45bAb65cf963533b810147c1F4";
let counterparty = "0xcf8a9d1e489c58f4c3d69b45380fb4a6c03ada47";

let req = CreateTransactionRequest::contract_call(
    ContractCallRequest::new(
        our_wallet,
        "ethereum",
        ContractCall::new(
            "0x<bytes32 quote id>",
            1789449058, // expiration, unix seconds
            ContractCallIncomingLeg {
                to: counterparty.into(),
                token: "0x59fb67f6778cff089484cf7115906725dfc44293".into(), // bUSDC
                amount: "10000000".into(),                                 // 10 bUSDC (6 dp)
            },
            ContractCallOutgoingLeg {
                from: counterparty.into(),
                to: our_wallet.into(),
                token: "0xa55a927f2211fe52188526ed7e779b7298646e75".into(), // AAPLx
                amount: "42000000000000000".into(),
            },
            "0x<counterparty EIP-712 signature>",
        ), // .with_permit_deadline(unix_seconds) — optional
    )
    .with_receive_address(our_wallet) // optional, default = from_address
    .with_reference_id("quote-<quoteId>"),
);

let resp = client.create_transaction(&req).await?;
```

### Handling the reply: 200 vs 202

```rust
let resp = client.create_transaction(&req).await?;

if resp.is_accepted() {
    // HTTP 202, status "PENDING" (or its Idempotency-Key replay as HTTP 200 with the
    // same PENDING body): the signing engine did not answer in time. The transaction
    // may or may not have been broadcast. Poll by tx_id.
    // Do NOT resubmit with the same reference_id (→ 400 Duplicate reference_id).
    let tx = client.get_transaction(&resp.tx_id).await?;
    println!("{}: {}", tx.tx_id, tx.status);
} else if resp.is_broadcast() {
    // HTTP 200, status "BROADCAST": on-chain hash available.
    println!("broadcast {}", resp.tx_hash.as_deref().unwrap_or(""));
} else {
    // HTTP 200, status "PENDING": TRANSFER queued for asynchronous signing.
}
```

`is_accepted()` is `true` for HTTP `202`, and also for a `PROGRAM_CALL` /
`CONTRACT_CALL` answered `200 PENDING` — those operations never answer `PENDING`
otherwise, and that is exactly how an `Idempotency-Key` replay of a `202` arrives.
The SDK fills `resp.operation` from the request to make that decision (the gateway
does not echo it); a `TRANSFER`'s normal `200 PENDING` is not "accepted". Same rule as
`Accepted()` in the Go SDK and `.accepted` in the Python SDK.

Follow-up state comes from `GET /api/v1/transactions/{tx_id}` or the webhooks
`transaction.confirming` / `transaction.confirmed` / `transaction.failed`.

### Handling errors and rejection tags

Every gateway error is `Error::Api { status, body }` with the gateway's
`{code, type, message}`. `Error::kind()` classifies it; `Error::reason_tag()` extracts
the machine-readable tag from `400 "Rejected: <tag>: …"` (policy / verifier rejection)
and from `400 transaction_failed "<OPERATION> failed: <tag>"` (engine failure).

```rust
use paratro_sdk::{reason_tag, ApiErrorKind, Error};

match client.create_transaction(&req).await {
    Ok(resp) => { /* see above */ }
    Err(err) => match err.kind() {
        Some(ApiErrorKind::Rejected) => match err.reason_tag() {
            Some(reason_tag::EXPIRATION_PASSED) => { /* fetch a fresh quote */ }
            Some(reason_tag::LIMIT_DAILY) | Some(reason_tag::LIMIT_PER_TRANSACTION) => { /* over policy limit */ }
            Some(tag) => eprintln!("rejected: {tag}"),
            None => eprintln!("rejected: {}", err.message().unwrap_or("")),
        },
        Some(ApiErrorKind::DuplicateReferenceId) => {
            // Already submitted under this reference_id. Use the tx_id you stored from the
            // original reply — the API cannot search by reference_id. Don't resend.
        }
        Some(ApiErrorKind::InsufficientBalance) => { /* top up */ }
        Some(ApiErrorKind::TransactionFailed) => {
            // Engine failed after the row was created; it is FAILED (a CONTRACT_CALL whose
            // permit was already signed is held PENDING until the permit deadline instead,
            // reservation locked). reference_id consumed either way. Tag may be present:
            eprintln!("failed: {:?}", err.reason_tag());
        }
        Some(ApiErrorKind::Forbidden) => { /* no OPERATION_RULES policy authorises this operation/chain */ }
        Some(ApiErrorKind::NotFound) => { /* address/asset not this client's, or token not credited yet */ }
        Some(ApiErrorKind::ChainRpcUnavailable) => {
            // 503 "Chain RPC unavailable; cannot verify request": raised before any row
            // exists. Retry later with the SAME reference_id.
        }
        Some(ApiErrorKind::EngineBusy) => {
            // 503 "Signing service is busy, retry later": raised AFTER the row was created;
            // it is FAILED (or held PENDING until the permit deadline when a CONTRACT_CALL's
            // permit was already signed) and the reference_id is consumed. Retry later with
            // a NEW reference_id (the old one now returns 400 Duplicate reference_id).
        }
        Some(ApiErrorKind::ServiceUnavailable) => { /* any other 503, e.g. "<OP> is not enabled on this gateway" */ }
        Some(ApiErrorKind::EndpointRetired) => { /* 410: you are on an old path */ }
        _ => return Err(err.into()),
    },
}
```

| HTTP | `code` | Meaning |
|---|---|---|
| 400 | `invalid_parameter` `"Rejected: <tag>: …"` | policy / verifier rejection; tag in `reason_tag()` and `paratro_sdk::reason_tag::*` |
| 400 | `invalid_parameter` `"Duplicate reference_id: …"` | reference already used |
| 400 | `invalid_parameter` `"Unsupported operation: …"` | only `TRANSFER`, `PROGRAM_CALL`, `CONTRACT_CALL` |
| 400 | `insufficient_balance` | not enough book balance |
| 400 | `transaction_failed` `"<OP> failed: <tag>"` | engine rejected / failed to broadcast after the row was created; row is `FAILED` (a CONTRACT_CALL whose permit was already signed is held `PENDING` until the permit deadline, reservation locked — a retry may first see `400 insufficient_balance`), `reference_id` consumed |
| 401 | `token_expired` | refreshed and retried once by the SDK |
| 403 | `forbidden` | no policy authorises the operation / chain; client inactive; IP not allowed |
| 404 | `resource_not_found` / `not_found` | address / asset not this client's; token not credited; unknown id |
| 410 | type `endpoint_retired` | `POST /transfer`, `POST /x402/sign` |
| 503 | `service_unavailable` `"Chain RPC unavailable; cannot verify request"` | `ChainRpcUnavailable`: nothing was created — retry later with the same `reference_id` |
| 503 | `service_unavailable` `"Signing service is busy, retry later"` | `EngineBusy`: the row was already created and is `FAILED` (or held `PENDING` like the `transaction_failed` case when a live permit exists) — retry later with a **new** `reference_id` |
| 503 | `service_unavailable` other | `ServiceUnavailable`: e.g. `"<OP> is not enabled on this gateway"` (nothing created) |

`is_service_unavailable()` is true for all three 503 kinds; `is_engine_busy()` /
`is_chain_rpc_unavailable()` tell them apart. Which errors leave the `reference_id`
free: `ChainRpcUnavailable`, `Forbidden`, `NotFound`, `UnsupportedOperation` and
validation `BadRequest`s are raised before a row exists; `TransactionFailed`,
`EngineBusy` and a `202` leave a row behind under that `reference_id`. A `Rejected`
is normally pre-row, except the `CONTRACT_CALL` post-permit re-verification, whose row
is held for reconciliation — when in doubt use a new `reference_id` and reconcile the
old `tx_id` via `GET /api/v1/transactions/{tx_id}`.

Known tags are exported as constants in `paratro_sdk::reason_tag` (grouped as
`PROGRAM_CALL_TAGS`, `CONTRACT_CALL_TAGS`, `GATEWAY_TAGS`, `ENGINE_FAILURE_TAGS`).
`ENGINE_FAILURE_TAGS` holds every literal the engine emits after `<OPERATION> failed:`
— the settle / broadcast path (`request_digest_mismatch`, `payer_mismatch`,
`receiver_not_ours`, `cosignature_invalid`, `signer_slot`, …) and the EIP-2612 permit
step of a `CONTRACT_CALL` (`permit_owner_mismatch`, `permit_spender_mismatch`,
`permit_value_mismatch`, `permit_digest_mismatch`, `permit_digest_missing`,
`permit_domain_mismatch`, `permit_domain_unverified`, `permit_params_invalid`,
`permit_token_mismatch`, `permit_token_not_registered`). The set is not closed: new
releases can add tags, and TSS / broadcast failures arrive without one (`engine
rejected the transaction`), so match on the tags you handle and treat the rest as
"failed, reason in the message".

### Reading transactions

```rust
use paratro_sdk::ListTransactionsRequest;

let tx = client.get_transaction("tx_id").await?;

let page = client
    .list_transactions(&ListTransactionsRequest {
        wallet_id: Some(wallet.wallet_id.clone()), // also account_id, chain
        page: Some(1),
        page_size: Some(20),
        ..Default::default()
    })
    .await?;
for tx in &page.items {
    println!("{} {} {} {}", tx.tx_id, tx.amount, tx.token_symbol, tx.status);
}
```

The gateway also honours an optional `Idempotency-Key` header on `POST /api/v1/transactions`
and `POST /api/v1/x402/settle` (24 h response replay, `paratro_sdk::HEADER_IDEMPOTENCY_KEY`).
Send it with `create_transaction_with_idempotency_key(&req, "key")` /
`x402_settle_with_idempotency_key(&payload, "key")`; `create_transaction` / `x402_settle`
never set it. `reference_id` uniqueness described above remains the idempotency contract
of the endpoint; the header lets you **recover the original reply** after a transport
failure by replaying the identical request (same `reference_id`, same key). A replayed
response is always HTTP **200** with the cached body — a first reply of `202 PENDING`
is replayed as `200 PENDING` without `tx_hash`. `is_accepted()` stays `true` on such a
replay because it also looks at `status` and `operation`, not only at `http_status`.

```rust
let resp = client
    .create_transaction_with_idempotency_key(&req, "order-42")
    .await?;
```

## x402 facilitator

```rust
use paratro_sdk::{ListX402SettlementsRequest, X402FacilitatorRequest};

let payload = X402FacilitatorRequest {
    x402_version: 2,
    payment_payload: serde_json::json!({ "payload": { /* … */ }, "accepted": { /* … */ } }),
    payment_requirements: None, // v1 only
};

let verify = client.x402_verify(&payload).await?;      // POST /api/v1/x402/verify
if verify.is_valid {
    let settle = client.x402_settle(&payload).await?;  // POST /api/v1/x402/settle
    if settle.success {
        let status = client.x402_settle_status(&settle.tx_id).await?; // GET /api/v1/x402/settle/{tx_id}
        println!("{} {}", status.status, status.tx_hash);
    } else {
        eprintln!("settle failed: {:?}", settle.error_reason);
    }
}

let settlements = client
    .x402_list_settlements(&ListX402SettlementsRequest {
        status: Some("SETTLED".into()),
        page: Some(1),
        page_size: Some(20),
    })
    .await?; // GET /api/v1/x402/settlements
```

Verify / settle bodies and replies use the Coinbase facilitator camelCase wire
format (`x402Version`, `paymentPayload`, `isValid`, `txId`, …); the SDK structs map
them to snake_case fields. `x402_verify` / `x402_settle` also accept a raw
`serde_json::Value` with the same shape.

`x402_sign` no longer exists: the gateway retired `POST /api/v1/x402/sign`.

## Webhooks

```rust
use paratro_sdk::webhook;

fn handle_webhook(body: &[u8], timestamp: &str, signature: &str) -> Result<(), Box<dyn std::error::Error>> {
    webhook::verify_payload(
        "whsec_your_webhook_secret",
        timestamp,          // X-Paratro-Timestamp
        body,
        signature,          // X-Paratro-Signature ("v1=<hex>")
        webhook::DEFAULT_TOLERANCE,
    )?;

    let event = webhook::parse_event(body)?;
    match event.event_type.as_str() {
        webhook::EVENT_TRANSACTION_CONFIRMING => { /* confirmations / required_confirmations */ }
        webhook::EVENT_TRANSACTION_CONFIRMED => { /* credit the customer */ }
        webhook::EVENT_TRANSACTION_FAILED => { /* mark failed */ }
        webhook::EVENT_TRANSFER_CREDITED => { /* internal transfer landed; transaction_type == "INTERNAL" */ }
        webhook::EVENT_X402_SETTLEMENT_CONFIRMED => { /* x402 settlement credited to our address; transaction_type == "INBOUND" */ }
        _ => {}
    }
    // event.operation (TRANSFER / PROGRAM_CALL / CONTRACT_CALL / DEPOSIT / X402) says what kind of
    // movement this is. PROGRAM_CALL / CONTRACT_CALL swaps also carry the counter-asset leg:
    if let Some(leg) = &event.swap_incoming {
        if leg.booked { /* credit leg.amount (smallest unit) of leg.token_address */ }
        else { /* not credited: see leg.reason / leg.audit_type */ }
    }
    Ok(())
}
```

`webhook::EVENT_TYPES` lists these five event types — everything the message service
emits today. `x402.settlement.confirmed` is the seller-side notification for a
facilitator settlement that landed on one of this client's addresses; it does not
also produce `transfer.credited`.

## Development

```
src/
├── lib.rs           # exports, version, compile_fail proof that x402_sign is gone
├── client.rs        # HTTP client, token_expired retry, status-aware POST
├── config.rs        # environments
├── error.rs         # Error, ErrorBody, ApiErrorKind, reason_tag()
├── reason_tag.rs    # known rejection / engine failure tags
├── token.rs         # JWT manager
├── transaction.rs   # POST/GET /api/v1/transactions
├── transfer.rs      # pre-1.8 create_transfer wrapper
├── wallet.rs / account.rs / asset.rs
├── x402.rs          # facilitator API
└── webhook.rs       # signature verification, events
tests/
├── http_contract.rs # wire contract against a scripted loopback server
└── integration_test.rs
```

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
SKIP_INTEGRATION_TESTS=true cargo test
```

`tests/integration_test.rs` talks to the sandbox when `MPC_API_KEY` / `MPC_API_SECRET`
are set and `SKIP_INTEGRATION_TESTS` is not `true`.

## Support

- Email: hello@paratro.com
- Documentation: https://docs.paratro.com
- Issues: https://github.com/paratro/paratro-sdk-rust/issues

## License

MIT.
