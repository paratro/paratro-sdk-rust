//! Wire-level contract tests against a scripted loopback server (no real gateway).
//!
//! Pins: request path / method / headers / exact JSON bytes for the three
//! operations, 200 vs 202 handling, error-body mapping (400 Rejected tags,
//! duplicate reference_id, 403 / 404 / 410, the three 503 messages split into
//! EngineBusy / ChainRpcUnavailable / ServiceUnavailable), the legacy transfer wrapper,
//! the auth header names and the single retry on `401 token_expired`, the optional
//! `Idempotency-Key` header, and the x402 facilitator paths and camelCase reply shapes.

mod common;

use common::{error_body, FakeGateway, AUTH_PATH};
use paratro_sdk::*;
use serde_json::json;

const FROM: &str = "0x96586e99CE724F45bAb65cf963533b810147c1F4";
const COUNTERPARTY: &str = "0xcf8a9d1e489c58f4c3d69b45380fb4a6c03ada47";

fn contract_call_request() -> CreateTransactionRequest {
    CreateTransactionRequest::contract_call(
        ContractCallRequest::new(
            FROM,
            "ethereum",
            ContractCall::new(
                "0x1111111111111111111111111111111111111111111111111111111111111111",
                1789449058,
                ContractCallIncomingLeg {
                    to: COUNTERPARTY.into(),
                    token: "0x59fb67f6778cff089484cf7115906725dfc44293".into(),
                    amount: "10000000".into(),
                },
                ContractCallOutgoingLeg {
                    from: COUNTERPARTY.into(),
                    to: FROM.into(),
                    token: "0xa55a927f2211fe52188526ed7e779b7298646e75".into(),
                    amount: "42000000000000000".into(),
                },
                "0xsig",
            ),
        )
        .with_receive_address(FROM)
        .with_reference_id("quote-1")
        .with_memo("optional"),
    )
}

// ───────────────────────── request wire format ─────────────────────────

#[tokio::test]
async fn transfer_wire_format_and_200_pending() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"tx_id": "tx-1", "status": "PENDING", "message": "Transfer task created"}),
    )])
    .await;

    let req = CreateTransactionRequest::transfer(
        TransferRequest::new(FROM, "0xbbbb", "ethereum", "USDT", "10.5")
            .with_memo("invoice 42")
            .with_reference_id("order-42"),
    );
    let resp = gw.client.create_transaction(&req).await.unwrap();

    let call = gw.only_api_call();
    assert_eq!(call.method, "POST");
    assert_eq!(call.path, "/api/v1/transactions");
    assert_eq!(call.header("authorization"), Some("Bearer token-1"));
    assert_eq!(call.header("content-type"), Some("application/json"));
    assert_eq!(
        call.body_str(),
        format!(
            r#"{{"operation":"TRANSFER","from_address":"{FROM}","to_address":"0xbbbb","chain":"ethereum","token_symbol":"USDT","amount":"10.5","memo":"invoice 42","reference_id":"order-42"}}"#
        )
    );

    assert_eq!(resp.http_status, 200);
    assert!(!resp.is_accepted());
    assert_eq!(resp.tx_id, "tx-1");
    assert_eq!(resp.status, STATUS_PENDING);
    assert_eq!(resp.tx_hash, None);
}

#[tokio::test]
async fn program_call_wire_format_and_200_broadcast() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"tx_id": "tx-2", "status": "BROADCAST", "message": "PROGRAM_CALL broadcast", "tx_hash": "5sig"}),
    )])
    .await;

    let req = CreateTransactionRequest::program_call(
        ProgramCallRequest::new("PayerPubkey", "solana", "AQIDBA==")
            .with_receive_address("ReceiverPubkey")
            .with_reference_id("quote-7"),
    );
    let resp = gw.client.create_transaction(&req).await.unwrap();

    let call = gw.only_api_call();
    assert_eq!(
        (call.method.as_str(), call.path.as_str()),
        ("POST", "/api/v1/transactions")
    );
    assert_eq!(
        call.body_str(),
        r#"{"operation":"PROGRAM_CALL","from_address":"PayerPubkey","chain":"solana","signed_transaction":"AQIDBA==","receive_address":"ReceiverPubkey","reference_id":"quote-7"}"#
    );

    assert_eq!(resp.http_status, 200);
    assert!(resp.is_broadcast());
    assert_eq!(resp.tx_hash.as_deref(), Some("5sig"));
}

