use serde::{Deserialize, Serialize};

/// Represents the API error response body (`common.ErrorBody` on the gateway).
///
/// Every non-2xx response from the gateway carries this shape:
/// `{"code": "...", "type": "...", "message": "..."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl std::fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} - {} (type: {})",
            self.code, self.message, self.error_type
        )
    }
}

/// Error `code` values emitted by the gateway (`common/errors.go`).
pub mod code {
    pub const BAD_REQUEST: &str = "bad_request";
    pub const INVALID_PARAMETER: &str = "invalid_parameter";
    pub const VALIDATION_FAILED: &str = "validation_failed";
    pub const UNAUTHORIZED: &str = "unauthorized";
    pub const INVALID_TOKEN: &str = "invalid_token";
    pub const TOKEN_EXPIRED: &str = "token_expired";
    pub const FORBIDDEN: &str = "forbidden";
    pub const NOT_FOUND: &str = "not_found";
    pub const RESOURCE_NOT_FOUND: &str = "resource_not_found";
    pub const CONFLICT: &str = "conflict";
    pub const RESOURCE_EXISTS: &str = "resource_exists";
    pub const TOO_MANY_REQUESTS: &str = "too_many_requests";
    pub const INTERNAL_ERROR: &str = "internal_error";
    pub const DATABASE_ERROR: &str = "database_error";
    pub const CACHE_ERROR: &str = "cache_error";
    pub const SERVICE_UNAVAILABLE: &str = "service_unavailable";
    pub const BUSINESS_ERROR: &str = "business_error";
    pub const WALLET_LIMIT_REACHED: &str = "wallet_limit_reached";
    pub const ACCOUNT_LIMIT_REACHED: &str = "account_limit_reached";
    pub const CHAIN_NOT_ALLOWED: &str = "chain_not_allowed";
    pub const WITHDRAWAL_LIMIT_REACHED: &str = "withdrawal_limit_reached";
    pub const API_QUOTA_EXCEEDED: &str = "api_quota_exceeded";
    pub const INSUFFICIENT_BALANCE: &str = "insufficient_balance";
    pub const INVALID_ADDRESS: &str = "invalid_address";
    pub const TRANSACTION_FAILED: &str = "transaction_failed";
    pub const CONCURRENCY_ERROR: &str = "concurrency_error";
    pub const ASSET_ALREADY_EXISTS: &str = "asset_already_exists";
    pub const WALLET_NOT_ACTIVE: &str = "wallet_not_active";
    pub const ACCOUNT_NOT_ACTIVE: &str = "account_not_active";
    pub const ADDRESS_BLACKLISTED: &str = "address_blacklisted";
}

/// Error `type` value the gateway uses for retired (HTTP 410) endpoints.
pub const ERROR_TYPE_ENDPOINT_RETIRED: &str = "endpoint_retired";

/// Message prefix of a policy / verifier rejection on `POST /api/v1/transactions`
/// (`"Rejected: <tag>: <detail>"`, HTTP 400, code `invalid_parameter`).
pub const REJECTED_MESSAGE_PREFIX: &str = "Rejected: ";

/// Message prefix of a duplicate `reference_id` (HTTP 400, code `invalid_parameter`).
pub const DUPLICATE_REFERENCE_ID_PREFIX: &str = "Duplicate reference_id";

/// Message prefix of an unsupported `operation` value (HTTP 400, code `invalid_parameter`).
pub const UNSUPPORTED_OPERATION_PREFIX: &str = "Unsupported operation";

/// Message of the `503 service_unavailable` the gateway returns when the engine's
/// synchronous signing lane is saturated (`common.ErrEngineBusy`). The transaction
/// row **already exists** at that point and is marked `FAILED` (or held when a live
/// permit exists), so the `reference_id` is consumed — see [`ApiErrorKind::EngineBusy`].
pub const ENGINE_BUSY_MESSAGE: &str = "Signing service is busy, retry later";

/// Message of the `503 service_unavailable` the gateway returns when a chain read
/// needed to verify the request failed (`common.ErrChainRPCUnavailable`). Raised
/// before any row is created — see [`ApiErrorKind::ChainRpcUnavailable`].
pub const CHAIN_RPC_UNAVAILABLE_MESSAGE: &str = "Chain RPC unavailable; cannot verify request";

/// SDK error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The gateway answered with a non-2xx status and a `{code,type,message}` body.
    #[error("API error: {body} (http_status: {status})")]
    Api { status: u16, body: ErrorBody },

    /// Transport-level failure (connection, timeout, TLS, ...).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The gateway answered 2xx but the body could not be decoded (`status` is the
    /// HTTP status; `0` means the request body itself failed to serialize).
    #[error("Decode error (http_status: {status}): {source}")]
    Decode {
        status: u16,
        #[source]
        source: serde_json::Error,
    },

    #[error("Invalid config: {0}")]
    InvalidConfig(String),
}

