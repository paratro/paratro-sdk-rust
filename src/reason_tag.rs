//! Machine-readable reason tags returned by `POST /api/v1/transactions` for
//! `PROGRAM_CALL` / `CONTRACT_CALL`.
//!
//! Two error shapes carry a tag (extract it with [`crate::Error::reason_tag`]):
//!
//! * `400 invalid_parameter` — `"Rejected: <tag>: <detail>"`: the gateway or its
//!   verifier refused the request before anything was signed. Tags come from
//!   `paratro-common/xchange/solana_verifier.go` ([`PROGRAM_CALL_TAGS`]),
//!   `paratro-common/xchange/execute_swap.go` ([`CONTRACT_CALL_TAGS`]) and the
//!   gateway's own pre-checks in `internal/service/{operation_service,program_call,contract_call}.go`
//!   ([`GATEWAY_TAGS`]).
//! * `400 transaction_failed` — `"<OPERATION> failed: <tag>"`: the signing engine
//!   rejected the request after the gateway accepted it; the transaction row is
//!   already `FAILED`. The gateway forwards only the leading tag of the engine's
//!   reason (`publicEngineReason` in `api/handler/transaction_handler.go`); free-text
//!   internals collapse to `"engine rejected the transaction"` (no tag). Tags come from
//!   `paratro-mpc-engine/mpc-engine/internal/syncsettle` (settle / broadcast path,
//!   both operations) and `internal/syncsign/permit.go` (the EIP-2612 permit a
//!   `CONTRACT_CALL` signs first) — [`ENGINE_FAILURE_TAGS`].
//!
//! The lists below contain every tag literal in the gateway, verifier and engine
//! code at the time of this release (gateway `develop` @ `b8bbea5`, engine `develop`
//! @ `3d4a0ed`). The set is **not closed**: a new gateway or engine release can add
//! tags, and TSS / broadcast errors arrive as free text (no tag at all). Treat an
//! unknown tag as a rejection you have not seen yet, not as a parse error.

// ── PROGRAM_CALL (Solana verifier, paratro-common/xchange/solana_verifier.go) ──

pub const ACCOUNT_UNRESOLVABLE: &str = "account_unresolvable";
pub const ALT_NOT_ALLOWED: &str = "alt_not_allowed";
pub const ATA_DERIVATION: &str = "ata_derivation";
pub const COUNTERPARTY_NOT_REGISTERED: &str = "counterparty_not_registered";
pub const INCOMING_DESTINATION: &str = "incoming_destination";
pub const LIMIT_DAILY: &str = "limit_daily";
pub const LIMIT_NOT_CONFIGURED: &str = "limit_not_configured";
pub const LIMIT_PER_TRANSACTION: &str = "limit_per_transaction";
pub const MALFORMED: &str = "malformed";
pub const MINT_NOT_REGISTERED: &str = "mint_not_registered";
pub const MINT_PROGRAM_UNKNOWN: &str = "mint_program_unknown";
pub const OUTGOING_AUTHORITY: &str = "outgoing_authority";
pub const OUTGOING_SOURCE: &str = "outgoing_source";
pub const PROGRAM_NOT_ALLOWED: &str = "program_not_allowed";
pub const PROGRAM_UNRESOLVABLE: &str = "program_unresolvable";
pub const SHAPE: &str = "shape";

/// Tags the Solana `PROGRAM_CALL` verifier can put in `"Rejected: <tag>: ..."`.
pub const PROGRAM_CALL_TAGS: &[&str] = &[
    ACCOUNT_UNRESOLVABLE,
    ALT_NOT_ALLOWED,
    ATA_DERIVATION,
    COUNTERPARTY_NOT_REGISTERED,
    INCOMING_DESTINATION,
    LIMIT_DAILY,
    LIMIT_NOT_CONFIGURED,
    LIMIT_PER_TRANSACTION,
    MALFORMED,
    MINT_NOT_REGISTERED,
    MINT_PROGRAM_UNKNOWN,
    OUTGOING_AUTHORITY,
    OUTGOING_SOURCE,
    PROGRAM_NOT_ALLOWED,
    PROGRAM_UNRESOLVABLE,
    SHAPE,
];