#[tokio::test]
async fn program_call_omits_receive_address_when_unset() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"tx_id": "tx-3", "status": "BROADCAST", "message": "PROGRAM_CALL broadcast", "tx_hash": "sig"}),
    )])
    .await;
    let req: CreateTransactionRequest = ProgramCallRequest::new("Payer", "solana", "0x0102").into();
    gw.client.create_transaction(&req).await.unwrap();
    assert_eq!(
        gw.only_api_call().body_str(),
        r#"{"operation":"PROGRAM_CALL","from_address":"Payer","chain":"solana","signed_transaction":"0x0102"}"#
    );
}

#[tokio::test]
async fn contract_call_wire_format_and_202_accepted() {
    let gw = FakeGateway::start(vec![(
        202,
        json!({
            "tx_id": "tx-4",
            "status": "PENDING",
            "message": "CONTRACT_CALL accepted but the signing engine did not answer in time; poll GET /api/v1/transactions/tx-4 for the outcome and do not resubmit with the same reference_id"
        }),
    )])
    .await;

    let resp = gw
        .client
        .create_transaction(&contract_call_request())
        .await
        .unwrap();

    let call = gw.only_api_call();
    assert_eq!(
        (call.method.as_str(), call.path.as_str()),
        ("POST", "/api/v1/transactions")
    );
    assert_eq!(
        call.body_str(),
        format!(
            concat!(
                r#"{{"operation":"CONTRACT_CALL","from_address":"{from}","chain":"ethereum","receive_address":"{from}","#,
                r#""contract_call":{{"quote_id":"0x1111111111111111111111111111111111111111111111111111111111111111","expiration":1789449058,"#,
                r#""incoming":{{"to":"{cp}","token":"0x59fb67f6778cff089484cf7115906725dfc44293","amount":"10000000"}},"#,
                r#""outgoing":{{"from":"{cp}","to":"{from}","token":"0xa55a927f2211fe52188526ed7e779b7298646e75","amount":"42000000000000000"}},"#,
                r#""counterparty_signature":"0xsig"}},"reference_id":"quote-1","memo":"optional"}}"#
            ),
            from = FROM,
            cp = COUNTERPARTY
        )
    );
    // Contract address and native value are never sent: the policy decides them.
    let body = call.json();
    assert!(body["contract_call"].get("contract_address").is_none());
    assert!(body["contract_call"].get("value").is_none());
    assert!(body["contract_call"].get("permit_deadline").is_none());

    assert_eq!(resp.http_status, 202);
    assert!(
        resp.is_accepted(),
        "202 must be reported as accepted / outcome unknown"
    );
    assert_eq!(resp.status, STATUS_PENDING);
    assert_eq!(resp.tx_id, "tx-4");
    assert_eq!(resp.tx_hash, None);
}

#[tokio::test]
async fn idempotency_replay_of_a_202_is_still_accepted() {
    // middleware/idempotency.go replays a cached 2xx body with c.Data(200, …):
    // the original 202 PENDING comes back as HTTP 200 with the same body. The
    // response is still "outcome unknown" for a PROGRAM_CALL / CONTRACT_CALL.
    let gw = FakeGateway::start(vec![(
        200,
        json!({
            "tx_id": "tx-4",
            "status": "PENDING",
            "message": "CONTRACT_CALL accepted but the signing engine did not answer in time; poll GET /api/v1/transactions/tx-4 for the outcome and do not resubmit with the same reference_id"
        }),
    )])
    .await;

    let resp = gw
        .client
        .create_transaction(&contract_call_request())
        .await
        .unwrap();

    assert_eq!(resp.http_status, 200);
    assert_eq!(resp.operation, OPERATION_CONTRACT_CALL);
    assert!(
        resp.is_accepted(),
        "200 PENDING for a CONTRACT_CALL is a replayed 202: still outcome unknown"
    );
    assert_eq!(resp.tx_hash, None);
}

