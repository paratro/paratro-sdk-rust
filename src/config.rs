use std::time::Duration;

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

/// Configuration for the MPC SDK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub base_url: String,
    /// Bounds every HTTP exchange the SDK makes (connect, request, response),
    /// including `POST /api/v1/auth/token`. Defaults to [`DEFAULT_TIMEOUT`]
    /// (200 s). Do not lower it for a client that sends `PROGRAM_CALL` or
    /// `CONTRACT_CALL`; shorter values are fine for `GET` calls and `TRANSFER`.
    pub timeout: Duration,
}

impl Config {
    /// Returns configuration for the sandbox environment.
    pub fn sandbox() -> Self {
        Self::custom("https://api-sandbox.paratro.com")
    }

    /// Returns configuration for the production environment.
    pub fn production() -> Self {
        Self::custom("https://api.paratro.com")
    }

    /// Returns a custom configuration with the specified base URL.
    pub fn custom(base_url: impl Into<String>) -> Self {
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
        assert_eq!(Config::sandbox().timeout, DEFAULT_TIMEOUT);
        assert_eq!(Config::production().timeout, DEFAULT_TIMEOUT);
        assert_eq!(
            Config::custom("http://127.0.0.1:1").timeout,
            DEFAULT_TIMEOUT
        );
    }

    #[test]
    fn with_timeout_overrides_the_default() {
        let cfg = Config::sandbox().with_timeout(Duration::from_secs(5));
        assert_eq!(cfg.timeout, Duration::from_secs(5));
        assert_eq!(cfg.base_url, "https://api-sandbox.paratro.com");
    }
}
