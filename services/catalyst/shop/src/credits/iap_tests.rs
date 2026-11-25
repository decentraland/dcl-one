//! The App Store routes over the module as `serve` mounts it, with a chain
//! generated per test (`apple::testkit`) as the configured root. DB-backed;
//! without `CATALYST_TEST_DATABASE_URL` they skip.

use std::collections::HashMap;
use std::sync::Arc;

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::Router;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::apple::testkit::{Chain, ChainSpec, chain};
use super::apple::{Environment, account_token};
use super::iap::{Iap, Limits, Refunder};
use super::tests::{StateCfg, Wallet, app, call, pool, scratch_wallet, send, test_state};
use crate::auth::{AuthLink, AuthLinkType};
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_legacy_payload,
};

const PRODUCT: &str = "org.example.credits100";

fn iap(c: &Chain, limits: Limits) -> Arc<Iap> {
    Arc::new(Iap {
        verifier: c.verifier(Environment::Sandbox),
        catalog: HashMap::from([(PRODUCT.to_string(), 100)]),
        limits,
    })
}

fn open_limits() -> Limits {
    Limits {
        wallet_total_cents: 0,
        wallet_daily_cents: 0,
        daily_cents: i64::MAX / 4,
    }
}

fn setup(pool: &PgPool, c: &Chain, limits: Limits) -> Router {
    app(test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(c, limits)),
            ..StateCfg::default()
        },
    ))
}

fn tx_id() -> String {
    format!("2000{}", uuid::Uuid::new_v4().as_u128() % 1_000_000_000_000)
}

async fn available(pool: &PgPool, addr: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT COALESCE((SELECT available::text FROM credits.wallets WHERE address = $1), '0')",
    )
    .bind(addr)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn notify(app: &Router, signed: String) -> (StatusCode, Value) {
    let (status, _, body) = send(
        app,
        "POST",
        "/credits/apple/webhook",
        HeaderMap::new(),
        Some(json!({ "signedPayload": signed })),
    )
    .await;
    (status, body)
}

async fn register(app: &Router, w: &Wallet) -> String {
    let (status, out) = call(app, w, "post", "/credits/credits/iap/register", &[], None).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    out["data"]["appAccountToken"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn a_signed_purchase_credits_once_and_its_refund_charges_it_back() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let token = register(&app, &w).await;
    assert_eq!(token, account_token(&w.address));
    let id = tx_id();
    let signed_tx = c.transaction(&id, PRODUCT, Some(&token));

    // A payload not signed through the configured root credits nothing.
    let other = chain(ChainSpec::default());
    let (status, _) = notify(
        &app,
        other.notification(
            "ONE_TIME_CHARGE",
            Some(other.transaction(&id, PRODUCT, Some(&token))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = send(
        &app,
        "POST",
        "/credits/apple/webhook",
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(available(&pool, &w.address).await, "0");

    let (status, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed_tx.clone())),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["alreadyExisted"], false);
    assert_eq!(available(&pool, &w.address).await, "100");

    // The device path and a redelivery find it credited.
    let (status, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": signed_tx })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(out["data"]["alreadyExisted"], true, "{out}");
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed_tx.clone())),
    )
    .await;
    assert_eq!(out["alreadyExisted"], true);
    assert_eq!(available(&pool, &w.address).await, "100");

    let (status, out) = notify(&app, c.notification("REFUND", Some(signed_tx.clone()))).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["action"], "refunded");
    assert_eq!(available(&pool, &w.address).await, "0");
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed_tx.clone()))).await;
    assert_eq!(out["action"], "already_refunded");

    // The original signed transaction is never credited again.
    let (_, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": signed_tx })),
    )
    .await;
    assert_eq!(out["code"], "refunded_transaction", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");

    let path = format!("/credits/users/{}/credit-orders", w.address);
    let (status, out) = call(&app, &w, "get", &path, &[], None).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["total"], 1);
    let order = &out["orders"][0];
    assert_eq!(order["provider"], "apple");
    assert_eq!(order["id"], format!("apple:{id}"));
    assert_eq!(order["status"], "reversed");
    assert_eq!(order["credits"], 100);
    assert_eq!(order["usdCents"], 1000);
    assert_eq!(order["packId"], PRODUCT);
}

#[tokio::test]
async fn a_refund_before_the_purchase_keeps_it_from_being_credited() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let token = register(&app, &w).await;
    let signed_tx = c.transaction(&tx_id(), PRODUCT, Some(&token));

    let (_, out) = notify(&app, c.notification("REFUND", Some(signed_tx.clone()))).await;
    assert_eq!(out["action"], "unknown_credit");
    let (status, out) = notify(&app, c.notification("ONE_TIME_CHARGE", Some(signed_tx))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(out["error"], "refunded_transaction", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn the_device_path_credits_only_a_transaction_bought_for_the_signer() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let buyer = scratch_wallet();
    let other = scratch_wallet();
    let signed_tx = c.transaction(&tx_id(), PRODUCT, Some(&account_token(&buyer.address)));

    let (status, out) = call(
        &app,
        &other,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": signed_tx })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(out["code"], "token_mismatch", "{out}");
    assert_eq!(available(&pool, &other.address).await, "0");

    let mut revoked = super::apple::testkit::transaction_payload(
        &tx_id(),
        PRODUCT,
        Some(&account_token(&buyer.address)),
    );
    revoked["revocationDate"] = json!(1_700_000_000_000i64);
    let (_, out) = call(
        &app,
        &buyer,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": c.sign(&revoked) })),
    )
    .await;
    assert_eq!(out["code"], "revoked_transaction", "{out}");

    let (_, out) = call(
        &app,
        &buyer,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": signed_tx })),
    )
    .await;
    assert_eq!(out["data"]["credited"], true, "{out}");
    assert_eq!(available(&pool, &buyer.address).await, "100");
}

#[tokio::test]
async fn a_refund_the_balance_cannot_cover_queues_signed_credits_up_to_the_uncovered_part() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let token = register(&app, &w).await;
    let signed_tx = c.transaction(&tx_id(), PRODUCT, Some(&token));
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed_tx.clone())),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");

    // 80 of the 100 Credits pledged to a signed, unexpired credit.
    let auth_id = format!("auth-{}", uuid::Uuid::new_v4().simple());
    let hash = format!(
        "0x{}",
        hex::encode(alloy_primitives::keccak256(auth_id.as_bytes()))
    );
    sqlx::query(
        "INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at, chain_id, contract, credit_hash) \
         VALUES ($1, $2, 800, 1, 'authorized', timezone('UTC', now()) + interval '5 minutes', 137, $3, $4)",
    )
    .bind(&auth_id)
    .bind(&w.address)
    .bind("0x00000000000000000000000000000000000000c2")
    .bind(&hash)
    .execute(&pool)
    .await
    .unwrap();

    // A second purchase: 200 available, 110 pledged, so a 100 refund leaves 10 uncovered,
    // which the newest signed credit covers; the older one stays live.
    let (_, out) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&tx_id(), PRODUCT, Some(&token))),
        ),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");
    let older_id = format!("auth-{}", uuid::Uuid::new_v4().simple());
    let older_hash = format!(
        "0x{}",
        hex::encode(alloy_primitives::keccak256(older_id.as_bytes()))
    );
    sqlx::query(
        "INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at, chain_id, contract, credit_hash, created_at) \
         VALUES ($1, $2, 300, 1, 'authorized', timezone('UTC', now()) + interval '5 minutes', 137, $3, $4, timezone('UTC', now()) - interval '1 minute')",
    )
    .bind(&older_id)
    .bind(&w.address)
    .bind("0x00000000000000000000000000000000000000c2")
    .bind(&older_hash)
    .execute(&pool)
    .await
    .unwrap();

    let (_, out) = notify(&app, c.notification("REFUND", Some(signed_tx))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(out["revocations"], 1);
    let older_queued: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM credits.revocations WHERE credit_hash = $1)",
    )
    .bind(&older_hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!older_queued, "only the uncovered part is revoked");
    assert_eq!(available(&pool, &w.address).await, "100");
    let (reason, status): (String, String) =
        sqlx::query_as("SELECT reason, status FROM credits.revocations WHERE credit_hash = $1")
            .bind(&hash)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(reason.starts_with("reversed:apple-iap:"), "{reason}");
    assert_eq!(status, "pending");
    let blocked: bool = sqlx::query_scalar(
        "SELECT is_blocked_for_claiming FROM credits.wallets WHERE address = $1",
    )
    .bind(&w.address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(blocked);
}

#[tokio::test]
async fn the_caps_hold_on_quote_and_on_credit() {
    let Some(own) = super::tests::OwnDatabase::create("iap_caps").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let app = setup(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 0,
            wallet_daily_cents: 1500,
            daily_cents: 100_000,
        },
    );
    let w = scratch_wallet();
    let (status, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/quote",
        &[],
        Some(json!({ "productId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["ok"], true);
    assert_eq!(out["data"]["credits"], 100);
    assert_eq!(out["data"]["appAccountToken"], account_token(&w.address));
    let (_, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/quote",
        &[],
        Some(json!({ "productId": "nope" })),
    )
    .await;
    assert_eq!(out["code"], "unknown_product");

    let token = account_token(&w.address);
    let (_, out) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&tx_id(), PRODUCT, Some(&token))),
        ),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");
    let (_, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/quote",
        &[],
        Some(json!({ "productId": PRODUCT })),
    )
    .await;
    assert_eq!(
        (out["code"].as_str(), out["reason"].as_str()),
        (Some("cap_exceeded"), Some("daily_limit")),
        "{out}"
    );
    let (_, out) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&tx_id(), PRODUCT, Some(&token))),
        ),
    )
    .await;
    assert_eq!(out["error"], "wallet_cap_exceeded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");

    // A purchase over the node's daily ceiling is left for Apple to retry.
    let node_capped = setup(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 0,
            wallet_daily_cents: 0,
            daily_cents: 1500,
        },
    );
    let w2 = scratch_wallet();
    let (status, out) = notify(
        &node_capped,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&tx_id(), PRODUCT, Some(&w2.address))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(available(&pool, &w2.address).await, "0");

    // An unknown product is retried by Apple.
    let (status, _) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&tx_id(), "nope", Some(&token))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    drop(app);
    own.drop().await;
}