// ── CONTRACT_CALL (EVM executeSwap verifier, paratro-common/xchange/execute_swap.go) ──

pub const ABI: &str = "abi";
pub const AMOUNT_NOT_POSITIVE: &str = "amount_not_positive";
pub const CALLDATA: &str = "calldata";
pub const CALLDATA_NOT_CANONICAL: &str = "calldata_not_canonical";
pub const CONTRACT_ADDRESS: &str = "contract_address";
pub const EXPIRATION: &str = "expiration";
pub const EXPIRATION_PASSED: &str = "expiration_passed";
pub const EXPIRATION_TOO_FAR: &str = "expiration_too_far";
pub const INCOMING_FROM: &str = "incoming_from";
pub const OUTGOING_TO: &str = "outgoing_to";
pub const PAYMENT_TOKEN_NOT_REGISTERED: &str = "payment_token_not_registered";
pub const PERMIT_DEADLINE: &str = "permit_deadline";
pub const PERMIT_DEADLINE_PASSED: &str = "permit_deadline_passed";
pub const PERMIT_DEADLINE_TOO_FAR: &str = "permit_deadline_too_far";
pub const PERMIT_OWNER: &str = "permit_owner";
pub const SELECTOR: &str = "selector";
pub const TARGET_TOKEN_NOT_REGISTERED: &str = "target_token_not_registered";
pub const VALUE_NOT_ZERO: &str = "value_not_zero";

/// Tags the EVM `CONTRACT_CALL` verifier can put in `"Rejected: <tag>: ..."`.
pub const CONTRACT_CALL_TAGS: &[&str] = &[
    ABI,
    AMOUNT_NOT_POSITIVE,
    CALLDATA,
    CALLDATA_NOT_CANONICAL,
    CONTRACT_ADDRESS,
    COUNTERPARTY_NOT_REGISTERED,
    EXPIRATION,
    EXPIRATION_PASSED,
    EXPIRATION_TOO_FAR,
    INCOMING_FROM,
    LIMIT_NOT_CONFIGURED,
    LIMIT_PER_TRANSACTION,
    OUTGOING_TO,
    PAYMENT_TOKEN_NOT_REGISTERED,
    PERMIT_DEADLINE,
    PERMIT_DEADLINE_PASSED,
    PERMIT_DEADLINE_TOO_FAR,
    PERMIT_OWNER,
    SELECTOR,
    TARGET_TOKEN_NOT_REGISTERED,
    VALUE_NOT_ZERO,
];

// ── Gateway pre-checks (paratro-mpc-gateway internal/service/*.go) ──

pub const COUNTERPARTY_SIGNATURE_INVALID: &str = "counterparty_signature_invalid";
pub const COUNTERPARTY_SIGNATURE_MISSING: &str = "counterparty_signature_missing";
pub const FEE_PAYER: &str = "fee_payer";
pub const LIMIT_DECIMALS_AMBIGUOUS: &str = "limit_decimals_ambiguous";
pub const PAYER_SIGNATURE_PRESENT: &str = "payer_signature_present";
pub const POLICY_INVALID: &str = "policy_invalid";

/// Tags the gateway itself can put in `"Rejected: <tag>: ..."` before calling the engine
/// (daily allowance, Solana signature slots, policy parsing).
pub const GATEWAY_TAGS: &[&str] = &[
    COUNTERPARTY_SIGNATURE_INVALID,
    COUNTERPARTY_SIGNATURE_MISSING,
    FEE_PAYER,
    LIMIT_DAILY,
    LIMIT_DECIMALS_AMBIGUOUS,
    LIMIT_NOT_CONFIGURED,
    MALFORMED,
    PAYER_SIGNATURE_PRESENT,
    POLICY_INVALID,
    PROGRAM_NOT_ALLOWED,
    SHAPE,
];