/// Machine-readable classification of an [`Error::Api`].
///
/// Every variant maps to a concrete status/code/message pattern produced by the
/// gateway (`api/handler/transaction_handler.go`, `common/errors.go`,
/// `api/handler/retired_endpoint.go`, `middleware/jwt_auth.go`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiErrorKind {
    /// 400 `invalid_parameter`, message `"Rejected: <tag>: ..."` — the request was
    /// well-formed but a policy / verifier check refused it. See [`Error::reason_tag`].
    Rejected,
    /// 400 `invalid_parameter`, message `"Duplicate reference_id: ..."`.
    DuplicateReferenceId,
    /// 400 `invalid_parameter`, message `"Unsupported operation: ..."` (e.g. `X402`).
    UnsupportedOperation,
    /// 400 `insufficient_balance`.
    InsufficientBalance,
    /// 400 `transaction_failed` — the signing engine rejected or failed to broadcast
    /// **after** the row was created. The row is `FAILED`, or — for a `CONTRACT_CALL`
    /// whose EIP-2612 permit was already signed — held `PENDING` with its reservation
    /// locked until the permit deadline (gateway `dispatchSettle` → `holdOperation`),
    /// so a retry can see `insufficient_balance` meanwhile. Either way the
    /// `reference_id` is consumed: resubmit with a **new** one.
    /// [`Error::reason_tag`] returns the engine tag when present.
    TransactionFailed,
    /// Any other 400 (`invalid_parameter`, `business_error`, `wallet_not_active`, ...).
    BadRequest,
    /// 401 `token_expired` — the SDK refreshes and retries once before surfacing this.
    TokenExpired,
    /// Any other 401 (`unauthorized`, `invalid_token`).
    Unauthorized,
    /// 403 — no policy authorises the operation/chain, client suspended, IP not allowed, ...
    Forbidden,
    /// 404 — address / asset does not belong to this client, token not credited yet, or unknown id.
    NotFound,
    /// 409 — conflict.
    Conflict,
    /// 410 `type=endpoint_retired` — the endpoint was removed (`POST /transfer`, `POST /x402/sign`).
    EndpointRetired,
    /// 429 — rate limited / API quota exceeded.
    RateLimited,
    /// 503 `service_unavailable`, message [`ENGINE_BUSY_MESSAGE`] — the engine's
    /// signing lane was saturated **after** the gateway had created the transaction
    /// row: the row is `FAILED` (or held while a live permit exists) and the
    /// `reference_id` is consumed. Retrying with the same `reference_id` returns
    /// `400 Duplicate reference_id`; use a **new** `reference_id`.
    EngineBusy,
    /// 503 `service_unavailable`, message [`CHAIN_RPC_UNAVAILABLE_MESSAGE`] — a chain
    /// read needed to verify the request failed **before** any row was created.
    /// Retrying later with the same `reference_id` is safe.
    ChainRpcUnavailable,
    /// Any other 503 `service_unavailable`. On the current gateway that is only
    /// `"<OPERATION> is not enabled on this gateway"`, raised before anything is
    /// created. Check [`Error::message`] rather than assuming the `reference_id` is free.
    ServiceUnavailable,
    /// 5xx other than 503.
    ServerError,
    /// Anything else.
    Other,
}

impl Error {
    /// HTTP status of an [`Error::Api`] / [`Error::Decode`]; `None` for transport / config errors.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api { status, .. } | Error::Decode { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// The gateway error `code` (e.g. `invalid_parameter`, `token_expired`).
    pub fn code(&self) -> Option<&str> {
        match self {
            Error::Api { body, .. } => Some(body.code.as_str()),
            _ => None,
        }
    }

    /// The gateway error `type` (e.g. `invalid_request_error`, `endpoint_retired`).
    pub fn error_type(&self) -> Option<&str> {
        match self {
            Error::Api { body, .. } => Some(body.error_type.as_str()),
            _ => None,
        }
    }

    /// The gateway error `message`.
    pub fn message(&self) -> Option<&str> {
        match self {
            Error::Api { body, .. } => Some(body.message.as_str()),
            _ => None,
        }
    }

