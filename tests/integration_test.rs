use std::env;
use std::sync::OnceLock;

use paratro_sdk::*;

static ACTIVE_WALLET_ID: OnceLock<String> = OnceLock::new();

fn get_test_client() -> Option<MpcClient> {
    let _ = dotenvy::dotenv();

    let api_key = env::var("MPC_API_KEY").ok()?;
    let api_secret = env::var("MPC_API_SECRET").ok()?;

    if api_key.is_empty() || api_secret.is_empty() {
        return None;
    }

    MpcClient::new(api_key, api_secret, Config::sandbox()).ok()
}

fn skip_integration() -> bool {
    env::var("SKIP_INTEGRATION_TESTS")
        .map(|v| v == "true")
        .unwrap_or(false)
}

fn active_wallet_id() -> Option<&'static str> {
    let _ = dotenvy::dotenv();
    ACTIVE_WALLET_ID
        .get_or_init(|| env::var("MPC_TEST_WALLET_ID").unwrap_or_default())
        .is_empty()
        .then_some("") // returns Some("") if empty — we actually want None
        .is_none()
        .then(|| ACTIVE_WALLET_ID.get().unwrap().as_str())
}

// ============ Wallet Tests ============

#[tokio::test]
async fn test_wallet_create() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    let wallet = client
        .create_wallet(&CreateWalletRequest {
            wallet_name: "SDK Test Wallet".to_string(),
            description: Some("Integration test wallet".to_string()),
        })
        .await
        .expect("failed to create wallet");

    assert!(!wallet.wallet_id.is_empty(), "expected wallet ID to be set");
    println!(
        "Created wallet: {} (status: {})",
        wallet.wallet_id, wallet.status
    );
}

#[tokio::test]
async fn test_wallet_get() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };
    let wallet_id = match active_wallet_id() {
        Some(id) => id,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let wallet = client
        .get_wallet(wallet_id)
        .await
        .expect("failed to get wallet");

    assert_eq!(wallet.wallet_id, wallet_id);
    println!(
        "Retrieved wallet: {} (status: {})",
        wallet.wallet_id, wallet.status
    );
}

#[tokio::test]
async fn test_wallet_list() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client
        .list_wallets(&ListWalletsRequest {
            page: Some(1),
            page_size: Some(20),
        })
        .await
        .expect("failed to list wallets");

    println!(
        "Found {} wallets (Total: {}, HasMore: {})",
        resp.items.len(),
        resp.total,
        resp.has_more
    );
}

// ============ Account Tests ============

#[tokio::test]
async fn test_account_create() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };
    let wallet_id = match active_wallet_id() {
        Some(id) => id,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let account = client
        .create_account(&CreateAccountRequest {
            wallet_id: wallet_id.to_string(),
            chain: "ethereum".to_string(),
            account_type: None,
            label: Some("SDK Test Account".to_string()),
        })
        .await
        .expect("failed to create account");

    assert!(
        !account.account_id.is_empty(),
        "expected account ID to be set"
    );
    println!(
        "Created account: {} (Address: {})",
        account.account_id, account.address
    );
}

#[tokio::test]
async fn test_account_list() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client
        .list_accounts(&ListAccountsRequest {
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .await
        .expect("failed to list accounts");

    println!(
        "Found {} accounts (Total: {}, HasMore: {})",
        resp.items.len(),
        resp.total,
        resp.has_more
    );
}

// ============ Asset Tests ============

#[tokio::test]
async fn test_asset_list() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client
        .list_assets(&ListAssetsRequest {
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .await
        .expect("failed to list assets");

    println!(
        "Found {} assets (Total: {}, HasMore: {})",
        resp.items.len(),
        resp.total,
        resp.has_more
    );
    for (i, asset) in resp.items.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, asset.symbol, asset.name);
    }
}

// ============ Transaction Tests ============

#[tokio::test]
async fn test_transaction_get_not_found() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let result = client.get_transaction("non-existent-tx-id").await;
    assert!(
        result.is_err(),
        "expected error for non-existent transaction"
    );

    println!(
        "Transaction Get API tested (expected error: {})",
        result.unwrap_err()
    );
}

#[tokio::test]
async fn test_transaction_list() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client
        .list_transactions(&ListTransactionsRequest {
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .await
        .expect("failed to list transactions");

    println!(
        "Found {} transactions (Total: {}, HasMore: {})",
        resp.items.len(),
        resp.total,
        resp.has_more
    );
    for (i, tx) in resp.items.iter().take(5).enumerate() {
        println!(
            "  {}. {} {} ({})",
            i + 1,
            tx.amount,
            tx.token_symbol,
            tx.status
        );
    }
}

#[tokio::test]
async fn test_transaction_list_by_wallet() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };
    let wallet_id = match active_wallet_id() {
        Some(id) => id,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let resp = client
        .list_transactions(&ListTransactionsRequest {
            wallet_id: Some(wallet_id.to_string()),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .await
        .expect("failed to list transactions");

    println!(
        "Found {} transactions for wallet {}",
        resp.items.len(),
        wallet_id
    );
}

// ============ Transfer Tests ============

#[tokio::test]
async fn test_create_transfer_invalid() {
    if skip_integration() {
        return;
    }
    let client = match get_test_client() {
        Some(c) => c,
        None => return,
    };

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let result = client
        .create_transfer(&CreateTransferRequest {
            from_address: "0xinvalid".to_string(),
            to_address: "0xinvalid".to_string(),
            chain: "ethereum".to_string(),
            token_symbol: "USDT".to_string(),
            amount: "1.0".to_string(),
            memo: None,
        })
        .await;

    assert!(
        result.is_err(),
        "expected error for invalid transfer request"
    );
    println!(
        "Transfer API tested (expected error: {})",
        result.unwrap_err()
    );
}

// ============ Version Tests ============

#[test]
fn test_version() {
    assert!(!VERSION.is_empty(), "expected version to be set");
    println!("SDK Version: {}", VERSION);
}

// ============ Client Validation Tests ============

#[test]
fn test_new_mpc_client_validation() {
    let result = MpcClient::new("", "secret", Config::sandbox());
    assert!(result.is_err(), "expected error for empty apiKey");

    let result = MpcClient::new("key", "", Config::sandbox());
    assert!(result.is_err(), "expected error for empty apiSecret");
}

// ============ Error Helper Tests ============

#[test]
fn test_error_helpers() {
    let not_found = Error::Api {
        status: 404,
        body: ErrorBody {
            code: "not_found".to_string(),
            error_type: "not_found".to_string(),
            message: "Resource not found".to_string(),
        },
    };
    assert!(is_not_found(&not_found));
    assert!(!is_rate_limited(&not_found));
    assert!(!is_auth_error(&not_found));

    let rate_limited = Error::Api {
        status: 429,
        body: ErrorBody {
            code: "too_many_requests".to_string(),
            error_type: "rate_limit".to_string(),
            message: "Rate limited".to_string(),
        },
    };
    assert!(is_rate_limited(&rate_limited));
    assert!(!is_not_found(&rate_limited));

    let auth_err = Error::Api {
        status: 401,
        body: ErrorBody {
            code: "unauthorized".to_string(),
            error_type: "auth".to_string(),
            message: "Unauthorized".to_string(),
        },
    };
    assert!(is_auth_error(&auth_err));

    let forbidden = Error::Api {
        status: 403,
        body: ErrorBody {
            code: "forbidden".to_string(),
            error_type: "auth".to_string(),
            message: "Forbidden".to_string(),
        },
    };
    assert!(is_auth_error(&forbidden));
}