#[tokio::test]
async fn contract_call_sends_permit_deadline_when_set() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"tx_id": "tx-5", "status": "BROADCAST", "message": "CONTRACT_CALL broadcast", "tx_hash": "0xabc"}),
    )])
    .await;
    let mut req = contract_call_request();
    if let CreateTransactionRequest::ContractCall(ref mut cc) = req {
        cc.contract_call.permit_deadline = Some(1789449358);
    }
    gw.client.create_transaction(&req).await.unwrap();
    assert_eq!(
        gw.only_api_call().json()["contract_call"]["permit_deadline"],
        json!(1789449358)
    );
}

#[tokio::test]
async fn legacy_create_transfer_goes_through_unified_entry() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"tx_id": "tx-6", "status": "PENDING", "message": "Transfer task created"}),
    )])
    .await;

    let resp = gw
        .client
        .create_transfer(&CreateTransferRequest {
            from_address: FROM.into(),
            to_address: "0xbbbb".into(),
            chain: "ethereum".into(),
            token_symbol: "USDT".into(),
            amount: "1".into(),
            memo: None,
            reference_id: Some("legacy-1".into()),
        })
        .await
        .unwrap();

    let call = gw.only_api_call();
    assert_eq!(
        call.path, "/api/v1/transactions",
        "must not call the retired /transfer"
    );
    assert_eq!(call.json()["operation"], json!("TRANSFER"));
    assert_eq!(call.json()["reference_id"], json!("legacy-1"));
    assert_eq!(resp.tx_id, "tx-6");
    assert_eq!(resp.http_status, 200);
}

// ───────────────────────── error mapping ─────────────────────────

#[tokio::test]
async fn rejected_400_exposes_reason_tag() {
    let gw = FakeGateway::start(vec![(
        400,
        error_body(
            "invalid_parameter",
            "invalid_request_error",
            "Rejected: expiration_passed: quote expired at 1789449058 (now 1789449100)",
        ),
    )])
    .await;
    let err = gw
        .client
        .create_transaction(&contract_call_request())
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(400));
    assert_eq!(err.kind(), Some(ApiErrorKind::Rejected));
    assert!(err.is_rejected());
    assert_eq!(err.reason_tag(), Some(reason_tag::EXPIRATION_PASSED));
    assert!(reason_tag::is_known(err.reason_tag().unwrap()));
}

#[tokio::test]
async fn transaction_failed_400_exposes_engine_tag() {
    let gw = FakeGateway::start(vec![
        (
            400,
            error_body(
                "transaction_failed",
                "business_error",
                "CONTRACT_CALL failed: receiver_not_ours",
            ),
        ),
        (
            400,
            error_body(
                "transaction_failed",
                "business_error",
                "PROGRAM_CALL failed: engine rejected the transaction",
            ),
        ),
    ])
    .await;
    let err = gw
        .client
        .create_transaction(&contract_call_request())
        .await
        .unwrap_err();
    assert_eq!(err.kind(), Some(ApiErrorKind::TransactionFailed));
    assert_eq!(err.reason_tag(), Some(reason_tag::RECEIVER_NOT_OURS));

    let err = gw
        .client
        .create_transaction(&contract_call_request())
        .await
        .unwrap_err();
    assert!(err.is_transaction_failed());
    assert_eq!(err.reason_tag(), None);
}

#[tokio::test]
async fn duplicate_reference_id_and_unsupported_operation() {
    let gw = FakeGateway::start(vec![
        (
            400,
            error_body(
                "invalid_parameter",
                "invalid_request_error",
                "Duplicate reference_id: this reference has already been used",
            ),
        ),
        (
            400,
            error_body(
                "invalid_parameter",
                "invalid_request_error",
                "Unsupported operation: only TRANSFER, PROGRAM_CALL and CONTRACT_CALL are available",
            ),
        ),
        (
            400,
            error_body("insufficient_balance", "business_error", "Insufficient balance"),
        ),
    ])
    .await;
    let req = contract_call_request();
    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert!(err.is_duplicate_reference_id());
    assert_eq!(err.reason_tag(), None);

    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert!(err.is_unsupported_operation());

    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert!(err.is_insufficient_balance());
}