#[tokio::test]
async fn the_operator_routes_credit_by_product_and_refund_once() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    // A wallet cap below one product: the operator credit reconciles what it refused.
    let app = app(test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(
                &c,
                Limits {
                    wallet_total_cents: 500,
                    ..open_limits()
                },
            )),
            admin_token: Some("op-secret-123456789012345678901234"),
            ..StateCfg::default()
        },
    ));
    let w = scratch_wallet();
    let id = tx_id();
    let mut bearer = HeaderMap::new();
    bearer.insert(
        "authorization",
        "Bearer op-secret-123456789012345678901234".parse().unwrap(),
    );
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        HeaderMap::new(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{out}");
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer.clone(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["credits"], 100);
    assert_eq!(available(&pool, &w.address).await, "100");
    let refund = json!({ "appleTransactionId": id, "refundId": "r-1", "reason": "support" });
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer.clone(),
        Some(refund.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    let (status, _, _) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer.clone(),
        Some(refund),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _, _) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer,
        Some(json!({ "appleTransactionId": tx_id(), "refundId": "r-2", "reason": "support" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_node_without_app_store_configuration_answers_503() {
    let state = test_state(
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody:nowhere@127.0.0.1:1/never")
            .unwrap(),
        StateCfg::default(),
    );
    let (status, _, _) = send(
        &app(state),
        "POST",
        "/credits/apple/webhook",
        HeaderMap::new(),
        Some(json!({ "signedPayload": "a.b.c" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

/// The iOS app's request: the wallet delegates to an ephemeral key ("Decentraland Login"),
/// which signs the pre-6.0.0 folded payload with camelCase metadata.
async fn login_signed_legacy(
    w: &Wallet,
    method: &str,
    path: &str,
    metadata: &str,
    expiration: &str,
) -> HeaderMap {
    let ephemeral = PrivateKeySigner::random();
    let login = format!(
        "Decentraland Login\nEphemeral address: {:#x}\nExpiration: {expiration}",
        ephemeral.address()
    );
    let login_signature = w
        .signer
        .sign_message(login.as_bytes())
        .await
        .unwrap()
        .to_string();
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = build_legacy_payload(method, path, &timestamp, metadata);
    let signature = ephemeral
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: w.address.clone(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaEphemeral,
            payload: login,
            signature: Some(login_signature),
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature),
        },
    ];
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTH_TIMESTAMP_HEADER,
        HeaderValue::from_str(&timestamp).unwrap(),
    );
    headers.insert(
        AUTH_METADATA_HEADER,
        HeaderValue::from_str(metadata).unwrap(),
    );
    for (i, link) in chain.iter().enumerate() {
        headers.insert(
            HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes()).unwrap(),
            HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
        );
    }
    headers
}

#[tokio::test]
async fn the_app_login_chain_signing_the_folded_payload_quotes_and_verifies() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let live = "2099-01-01T00:00:00.000Z";

    let path = "/credits/credits/iap/quote";
    let metadata = json!({ "productId": PRODUCT }).to_string();
    let headers = login_signed_legacy(&w, "post", path, &metadata, live).await;
    let (status, _, out) = send(
        &app,
        "POST",
        path,
        headers,
        Some(json!({ "productId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["appAccountToken"], account_token(&w.address));

    let signed_tx = c.transaction(&tx_id(), PRODUCT, Some(&account_token(&w.address)));
    let path = "/credits/credits/iap/verify";
    let metadata = json!({ "jwsRepresentation": signed_tx }).to_string();
    let expired =
        login_signed_legacy(&w, "post", path, &metadata, "2001-01-01T00:00:00.000Z").await;
    let (status, _, out) = send(
        &app,
        "POST",
        path,
        expired,
        Some(json!({ "jwsRepresentation": signed_tx })),
    )
    .await;
    assert!(
        status.is_client_error(),
        "an expired login delegation: {out}"
    );
    assert_eq!(out["error"], "Invalid Auth Chain", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");

    let headers = login_signed_legacy(&w, "post", path, &metadata, live).await;
    let (status, _, out) = send(
        &app,
        "POST",
        path,
        headers,
        Some(json!({ "jwsRepresentation": signed_tx })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["credited"], true, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn app_store_grants_reconcile_with_their_purchases() {
    let Some(own) = super::tests::OwnDatabase::create("iap_reconcile").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let token = register(&app, &w).await;
    let signed_tx = c.transaction(&tx_id(), PRODUCT, Some(&token));
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed_tx.clone())),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");
    let report = state.credits.reconcile().await.unwrap();
    assert!(
        report.purchase_grant_mismatches.is_empty(),
        "{:?}",
        report.purchase_grant_mismatches
    );

    let (_, out) = notify(&app, c.notification("REFUND", Some(signed_tx))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let report = state.credits.reconcile().await.unwrap();
    assert!(
        report.purchase_grant_mismatches.is_empty(),
        "{:?}",
        report.purchase_grant_mismatches
    );
    drop(app);
    drop(state);
    own.drop().await;
}

#[tokio::test]
async fn concurrent_registrations_of_one_wallet_all_return_its_token() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let calls = (0..16).map(|_| call(&app, &w, "post", "/credits/credits/iap/register", &[], None));
    for (status, out) in futures::future::join_all(calls).await {
        assert_eq!(status, StatusCode::OK, "{out}");
        assert_eq!(out["data"]["appAccountToken"], account_token(&w.address));
    }
}

fn bearer() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        "Bearer op-secret-123456789012345678901234".parse().unwrap(),
    );
    h
}

fn setup_with_admin(pool: &PgPool, c: &Chain, limits: Limits) -> Router {
    app(test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(c, limits)),
            admin_token: Some("op-secret-123456789012345678901234"),
            ..StateCfg::default()
        },
    ))
}

async fn row(pool: &PgPool, id: &str) -> Option<(String, Option<String>, Option<String>)> {
    sqlx::query_as(
        "SELECT status, address, held_reason FROM credits.iap_transactions WHERE transaction_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn held_ids(app: &Router) -> Vec<String> {
    let (status, _, out) = send(
        app,
        "GET",
        "/credits/credits/iap/held?limit=200",
        bearer(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    out["held"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["appleTransactionId"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_purchase_over_a_wallet_cap_is_held_until_an_operator_credits_it() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 500,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let token = register(&app, &w).await;
    let id = tx_id();
    let (status, out) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&id, PRODUCT, Some(&token))),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["error"], "wallet_cap_exceeded");
    assert_eq!(out["held"], true);
    assert_eq!(
        row(&pool, &id).await,
        Some((
            "held".into(),
            Some(w.address.clone()),
            Some("total_limit".into())
        ))
    );
    assert!(held_ids(&app).await.contains(&id));
    assert_eq!(available(&pool, &w.address).await, "0");

    // A held purchase is never credited to another wallet.
    let other = scratch_wallet();
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": other.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");

    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["alreadyExisted"], false);
    assert_eq!(available(&pool, &w.address).await, "100");
    assert_eq!(row(&pool, &id).await.unwrap().0, "paid");
    assert!(!held_ids(&app).await.contains(&id));

    // The same transaction again, for another wallet: already credited elsewhere.
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": other.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(available(&pool, &other.address).await, "0");
}

#[tokio::test]
async fn a_held_purchase_is_credited_by_a_later_delivery() {
    let Some(own) = super::tests::OwnDatabase::create("iap_held_retry").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let w = scratch_wallet();
    let token = w.address.clone();

    // Over the node's ceiling, then an unknown product: both held and retried by Apple.
    let closed = setup(
        &pool,
        &c,
        Limits {
            daily_cents: 0,
            ..open_limits()
        },
    );
    let capped = tx_id();
    let signed = c.transaction(&capped, PRODUCT, Some(&token));
    let (status, _) = notify(
        &closed,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        row(&pool, &capped).await.unwrap().2.as_deref(),
        Some("service_daily_limit")
    );
    let unknown = tx_id();
    let unknown_signed = c.transaction(&unknown, "org.example.later", Some(&token));
    let (status, _) = notify(
        &closed,
        c.notification("ONE_TIME_CHARGE", Some(unknown_signed.clone())),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        row(&pool, &unknown).await.unwrap().2.as_deref(),
        Some("unknown_product")
    );

    // The ceiling resets and the product is configured: the retries credit both.
    let mut reopened = iap(&c, open_limits());
    Arc::get_mut(&mut reopened)
        .unwrap()
        .catalog
        .insert("org.example.later".into(), 50);
    let app = app(test_state(
        pool.clone(),
        StateCfg {
            iap: Some(reopened),
            ..StateCfg::default()
        },
    ));
    let (_, out) = notify(&app, c.notification("ONE_TIME_CHARGE", Some(signed))).await;
    assert_eq!(out["action"], "minted", "{out}");
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(unknown_signed)),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(available(&pool, &w.address).await, "150");
    assert_eq!(row(&pool, &capped).await.unwrap().0, "paid");
    let report = test_state(pool.clone(), StateCfg::default())
        .credits
        .reconcile()
        .await
        .unwrap();
    assert!(report.purchase_grant_mismatches.is_empty());
    drop(app);
    drop(closed);
    own.drop().await;
}

#[tokio::test]
async fn a_purchase_naming_no_wallet_is_held_and_credited_by_the_device() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let token = account_token(&w.address);
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&token));
    let (status, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["error"], "unresolvable_app_account_token");
    assert_eq!(out["held"], true);
    let token_kept: Option<String> = sqlx::query_scalar(
        "SELECT app_account_token FROM credits.iap_transactions WHERE transaction_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(token_kept, Some(token));
    assert!(held_ids(&app).await.contains(&id));

    let (_, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": signed })),
    )
    .await;
    assert_eq!(out["data"]["credited"], true, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    assert_eq!(row(&pool, &id).await.unwrap().0, "paid");
}

#[tokio::test]
async fn a_quantity_out_of_range_is_held_without_a_retry() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let id = tx_id();
    let mut p = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&w.address));
    p["quantity"] = json!(11);
    let (status, out) = notify(&app, c.notification("ONE_TIME_CHARGE", Some(c.sign(&p)))).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["error"], "quantity_out_of_range");
    assert_eq!(
        row(&pool, &id).await,
        Some((
            "held".into(),
            Some(w.address.clone()),
            Some("quantity_out_of_range".into())
        ))
    );
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn a_transaction_revoked_when_first_seen_is_never_credited() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let id = tx_id();
    let mut revoked = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&w.address));
    revoked["revocationDate"] = json!(1_700_000_000_000i64);
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(c.sign(&revoked))),
    )
    .await;
    assert_eq!(out["error"], "revoked_transaction", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");

    let (_, out) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&id, PRODUCT, Some(&w.address))),
        ),
    )
    .await;
    assert_eq!(out["error"], "refunded_transaction", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn a_reversed_refund_grants_the_charge_back_again() {
    let Some(own) = super::tests::OwnDatabase::create("iap_refund_reversed").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let signed = c.transaction(&tx_id(), PRODUCT, Some(&w.address));
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");
    for _ in 0..2 {
        let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
        assert_eq!(out["action"], "refunded", "{out}");
        assert_eq!(available(&pool, &w.address).await, "0");
        let (_, out) = notify(
            &app,
            c.notification("REFUND_REVERSED", Some(signed.clone())),
        )
        .await;
        assert_eq!(out["action"], "reinstated", "{out}");
        assert_eq!(available(&pool, &w.address).await, "100");
    }
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(out["action"], "not_refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    let report = state.credits.reconcile().await.unwrap();
    assert!(report.ok, "{report:?}");

    // A refund that arrived before the purchase, then reversed: the purchase credits.
    let early = c.transaction(&tx_id(), PRODUCT, Some(&w.address));
    let (_, out) = notify(&app, c.notification("REFUND", Some(early.clone()))).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(early))).await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(available(&pool, &w.address).await, "200");
    drop(app);
    drop(state);
    own.drop().await;
}

