pub mod client;
pub mod config;
pub mod error;
mod token;
pub mod wallet;
pub mod account;
pub mod asset;
pub mod transaction;
pub mod transfer;
pub mod webhook;

/// Current version of the MPC SDK.
pub const VERSION: &str = "1.0.0";

pub use client::MpcClient;
pub use config::Config;
pub use error::{Error, ErrorBody, is_not_found, is_rate_limited, is_auth_error};
pub use wallet::*;
pub use account::*;
pub use asset::*;
pub use transaction::*;
pub use transfer::*;