    /// Classifies an [`Error::Api`]; transport / decode / config errors return `None`.
    pub fn kind(&self) -> Option<ApiErrorKind> {
        let Error::Api { status, body } = self else {
            return None;
        };
        let kind = match *status {
            400 => {
                if body.message.starts_with(REJECTED_MESSAGE_PREFIX) {
                    ApiErrorKind::Rejected
                } else if body.message.starts_with(DUPLICATE_REFERENCE_ID_PREFIX) {
                    ApiErrorKind::DuplicateReferenceId
                } else if body.message.starts_with(UNSUPPORTED_OPERATION_PREFIX) {
                    ApiErrorKind::UnsupportedOperation
                } else if body.code == code::INSUFFICIENT_BALANCE {
                    ApiErrorKind::InsufficientBalance
                } else if body.code == code::TRANSACTION_FAILED {
                    ApiErrorKind::TransactionFailed
                } else {
                    ApiErrorKind::BadRequest
                }
            }
            401 if body.code == code::TOKEN_EXPIRED => ApiErrorKind::TokenExpired,
            401 => ApiErrorKind::Unauthorized,
            403 => ApiErrorKind::Forbidden,
            404 => ApiErrorKind::NotFound,
            409 => ApiErrorKind::Conflict,
            410 => ApiErrorKind::EndpointRetired,
            429 => ApiErrorKind::RateLimited,
            503 if body.message == ENGINE_BUSY_MESSAGE => ApiErrorKind::EngineBusy,
            503 if body.message == CHAIN_RPC_UNAVAILABLE_MESSAGE => {
                ApiErrorKind::ChainRpcUnavailable
            }
            503 => ApiErrorKind::ServiceUnavailable,
            500..=599 => ApiErrorKind::ServerError,
            _ => ApiErrorKind::Other,
        };
        Some(kind)
    }

    /// Extracts the machine-readable reason tag from a rejection.
    ///
    /// * `400 "Rejected: <tag>: <detail>"` → `Some("<tag>")` (policy / verifier rejection,
    ///   see [`crate::reason_tag`] for the known values).
    /// * `400 transaction_failed "<OPERATION> failed: <tag>"` → `Some("<tag>")` when the
    ///   engine exposed a tag; the generic `"engine rejected the transaction"` yields `None`.
    ///
    /// A tag is lowercase words joined by underscores (the gateway's `engineReasonTag`
    /// regex); anything else is not a tag and yields `None`.
    pub fn reason_tag(&self) -> Option<&str> {
        let Error::Api { status: 400, body } = self else {
            return None;
        };
        let rest = if let Some(rest) = body.message.strip_prefix(REJECTED_MESSAGE_PREFIX) {
            rest
        } else if body.code == code::TRANSACTION_FAILED {
            body.message.split_once(" failed: ")?.1
        } else {
            return None;
        };
        let tag = rest.split(':').next()?.trim();
        is_reason_tag(tag).then_some(tag)
    }

    pub fn is_rejected(&self) -> bool {
        self.kind() == Some(ApiErrorKind::Rejected)
    }

    pub fn is_duplicate_reference_id(&self) -> bool {
        self.kind() == Some(ApiErrorKind::DuplicateReferenceId)
    }

    pub fn is_unsupported_operation(&self) -> bool {
        self.kind() == Some(ApiErrorKind::UnsupportedOperation)
    }

    pub fn is_insufficient_balance(&self) -> bool {
        self.kind() == Some(ApiErrorKind::InsufficientBalance)
    }

    pub fn is_transaction_failed(&self) -> bool {
        self.kind() == Some(ApiErrorKind::TransactionFailed)
    }

    pub fn is_token_expired(&self) -> bool {
        self.kind() == Some(ApiErrorKind::TokenExpired)
    }

    pub fn is_forbidden(&self) -> bool {
        self.kind() == Some(ApiErrorKind::Forbidden)
    }

    pub fn is_not_found(&self) -> bool {
        self.kind() == Some(ApiErrorKind::NotFound)
    }

    pub fn is_endpoint_retired(&self) -> bool {
        self.kind() == Some(ApiErrorKind::EndpointRetired)
    }

    pub fn is_rate_limited(&self) -> bool {
        self.kind() == Some(ApiErrorKind::RateLimited)
    }

    /// `true` for [`ApiErrorKind::EngineBusy`]: the row exists and is `FAILED` (or
    /// held `PENDING` until the permit deadline when a `CONTRACT_CALL`'s permit was
    /// already signed), the `reference_id` is consumed — resubmit with a new one.
    pub fn is_engine_busy(&self) -> bool {
        self.kind() == Some(ApiErrorKind::EngineBusy)
    }

    /// `true` for [`ApiErrorKind::ChainRpcUnavailable`]: nothing was created, the same
    /// `reference_id` can be retried.
    pub fn is_chain_rpc_unavailable(&self) -> bool {
        self.kind() == Some(ApiErrorKind::ChainRpcUnavailable)
    }