#[tokio::test]
async fn a_partial_refund_charges_back_its_share() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let id = tx_id();
    let (_, out) = notify(
        &app,
        c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&id, PRODUCT, Some(&w.address))),
        ),
    )
    .await;
    assert_eq!(out["action"], "minted", "{out}");
    let mut refunded = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&w.address));
    refunded["revocationDate"] = json!(1_700_000_000_000i64);
    refunded["revocationPercentage"] = json!(25_000);
    let (_, out) = notify(&app, c.notification("REFUND", Some(c.sign(&refunded)))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "75");
    let orders = test_state(pool.clone(), StateCfg::default())
        .credits
        .credit_orders_history(&w.address, 10, 0)
        .await
        .unwrap();
    assert_eq!(orders.orders[0].status, "partially_reversed");
}

#[tokio::test]
async fn the_device_path_answers_the_cap_codes() {
    let Some(own) = super::tests::OwnDatabase::create("iap_verify_caps").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let w = scratch_wallet();
    let token = account_token(&w.address);
    let wallet_capped = setup(
        &pool,
        &c,
        Limits {
            wallet_daily_cents: 500,
            ..open_limits()
        },
    );
    let verify = |app: Router, id: String| {
        let w = &w;
        let signed = c.transaction(&id, PRODUCT, Some(&token));
        async move {
            call(
                &app,
                w,
                "post",
                "/credits/credits/iap/verify",
                &[],
                Some(json!({ "jwsRepresentation": signed })),
            )
            .await
            .1
        }
    };
    let out = verify(wallet_capped.clone(), tx_id()).await;
    assert_eq!(
        (out["code"].as_str(), out["reason"].as_str()),
        (Some("cap_exceeded"), Some("daily_limit")),
        "{out}"
    );
    let node_capped = setup(
        &pool,
        &c,
        Limits {
            daily_cents: 500,
            ..open_limits()
        },
    );
    let out = verify(node_capped.clone(), tx_id()).await;
    assert_eq!(out["code"], "service_daily_limit", "{out}");
    let mut other = super::apple::testkit::transaction_payload(&tx_id(), "nope", Some(&token));
    other["quantity"] = json!(1);
    let (_, out) = call(
        &node_capped,
        &w,
        "post",
        "/credits/credits/iap/verify",
        &[],
        Some(json!({ "jwsRepresentation": c.sign(&other) })),
    )
    .await;
    assert_eq!(out["code"], "unknown_product", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    drop(wallet_capped);
    drop(node_capped);
    own.drop().await;
}

#[tokio::test]
async fn the_operator_credit_is_held_to_the_node_ceiling() {
    let Some(own) = super::tests::OwnDatabase::create("iap_operator_ceiling").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(
        &pool,
        &c,
        Limits {
            daily_cents: 500,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": tx_id(), "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    assert!(
        held_ids(&app).await.is_empty(),
        "an operator credit records no held row"
    );
    drop(app);
    own.drop().await;
}

#[tokio::test]
async fn imported_purchases_count_toward_the_caps() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1500,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    sqlx::query(
        "INSERT INTO credits.iap_transactions (transaction_id, product_id, address, credits, amount_cents, status, created_at) \
         VALUES ($1, $2, $3, 100, 1000, 'imported', timezone('UTC', now()) - interval '30 days')",
    )
    .bind(tx_id())
    .bind(PRODUCT)
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    let (_, out) = call(
        &app,
        &w,
        "post",
        "/credits/credits/iap/quote",
        &[],
        Some(json!({ "productId": PRODUCT })),
    )
    .await;
    assert_eq!(
        (out["code"].as_str(), out["reason"].as_str()),
        (Some("cap_exceeded"), Some("total_limit")),
        "{out}"
    );
    // An imported purchase is not this node's credit order.
    let path = format!("/credits/users/{}/credit-orders", w.address);
    let (_, out) = call(&app, &w, "get", &path, &[], None).await;
    assert_eq!(out["total"], 0, "{out}");
}

#[tokio::test]
async fn credit_orders_page_through_card_and_app_store_rows_once_each() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    // Every row at one instant: only the tie-break orders them.
    sqlx::query(
        "INSERT INTO credits.purchases (address, sku, credits, amount_cents, currency, status, created_at) \
         VALUES ($1, 'pack', 10, 100, 'usd', 'paid', '2026-01-01')",
    )
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    for _ in 0..6 {
        sqlx::query(
            "INSERT INTO credits.iap_transactions (transaction_id, product_id, address, credits, amount_cents, status, credited_by, created_at) \
             VALUES ($1, $2, $3, 100, 1000, 'paid', 'device', '2026-01-01')",
        )
        .bind(tx_id())
        .bind(PRODUCT)
        .bind(&w.address)
        .execute(&pool)
        .await
        .unwrap();
    }
    let state = test_state(pool.clone(), StateCfg::default());
    let mut seen = std::collections::HashSet::new();
    for offset in 0..7 {
        let page = state
            .credits
            .credit_orders_history(&w.address, 1, offset)
            .await
            .unwrap();
        assert_eq!(page.total, 7);
        assert!(seen.insert(page.orders[0].id.clone()), "offset {offset}");
    }
    assert_eq!(seen.len(), 7);
    drop(app);
}

#[tokio::test]
async fn a_held_purchase_refunded_is_never_credited() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 500,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["held"], true, "{out}");
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed))).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
    assert!(!held_ids(&app).await.contains(&id));
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn an_imported_purchase_is_charged_back_once_and_never_credited_again() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let (usd, mana) = (tx_id(), tx_id());
    // Upstream's grant arrives as a plain ledger grant; the import records the purchase.
    state
        .credits
        .admin_grant_credits(super::admin_store::GrantRequest {
            address: &w.address,
            amount: "100",
            kind: "grant",
            reason: Some("imported"),
            actor: Some("migration"),
            idempotency_key: None,
            detail: &json!({}),
            studio_name: None,
        })
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO credits.iap_transactions (transaction_id, product_id, address, credits, amount_cents, status) \
         VALUES ($1, $2, $3, 100, 1000, 'imported')",
    )
    .bind(&usd)
    .bind(PRODUCT)
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO credits.iap_transactions (transaction_id, product_id, status, refund_ref, refund_reason, refunded_at) \
         VALUES ($1, $2, 'refunded', 'credits-server', 'credited upstream', timezone('UTC', now()))",
    )
    .bind(&mana)
    .bind(PRODUCT)
    .execute(&pool)
    .await
    .unwrap();
    let signed = c.transaction(&usd, PRODUCT, Some(&w.address));
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["alreadyExisted"], true, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");

    // A refund after the cutover charges it back once; the row stays imported.
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(out["removed"], "100", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    assert_eq!(row(&pool, &usd).await.unwrap().0, "imported");
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["error"], "refunded_transaction", "{out}");
    let totals = super::store::Credits::iap_totals(&pool, &w.address)
        .await
        .unwrap();
    assert_eq!(totals.total_cents, 1000, "a refunded import still counts");

    // Its reversal grants the charge back and keeps it imported.
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    assert_eq!(row(&pool, &usd).await.unwrap().0, "imported");

    // A reversed refund of an upstream purchase keeps its marker.
    let signed = c.transaction(&mana, PRODUCT, Some(&w.address));
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(out["action"], "not_refunded", "{out}");
    assert_eq!(row(&pool, &mana).await.unwrap().0, "refunded");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_reversed_refund_of_an_uncredited_purchase_survives_a_failed_credit() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();

    // The reversal is recorded, then the credit never runs: a redelivery credits it.
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let first = state
        .credits
        .reinstate_iap(&id, operator_reversal("operator:first"))
        .await
        .unwrap();
    assert_eq!(first, super::iap::ReinstateOutcome::Uncredited);
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    assert_eq!(
        held_reason(&pool, &id).await.as_deref(),
        Some("refund_reversed")
    );
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");

    // The node's ceiling refuses the credit (503): the redelivery finds it held.
    let closed = setup(
        &pool,
        &c,
        Limits {
            daily_cents: 0,
            ..open_limits()
        },
    );
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    notify(&closed, c.notification("REFUND", Some(signed.clone()))).await;
    let (status, out) = notify(
        &closed,
        c.notification("REFUND_REVERSED", Some(signed.clone())),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(available(&pool, &w.address).await, "200");

    // A purchase held for a cap keeps its wallet through the refund and its reversal.
    let capped = setup(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1,
            ..open_limits()
        },
    );
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(
        &capped,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["held"], true, "{out}");
    notify(&capped, c.notification("REFUND", Some(signed))).await;
    state
        .credits
        .reinstate_iap(&id, operator_reversal("operator:capped"))
        .await
        .unwrap();
    let (status, address, _) = row(&pool, &id).await.unwrap();
    assert_eq!(
        (status.as_str(), address.as_deref()),
        ("held", Some(w.address.as_str()))
    );
}

#[tokio::test]
async fn a_refund_redelivered_after_its_reversal_applies_once() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let refund = c.notification("REFUND", Some(bought[0].clone()));
    let (_, out) = notify(&app, refund.clone()).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    // Defense in depth: without its notification record, the refund's reference alone keeps it applied once.
    forget(&pool, &refund).await;
    let (_, out) = notify(&app, refund).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");

    // A new refund of the same purchase still applies.
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");

    // A purchase first credited by the reversal keeps the refund's reference.
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let refund = c.notification("REFUND", Some(signed.clone()));
    let (_, out) = notify(&app, refund.clone()).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let (_, out) = notify(&app, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(out["action"], "minted", "{out}");
    // Defense in depth: the refund's reference survives the credit of the held row.
    forget(&pool, &refund).await;
    let (_, out) = notify(&app, refund).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "paid");
    assert_eq!(available(&pool, &w.address).await, "100");
}

/// A string field of a signed payload.
fn signed_field(signed: &str, field: &str) -> String {
    use base64::Engine as _;
    let payload = signed.split('.').nth(1).unwrap();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()[field]
        .as_str()
        .unwrap()
        .to_string()
}

/// The `notificationUUID` a signed notification carries.
/// An operator reversal signed now.
fn operator_reversal(uuid: &str) -> super::iap::Reverser<'_> {
    super::iap::Reverser::Operator {
        notice: super::iap::Notice {
            uuid,
            signed_date_ms: chrono::Utc::now().timestamp_millis(),
        },
        actor: "token:test",
    }
}

fn uuid_of(signed: &str) -> String {
    signed_field(signed, "notificationUUID")
}

/// Drops a notification's record. The purge outlasts the window, so a
/// delivery never finds its record gone; tests use this to show the other
/// guards hold on their own.
async fn forget(pool: &PgPool, signed: &str) {
    sqlx::query("DELETE FROM credits.iap_notifications WHERE notification_uuid = $1")
        .bind(uuid_of(signed))
        .execute(pool)
        .await
        .unwrap();
}

