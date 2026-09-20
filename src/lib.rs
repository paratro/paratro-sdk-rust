//! Official Rust SDK for the Paratro MPC Wallet Gateway.
//!
//! # 1.9.0 at a glance
//!
//! * No built-in gateway address: [`Config::new`] takes the base URL of the
//!   gateway you were given — Paratro cloud or a private deployment — and
//!   [`MpcClient::new`] rejects anything that is not an absolute `http(s)://`
//!   URL. There are no per-environment presets; the Paratro cloud hosts are
//!   listed in the README only.
//! * One transaction entry: [`MpcClient::create_transaction`] → `POST /api/v1/transactions`
//!   with `operation` = `TRANSFER` / `PROGRAM_CALL` / `CONTRACT_CALL`
//!   ([`CreateTransactionRequest`]). [`MpcClient::create_transfer`] is kept as a
//!   wrapper over the same endpoint.
//! * `202 Accepted` is surfaced through [`CreateTransactionResponse::is_accepted`]
//!   (also `true` for its `Idempotency-Key` replay, which arrives as `200 PENDING`):
//!   outcome unknown, poll by `tx_id`, never resubmit with the same `reference_id`.
//! * The HTTP timeout defaults to 200 s ([`config::DEFAULT_TIMEOUT`]) because
//!   `PROGRAM_CALL` / `CONTRACT_CALL` are synchronous on the gateway side (it
//!   answers `202` after 150 s and closes the connection at its 180 s write
//!   timeout); tune it with [`Config::with_timeout`].
//! * An optional `Idempotency-Key` can be sent with
//!   [`MpcClient::create_transaction_with_idempotency_key`] and
//!   [`MpcClient::x402_settle_with_idempotency_key`] ([`HEADER_IDEMPOTENCY_KEY`]).
//! * Errors carry the gateway's `{code,type,message}` ([`ErrorBody`]); classify
//!   them with [`Error::kind`] and read policy rejection tags with
//!   [`Error::reason_tag`] (known values in [`reason_tag`]).
//! * `x402_sign` is gone — the gateway retired `POST /api/v1/x402/sign` (HTTP 410).
//!   The facilitator endpoints (verify / settle / settle status / settlements) remain.
//!
//! The retired sign API is not just deprecated, it no longer exists in the crate:
//!
//! ```compile_fail
//! let _ = paratro_sdk::X402SignRequest {
//!     from_address: String::new(),
//!     to_address: String::new(),
//!     chain: String::new(),
//!     amount: String::new(),
//!     valid_before: String::new(),
//! };
//! ```
//!
//! ```compile_fail
//! async fn f(c: &paratro_sdk::MpcClient) {
//!     let _ = c.x402_sign(&serde_json::json!({})).await;
//! }
//! ```

pub mod account;
pub mod asset;
pub mod client;
pub mod config;
pub mod error;
pub mod reason_tag;
mod token;
pub mod transaction;
pub mod transfer;
pub mod wallet;
pub mod webhook;
pub mod x402;

/// Current version of the MPC SDK.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use account::*;
pub use asset::*;
pub use client::{MpcClient, HEADER_IDEMPOTENCY_KEY};
pub use config::Config;
pub use error::{is_auth_error, is_not_found, is_rate_limited, ApiErrorKind, Error, ErrorBody};
pub use token::{HEADER_API_KEY, HEADER_API_SECRET};
pub use transaction::*;
pub use transfer::*;
pub use wallet::*;
pub use webhook::*;
pub use x402::*;