#[tokio::test]
async fn retired_endpoint_410_maps_to_endpoint_retired() {
    let gw = FakeGateway::start(vec![(
        410,
        error_body(
            "invalid_parameter",
            "endpoint_retired",
            "This endpoint has been retired. Use POST /api/v1/transactions (operation=TRANSFER).",
        ),
    )])
    .await;
    let err = gw
        .client
        .create_transaction(&contract_call_request())
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(410));
    assert_eq!(err.kind(), Some(ApiErrorKind::EndpointRetired));
    assert!(err.is_endpoint_retired());
    assert_eq!(err.error_type(), Some(error::ERROR_TYPE_ENDPOINT_RETIRED));
    assert_eq!(err.code(), Some(error::code::INVALID_PARAMETER));
}

#[tokio::test]
async fn forbidden_not_found_service_unavailable() {
    let gw = FakeGateway::start(vec![
        (
            403,
            error_body(
                "forbidden",
                "permission_error",
                "no OPERATION_RULES policy authorises CONTRACT_CALL on ethereum",
            ),
        ),
        (
            404,
            error_body(
                "resource_not_found",
                "not_found_error",
                "no asset registered for token 0x59fb…",
            ),
        ),
        (
            503,
            error_body(
                "service_unavailable",
                "api_error",
                "Signing service is busy, retry later",
            ),
        ),
        (
            503,
            error_body(
                "service_unavailable",
                "api_error",
                "Chain RPC unavailable; cannot verify request",
            ),
        ),
        (
            503,
            error_body(
                "service_unavailable",
                "api_error",
                "CONTRACT_CALL is not enabled on this gateway",
            ),
        ),
    ])
    .await;
    let req = contract_call_request();

    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert!(err.is_forbidden());
    assert!(is_auth_error(&err));

    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert!(err.is_not_found());
    assert!(is_not_found(&err));

    // "Signing service is busy" is raised AFTER the row exists (row FAILED,
    // reference_id consumed) — it must not be classified as a plain retryable 503.
    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert_eq!(err.kind(), Some(ApiErrorKind::EngineBusy));
    assert!(err.is_engine_busy());
    assert!(!err.is_chain_rpc_unavailable());
    assert!(err.is_service_unavailable());
    assert_eq!(err.message(), Some(error::ENGINE_BUSY_MESSAGE));

    // "Chain RPC unavailable" is raised BEFORE any row exists — same reference_id
    // may be retried.
    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert_eq!(err.kind(), Some(ApiErrorKind::ChainRpcUnavailable));
    assert!(err.is_chain_rpc_unavailable());
    assert!(!err.is_engine_busy());
    assert!(err.is_service_unavailable());
    assert_eq!(err.message(), Some(error::CHAIN_RPC_UNAVAILABLE_MESSAGE));

    // Any other 503 stays ServiceUnavailable.
    let err = gw.client.create_transaction(&req).await.unwrap_err();
    assert_eq!(err.kind(), Some(ApiErrorKind::ServiceUnavailable));
    assert!(err.is_service_unavailable());
    assert!(!err.is_engine_busy());
    assert!(!err.is_chain_rpc_unavailable());
}

#[tokio::test]
async fn undecodable_error_body_still_surfaces_status() {
    let gw = FakeGateway::start(vec![(502, json!("bad gateway"))]).await;
    let err = gw.client.get_transaction("tx").await.unwrap_err();
    assert_eq!(err.status(), Some(502));
    assert_eq!(err.kind(), Some(ApiErrorKind::ServerError));
    assert_eq!(err.code(), Some("unknown"));
}

// ───────────────────────── auth ─────────────────────────

#[tokio::test]
async fn auth_uses_api_key_and_secret_headers() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"tx_id": "t", "wallet_id": "w", "client_id": "c", "chain": "ethereum",
               "transaction_type": "OUTBOUND", "from_address": "a", "to_address": "b",
               "token_symbol": "USDT", "amount": "1", "status": "CONFIRMED", "tx_hash": "0x",
               "created_at": "2026-09-15T00:00:00Z"}),
    )])
    .await;
    gw.client.get_transaction("t").await.unwrap();

    let auth = gw.auth_calls();
    assert_eq!(auth.len(), 1);
    assert_eq!(auth[0].method, "POST");
    assert_eq!(auth[0].path, AUTH_PATH);
    assert_eq!(auth[0].header(HEADER_API_KEY), Some("fixture-key"));
    assert_eq!(auth[0].header(HEADER_API_SECRET), Some("fixture-secret"));
    assert_eq!(auth[0].header("authorization"), None);
    assert!(auth[0].body.is_empty());

    // The token is cached: a second API call does not re-authenticate.
    let _ = gw.client.get_transaction("t").await; // unmocked → 500, fine
    assert_eq!(gw.auth_calls().len(), 1);
}