/// How many records a notification left.
async fn records(pool: &PgPool, signed: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM credits.iap_notifications WHERE notification_uuid = $1",
    )
    .bind(uuid_of(signed))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn held_reason(pool: &PgPool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT held_reason FROM credits.iap_transactions WHERE transaction_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A signed credit of `address` pledging `cents`, `age_secs` old, expiring
/// `expires_in_secs` from now; returns its hash.
async fn pledge(
    pool: &PgPool,
    address: &str,
    cents: i64,
    age_secs: i64,
    expires_in_secs: i64,
) -> String {
    let id = format!("auth-{}", uuid::Uuid::new_v4().simple());
    let hash = format!(
        "0x{}",
        hex::encode(alloy_primitives::keccak256(id.as_bytes()))
    );
    sqlx::query(
        "INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at, chain_id, contract, credit_hash, created_at) \
         VALUES ($1, $2, $3, 1, 'authorized', timezone('UTC', now()) + make_interval(secs => $4), 137, $5, $6, \
                 timezone('UTC', now()) - make_interval(secs => $7))",
    )
    .bind(&id)
    .bind(address)
    .bind(cents)
    .bind(expires_in_secs as f64)
    .bind("0x00000000000000000000000000000000000000c2")
    .bind(&hash)
    .bind(age_secs as f64)
    .execute(pool)
    .await
    .unwrap();
    hash
}

async fn queued(pool: &PgPool, hash: &str) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM credits.revocations WHERE credit_hash = $1)")
        .bind(hash)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn blocked(pool: &PgPool, address: &str) -> bool {
    sqlx::query_scalar("SELECT is_blocked_for_claiming FROM credits.wallets WHERE address = $1")
        .bind(address)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_notification_signed_outside_the_window_is_refused_or_retried() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let charge_at = |id: &str, signed_date_ms: i64| {
        let t = c.transaction(id, PRODUCT, Some(&w.address));
        c.notification_signed_at(
            "ONE_TIME_CHARGE",
            Some(t),
            Chain::app_data(),
            signed_date_ms,
        )
    };
    let max_age_ms = super::apple::NOTIFICATION_MAX_AGE_SECS * 1000;
    let skew_ms = super::apple::NOTIFICATION_MAX_FUTURE_SECS * 1000;
    // Too old: refused for good, since a retry carries the same signedDate.
    let id = tx_id();
    let (status, out) = notify(&app, charge_at(&id, now_ms - max_age_ms - 60_000)).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["error"], "signed_date_out_of_window", "{out}");
    assert!(row(&pool, &id).await.is_none());

    // Ahead of this node's clock: answered for a retry, so it is not lost.
    let id = tx_id();
    let ahead = charge_at(&id, now_ms + skew_ms + 60_000);
    let (status, out) = notify(&app, ahead.clone()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert!(row(&pool, &id).await.is_none());
    assert_eq!(records(&pool, &ahead).await, 0);
    assert_eq!(available(&pool, &w.address).await, "0");

    // Apple's last retry, 157 hours after the first attempt, still credits.
    let (status, out) = notify(&app, charge_at(&tx_id(), now_ms - 157 * 3_600_000)).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_redelivered_notification_applies_nothing() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let charge = c.notification("ONE_TIME_CHARGE", Some(signed.clone()));
    let (_, out) = notify(&app, charge.clone()).await;
    assert_eq!(out["action"], "minted", "{out}");
    let (status, out) = notify(&app, charge).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["duplicate"], true, "{out}");

    // Refund, reversal, a second refund: the first reversal redelivered keeps the second refund.
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let reversal = c.notification("REFUND_REVERSED", Some(signed.clone()));
    let (_, out) = notify(&app, reversal.clone()).await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    let (status, out) = notify(&app, reversal).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["duplicate"], true, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn a_notification_answered_for_a_retry_is_applied_by_the_retry() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let closed = Limits {
        daily_cents: 0,
        ..open_limits()
    };
    let w = scratch_wallet();
    let id = tx_id();
    let charge = c.notification(
        "ONE_TIME_CHARGE",
        Some(c.transaction(&id, PRODUCT, Some(&w.address))),
    );
    let (status, out) = notify(&setup(&pool, &c, closed), charge.clone()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    let (_, out) = notify(&setup(&pool, &c, open_limits()), charge).await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_concurrent_delivery_of_the_same_notification_answers_409() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let id = tx_id();
    let charge = c.notification(
        "ONE_TIME_CHARGE",
        Some(c.transaction(&id, PRODUCT, Some(&w.address))),
    );
    // Another delivery holds the uuid's record, not committed yet.
    let mut other = pool.begin().await.unwrap();
    let other_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *other)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO credits.iap_notifications (notification_uuid, signed_at) \
         VALUES ($1, timezone('UTC', now()))",
    )
    .bind(uuid_of(&charge))
    .execute(&mut *other)
    .await
    .unwrap();
    let delivery = tokio::spawn({
        let (app, charge) = (app.clone(), charge.clone());
        async move { notify(&app, charge).await }
    });
    // Wait until this delivery's claim waits on the other's record.
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
                 WHERE $1 = ANY (pg_blocking_pids(pid)))",
            )
            .bind(other_pid)
            .fetch_one(&pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the delivery waits on the other record");
    other.commit().await.unwrap();
    let (status, out) = delivery.await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert!(row(&pool, &id).await.is_none());
    assert_eq!(available(&pool, &w.address).await, "0");
    assert_eq!(records(&pool, &charge).await, 1);
}

#[tokio::test]
async fn a_reversal_credited_by_its_purchase_records_its_uuid_once() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let closed = setup(
        &pool,
        &c,
        Limits {
            daily_cents: 0,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let reversal = c.notification("REFUND_REVERSED", Some(signed));
    // Answered for a retry: nothing recorded, so the retry applies it.
    let (status, out) = notify(&closed, reversal.clone()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(records(&pool, &reversal).await, 0);
    let (_, out) = notify(&app, reversal.clone()).await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(records(&pool, &reversal).await, 1);
    let (_, out) = notify(&app, reversal.clone()).await;
    assert_eq!(out["duplicate"], true, "{out}");
    assert_eq!(records(&pool, &reversal).await, 1);
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_hold_not_retried_keeps_its_notification_record() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let capped = setup(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    // A wallet cap and an account token naming no wallet are held for an operator, not retried.
    for token in [w.address.clone(), account_token(&scratch_wallet().address)] {
        let id = tx_id();
        let charge = c.notification(
            "ONE_TIME_CHARGE",
            Some(c.transaction(&id, PRODUCT, Some(&token))),
        );
        let (status, out) = notify(&capped, charge.clone()).await;
        assert_eq!(status, StatusCode::OK, "{out}");
        assert_eq!(out["held"], true, "{out}");
        assert_eq!(records(&pool, &charge).await, 1);
        let (_, out) = notify(&capped, charge).await;
        assert_eq!(out["duplicate"], true, "{out}");
        assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    }
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn a_reversal_signed_before_the_current_refund_applies_nothing() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(bought[0].clone()), Chain::app_data(), ms)
    };
    let (_, out) = notify(&app, at("REFUND", now_ms - 3_000)).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let first_reversal = at("REFUND_REVERSED", now_ms - 2_000);
    let (_, out) = notify(&app, first_reversal.clone()).await;
    assert_eq!(out["action"], "reinstated", "{out}");
    let (_, out) = notify(&app, at("REFUND", now_ms - 1_000)).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");

    // Defense in depth: without its record, the first reversal redelivered does not undo the second refund.
    forget(&pool, &first_reversal).await;
    let (status, out) = notify(&app, first_reversal).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["action"], "superseded", "{out}");
    let id = signed_field(&bought[0], "transactionId");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
    assert_eq!(available(&pool, &w.address).await, "0");
}

/// Status of the purchase, balance, whether the pledge is queued for
/// revocation and whether the wallet is blocked.
async fn standing(pool: &PgPool, id: &str, w: &Wallet, hash: &str) -> (String, String, bool, bool) {
    (
        row(pool, id).await.unwrap().0,
        available(pool, &w.address).await,
        queued(pool, hash).await,
        blocked(pool, &w.address).await,
    )
}

#[tokio::test]
async fn a_reversal_delivered_before_its_refund_keeps_the_purchase_credited() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    // An applied refund would queue this pledge and block the wallet.
    let hash = pledge(&pool, &w.address, 800, 0, 300).await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(bought[0].clone()), Chain::app_data(), ms)
    };
    let refund = at("REFUND", now_ms - 2_000);
    let reversal = at("REFUND_REVERSED", now_ms - 1_000);
    let credited = ("paid".to_string(), "100".to_string(), false, false);

    let (_, out) = notify(&app, reversal.clone()).await;
    assert_eq!(out["action"], "not_refunded", "{out}");
    assert_eq!(standing(&pool, &id, &w, &hash).await, credited);
    let (status, out) = notify(&app, refund.clone()).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(standing(&pool, &id, &w, &hash).await, credited);

    for signed in [&reversal, &refund] {
        let (_, out) = notify(&app, signed.clone()).await;
        assert_eq!(out["duplicate"], true, "{out}");
        assert_eq!(standing(&pool, &id, &w, &hash).await, credited);
    }
    // Defense in depth: without their records, the decision alone still refuses the refund.
    forget(&pool, &reversal).await;
    forget(&pool, &refund).await;
    let (_, out) = notify(&app, reversal).await;
    assert_eq!(out["action"], "not_refunded", "{out}");
    let (_, out) = notify(&app, refund).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(standing(&pool, &id, &w, &hash).await, credited);
}

#[tokio::test]
async fn a_reversal_before_any_purchase_row_keeps_its_older_refund_from_charging() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(signed.clone()), Chain::app_data(), ms)
    };
    let (_, out) = notify(&app, at("REFUND_REVERSED", now_ms - 1_000)).await;
    assert_eq!(out["action"], "minted", "{out}");
    let hash = pledge(&pool, &w.address, 800, 0, 300).await;
    let (_, out) = notify(&app, at("REFUND", now_ms - 2_000)).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(
        standing(&pool, &id, &w, &hash).await,
        ("paid".to_string(), "100".to_string(), false, false)
    );
}

#[tokio::test]
async fn a_refund_delivered_before_an_older_reversal_stays_charged_back() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let hash = pledge(&pool, &w.address, 800, 0, 300).await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(bought[0].clone()), Chain::app_data(), ms)
    };
    let refund = at("REFUND", now_ms - 1_000);
    let reversal = at("REFUND_REVERSED", now_ms - 2_000);
    let charged = ("refunded".to_string(), "0".to_string(), true, true);

    let (_, out) = notify(&app, refund.clone()).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(standing(&pool, &id, &w, &hash).await, charged);
    let (_, out) = notify(&app, reversal.clone()).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(standing(&pool, &id, &w, &hash).await, charged);

    for signed in [&refund, &reversal] {
        let (_, out) = notify(&app, signed.clone()).await;
        assert_eq!(out["duplicate"], true, "{out}");
        assert_eq!(standing(&pool, &id, &w, &hash).await, charged);
    }
    // Defense in depth: without their records, the reference and the decision still hold.
    forget(&pool, &refund).await;
    forget(&pool, &reversal).await;
    let (_, out) = notify(&app, refund).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    let (_, out) = notify(&app, reversal).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(standing(&pool, &id, &w, &hash).await, charged);
}

