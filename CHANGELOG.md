# Changelog

## 1.8.1 — 2026-09-15

Aligns the SDK with the gateway's unified transaction API. Every field, path,
status code and error tag in this release was checked against the gateway source
(`paratro-mpc-gateway` develop, deployed to sandbox on 2026-09-15). The crate keeps
its name and stays in the 1.x line, but **1.8.1 is not a purely additive release** —
read the ⚠️ Breaking section before upgrading.

### ⚠️ Breaking

- **`POST /api/v1/x402/sign` is retired (HTTP 410).** `MpcClient::x402_sign`,
  `X402SignRequest` and `X402SignResponse` are removed. There is no replacement:
  the unified entry answers `400 Unsupported operation` for `operation = X402`.
  The facilitator endpoints are unaffected (see below).
- **`POST /api/v1/transfer` is retired (HTTP 410).** `MpcClient::create_transfer`
  is kept but now sends `POST /api/v1/transactions` with `operation = "TRANSFER"`.
  `CreateTransferRequest` is an alias of the new `TransferRequest` and gained an
  optional `reference_id` field (struct literals must add `reference_id: None`).
  `TransferResponse` is an alias of `CreateTransactionResponse` (adds `tx_hash`,
  `http_status`).
- **x402 facilitator reply types now match the wire format.** The gateway speaks
  the Coinbase camelCase format; the 1.6 structs expected snake_case and could not
  decode real replies. `X402VerifyResponse { is_valid, invalid_reason: Option, payer }`,
  `X402SettleResponse { success, tx_id, transaction, error_reason: Option, payer, network }`,
  `X402SettleStatusResponse { success, tx_id, status, tx_hash, network }` map
  `isValid` / `txId` / `txHash` / `errorReason` / `invalidReason`. `X402Settlement`
  now has the gateway's fields (`tx_id`, `chain`, `from_address`, `to_address`,
  `amount`, `status`, `valid_before: i64`, `signature_v/r/s: Option`, `created_at`);
  the non-existent `settlement_id`, `signed_by`, `x402_nonce`, `eip712_hash`,
  `settle_tx_hash` are gone. `ListX402SettlementsRequest` gained `status`.
  `x402_verify` / `x402_settle` accept any `Serialize` body (the new
  `X402FacilitatorRequest` or a raw `serde_json::Value`).
- **`Transaction` matches `GET /api/v1/transactions/{id}`.** Removed `direction`,
  `block_number`, `confirmations` (never returned by the gateway); added
  `risk_score: Option<String>`, `risk_level: Option<String>`.
- **Security-factor API removed** (`list_security_factors`, `add_security_factor`,
  `delete_security_factor`, `set_security_factor_status` and their types). Those
  routes live on the customer portal API behind a portal user session and an ADMIN
  role — they do not exist on the gateway this SDK authenticates against.
- `Error` gained a `Decode { status, source }` variant (2xx body that failed to
  deserialize). Exhaustive matches on `Error` must handle it.
- **`Config` gained `timeout: Duration` and the default HTTP timeout is 200 s
  (`config::DEFAULT_TIMEOUT`), was a hard-coded 30 s.** `PROGRAM_CALL` /
  `CONTRACT_CALL` are synchronous: the gateway's engine budget is 120 s, it waits
  150 s (budget + 30 s broadcast margin) for the engine before answering `202`, and
  its server write timeout closes the connection at 180 s. A client that gives up
  before the gateway loses the `tx_id` while the row already exists under the
  `reference_id`, so the default sits above the write timeout (150 s would be cut
  off exactly when the gateway answers `202`). Construct `Config` via `sandbox()` /
  `production()` / `custom()` and override with `with_timeout()`; struct literals
  must now set `timeout`. The same timeout applies to `POST /api/v1/auth/token`.
  Same default as `paratro.DefaultTimeout` in the Go SDK and `paratro.DEFAULT_TIMEOUT`
  in the Python SDK.
- **`CreateTransactionResponse::is_accepted()` no longer means `http_status == 202`
  alone.** It is also `true` for a `PROGRAM_CALL` / `CONTRACT_CALL` answered
  `200 status=PENDING`, which is how an `Idempotency-Key` replay of a 202 arrives
  (the gateway replays cached bodies with HTTP 200). `CreateTransactionResponse`
  gained `operation: String` (SDK-filled, `#[serde(skip)]`) to make that decision;
  struct literals must set it. Same rule as `Accepted()` in the Go SDK.