    /// `true` for any HTTP 503 ([`ApiErrorKind::EngineBusy`],
    /// [`ApiErrorKind::ChainRpcUnavailable`] or [`ApiErrorKind::ServiceUnavailable`]).
    /// Use [`Error::is_engine_busy`] / [`Error::is_chain_rpc_unavailable`] to decide
    /// whether the `reference_id` can be reused.
    pub fn is_service_unavailable(&self) -> bool {
        matches!(
            self.kind(),
            Some(
                ApiErrorKind::EngineBusy
                    | ApiErrorKind::ChainRpcUnavailable
                    | ApiErrorKind::ServiceUnavailable
            )
        )
    }
}

/// Same shape the gateway accepts as a public engine reason:
/// `^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$`, at most 64 bytes.
fn is_reason_tag(tag: &str) -> bool {
    if tag.is_empty() || tag.len() > 64 {
        return false;
    }
    let mut prev_underscore = true; // disallow leading underscore
    let mut first = true;
    for c in tag.chars() {
        match c {
            'a'..='z' => {
                prev_underscore = false;
            }
            '0'..='9' if !first => {
                prev_underscore = false;
            }
            '_' if !prev_underscore => {
                prev_underscore = true;
            }
            _ => return false,
        }
        first = false;
    }
    !prev_underscore
}

/// Reports whether the error is a 404 Not Found response.
pub fn is_not_found(err: &Error) -> bool {
    matches!(err, Error::Api { status: 404, .. })
}

/// Reports whether the error is a 429 Too Many Requests response.
pub fn is_rate_limited(err: &Error) -> bool {
    matches!(err, Error::Api { status: 429, .. })
}