#[tokio::test]
async fn a_second_refund_signed_after_a_reversal_keeps_that_reversal_from_applying() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let now_ms = chrono::Utc::now().timestamp_millis();
    // A second refund reference, signed after a reversal it is delivered
    // before, is the latest decision even though it charges nothing again.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(bought[0].clone()), Chain::app_data(), ms)
    };
    let (_, out) = notify(&app, at("REFUND", now_ms - 3_000)).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let (_, out) = notify(&app, at("REFUND", now_ms - 1_000)).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    let (_, out) = notify(&app, at("REFUND_REVERSED", now_ms - 2_000)).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn a_second_refund_of_a_refunded_purchase_charges_nothing_back() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 2).await;
    let mut p = super::apple::testkit::transaction_payload(
        &signed_field(&bought[0], "transactionId"),
        PRODUCT,
        Some(&w.address),
    );
    p["revocationPercentage"] = json!(50_000);
    p["revocationReason"] = json!(0);
    let (_, out) = notify(&app, c.notification("REFUND", Some(c.sign(&p)))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "150");

    // This node refunds a transaction once, as credits-server does: another reference is recorded, not charged again.
    p["revocationPercentage"] = json!(100_000);
    let second = c.notification("REFUND", Some(c.sign(&p)));
    let (_, out) = notify(&app, second.clone()).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    assert_eq!(records(&pool, &second).await, 1);
    assert_eq!(available(&pool, &w.address).await, "150");
}

#[tokio::test]
async fn crediting_a_held_purchase_keeps_its_row_and_counts_toward_the_caps() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let token = account_token(&w.address);
    let id = tx_id();
    let mut p = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&token));
    p["originalTransactionId"] = json!(format!("{id}0"));
    let signed = c.sign(&p);
    let (_, out) = notify(
        &app,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["held"], true, "{out}");
    // Held for more than a day before the wallet registers.
    sqlx::query(
        "UPDATE credits.iap_transactions SET created_at = created_at - interval '25 hours' \
         WHERE transaction_id = $1",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();
    register(&app, &w).await;
    // The wallet's daily cap allows this purchase and no other.
    let ceiling = setup(
        &pool,
        &c,
        Limits {
            wallet_daily_cents: 1_000,
            ..open_limits()
        },
    );
    let (_, out) = notify(&ceiling, c.notification("ONE_TIME_CHARGE", Some(signed))).await;
    assert_eq!(out["action"], "minted", "{out}");
    let (status, token_kept, original, recent): (String, Option<String>, Option<String>, bool) =
        sqlx::query_as(
            "SELECT status, app_account_token, original_transaction_id, \
                    created_at > timezone('UTC', now()) - interval '1 hour' \
             FROM credits.iap_transactions WHERE transaction_id = $1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "paid");
    assert_eq!(token_kept, Some(token.clone()));
    assert_eq!(original, Some(format!("{id}0")));
    let next = c.transaction(&tx_id(), PRODUCT, Some(&token));
    let (_, out) = notify(&ceiling, c.notification("ONE_TIME_CHARGE", Some(next))).await;
    assert_eq!(out["held"], true, "the daily cap counts the credit: {out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    assert!(recent, "the credit's time is what the daily caps count");
}

/// Credits `n` signed purchases of `PRODUCT` to `w`; returns their signed transactions.
async fn buy(app: &Router, c: &Chain, w: &Wallet, n: usize) -> Vec<String> {
    let mut signed = Vec::new();
    for _ in 0..n {
        let t = c.transaction(&tx_id(), PRODUCT, Some(&w.address));
        let (_, out) = notify(app, c.notification("ONE_TIME_CHARGE", Some(t.clone()))).await;
        assert_eq!(out["action"], "minted", "{out}");
        signed.push(t);
    }
    signed
}

#[tokio::test]
async fn an_operator_credits_a_held_purchase_as_apple_signed_it() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 500,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let mut p = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&w.address));
    p["quantity"] = json!(10);
    let (_, out) = notify(&app, c.notification("ONE_TIME_CHARGE", Some(c.sign(&p)))).await;
    assert_eq!(out["held"], true, "{out}");

    for body in [
        json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT, "quantity": 1 }),
        json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": "org.example.other" }),
    ] {
        let (status, _, out) =
            send(&app, "POST", "/credits/credits/iap", bearer(), Some(body)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{out}");
        assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    }

    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["credits"], 1000);
    assert_eq!(available(&pool, &w.address).await, "1000");
    // The request names no original id or token: the held row's stay.
    let kept: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, original_transaction_id, app_account_token \
         FROM credits.iap_transactions WHERE transaction_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        kept,
        (
            "paid".to_string(),
            Some(id.clone()),
            Some(w.address.to_ascii_lowercase())
        )
    );
}

#[tokio::test]
async fn an_operator_refund_of_a_held_purchase_reports_it() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 500,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(&app, c.notification("ONE_TIME_CHARGE", Some(signed))).await;
    assert_eq!(out["held"], true, "{out}");
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(json!({ "appleTransactionId": id, "refundId": "r-1", "reason": "support" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["held"], true);
    assert_eq!(out["removed"], "0");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
}

#[tokio::test]
async fn a_later_refund_revokes_credits_an_earlier_one_did_not_queue() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 3).await;
    // 300 available; a newer 200 and an older 90 pledged.
    let newer = pledge(&pool, &w.address, 2000, 0, 300).await;
    let older = pledge(&pool, &w.address, 900, 60, 300).await;

    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["revocations"], 1, "{out}");
    assert!(queued(&pool, &newer).await);
    assert!(!queued(&pool, &older).await);
    // 100 left covers the older 90.
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[1].clone()))).await;
    assert_eq!(out["revocations"], 0, "{out}");
    assert!(!queued(&pool, &older).await);
    // Nothing left: the older credit is revoked, though the newer one fills the window.
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[2].clone()))).await;
    assert_eq!(out["revocations"], 1, "{out}");
    assert!(queued(&pool, &older).await);
}

#[tokio::test]
async fn an_expired_pledge_does_not_widen_the_revocations() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 2).await;
    // 200 available; 150 on a credit past its expiry (not swept yet), 50 and 40 live.
    let expired = pledge(&pool, &w.address, 1500, 0, -60).await;
    let live_newer = pledge(&pool, &w.address, 500, 30, 300).await;
    let live_older = pledge(&pool, &w.address, 400, 60, 300).await;

    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    // The 100 left covers the 90 still spendable.
    assert_eq!(out["revocations"], 0, "{out}");
    for hash in [&expired, &live_newer, &live_older] {
        assert!(!queued(&pool, hash).await);
    }
    assert!(
        blocked(&pool, &w.address).await,
        "the block still counts every pledge"
    );
}

#[tokio::test]
async fn a_reversed_refund_withdraws_its_unsent_revocations_and_lifts_its_block() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());

    // The refund blocked the wallet and queued two credits; one was sent since.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let unsent = pledge(&pool, &w.address, 400, 0, 300).await;
    let sent = pledge(&pool, &w.address, 400, 60, 300).await;
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["revocations"], 2, "{out}");
    assert!(blocked(&pool, &w.address).await);
    sqlx::query(
        "UPDATE credits.revocations SET status = 'sent', tx_hash = $2 WHERE credit_hash = $1",
    )
    .bind(&sent)
    .bind(format!("0x{}", "ab".repeat(32)))
    .execute(&pool)
    .await
    .unwrap();
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(out["revocationsWithdrawn"], 1, "{out}");
    assert!(!queued(&pool, &unsent).await);
    assert!(queued(&pool, &sent).await, "a sent revocation is final");
    assert!(!blocked(&pool, &w.address).await);

    // A wallet already blocked stays blocked.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    pledge(&pool, &w.address, 800, 0, 300).await;
    sqlx::query("UPDATE credits.wallets SET is_blocked_for_claiming = true WHERE address = $1")
        .bind(&w.address)
        .execute(&pool)
        .await
        .unwrap();
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert!(blocked(&pool, &w.address).await);
}

#[tokio::test]
async fn a_reversed_refund_keeps_a_revocation_a_later_refund_relies_on() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 2).await;
    // 200 available; a newer 150 and an older 40 pledged.
    let newer = pledge(&pool, &w.address, 1500, 0, 300).await;
    let older = pledge(&pool, &w.address, 400, 60, 300).await;
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["revocations"], 1, "{out}");
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[1].clone()))).await;
    assert_eq!(out["revocations"], 1, "{out}");
    assert!(queued(&pool, &older).await);

    // 100 back cannot cover the newer 150 once more.
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(out["revocationsWithdrawn"], 0, "{out}");
    assert!(queued(&pool, &newer).await);
    assert!(blocked(&pool, &w.address).await);
}

#[tokio::test]
async fn a_reversed_refund_keeps_a_revocation_the_job_claimed() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());

    // The job claimed the revocation before the reversal: it may be on chain.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let claimed = pledge(&pool, &w.address, 400, 0, 300).await;
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["revocations"], 1, "{out}");
    assert!(
        super::settlement::claim_revocation(&pool, &claimed, 0)
            .await
            .unwrap()
    );
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(out["revocationsWithdrawn"], 0, "{out}");
    assert!(queued(&pool, &claimed).await);

    // The reversal withdrew it first: the job's claim finds nothing to send.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let withdrawn = pledge(&pool, &w.address, 400, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["revocationsWithdrawn"], 1, "{out}");
    assert!(
        !super::settlement::claim_revocation(&pool, &withdrawn, 0)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_sent_revocation_keeps_its_failure_budget_for_polling() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let hash = pledge(&pool, &w.address, 400, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    let last = super::settlement::REVOCATION_MAX_ATTEMPTS - 1;
    sqlx::query("UPDATE credits.revocations SET attempts = $2 WHERE credit_hash = $1")
        .bind(&hash)
        .bind(last)
        .execute(&pool)
        .await
        .unwrap();
    // The last allowed broadcast succeeds: the row is still polled for its receipt.
    assert!(
        super::settlement::claim_revocation(&pool, &hash, last)
            .await
            .unwrap()
    );
    let tx = format!("0x{}", "ab".repeat(32));
    super::settlement::mark_revocation(&pool, &hash, "sent", Some(&tx), None, -1)
        .await
        .unwrap();
    let (status, attempts): (String, i32) =
        sqlx::query_as("SELECT status, attempts FROM credits.revocations WHERE credit_hash = $1")
            .bind(&hash)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), attempts), ("sent", last));
    assert!(attempts < super::settlement::REVOCATION_MAX_ATTEMPTS);
}

async fn unblock_audits(pool: &PgPool, address: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM ops.admin_audit \
         WHERE domain = 'credits' AND action = 'user.unblock' AND actor = 'apple' AND target = $1",
    )
    .bind(address)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_reversed_refund_audits_the_block_it_lifts() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    pledge(&pool, &w.address, 400, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert!(blocked(&pool, &w.address).await);
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert!(!blocked(&pool, &w.address).await);
    assert_eq!(unblock_audits(&pool, &w.address).await, 1);
}

#[tokio::test]
async fn a_reversed_refund_leaves_a_block_an_operator_set_again() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    pledge(&pool, &w.address, 400, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert!(blocked(&pool, &w.address).await);
    // The operator reviewed the refund and kept the block.
    state
        .credits
        .admin_set_blocked(
            &w.address,
            true,
            Some("review"),
            Some("token:op"),
            &json!({}),
        )
        .await
        .unwrap();
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert!(blocked(&pool, &w.address).await);
    assert_eq!(unblock_audits(&pool, &w.address).await, 0);
}

