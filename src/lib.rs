pub mod account;
pub mod asset;
pub mod client;
pub mod config;
pub mod error;
mod token;
pub mod transaction;
pub mod transfer;
pub mod wallet;
pub mod webhook;
pub mod x402;

/// Current version of the MPC SDK.
pub const VERSION: &str = "1.1.0";

pub use account::*;
pub use asset::*;
pub use client::MpcClient;
pub use config::Config;
pub use error::{is_auth_error, is_not_found, is_rate_limited, Error, ErrorBody};
pub use transaction::*;
pub use transfer::*;
pub use wallet::*;
pub use x402::*;