### Added

- `MpcClient::create_transaction(&CreateTransactionRequest)` → `POST /api/v1/transactions`.
  `CreateTransactionRequest` is an enum tagged by `operation`:
  `Transfer(TransferRequest)`, `ProgramCall(ProgramCallRequest)`,
  `ContractCall(ContractCallRequest)`, with `transfer()` / `program_call()` /
  `contract_call()` constructors and `From` impls. Request structs have `new(...)`
  constructors and `with_*` setters; field names equal the JSON names.
- `CreateTransactionResponse { tx_id, status, message, tx_hash: Option, http_status, operation }`
  with `is_accepted()` (HTTP 202, or its replay as 200 `PENDING` for a `PROGRAM_CALL` /
  `CONTRACT_CALL` — outcome unknown, poll by `tx_id`, never resubmit with the same
  `reference_id`) and `is_broadcast()`.
- Error classification: `Error::kind() -> Option<ApiErrorKind>` (`Rejected`,
  `DuplicateReferenceId`, `UnsupportedOperation`, `InsufficientBalance`,
  `TransactionFailed`, `BadRequest`, `TokenExpired`, `Unauthorized`, `Forbidden`,
  `NotFound`, `Conflict`, `EndpointRetired`, `RateLimited`, `EngineBusy`,
  `ChainRpcUnavailable`, `ServiceUnavailable`, `ServerError`, `Other`),
  `Error::reason_tag()`, `status()` / `code()` / `error_type()` / `message()`
  accessors and `is_*` predicates. `paratro_sdk::error::code::*` holds the gateway
  error codes.
- The gateway's 503 is split by its fixed message because the two cases differ in
  whether the `reference_id` was consumed: `EngineBusy`
  (`"Signing service is busy, retry later"`, raised after the row was created — it is
  `FAILED`, or held `PENDING` until the permit deadline when a `CONTRACT_CALL`'s permit
  was already signed; resubmit with a **new** `reference_id`) vs `ChainRpcUnavailable`
  (`"Chain RPC unavailable; cannot verify request"`, raised before any row — the same
  `reference_id` can be retried). Other 503s stay `ServiceUnavailable`.
  `is_service_unavailable()` is true for all three; `is_engine_busy()` /
  `is_chain_rpc_unavailable()` distinguish them. Message constants:
  `error::ENGINE_BUSY_MESSAGE`, `error::CHAIN_RPC_UNAVAILABLE_MESSAGE`.
- `paratro_sdk::reason_tag` — every known rejection tag as a constant, grouped in
  `PROGRAM_CALL_TAGS`, `CONTRACT_CALL_TAGS`, `GATEWAY_TAGS`, `ENGINE_FAILURE_TAGS`,
  plus `is_known()`. `ENGINE_FAILURE_TAGS` (38 values) covers both engine paths that
  answer `400 transaction_failed "<OPERATION> failed: <tag>"`: `internal/syncsettle`
  and the `CONTRACT_CALL` permit step `internal/syncsign/permit.go`
  (`permit_owner_mismatch`, `permit_spender_mismatch`, `permit_value_mismatch`,
  `permit_digest_mismatch`, `permit_digest_missing`, `permit_domain_mismatch`,
  `permit_domain_unverified`, `permit_params_invalid`, `permit_token_mismatch`,
  `permit_token_not_registered`). The lists are the literals in the code at release
  time, not a closed set. Same values as `RejectionReason.ENGINE_FAILURE_TAGS` in
  the Python SDK and the engine-failure `Reason*` constants in the Go SDK.
- `Config::with_timeout(Duration)` and `config::DEFAULT_TIMEOUT` (200 s).
- `MpcClient::create_transaction_with_idempotency_key(&req, key)` and
  `MpcClient::x402_settle_with_idempotency_key(&payload, key)` send the optional
  `Idempotency-Key` header (`HEADER_IDEMPOTENCY_KEY`) that `POST /api/v1/transactions`
  and `POST /api/v1/x402/settle` honour (2xx body cached 24 h, replayed with HTTP 200).
  Same capability as `WithIdempotencyKey` in the Go SDK and `idempotency_key=` in the
  Python SDK; `create_transaction` / `x402_settle` still send no such header.