#[tokio::test]
async fn a_reversed_refund_leaves_a_block_a_later_reversal_relies_on() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 2).await;
    // 200 available, 150 pledged: each refund leaves the pledge uncovered.
    pledge(&pool, &w.address, 1500, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    notify(&app, c.notification("REFUND", Some(bought[1].clone()))).await;
    assert!(blocked(&pool, &w.address).await);
    // Both reversed: the balance covers the pledge again, but the block is
    // no longer the first refund's alone.
    for signed in [&bought[1], &bought[0]] {
        let (_, out) = notify(
            &app,
            c.notification("REFUND_REVERSED", Some(signed.clone())),
        )
        .await;
        assert_eq!(out["action"], "reinstated", "{out}");
    }
    assert_eq!(available(&pool, &w.address).await, "200");
    assert!(blocked(&pool, &w.address).await);
}

/// Waits until a backend of this database waits on a lock.
async fn until_a_lock_wait(pool: &PgPool) {
    for _ in 0..200 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity \
             WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("no backend waited on the wallet row");
}

/// Whether the transaction's row can be locked now, from another connection.
async fn row_lockable(pool: &PgPool, id: &str) -> bool {
    let mut probe = pool.begin().await.unwrap();
    let ok = sqlx::query(
        "SELECT 1 FROM credits.iap_transactions WHERE transaction_id = $1 FOR UPDATE NOWAIT",
    )
    .bind(id)
    .execute(&mut *probe)
    .await
    .is_ok();
    probe.rollback().await.unwrap();
    ok
}

#[tokio::test]
async fn refunds_and_reversals_lock_the_wallet_before_the_transaction_row() {
    let Some(own) = super::tests::OwnDatabase::create("iap_lock_order").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    pledge(&pool, &w.address, 400, 0, 300).await;
    let id: String = sqlx::query_scalar(
        "SELECT transaction_id FROM credits.iap_transactions WHERE address = $1",
    )
    .bind(&w.address)
    .fetch_one(&pool)
    .await
    .unwrap();

    // An operator block holds the wallet row and then writes the wallet's App Store rows.
    for step in ["refund", "reversal"] {
        let mut operator = pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM credits.wallets WHERE address = $1 FOR UPDATE")
            .bind(&w.address)
            .execute(&mut *operator)
            .await
            .unwrap();
        let credits = state.credits.clone();
        let id2 = id.clone();
        let task = tokio::spawn(async move {
            if step == "refund" {
                credits
                    .refund_iap(
                        &id2,
                        Some(PRODUCT),
                        "r1",
                        "apple_refund:test",
                        None,
                        Refunder::Operator {
                            actor: "test",
                            override_reversal: false,
                        },
                    )
                    .await
                    .map(|_| ())
            } else {
                credits
                    .reinstate_iap(&id2, operator_reversal("operator:r1"))
                    .await
                    .map(|_| ())
            }
        });
        until_a_lock_wait(&pool).await;
        assert!(
            row_lockable(&pool, &id).await,
            "the {step} waits for the wallet before it locks the transaction's row"
        );
        operator.rollback().await.unwrap();
        task.await.unwrap().unwrap();
    }
    assert_eq!(row(&pool, &id).await.unwrap().0, "paid");
    drop(bought);
    drop(app);
    drop(state);
    own.drop().await;
}

/// A JSON-RPC chain for the revoker: fixed fees and nonce, every sent
/// transaction recorded and confirmed.
async fn spawn_revocation_chain() -> (String, Arc<std::sync::Mutex<usize>>) {
    let sent = Arc::new(std::sync::Mutex::new(0usize));
    let seen = sent.clone();
    let rpc = axum::Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(req): axum::Json<Value>| {
            let seen = seen.clone();
            async move {
                let id = req.get("id").cloned().unwrap_or(json!(1));
                let result = match req["method"].as_str().unwrap_or("") {
                    "eth_chainId" => json!("0x89"),
                    "eth_estimateGas" => json!("0x5208"),
                    "eth_gasPrice" => json!("0x3b9aca00"),
                    "eth_maxPriorityFeePerGas" => json!("0x59682f00"),
                    "eth_getTransactionCount" => json!("0x5"),
                    "eth_blockNumber" => json!("0x50"),
                    "eth_getBlockByNumber" => {
                        json!({ "number": "0x50", "baseFeePerGas": "0x3b9aca00", "timestamp": "0x6553f100" })
                    }
                    "eth_sendRawTransaction" => {
                        let raw = req["params"][0].as_str().unwrap_or("");
                        let bytes = hex::decode(raw.trim_start_matches("0x")).unwrap_or_default();
                        *seen.lock().unwrap() += 1;
                        json!(format!("{:#x}", alloy_primitives::keccak256(&bytes)))
                    }
                    "eth_getTransactionReceipt" => {
                        json!({ "status": "0x1", "blockNumber": "0x10", "logs": [] })
                    }
                    _ => json!(null),
                };
                axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, rpc).await;
    });
    (format!("http://{addr}"), sent)
}

async fn revocation(pool: &PgPool, hash: &str) -> (String, i32, Option<String>) {
    sqlx::query_as(
        "SELECT status, attempts, tx_hash FROM credits.revocations WHERE credit_hash = $1",
    )
    .bind(hash)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_revocation_pass_claims_sends_and_skips_withdrawn_rows() {
    use super::settlement::{RevocationOutcome, Revoker};
    use crate::evm::chain::ChainComponent;
    use crate::evm::signer::DirectSigner;
    let Some(own) = super::tests::OwnDatabase::create("iap_revocation_pass").await else {
        return;
    };
    let pool = own.pool.clone();
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let (url, sent) = spawn_revocation_chain().await;
    let rpc = Arc::new(ChainComponent::new(
        crate::http::Outbound::for_tests(),
        &url,
        std::time::Duration::from_secs(5),
    ));
    let key = hex::encode(PrivateKeySigner::random().to_bytes());
    let revoker = |fee_cap: Option<u128>| Revoker {
        credits: state.credits.clone(),
        signer: Arc::new(
            DirectSigner::new(
                rpc.clone(),
                &key,
                137,
                1_500_000,
                std::time::Duration::from_millis(1),
                std::time::Duration::from_millis(50),
            )
            .unwrap()
            .with_max_fee_per_gas(fee_cap),
        ),
        chain_id: 137,
    };

    // A refund the balance cannot cover queues one signed credit.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let hash = pledge(&pool, &w.address, 400, 0, 300).await;
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["revocations"], 1, "{out}");

    // A broadcast refused before it is sent keeps the attempt: the revocation is final.
    let out = revoker(Some(1)).run_pass(None).await.unwrap();
    assert_eq!(
        out,
        RevocationOutcome {
            errors: 1,
            ..Default::default()
        }
    );
    assert_eq!(revocation(&pool, &hash).await, ("pending".into(), 1, None));
    assert_eq!(*sent.lock().unwrap(), 0);
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["revocationsWithdrawn"], 0, "{out}");

    // The next pass sends it and gives the claim back, then polls it confirmed.
    let out = revoker(None).run_pass(None).await.unwrap();
    assert_eq!(out.sent, 1, "{out:?}");
    let (status, attempts, tx_hash) = revocation(&pool, &hash).await;
    assert_eq!((status.as_str(), attempts), ("sent", 1));
    assert!(tx_hash.is_some());
    assert_eq!(*sent.lock().unwrap(), 1);
    let out = revoker(None).run_pass(None).await.unwrap();
    assert_eq!(out.confirmed, 1, "{out:?}");
    assert_eq!(revocation(&pool, &hash).await.0, "confirmed");

    // A revocation the reversal withdrew is never sent.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let withdrawn = pledge(&pool, &w.address, 400, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["revocationsWithdrawn"], 1, "{out}");
    let out = revoker(None).run_pass(None).await.unwrap();
    assert_eq!(out, RevocationOutcome::default());
    assert!(!queued(&pool, &withdrawn).await);
    assert_eq!(*sent.lock().unwrap(), 1);
    drop(app);
    drop(state);
    own.drop().await;
}

/// A refund and a reversal of `signed` both signed at `ms`, the one
/// ordered last (by uuid, bytewise) a reversal when `reversal_last`.
fn same_millisecond(c: &Chain, signed: &str, ms: i64, reversal_last: bool) -> (String, String) {
    loop {
        let refund = c.notification_signed_at("REFUND", Some(signed.into()), Chain::app_data(), ms);
        let reversal = c.notification_signed_at(
            "REFUND_REVERSED",
            Some(signed.into()),
            Chain::app_data(),
            ms,
        );
        if (uuid_of(&reversal).as_bytes() > uuid_of(&refund).as_bytes()) == reversal_last {
            return (refund, reversal);
        }
    }
}

#[tokio::test]
async fn a_refund_and_a_reversal_signed_in_one_millisecond_end_alike_in_either_order() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let ms = chrono::Utc::now().timestamp_millis() - 1_000;
    for reversal_last in [true, false] {
        for refund_first in [true, false] {
            let w = scratch_wallet();
            let bought = buy(&app, &c, &w, 1).await;
            let id = signed_field(&bought[0], "transactionId");
            let (refund, reversal) = same_millisecond(&c, &bought[0], ms, reversal_last);
            let order = if refund_first {
                [refund, reversal]
            } else {
                [reversal, refund]
            };
            for signed in order {
                let (status, out) = notify(&app, signed).await;
                assert_eq!(status, StatusCode::OK, "{out}");
            }
            let expected = if reversal_last {
                ("paid", "100")
            } else {
                ("refunded", "0")
            };
            assert_eq!(
                (
                    row(&pool, &id).await.unwrap().0.as_str(),
                    available(&pool, &w.address).await.as_str()
                ),
                expected,
                "reversal last: {reversal_last}, refund first: {refund_first}"
            );
        }
    }
}

#[tokio::test]
async fn an_operator_refund_after_apples_reversal_needs_an_override() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    // Apple's refund was too old to deliver; its reversal was recorded.
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "not_refunded", "{out}");
    let refund = json!({ "appleTransactionId": id, "refundId": "r-1", "reason": "support" });
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(refund.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    let mut forced = refund;
    forced["overrideReversal"] = json!(true);
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(forced),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
}