/// Reports whether the error is an authentication/authorization error (401 or 403).
pub fn is_auth_error(err: &Error) -> bool {
    matches!(
        err,
        Error::Api {
            status: 401 | 403,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(status: u16, code: &str, error_type: &str, message: &str) -> Error {
        Error::Api {
            status,
            body: ErrorBody {
                code: code.into(),
                error_type: error_type.into(),
                message: message.into(),
            },
        }
    }

    #[test]
    fn reason_tag_from_rejected() {
        let e = api(
            400,
            "invalid_parameter",
            "invalid_request_error",
            "Rejected: expiration_passed: quote expired at 1789449058",
        );
        assert_eq!(e.reason_tag(), Some("expiration_passed"));
        assert_eq!(e.kind(), Some(ApiErrorKind::Rejected));
        assert!(e.is_rejected());
    }

    #[test]
    fn reason_tag_detail_with_colons() {
        let e = api(
            400,
            "invalid_parameter",
            "invalid_request_error",
            "Rejected: malformed: signed_transaction cannot be decoded: bad base64: x",
        );
        assert_eq!(e.reason_tag(), Some("malformed"));
    }

    #[test]
    fn reason_tag_from_engine_failure() {
        let e = api(
            400,
            "transaction_failed",
            "business_error",
            "CONTRACT_CALL failed: receiver_not_ours",
        );
        assert_eq!(e.reason_tag(), Some("receiver_not_ours"));
        assert_eq!(e.kind(), Some(ApiErrorKind::TransactionFailed));

        let generic = api(
            400,
            "transaction_failed",
            "business_error",
            "PROGRAM_CALL failed: engine rejected the transaction",
        );
        assert_eq!(generic.reason_tag(), None);
        assert!(generic.is_transaction_failed());
    }

    #[test]
    fn reason_tag_absent_for_other_errors() {
        assert_eq!(
            api(
                400,
                "insufficient_balance",
                "business_error",
                "Insufficient balance"
            )
            .reason_tag(),
            None
        );
        assert_eq!(
            api(403, "forbidden", "permission_error", "Rejected: x: y").reason_tag(),
            None
        );
        assert_eq!(Error::InvalidConfig("x".into()).reason_tag(), None);
    }

    #[test]
    fn tag_shape() {
        assert!(is_reason_tag("limit_daily"));
        assert!(is_reason_tag("shape"));
        assert!(is_reason_tag("limit_per_transaction"));
        assert!(!is_reason_tag(""));
        assert!(!is_reason_tag("_leading"));
        assert!(!is_reason_tag("trailing_"));
        assert!(!is_reason_tag("double__underscore"));
        assert!(!is_reason_tag("1starts_with_digit"));
        assert!(!is_reason_tag("Upper"));
        assert!(!is_reason_tag("has space"));
        assert!(!is_reason_tag(&"a".repeat(65)));
    }

    #[test]
    fn kinds() {
        assert_eq!(
            api(
                400,
                "invalid_parameter",
                "invalid_request_error",
                "Duplicate reference_id: this reference has already been used"
            )
            .kind(),
            Some(ApiErrorKind::DuplicateReferenceId)
        );
        assert_eq!(
            api(
                400,
                "invalid_parameter",
                "invalid_request_error",
                "Unsupported operation: only TRANSFER, PROGRAM_CALL and CONTRACT_CALL are available"
            )
            .kind(),
            Some(ApiErrorKind::UnsupportedOperation)
        );
        assert_eq!(
            api(
                400,
                "insufficient_balance",
                "business_error",
                "Insufficient balance"
            )
            .kind(),
            Some(ApiErrorKind::InsufficientBalance)
        );
        assert_eq!(
            api(
                400,
                "wallet_not_active",
                "business_error",
                "Wallet is not active"
            )
            .kind(),
            Some(ApiErrorKind::BadRequest)
        );
        assert_eq!(
            api(
                401,
                "token_expired",
                "authentication_error",
                "Token expired"
            )
            .kind(),
            Some(ApiErrorKind::TokenExpired)
        );
        assert_eq!(
            api(
                401,
                "invalid_token",
                "authentication_error",
                "Invalid token"
            )
            .kind(),
            Some(ApiErrorKind::Unauthorized)
        );
        assert_eq!(
            api(403, "forbidden", "permission_error", "no policy").kind(),
            Some(ApiErrorKind::Forbidden)
        );
        assert_eq!(
            api(404, "resource_not_found", "not_found_error", "no asset").kind(),
            Some(ApiErrorKind::NotFound)
        );
        assert_eq!(
            api(410, "invalid_parameter", "endpoint_retired", "retired").kind(),
            Some(ApiErrorKind::EndpointRetired)
        );
        assert_eq!(
            api(429, "too_many_requests", "rate_limit_error", "slow down").kind(),
            Some(ApiErrorKind::RateLimited)
        );
        assert_eq!(
            api(
                503,
                "service_unavailable",
                "api_error",
                "CONTRACT_CALL is not enabled on this gateway"
            )
            .kind(),
            Some(ApiErrorKind::ServiceUnavailable)
        );
        assert_eq!(
            api(500, "internal_error", "api_error", "boom").kind(),
            Some(ApiErrorKind::ServerError)
        );
        assert_eq!(Error::InvalidConfig("x".into()).kind(), None);
    }

    /// The two fixed 503 messages of `writeOperationError` map to distinct kinds
    /// because they differ in whether the `reference_id` was consumed.
    #[test]
    fn service_unavailable_split_by_message() {
        let busy = api(503, "service_unavailable", "api_error", ENGINE_BUSY_MESSAGE);
        assert_eq!(busy.kind(), Some(ApiErrorKind::EngineBusy));
        assert!(busy.is_engine_busy());
        assert!(!busy.is_chain_rpc_unavailable());
        assert!(busy.is_service_unavailable());

        let rpc = api(
            503,
            "service_unavailable",
            "api_error",
            CHAIN_RPC_UNAVAILABLE_MESSAGE,
        );
        assert_eq!(rpc.kind(), Some(ApiErrorKind::ChainRpcUnavailable));
        assert!(rpc.is_chain_rpc_unavailable());
        assert!(!rpc.is_engine_busy());
        assert!(rpc.is_service_unavailable());

        let other = api(
            503,
            "service_unavailable",
            "api_error",
            "PROGRAM_CALL is not enabled on this gateway",
        );
        assert_eq!(other.kind(), Some(ApiErrorKind::ServiceUnavailable));
        assert!(other.is_service_unavailable());
        assert!(!other.is_engine_busy());
        assert!(!other.is_chain_rpc_unavailable());

        // Only an exact 503 message qualifies; a 500 with the same text does not.
        assert_eq!(
            api(500, "internal_error", "api_error", ENGINE_BUSY_MESSAGE).kind(),
            Some(ApiErrorKind::ServerError)
        );
    }

    #[test]
    fn legacy_helpers_still_work() {
        let nf = api(404, "not_found", "not_found_error", "x");
        assert!(is_not_found(&nf));
        assert!(!is_rate_limited(&nf));
        assert!(!is_auth_error(&nf));
        assert!(is_auth_error(&api(
            401,
            "unauthorized",
            "authentication_error",
            "x"
        )));
        assert!(is_auth_error(&api(
            403,
            "forbidden",
            "permission_error",
            "x"
        )));
        assert!(is_rate_limited(&api(
            429,
            "too_many_requests",
            "rate_limit_error",
            "x"
        )));
    }
}