#[tokio::test]
async fn token_expired_refreshes_once_and_retries() {
    let gw = FakeGateway::start(vec![
        (
            401,
            error_body("token_expired", "authentication_error", "Token expired"),
        ),
        (
            200,
            json!({"tx_id": "tx-9", "status": "BROADCAST", "message": "PROGRAM_CALL broadcast", "tx_hash": "sig"}),
        ),
    ])
    .await;
    let req: CreateTransactionRequest = ProgramCallRequest::new("Payer", "solana", "AQ==").into();
    let resp = gw.client.create_transaction(&req).await.unwrap();
    assert_eq!(resp.tx_id, "tx-9");

    let api = gw.api_calls();
    assert_eq!(api.len(), 2, "exactly one retry");
    assert_eq!(api[0].header("authorization"), Some("Bearer token-1"));
    assert_eq!(api[1].header("authorization"), Some("Bearer token-2"));
    assert_eq!(
        api[0].body, api[1].body,
        "retry must resend the identical body"
    );
    assert_eq!(gw.auth_calls().len(), 2);
}

#[tokio::test]
async fn token_expired_twice_is_returned_not_looped() {
    let gw = FakeGateway::start(vec![
        (
            401,
            error_body("token_expired", "authentication_error", "Token expired"),
        ),
        (
            401,
            error_body("token_expired", "authentication_error", "Token expired"),
        ),
    ])
    .await;
    let err = gw.client.get_transaction("t").await.unwrap_err();
    assert!(err.is_token_expired());
    assert_eq!(err.kind(), Some(ApiErrorKind::TokenExpired));
    assert_eq!(gw.api_calls().len(), 2);
    assert_eq!(gw.auth_calls().len(), 2);
}

#[tokio::test]
async fn other_401_is_not_retried() {
    let gw = FakeGateway::start(vec![(
        401,
        error_body("invalid_token", "authentication_error", "Invalid token"),
    )])
    .await;
    let err = gw.client.get_transaction("t").await.unwrap_err();
    assert_eq!(err.kind(), Some(ApiErrorKind::Unauthorized));
    assert_eq!(gw.api_calls().len(), 1);
    assert_eq!(gw.auth_calls().len(), 1);
}

// ───────────────────────── reads ─────────────────────────