#[tokio::test]
async fn apples_reversal_undoes_an_operator_refund() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(json!({ "appleTransactionId": id, "refundId": "r-1", "reason": "support" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    let (_, out) = notify(
        &app,
        c.notification("REFUND_REVERSED", Some(bought[0].clone())),
    )
    .await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "paid");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn an_operator_reversal_undoes_a_refund_and_holds_a_purchase_never_credited() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let reverse = |id: &str, reversal: &str| {
        let signed_date = chrono::Utc::now().timestamp_millis();
        let body =
            json!({ "appleTransactionId": id, "reversalId": reversal, "signedDate": signed_date });
        let app = app.clone();
        async move {
            let (status, _, out) = send(
                &app,
                "POST",
                "/credits/credits/iap/reversal",
                bearer(),
                Some(body),
            )
            .await;
            (status, out)
        }
    };
    let (status, _, _) = send(
        &app,
        "POST",
        "/credits/credits/iap/reversal",
        HeaderMap::new(),
        Some(json!({ "appleTransactionId": tx_id(), "reversalId": "v-1", "signedDate": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A credited purchase refunded by Apple: the charge back is granted again, once.
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(available(&pool, &w.address).await, "0");
    let (status, out) = reverse(&id, "v-1").await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["credits"], "100", "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    let (status, out) = reverse(&id, "v-1").await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");

    // Refunded before it was credited: held, then the operator credits it.
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    notify(&app, c.notification("REFUND", Some(signed))).await;
    let (status, out) = reverse(&id, "v-2").await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["held"], true, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(available(&pool, &w.address).await, "200");
}

/// Posts an operator reversal of `id` signed at `ms`.
async fn operator_reverse(app: &Router, id: &str, reversal: &str, ms: i64) -> (StatusCode, Value) {
    let body = json!({ "appleTransactionId": id, "reversalId": reversal, "signedDate": ms });
    let (status, _, out) = send(
        app,
        "POST",
        "/credits/credits/iap/reversal",
        bearer(),
        Some(body),
    )
    .await;
    (status, out)
}

#[tokio::test]
async fn a_reused_reversal_id_grants_a_later_refund_back() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let now_ms = chrono::Utc::now().timestamp_millis();
    let refund_at = |ms: i64| {
        c.notification_signed_at("REFUND", Some(bought[0].clone()), Chain::app_data(), ms)
    };
    let (_, out) = notify(&app, refund_at(now_ms - 4_000)).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let (status, out) = operator_reverse(&app, &id, "rev-1", now_ms - 3_000).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
    // Apple refunds it again; the operator reverses that one under the same id.
    let (_, out) = notify(&app, refund_at(now_ms - 2_000)).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    let (status, out) = operator_reverse(&app, &id, "rev-1", now_ms - 1_000).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["credits"], "100", "{out}");
    assert_eq!(
        (
            row(&pool, &id).await.unwrap().0.as_str(),
            available(&pool, &w.address).await.as_str()
        ),
        ("paid", "100")
    );
    // The operator's reversal is the latest decision: a refund needs an override.
    let refund = json!({ "appleTransactionId": id, "refundId": "r-op", "reason": "support" });
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(refund),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn an_operator_reversal_follows_apples_latest_decision() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let now_ms = chrono::Utc::now().timestamp_millis();

    // Apple's newer refund stands against an older reversal.
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let refund_at = |ms: i64| {
        c.notification_signed_at("REFUND", Some(bought[0].clone()), Chain::app_data(), ms)
    };
    notify(&app, refund_at(now_ms - 4_000)).await;
    let (_, out) = notify(&app, refund_at(now_ms - 2_000)).await;
    assert_eq!(out["action"], "already_refunded", "{out}");
    let (status, out) = operator_reverse(&app, &id, "v-old", now_ms - 3_000).await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
    assert_eq!(available(&pool, &w.address).await, "0");
    // A date ahead of the node's clock is refused.
    let ahead = now_ms + (super::apple::NOTIFICATION_MAX_FUTURE_SECS + 60) * 1000;
    let (status, out) = operator_reverse(&app, &id, "v-ahead", ahead).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
}

#[tokio::test]
async fn an_operator_reversal_keeps_a_held_purchase_from_an_older_revoked_copy() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(
        &app,
        c.notification_signed_at("REFUND", Some(signed), Chain::app_data(), now_ms - 3_000),
    )
    .await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let (status, out) = operator_reverse(&app, &id, "v-held", now_ms - 1_000).await;
    assert_eq!(out["held"], true, "{status} {out}");
    let revoked_at = |ms: i64| {
        let mut p = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&w.address));
        p["revocationDate"] = json!(now_ms - 3_000);
        p["signedDate"] = json!(ms);
        c.sign(&p)
    };
    let verify = "/credits/credits/iap/verify";
    let body = json!({ "jwsRepresentation": revoked_at(now_ms - 2_000) });
    let (_, out) = call(&app, &w, "post", verify, &[], Some(body)).await;
    assert_eq!(out["code"], "revoked_transaction", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    let body = json!({ "jwsRepresentation": revoked_at(now_ms) });
    call(&app, &w, "post", verify, &[], Some(body)).await;
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
}

#[tokio::test]
async fn a_replayed_reversal_grant_is_an_internal_error_and_rolled_back() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let refund = c.notification("REFUND", Some(bought[0].clone()));
    let (_, out) = notify(&app, refund.clone()).await;
    assert_eq!(out["action"], "refunded", "{out}");
    // The grant this refund's reversal would make is already recorded.
    let key = format!("apple-iap:{id}:reinstated:{}", uuid_of(&refund));
    sqlx::query(
        "INSERT INTO credits.idempotency (scope, idempotency_key, address, amount, available, actor) \
         VALUES ('grant', $1, $2, 100, 0, 'apple')",
    )
    .bind(&key)
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    let ms = chrono::Utc::now().timestamp_millis();
    let (status, out) = operator_reverse(&app, &id, "v-1", ms).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{out}");
    assert_eq!(
        (
            row(&pool, &id).await.unwrap().0.as_str(),
            available(&pool, &w.address).await.as_str()
        ),
        ("refunded", "0")
    );
}

/// The refund decisions recorded for `id`.
async fn decisions(pool: &PgPool, id: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM credits.iap_refund_decisions WHERE transaction_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn an_operator_reversal_with_no_refund_to_undo_records_nothing() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let now_ms = chrono::Utc::now().timestamp_millis();

    // An unknown transaction.
    let unknown = tx_id();
    let (status, out) = operator_reverse(&app, &unknown, "v-unknown", now_ms).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{out}");
    assert_eq!(decisions(&pool, &unknown).await, 0);
    assert!(row(&pool, &unknown).await.is_none());

    // A paid purchase: refused, and Apple's earlier-signed refund still charges it back.
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let (status, out) = operator_reverse(&app, &id, "v-paid", now_ms - 1_000).await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(decisions(&pool, &id).await, 0);
    let refund = c.notification_signed_at(
        "REFUND",
        Some(bought[0].clone()),
        Chain::app_data(),
        now_ms - 3_000,
    );
    let (_, out) = notify(&app, refund).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");

    // A purchase held for a cap, never refunded.
    let capped = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1,
            ..open_limits()
        },
    );
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let (_, out) = notify(&capped, c.notification("ONE_TIME_CHARGE", Some(signed))).await;
    assert_eq!(out["held"], true, "{out}");
    let (status, out) = operator_reverse(&capped, &id, "v-held", now_ms).await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(decisions(&pool, &id).await, 0);
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
}

#[tokio::test]
async fn an_operator_reversal_is_audited_as_the_operator() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    pledge(&pool, &w.address, 400, 0, 300).await;
    notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert!(blocked(&pool, &w.address).await);
    let ms = chrono::Utc::now().timestamp_millis();
    let (status, out) = operator_reverse(&app, &id, "v-1", ms).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert!(!blocked(&pool, &w.address).await);
    let actors: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT action, actor, detail->>'source' FROM ops.admin_audit \
         WHERE domain = 'credits' AND target = $1 AND action IN ('grant', 'user.unblock') \
           AND detail->>'reversalRef' = 'operator:v-1' ORDER BY action",
    )
    .bind(&w.address)
    .fetch_all(&pool)
    .await
    .unwrap();
    let operator = format!("token:{}", super::admin::ADMIN_TOKEN_ENV);
    assert_eq!(
        actors,
        vec![
            (
                "grant".to_string(),
                operator.clone(),
                "operator".to_string()
            ),
            (
                "user.unblock".to_string(),
                operator.clone(),
                "operator".to_string()
            )
        ]
    );
    assert_eq!(unblock_audits(&pool, &w.address).await, 0);
    // The grant's ledger row points at the operator's audit row.
    let ledger_actors: Vec<String> = sqlx::query_scalar(
        "SELECT a.actor FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id \
         WHERE l.address = $1 AND l.kind = 'refund' AND a.detail->>'reversalRef' = 'operator:v-1'",
    )
    .bind(&w.address)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(ledger_actors, vec![operator]);
}

#[tokio::test]
async fn an_operator_credit_follows_apples_latest_decision() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let closed = setup(
        &pool,
        &c,
        Limits {
            daily_cents: 0,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let credit =
        json!({ "userAddress": w.address, "appleTransactionId": id, "appleProductId": PRODUCT });
    // Refunded by Apple before any credit: the operator cannot credit it.
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(credit.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    // Apple reversed it, but the node's ceiling held the credit: the operator credits it once.
    let (status, out) = notify(&closed, c.notification("REFUND_REVERSED", Some(signed))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap",
        bearer(),
        Some(credit.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["alreadyExisted"], false, "{out}");
    let (_, _, out) = send(&app, "POST", "/credits/credits/iap", bearer(), Some(credit)).await;
    assert_eq!(out["alreadyExisted"], true, "{out}");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_stale_refund_leaves_a_held_purchase_held() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let capped = setup(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(signed.clone()), Chain::app_data(), ms)
    };
    let (_, out) = notify(
        &capped,
        c.notification("ONE_TIME_CHARGE", Some(signed.clone())),
    )
    .await;
    assert_eq!(out["held"], true, "{out}");
    // The reversal, delivered first, leaves it held for the cap.
    let (_, out) = notify(&capped, at("REFUND_REVERSED", now_ms - 1_000)).await;
    assert_eq!(out["held"], true, "{out}");
    let (_, out) = notify(&capped, at("REFUND", now_ms - 2_000)).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
}

#[tokio::test]
async fn an_imported_purchase_follows_apples_latest_decision() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let id = tx_id();
    state
        .credits
        .admin_grant_credits(super::admin_store::GrantRequest {
            address: &w.address,
            amount: "100",
            kind: "grant",
            reason: Some("imported"),
            actor: Some("migration"),
            idempotency_key: None,
            detail: &json!({}),
            studio_name: None,
        })
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO credits.iap_transactions (transaction_id, product_id, address, credits, amount_cents, status) \
         VALUES ($1, $2, $3, 100, 1000, 'imported')",
    )
    .bind(&id)
    .bind(PRODUCT)
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(signed.clone()), Chain::app_data(), ms)
    };
    let (_, out) = notify(&app, at("REFUND", now_ms - 2_000)).await;
    assert_eq!(out["action"], "refunded", "{out}");
    let (_, out) = notify(&app, at("REFUND_REVERSED", now_ms - 3_000)).await;
    assert_eq!(out["action"], "superseded", "{out}");
    assert_eq!(available(&pool, &w.address).await, "0");
    let (_, out) = notify(&app, at("REFUND_REVERSED", now_ms - 1_000)).await;
    assert_eq!(out["action"], "reinstated", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "imported");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_reversal_committed_before_its_credit_is_recorded_by_the_retry() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    notify(&app, c.notification("REFUND", Some(signed.clone()))).await;
    let reversal = c.notification("REFUND_REVERSED", Some(signed));
    let uuid = uuid_of(&reversal);
    let signed_date_ms: i64 = {
        use base64::Engine as _;
        let payload = reversal.split('.').nth(1).unwrap();
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .unwrap();
        serde_json::from_slice::<Value>(&bytes).unwrap()["signedDate"]
            .as_i64()
            .unwrap()
    };
    // The reversal's transaction commits, then the node stops before the credit.
    let outcome = state
        .credits
        .reinstate_iap(
            &id,
            super::iap::Reverser::Apple(super::iap::Notice {
                uuid: &uuid,
                signed_date_ms,
            }),
        )
        .await
        .unwrap();
    assert_eq!(outcome, super::iap::ReinstateOutcome::Uncredited);
    assert_eq!(records(&pool, &reversal).await, 0);
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    // Apple's retry is not stale against its own decision: it credits and records once.
    let (_, out) = notify(&app, reversal.clone()).await;
    assert_eq!(out["action"], "minted", "{out}");
    assert_eq!(records(&pool, &reversal).await, 1);
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn a_revoked_transaction_signed_before_a_reversal_leaves_the_purchase_held() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup(&pool, &c, open_limits());
    let closed = setup(
        &pool,
        &c,
        Limits {
            daily_cents: 0,
            ..open_limits()
        },
    );
    let w = scratch_wallet();
    let id = tx_id();
    let signed = c.transaction(&id, PRODUCT, Some(&w.address));
    let now_ms = chrono::Utc::now().timestamp_millis();
    let at = |kind: &str, ms: i64| {
        c.notification_signed_at(kind, Some(signed.clone()), Chain::app_data(), ms)
    };
    notify(&app, at("REFUND", now_ms - 3_000)).await;
    let (status, out) = notify(&closed, at("REFUND_REVERSED", now_ms - 1_000)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    let revoked_at = |ms: i64| {
        let mut p = super::apple::testkit::transaction_payload(&id, PRODUCT, Some(&w.address));
        p["revocationDate"] = json!(now_ms - 3_000);
        p["signedDate"] = json!(ms);
        c.sign(&p)
    };
    let verify = "/credits/credits/iap/verify";
    // The device's copy from before the reversal neither credits nor cancels it.
    let body = json!({ "jwsRepresentation": revoked_at(now_ms - 2_000) });
    let (_, out) = call(&app, &w, "post", verify, &[], Some(body)).await;
    assert_eq!(out["code"], "revoked_transaction", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "held");
    // One signed after the reversal is Apple's newer word: refunded.
    let body = json!({ "jwsRepresentation": revoked_at(now_ms) });
    let (_, out) = call(&app, &w, "post", verify, &[], Some(body)).await;
    assert_eq!(out["code"], "revoked_transaction", "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "refunded");
}

/// `(action, actor, source)` of the credits audit rows about `id`, by action.
async fn audits_of(pool: &PgPool, id: &str) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT action, actor, detail->>'source' FROM ops.admin_audit \
         WHERE domain = 'credits' AND detail->>'transactionId' = $1 ORDER BY action",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap()
}