- `ApiErrorKind::TransactionFailed` / `is_engine_busy()` docs: the row behind a
  `400 transaction_failed` or engine-busy `503` is `FAILED`, or — for a `CONTRACT_CALL`
  whose EIP-2612 permit was already signed — held `PENDING` with its reservation locked
  until the permit deadline (gateway `dispatchSettle` → `holdOperation`); a retry with a
  new `reference_id` can see `insufficient_balance` meanwhile.
- Automatic retry: a `401 token_expired` triggers one token refresh and one retry
  of the same request. Proactive refresh (2 minutes before `expires_in`) is unchanged.
- `webhook::EVENT_TRANSFER_CREDITED` (`transfer.credited`),
  `webhook::EVENT_X402_SETTLEMENT_CONFIRMED` (`x402.settlement.confirmed`, the
  seller-side credit of a facilitator settlement) and `webhook::EVENT_TYPES` (all five
  event types the message service emits).
- `HEADER_API_KEY` / `HEADER_API_SECRET` constants; `OPERATION_*`, `STATUS_PENDING`,
  `STATUS_BROADCAST`, `TRANSACTIONS_PATH` constants.
- `tests/http_contract.rs`: wire-level tests against a scripted loopback server
  (exact JSON bytes and paths for all three operations, 202 handling, error mapping,
  legacy wrapper routing, auth headers, token_expired retry, facilitator shapes).

### Packaging

- `Cargo.toml` regained the crates.io metadata (`description`, `license`, `repository`,
  `keywords`, `categories`) that had been dropped in 1.4.0; without it crates.io
  refuses `cargo publish` ("missing or empty metadata fields"), and crates.io still
  lists 1.1.6 as the latest version. Declared `rust-version = "1.85"` (verified with
  `cargo +1.85 check --locked --all-targets`; 1.83 fails on the locked dependency tree).

### Migration from 1.6

The crate name and module paths do not change (`paratro-sdk = "1.8"`, `use paratro_sdk::…`).

| 1.6 | 1.8.1 |
|---|---|
| `client.x402_sign(&X402SignRequest { .. })` | removed — endpoint retired; no replacement |
| `client.create_transfer(&CreateTransferRequest { .. })` | still works (add `reference_id: None`); or `client.create_transaction(&CreateTransactionRequest::transfer(TransferRequest::new(..)))` |
| `TransferResponse { tx_id, status, message }` | same fields, plus `tx_hash: Option<String>`, `http_status: u16`, `operation: String` |
| `Config { base_url }` | `Config { base_url, timeout }` — use `Config::sandbox()` / `custom(..)` and `.with_timeout(..)` |
| `verify_resp.is_valid` / `invalid_reason: String` | `is_valid` / `invalid_reason: Option<String>` |
| `settle_resp.error` / `tx_id` | `error_reason: Option<String>` / `tx_id` (from `txId`) |
| `settle_status.tx_hash` | unchanged name, now decoded from `txHash` |
| `settlement.settlement_id` | `settlement.tx_id` |
| `tx.direction` / `block_number` / `confirmations` | removed (use webhooks for confirmations) |
| `client.list_security_factors(..)` etc. | removed — portal API, not gateway |
| `matches!(err, Error::Api { status: 404, .. })` | still works; or `err.is_not_found()` / `err.kind()` |

New behaviour to handle:

1. `create_transaction` may return `Ok(resp)` with `resp.is_accepted()` (HTTP 202,
   or a `PROGRAM_CALL` / `CONTRACT_CALL` replayed as 200 `PENDING` under an
   `Idempotency-Key`). Poll `get_transaction(&resp.tx_id)`. Do not resubmit with the
   same `reference_id`.
2. `400 "Rejected: <tag>: …"` — read `err.reason_tag()` and match against
   `paratro_sdk::reason_tag::*`.
3. `403` on `PROGRAM_CALL` / `CONTRACT_CALL` means no `OPERATION_RULES` policy
   authorises that operation / chain for the client.
4. On a 503, check `err.is_engine_busy()`: the row exists and is `FAILED` (or held
   `PENDING` until the permit deadline when a `CONTRACT_CALL`'s permit was already
   signed), so retry with a new `reference_id`. Only `err.is_chain_rpc_unavailable()`
   may be retried under the same `reference_id`. Persist `reference_id → tx_id` from
   every reply; the API cannot search by `reference_id` — or send an `Idempotency-Key`
   via `create_transaction_with_idempotency_key` so a replay returns the original body.

## 1.6.0 and earlier

See the git history.
