use std::time::Duration;

use crate::error::Error;

/// HTTP timeout used when [`Config::timeout`] is left at its default.
///
/// `PROGRAM_CALL` / `CONTRACT_CALL` are synchronous: the gateway keeps the
/// connection open while the signing engine signs and broadcasts. The gateway's
/// own ceilings (develop, `main.go` / `internal/client` / `internal/service`):
///
/// | | |
/// |---|---|
/// | engine budget | 120 s (`service.DefaultEngineTimeoutSeconds`) |
/// | wait for the engine before a `202` | 150 s (budget + `SyncSettleClientMargin` 30 s) |
/// | server `WriteTimeout` | 180 s (the connection is closed after this) |
///
/// The SDK default sits **above the `WriteTimeout`** so that the gateway, not the
/// SDK, is the side that gives up: a slow engine then ends in a `202` with a
/// `tx_id`, and past 180 s the gateway closes the connection itself. A
/// client-side timeout on that call has no `tx_id` while the row already exists
/// under your `reference_id` (see [`crate::MpcClient::create_transaction`]).
/// 150 s would not do — it is exactly how long the gateway waits for the engine
/// before answering `202`, and the SDK's timer starts earlier than the gateway's.
/// Same value as `paratro.DefaultTimeout` in the Go SDK and
/// `paratro.DEFAULT_TIMEOUT` in the Python SDK.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(200);

/// Message of the [`Error::InvalidConfig`] that [`crate::MpcClient::new`] returns
/// for a base URL that is not an absolute `http(s)://` URL. Word for word the
/// same in the Go and Python SDKs.
pub(crate) const BASE_URL_ERROR: &str = "base URL must be an absolute http(s) URL, e.g. https://api-sandbox.paratro.com or your private gateway";

/// Configuration for the MPC SDK.
///
/// The SDK has no built-in environment: the gateway base URL is required and is
/// always passed explicitly, because the same SDK talks to Paratro cloud and to
/// private deployments of the gateway. Use the address you were given — the
/// Paratro cloud hosts are listed in the README; a private deployment's address
/// comes from its operations team.
///
/// ```
/// use std::time::Duration;
/// use paratro_sdk::Config;
///
/// let cfg = Config::new("https://<gateway-host>").with_timeout(Duration::from_secs(10));
/// assert_eq!(cfg.base_url, "https://<gateway-host>");
/// assert_eq!(cfg.timeout, Duration::from_secs(10));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Base URL of the gateway: an absolute `http://` / `https://` URL with no
    /// path. Stored as given; [`crate::MpcClient::new`] validates it and strips
    /// trailing slashes (see [`Config::new`]).
    pub base_url: String,
    /// Bounds every HTTP exchange the SDK makes (connect, request, response),
    /// including `POST /api/v1/auth/token`. Defaults to [`DEFAULT_TIMEOUT`]
    /// (200 s). Do not lower it for a client that sends `PROGRAM_CALL` or
    /// `CONTRACT_CALL`; shorter values are fine for `GET` calls and `TRANSFER`.
    pub timeout: Duration,
}

impl Config {
    /// Returns the configuration for the gateway at `base_url`, with the default
    /// timeout ([`DEFAULT_TIMEOUT`]). This is the only constructor; there are no
    /// per-environment presets.
    ///
    /// `base_url` is not checked here — constructing a `Config` never fails.
    /// [`crate::MpcClient::new`] requires it to be non-empty and to start with
    /// `http://` or `https://`, strips trailing slashes, and otherwise fails with
    /// [`Error::InvalidConfig`].
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Overrides the HTTP timeout (see [`Config::timeout`]).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Validates a gateway base URL and returns it without trailing slashes.
///
/// Accepted: non-empty, starts with `http://` or `https://`, and has something
/// after the scheme. The Go and Python SDKs apply the same rule.
pub(crate) fn normalize_base_url(base_url: &str) -> Result<String, Error> {
    let url = base_url.trim_end_matches('/');
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .filter(|rest| !rest.is_empty());
    match host {
        Some(_) => Ok(url.to_string()),
        None => Err(Error::InvalidConfig(BASE_URL_ERROR.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_timeout_sits_above_the_gateway_write_timeout() {
        // Gateway: engine budget 120 s, answers 202 at 150 s (budget + 30 s
        // margin), server WriteTimeout 180 s. The SDK must outlast the gateway
        // so a slow engine ends in a 202 with a tx_id, not a client timeout.
        // Same default as Go (paratro.DefaultTimeout) / Python (DEFAULT_TIMEOUT).
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(200));
        assert!(DEFAULT_TIMEOUT > Duration::from_secs(180));
        assert_eq!(Config::new("http://127.0.0.1:1").timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn new_stores_the_base_url_as_given() {
        // Validation lives in MpcClient::new: constructing a Config never
        // fails, even for a URL the client will later refuse.
        assert_eq!(
            Config::new("https://gateway.example/").base_url,
            "https://gateway.example/"
        );
        assert_eq!(Config::new("").base_url, "");
    }

    #[test]
    fn with_timeout_overrides_the_default() {
        let cfg = Config::new("https://gateway.example").with_timeout(Duration::from_secs(5));
        assert_eq!(cfg.timeout, Duration::from_secs(5));
        assert_eq!(cfg.base_url, "https://gateway.example");
    }

    #[test]
    fn normalize_base_url_accepts_absolute_http_urls_and_strips_trailing_slashes() {
        for (given, want) in [
            ("https://gateway.example", "https://gateway.example"),
            ("https://gateway.example/", "https://gateway.example"),
            ("https://gateway.example///", "https://gateway.example"),
            ("http://127.0.0.1:8080/", "http://127.0.0.1:8080"),
        ] {
            assert_eq!(normalize_base_url(given).unwrap(), want, "{given:?}");
        }
    }

    #[test]
    fn normalize_base_url_rejects_anything_else() {
        for given in [
            "",
            "/",
            "gateway.example",
            "//gateway.example",
            "ftp://gateway.example",
            "https://",
            "https:///",
            "HTTPS://gateway.example",
            " https://gateway.example",
        ] {
            match normalize_base_url(given) {
                Err(Error::InvalidConfig(msg)) => assert_eq!(msg, BASE_URL_ERROR, "{given:?}"),
                other => panic!("{given:?}: expected InvalidConfig, got {other:?}"),
            }
        }
    }
}