fn audit(action: &str, actor: &str, source: &str) -> (String, String, String) {
    (action.to_string(), actor.to_string(), source.to_string())
}

#[tokio::test]
async fn an_operator_credit_and_refund_are_audited_as_the_operator() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let capped = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1,
            ..open_limits()
        },
    );
    let operator = format!("token:{}", super::admin::ADMIN_TOKEN_ENV);
    let w = scratch_wallet();

    // A purchase held for a cap, credited and refunded by the operator.
    let held = tx_id();
    let signed = c.transaction(&held, PRODUCT, Some(&w.address));
    let (_, out) = notify(&capped, c.notification("ONE_TIME_CHARGE", Some(signed))).await;
    assert_eq!(out["held"], true, "{out}");
    let credit =
        json!({ "userAddress": w.address, "appleTransactionId": held, "appleProductId": PRODUCT });
    let (status, _, out) = send(&app, "POST", "/credits/credits/iap", bearer(), Some(credit)).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    let refund = json!({ "appleTransactionId": held, "refundId": "r-op", "reason": "support" });
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(refund),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(
        audits_of(&pool, &held).await,
        vec![
            audit("grant", &operator, "operator"),
            audit("revoke", &operator, "operator")
        ]
    );

    // Apple's credit and refund keep their actors.
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let (_, out) = notify(&app, c.notification("REFUND", Some(bought[0].clone()))).await;
    assert_eq!(out["action"], "refunded", "{out}");
    assert_eq!(
        audits_of(&pool, &id).await,
        vec![
            audit("grant", "apple", "apple"),
            audit("revoke", "apple", "apple")
        ]
    );
}

#[tokio::test]
async fn every_operator_app_store_action_it_applies_is_audited() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let state = test_state(
        pool.clone(),
        StateCfg {
            iap: Some(iap(&c, open_limits())),
            ..StateCfg::default()
        },
    );
    let app = setup_with_admin(&pool, &c, open_limits());
    let capped = setup_with_admin(
        &pool,
        &c,
        Limits {
            wallet_total_cents: 1,
            ..open_limits()
        },
    );
    let operator = format!("token:{}", super::admin::ADMIN_TOKEN_ENV);
    let w = scratch_wallet();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let refund = |id: &str, refund_id: &str| json!({ "appleTransactionId": id, "refundId": refund_id, "reason": "support" });

    // A held purchase refunded by the operator: nothing credited, still audited.
    let held = tx_id();
    let signed = c.transaction(&held, PRODUCT, Some(&w.address));
    let (_, out) = notify(&capped, c.notification("ONE_TIME_CHARGE", Some(signed))).await;
    assert_eq!(out["held"], true, "{out}");
    let path = "/credits/credits/iap/refund";
    let (status, _, out) = send(&app, "POST", path, bearer(), Some(refund(&held, "r-held"))).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["held"], true, "{out}");
    assert_eq!(
        audits_of(&pool, &held).await,
        vec![audit("iap.refund", &operator, "operator")]
    );

    // A refund of a purchase never credited, held again by the operator's reversal.
    let rehold = tx_id();
    let signed = c.transaction(&rehold, PRODUCT, Some(&w.address));
    let (_, out) = notify(&app, c.notification("REFUND", Some(signed))).await;
    assert_eq!(out["action"], "unknown_credit", "{out}");
    let (status, out) = operator_reverse(
        &app,
        &rehold,
        "v-rehold",
        chrono::Utc::now().timestamp_millis(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["held"], true, "{out}");
    assert_eq!(row(&pool, &rehold).await.unwrap().0, "held");
    assert_eq!(
        audits_of(&pool, &rehold).await,
        vec![audit("iap.hold", &operator, "operator")]
    );

    // An operator refund that charges back nothing. The endpoint charges back
    // the whole purchase (`credits > 0` by the table), so only a partial
    // percentage reaches this branch: the store is called directly.
    let bought = buy(&app, &c, &w, 1).await;
    let zero = signed_field(&bought[0], "transactionId");
    let outcome = state
        .credits
        .refund_iap(
            &zero,
            None,
            "r-zero",
            "support",
            Some(1),
            Refunder::Operator {
                actor: "token:test",
                override_reversal: false,
            },
        )
        .await
        .unwrap();
    assert!(
        matches!(&outcome, super::iap::RefundOutcome::Reversed { removed, .. } if removed == "0"),
        "{outcome:?}"
    );
    assert_eq!(
        audits_of(&pool, &zero).await,
        vec![
            audit("grant", "apple", "apple"),
            audit("revoke", "token:test", "operator")
        ]
    );

    // Refused actions record nothing.
    let paid = signed_field(&buy(&app, &c, &w, 1).await[0], "transactionId");
    let unknown = tx_id();
    let (status, _, out) = send(&app, "POST", path, bearer(), Some(refund(&unknown, "r-x"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{out}");
    let (status, out) = operator_reverse(&app, &unknown, "v-x", now_ms).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{out}");
    assert!(audits_of(&pool, &unknown).await.is_empty());
    let (status, out) = operator_reverse(&app, &paid, "v-paid", now_ms).await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    let (status, _, out) = send(&app, "POST", path, bearer(), Some(refund(&held, "r-again"))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
    assert_eq!(
        audits_of(&pool, &paid).await,
        vec![audit("grant", "apple", "apple")]
    );
    assert_eq!(
        audits_of(&pool, &held).await,
        vec![audit("iap.refund", &operator, "operator")]
    );
}

#[tokio::test]
async fn an_operator_refund_cannot_take_the_upstream_marker() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let bought = buy(&app, &c, &w, 1).await;
    let id = signed_field(&bought[0], "transactionId");
    let refund =
        json!({ "appleTransactionId": id, "refundId": "credits-server", "reason": "support" });
    let (status, _, out) = send(
        &app,
        "POST",
        "/credits/credits/iap/refund",
        bearer(),
        Some(refund),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");
    assert_eq!(row(&pool, &id).await.unwrap().0, "paid");
    assert_eq!(available(&pool, &w.address).await, "100");
}

#[tokio::test]
async fn an_operator_reversal_of_an_imported_purchase_needs_a_refund_to_undo() {
    let Some(pool) = pool().await else { return };
    let c = chain(ChainSpec::default());
    let app = setup_with_admin(&pool, &c, open_limits());
    let w = scratch_wallet();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (refunded, unrefunded, upstream) = (tx_id(), tx_id(), tx_id());
    sqlx::query(
        "INSERT INTO credits.iap_transactions \
             (transaction_id, product_id, address, credits, amount_cents, status, \
              revoked_credits, refunded_cents, refund_ref, refund_reason, refunded_at, refund_refs) \
         VALUES ($1, $2, $3, 100, 1000, 'imported', 100, 1000, 'r-up', 'refunded upstream', \
                 timezone('UTC', now()), ARRAY['r-up'])",
    )
    .bind(&refunded)
    .bind(PRODUCT)
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO credits.iap_transactions (transaction_id, product_id, address, credits, amount_cents, status) \
         VALUES ($1, $2, $3, 100, 1000, 'imported')",
    )
    .bind(&unrefunded)
    .bind(PRODUCT)
    .bind(&w.address)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO credits.iap_transactions (transaction_id, product_id, status, refund_ref, refund_reason, refunded_at) \
         VALUES ($1, $2, 'refunded', 'credits-server', 'credited upstream', timezone('UTC', now()))",
    )
    .bind(&upstream)
    .bind(PRODUCT)
    .execute(&pool)
    .await
    .unwrap();

    // Not refunded, or an upstream marker: refused, and no decision is left.
    for id in [&unrefunded, &upstream] {
        let (status, out) = operator_reverse(&app, id, "v-none", now_ms).await;
        assert_eq!(status, StatusCode::CONFLICT, "{out}");
        assert_eq!(decisions(&pool, id).await, 0);
    }
    assert_eq!(row(&pool, &upstream).await.unwrap().0, "refunded");

    // A refunded import is granted back and stays imported.
    let (status, out) = operator_reverse(&app, &refunded, "v-up", now_ms).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["credits"], "100", "{out}");
    assert_eq!(decisions(&pool, &refunded).await, 1);
    assert_eq!(row(&pool, &refunded).await.unwrap().0, "imported");
    assert_eq!(available(&pool, &w.address).await, "100");
}