// ── Engine failures (paratro-mpc-engine internal/syncsettle + internal/syncsign),
//    surfaced as 400 transaction_failed "<OPERATION> failed: <tag>" ──

pub const CALLDATA_INVALID: &str = "calldata_invalid";
pub const CONTRACT_ADDRESS_INVALID: &str = "contract_address_invalid";
pub const CONTRACT_NOT_REGISTERED: &str = "contract_not_registered";
pub const COSIGNATURE_INVALID: &str = "cosignature_invalid";
pub const DAILY_ALLOWANCE_MISSING: &str = "daily_allowance_missing";
pub const DAILY_USAGE_UNAVAILABLE: &str = "daily_usage_unavailable";
pub const INTERNAL: &str = "internal";
pub const MINT_TOKEN_PROGRAM_UNRESOLVED: &str = "mint_token_program_unresolved";
pub const PAYER_INVALID: &str = "payer_invalid";
pub const PAYER_MISMATCH: &str = "payer_mismatch";
// syncsign/permit.go: the engine re-derives the EIP-2612 permit from the row and
// the policy before signing it, and refuses when its view differs from the
// gateway's request.
pub const PERMIT_DIGEST_MISMATCH: &str = "permit_digest_mismatch";
pub const PERMIT_DIGEST_MISSING: &str = "permit_digest_missing";
pub const PERMIT_DOMAIN_MISMATCH: &str = "permit_domain_mismatch";
pub const PERMIT_DOMAIN_UNVERIFIED: &str = "permit_domain_unverified";
pub const PERMIT_OWNER_MISMATCH: &str = "permit_owner_mismatch";
pub const PERMIT_PARAMS_INVALID: &str = "permit_params_invalid";
pub const PERMIT_SPENDER_MISMATCH: &str = "permit_spender_mismatch";
pub const PERMIT_TOKEN_MISMATCH: &str = "permit_token_mismatch";
pub const PERMIT_TOKEN_NOT_REGISTERED: &str = "permit_token_not_registered";
pub const PERMIT_VALUE_MISMATCH: &str = "permit_value_mismatch";
pub const POLICY_NOT_AUTHORIZED: &str = "policy_not_authorized";
pub const RECEIVER_INVALID: &str = "receiver_invalid";
pub const RECEIVER_LOOKUP_FAILED: &str = "receiver_lookup_failed";
pub const RECEIVER_MISSING: &str = "receiver_missing";
pub const RECEIVER_NOT_OURS: &str = "receiver_not_ours";
pub const REQUEST_DIGEST_MISMATCH: &str = "request_digest_mismatch";
pub const REQUEST_DIGEST_MISSING: &str = "request_digest_missing";
pub const SIGNER_SLOT: &str = "signer_slot";