#[tokio::test]
async fn get_and_list_transactions_paths() {
    let tx = json!({"tx_id": "t", "wallet_id": "w", "client_id": "c", "chain": "ethereum",
                    "transaction_type": "OUTBOUND", "from_address": "a", "to_address": "b",
                    "token_symbol": "USDT", "amount": "1", "status": "CONFIRMED", "tx_hash": "0x",
                    "risk_score": "12.5", "risk_level": "LOW",
                    "created_at": "2026-09-15T00:00:00Z"});
    let gw = FakeGateway::start(vec![
        (200, tx.clone()),
        (200, json!({"data": [tx], "total": 1, "has_more": false})),
    ])
    .await;

    let got = gw.client.get_transaction("t").await.unwrap();
    assert_eq!(got.risk_score.as_deref(), Some("12.5"));

    let list = gw
        .client
        .list_transactions(&ListTransactionsRequest {
            wallet_id: Some("w".into()),
            chain: Some("ethereum".into()),
            page: Some(2),
            page_size: Some(50),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list.total, 1);
    assert_eq!(list.items.len(), 1);

    let api = gw.api_calls();
    assert_eq!(
        (api[0].method.as_str(), api[0].path.as_str()),
        ("GET", "/api/v1/transactions/t")
    );
    assert_eq!(api[1].method, "GET");
    assert_eq!(
        api[1].path,
        "/api/v1/transactions?wallet_id=w&chain=ethereum&page=2&page_size=50"
    );
}

// ───────────────────────── x402 facilitator ─────────────────────────

#[tokio::test]
async fn x402_facilitator_paths_and_camel_case_shapes() {
    let gw = FakeGateway::start(vec![
        (200, json!({"isValid": false, "invalidReason": "invalid_signature", "payer": "0xp"})),
        (200, json!({"success": true, "txId": "st-1", "transaction": "0xhash", "payer": "0xp", "network": "base-sepolia"})),
        (200, json!({"success": true, "txId": "st-1", "status": "SETTLED", "txHash": "0xhash", "network": "base-sepolia"})),
        (200, json!({"data": [{"tx_id": "st-1", "chain": "base", "from_address": "a", "to_address": "b",
                                "amount": "1000000", "status": "SETTLED", "valid_before": 1800000000,
                                "created_at": "2026-09-15T00:00:00Z"}],
                     "total": 1, "has_more": false})),
    ])
    .await;

    let payload = X402FacilitatorRequest {
        x402_version: 2,
        payment_payload: json!({"payload": {"signature": "0x"}, "accepted": {"scheme": "exact"}}),
        payment_requirements: None,
    };
    let v = gw.client.x402_verify(&payload).await.unwrap();
    assert!(!v.is_valid);
    assert_eq!(v.invalid_reason.as_deref(), Some("invalid_signature"));

    let s = gw.client.x402_settle(&payload).await.unwrap();
    assert!(s.success);
    assert_eq!(s.tx_id, "st-1");

    let st = gw.client.x402_settle_status("st-1").await.unwrap();
    assert_eq!(st.status, "SETTLED");
    assert_eq!(st.tx_hash, "0xhash");

    let list = gw
        .client
        .x402_list_settlements(&ListX402SettlementsRequest {
            status: Some("SETTLED".into()),
            page: Some(1),
            page_size: Some(20),
        })
        .await
        .unwrap();
    assert_eq!(list.items[0].tx_id, "st-1");
    assert_eq!(list.items[0].signature_v, None);

    let api = gw.api_calls();
    assert_eq!(
        (api[0].method.as_str(), api[0].path.as_str()),
        ("POST", "/api/v1/x402/verify")
    );
    assert_eq!(
        api[0].json(),
        json!({"x402Version": 2, "paymentPayload": {"payload": {"signature": "0x"}, "accepted": {"scheme": "exact"}}})
    );
    assert_eq!(
        (api[1].method.as_str(), api[1].path.as_str()),
        ("POST", "/api/v1/x402/settle")
    );
    assert_eq!(
        (api[2].method.as_str(), api[2].path.as_str()),
        ("GET", "/api/v1/x402/settle/st-1")
    );
    assert_eq!(api[3].method, "GET");
    assert_eq!(
        api[3].path,
        "/api/v1/x402/settlements?status=SETTLED&page=1&page_size=20"
    );
}

#[tokio::test]
async fn x402_verify_accepts_raw_json_value() {
    let gw = FakeGateway::start(vec![(200, json!({"isValid": true, "payer": "0xp"}))]).await;
    let raw = json!({"x402Version": 1, "paymentPayload": {}, "paymentRequirements": {}});
    let v = gw.client.x402_verify(&raw).await.unwrap();
    assert!(v.is_valid);
    assert_eq!(gw.only_api_call().json(), raw);
}

// ───────────────────────── Idempotency-Key ─────────────────────────

#[tokio::test]
async fn create_transaction_sends_idempotency_key_and_plain_call_does_not() {
    let gw = FakeGateway::start(vec![
        (
            202,
            json!({"tx_id": "tx-k", "status": "PENDING", "message": "CONTRACT_CALL accepted but the signing engine did not answer in time"}),
        ),
        // Gateway replays a cached 2xx body with HTTP 200 (middleware/idempotency.go).
        (
            200,
            json!({"tx_id": "tx-k", "status": "PENDING", "message": "CONTRACT_CALL accepted but the signing engine did not answer in time"}),
        ),
        (
            200,
            json!({"tx_id": "tx-n", "status": "BROADCAST", "message": "CONTRACT_CALL broadcast", "tx_hash": "0xh"}),
        ),
    ])
    .await;
    let req = contract_call_request();

    let first = gw
        .client
        .create_transaction_with_idempotency_key(&req, "idem-1")
        .await
        .unwrap();
    assert_eq!(first.http_status, 202);
    assert!(first.is_accepted());

    let replay = gw
        .client
        .create_transaction_with_idempotency_key(&req, "idem-1")
        .await
        .unwrap();
    assert_eq!(replay.http_status, 200);
    assert_eq!(replay.tx_id, "tx-k");
    assert!(
        replay.is_accepted(),
        "a replayed 202 arrives as 200 PENDING and must still read as accepted"
    );

    let plain = gw.client.create_transaction(&req).await.unwrap();
    assert!(plain.is_broadcast());

    let api = gw.api_calls();
    assert_eq!(api.len(), 3);
    assert_eq!(api[0].header(HEADER_IDEMPOTENCY_KEY), Some("idem-1"));
    assert_eq!(api[1].header(HEADER_IDEMPOTENCY_KEY), Some("idem-1"));
    assert_eq!(
        api[2].header(HEADER_IDEMPOTENCY_KEY),
        None,
        "create_transaction must not invent a key"
    );
    assert_eq!(api[0].body, api[1].body, "replay is byte-identical");
    assert_eq!(api[0].path, "/api/v1/transactions");
    assert_eq!(api[0].header("authorization"), Some("Bearer token-1"));
}

#[tokio::test]
async fn idempotency_key_survives_the_token_expired_retry() {
    let gw = FakeGateway::start(vec![
        (
            401,
            error_body("token_expired", "authentication_error", "Token expired"),
        ),
        (
            200,
            json!({"tx_id": "tx-r", "status": "BROADCAST", "message": "PROGRAM_CALL broadcast", "tx_hash": "sig"}),
        ),
    ])
    .await;
    let req: CreateTransactionRequest = ProgramCallRequest::new("Payer", "solana", "AQ==").into();
    let resp = gw
        .client
        .create_transaction_with_idempotency_key(&req, "idem-2")
        .await
        .unwrap();
    assert_eq!(resp.tx_id, "tx-r");

    let api = gw.api_calls();
    assert_eq!(api.len(), 2);
    assert_eq!(api[0].header(HEADER_IDEMPOTENCY_KEY), Some("idem-2"));
    assert_eq!(api[1].header(HEADER_IDEMPOTENCY_KEY), Some("idem-2"));
    assert_eq!(api[1].header("authorization"), Some("Bearer token-2"));
}

#[tokio::test]
async fn x402_settle_with_idempotency_key_sends_the_header() {
    let gw = FakeGateway::start(vec![(
        200,
        json!({"success": true, "txId": "st-k", "transaction": "0xhash", "payer": "0xpayer", "network": "eip155:11155111"}),
    )])
    .await;
    let payload = json!({"x402Version": 2, "paymentPayload": {"payload": {"signature": "0x"}}});
    let s = gw
        .client
        .x402_settle_with_idempotency_key(&payload, "settle-1")
        .await
        .unwrap();
    assert!(s.success);
    assert_eq!(s.tx_id, "st-k");

    let call = gw.only_api_call();
    assert_eq!(
        (call.method.as_str(), call.path.as_str()),
        ("POST", "/api/v1/x402/settle")
    );
    assert_eq!(call.header(HEADER_IDEMPOTENCY_KEY), Some("settle-1"));
    assert_eq!(call.json(), payload);
}

// ───────────────────────── HTTP timeout ─────────────────────────

#[tokio::test]
async fn configured_timeout_bounds_the_request() {
    // A listener that accepts but never answers: the SDK must give up after
    // Config::timeout, not hang for the 30 s the 1.6 client had hard-coded.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let hold = tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            held.push(stream);
        }
    });

    let client = MpcClient::new(
        "fixture-key",
        "fixture-secret",
        Config::custom(&base_url).with_timeout(std::time::Duration::from_millis(300)),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let err = client
        .get_transaction("tx-timeout")
        .await
        .expect_err("a silent server must time out");
    hold.abort();

    assert!(
        matches!(&err, Error::Http(e) if e.is_timeout()),
        "expected a reqwest timeout, got {err:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "timed out after {:?}, the configured 300 ms was not applied",
        started.elapsed()
    );
}