/// Tags the signing engine itself emits in `"<OPERATION> failed: <tag>"` (HTTP 400,
/// code `transaction_failed`): every `reject("…")` literal in
/// `mpc-engine/internal/syncsettle` plus every `rejectPermit("…")` literal in
/// `mpc-engine/internal/syncsign/permit.go`. The engine also re-runs the
/// `paratro-common` verifiers, so any `PROGRAM_CALL` / `CONTRACT_CALL` tag above can
/// appear here as well. Same 38 values as `RejectionReason.ENGINE_FAILURE_TAGS` in the
/// Python SDK and the engine-failure `Reason*` block in the Go SDK.
pub const ENGINE_FAILURE_TAGS: &[&str] = &[
    ALT_NOT_ALLOWED,
    AMOUNT_NOT_POSITIVE,
    CALLDATA_INVALID,
    CONTRACT_ADDRESS_INVALID,
    CONTRACT_NOT_REGISTERED,
    COSIGNATURE_INVALID,
    DAILY_ALLOWANCE_MISSING,
    DAILY_USAGE_UNAVAILABLE,
    INTERNAL,
    LIMIT_DAILY,
    LIMIT_NOT_CONFIGURED,
    LIMIT_PER_TRANSACTION,
    MALFORMED,
    MINT_TOKEN_PROGRAM_UNRESOLVED,
    PAYER_INVALID,
    PAYER_MISMATCH,
    PERMIT_DEADLINE_PASSED,
    PERMIT_DEADLINE_TOO_FAR,
    PERMIT_DIGEST_MISMATCH,
    PERMIT_DIGEST_MISSING,
    PERMIT_DOMAIN_MISMATCH,
    PERMIT_DOMAIN_UNVERIFIED,
    PERMIT_OWNER_MISMATCH,
    PERMIT_PARAMS_INVALID,
    PERMIT_SPENDER_MISMATCH,
    PERMIT_TOKEN_MISMATCH,
    PERMIT_TOKEN_NOT_REGISTERED,
    PERMIT_VALUE_MISMATCH,
    POLICY_INVALID,
    POLICY_NOT_AUTHORIZED,
    PROGRAM_NOT_ALLOWED,
    RECEIVER_INVALID,
    RECEIVER_LOOKUP_FAILED,
    RECEIVER_MISSING,
    RECEIVER_NOT_OURS,
    REQUEST_DIGEST_MISMATCH,
    REQUEST_DIGEST_MISSING,
    SIGNER_SLOT,
];

/// Reports whether `tag` is one of the tags this SDK release knows about.
pub fn is_known(tag: &str) -> bool {
    PROGRAM_CALL_TAGS
        .iter()
        .chain(CONTRACT_CALL_TAGS)
        .chain(GATEWAY_TAGS)
        .chain(ENGINE_FAILURE_TAGS)
        .any(|t| *t == tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tags() {
        assert!(is_known(EXPIRATION_PASSED));
        assert!(is_known(FEE_PAYER));
        assert!(is_known(RECEIVER_NOT_OURS));
        assert!(is_known(PERMIT_OWNER_MISMATCH));
        assert!(!is_known("something_new"));
    }

    #[test]
    fn engine_failure_tags_pin_the_engine_literals() {
        // 24 reject("…") literals in syncsettle + 14 more rejectPermit("…")
        // literals in syncsign/permit.go (10 permit_* plus four verifier tags
        // the permit path emits by name). Same count in the Go and Python SDKs.
        assert_eq!(ENGINE_FAILURE_TAGS.len(), 38);
        for permit_tag in [
            PERMIT_DIGEST_MISMATCH,
            PERMIT_DIGEST_MISSING,
            PERMIT_DOMAIN_MISMATCH,
            PERMIT_DOMAIN_UNVERIFIED,
            PERMIT_OWNER_MISMATCH,
            PERMIT_PARAMS_INVALID,
            PERMIT_SPENDER_MISMATCH,
            PERMIT_TOKEN_MISMATCH,
            PERMIT_TOKEN_NOT_REGISTERED,
            PERMIT_VALUE_MISMATCH,
        ] {
            assert!(ENGINE_FAILURE_TAGS.contains(&permit_tag), "{permit_tag}");
        }
        // Every tag has the shape the gateway forwards (publicEngineReason).
        for tag in PROGRAM_CALL_TAGS
            .iter()
            .chain(CONTRACT_CALL_TAGS)
            .chain(GATEWAY_TAGS)
            .chain(ENGINE_FAILURE_TAGS)
        {
            assert!(
                tag.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    && !tag.starts_with('_')
                    && !tag.ends_with('_')
                    && !tag.contains("__")
                    && tag.len() <= 64,
                "{tag}"
            );
        }
    }

    #[test]
    fn no_duplicates_within_a_list() {
        for list in [
            PROGRAM_CALL_TAGS,
            CONTRACT_CALL_TAGS,
            GATEWAY_TAGS,
            ENGINE_FAILURE_TAGS,
        ] {
            let mut sorted = list.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), list.len());
        }
    }
}
