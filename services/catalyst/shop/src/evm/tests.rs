//! The old crate's integration tests over the v2 module: the quota
//! reservation, the idempotent claims, the sender binding of the relay, the
//! upstream forward and the payments readers, each against an in-process
//! stub (a fake JSON-RPC chain, a fake OZ relayer, a mock upstream) and the
//! process's scratch database (`db::scratch`; skipped without
//! `CATALYST_TEST_DATABASE_URL`). New here: the write gate, the collections
//! mirror envelope and the direct signer's raw transaction. One live check
//! runs only with `ECONOMY_LIVE_RPC_URL`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use alloy::signers::SignerSync;
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::{Address, B256, Bytes, FixedBytes, U256};
use alloy_sol_types::SolCall;
use axum::Json;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::abi::executeMetaTransactionCall;
use super::admin::RuntimeConfig;
use super::errors::ApiError;
use super::meta_tx::{MetaTxStruct, meta_tx_digest};
use super::transaction::{MetaTxSender, TransactionComponent};
use super::{Settings, SharedState, State, now};
use crate::checkout::claims::{
    Claim, EscrowActionClaim, NamePurchaseClaim, NameTransferClaim, claim_escrow_action,
    claim_name_purchase, claim_name_transfer,
};
use crate::http::Outbound;

/// The old crate's whole `api_router`: the relay plus the Shop's checkout
/// and treasury routes over the same state, as `main.rs` mounts them.
fn router(state: SharedState) -> axum::Router {
    super::router(state.clone())
        .merge(crate::checkout::router(state.clone()))
        .merge(crate::treasury::router(state))
}

const COLLECTION: &str = "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0";
const VICTIM: &str = "0x1111111111111111111111111111111111111111";
const BUYER: &str = "0x3333333333333333333333333333333333333333";
const FS: &[u8] = &[0xaa, 0xbb, 0xcc, 0xdd];
const ADMIN: &str = "economy-admin-under-test";
/// The NAME the fake registrar reports as registered (to VICTIM); every other NAME is available.
const REGISTERED_NAME: &str = "taken";

/// The separator the fake chain reports for every target except
/// [`CHAIN_UNVERIFIABLE_CONTRACT`].
const TEST_DOMAIN_SEPARATOR: B256 = B256::new([0x5a; 32]);
/// A target the fake chain answers `getNonce` for but whose domain separator
/// getters revert; not in the audited table, so unverifiable and refused.
const CHAIN_UNVERIFIABLE_CONTRACT: &str = "0x00000000000000000000000000000000dead0001";
/// The one wallet the fake chain reports as holding no MANA.
const CHAIN_EMPTY_WALLET: &str = "0x00000000000000000000000000000000dead0002";
const TEST_MANA_BALANCE_WEI: u128 = 5_000_000_000_000_000_000;
/// A mirrored "collection" whose code is not a proxy over Decentraland's
/// collection code, though its beacon slot names the recorded beacon.
const CHAIN_FOREIGN_CODE_CONTRACT: &str = "0x00000000000000000000000000000000dead0003";

/// The fake chain's code for every other address: an EIP-1167 minimal proxy
/// over Polygon's CollectionImplementation.
fn collection_proxy_code() -> String {
    format!(
        "0x363d3d373d3d3d363d73{}5af43d82803e903d91602b57fd5bf3",
        "006080c6061c4af79b39da0842a3a22a7b3f185e"
    )
}

mod chain_abi {
    alloy_sol_types::sol! {
        function getNonce(address user) external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
        function domainSeparator() external view returns (bytes32);
        function getDomainSeperator() external view returns (bytes32);
        function ownerOf(uint256 tokenId) external view returns (address);
    }
}

/// What the fake chain saw broadcast: the raw transactions it accepted.
type RawTxs = Arc<Mutex<Vec<Vec<u8>>>>;

/// A fake JSON-RPC endpoint: `getNonce` 0 for every contract,
/// `domainSeparator()` the test separator (reverting for the unverifiable
/// target), `eth_chainId` Polygon, a 1559 fee market, and
/// `eth_sendRawTransaction` that records the raw bytes and answers their hash
/// with a confirmed empty receipt.
async fn spawn_fake_chain() -> (String, RawTxs) {
    let raws: RawTxs = Arc::new(Mutex::new(Vec::new()));
    let seen = raws.clone();
    async fn handle(
        axum::extract::State(seen): axum::extract::State<RawTxs>,
        Json(req): Json<Value>,
    ) -> Json<Value> {
        let id = req.get("id").cloned().unwrap_or(json!(1));
        let ok = |v: Value| json!({ "jsonrpc": "2.0", "id": id, "result": v });
        let revert = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": 3, "message": "execution reverted" } });
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        match method {
            "eth_chainId" => Json(ok(json!("0x89"))),
            "eth_getCode" => {
                let target = req["params"][0].as_str().unwrap_or("").to_lowercase();
                if target == CHAIN_FOREIGN_CODE_CONTRACT {
                    return Json(ok(json!("0x6080604052")));
                }
                Json(ok(json!(collection_proxy_code())))
            }
            "eth_getStorageAt" => Json(ok(json!(format!(
                "0x{}{}",
                "0".repeat(24),
                "ddb3781fff645325c8896aa1f067baa381607ecc"
            )))),
            "eth_estimateGas" => Json(ok(json!("0x5208"))),
            "eth_gasPrice" => Json(ok(json!("0x3b9aca00"))),
            "eth_maxPriorityFeePerGas" => Json(ok(json!("0x59682f00"))),
            "eth_getTransactionCount" => Json(ok(json!("0x5"))),
            "eth_getBlockByNumber" => Json(ok(
                json!({ "number": "0x1", "baseFeePerGas": "0x3b9aca00" }),
            )),
            "eth_sendRawTransaction" => {
                let raw = req["params"][0].as_str().unwrap_or("");
                let bytes = hex::decode(raw.trim_start_matches("0x")).unwrap_or_default();
                let hash = alloy_primitives::keccak256(&bytes);
                seen.lock().unwrap().push(bytes);
                Json(ok(json!(format!("{hash:#x}"))))
            }
            "eth_getTransactionReceipt" => Json(ok(json!({ "status": "0x1", "logs": [] }))),
            "eth_call" => {
                let call = &req["params"][0];
                let to = call
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let data = call
                    .get("input")
                    .or_else(|| call.get("data"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let bytes = hex::decode(data.trim_start_matches("0x")).unwrap_or_default();
                let selector = bytes.get(0..4).map(<[u8]>::to_vec).unwrap_or_default();
                if selector == <chain_abi::getNonceCall as SolCall>::SELECTOR {
                    return Json(ok(json!(format!("0x{}", "0".repeat(64)))));
                }
                if selector == <chain_abi::balanceOfCall as SolCall>::SELECTOR {
                    let owner = bytes
                        .get(16..36)
                        .map(Address::from_slice)
                        .unwrap_or(Address::ZERO);
                    let wei = if format!("{owner:#x}") == CHAIN_EMPTY_WALLET {
                        0
                    } else {
                        TEST_MANA_BALANCE_WEI
                    };
                    return Json(ok(json!(format!("0x{wei:064x}"))));
                }
                if selector == <chain_abi::ownerOfCall as SolCall>::SELECTOR {
                    let token = bytes
                        .get(4..36)
                        .map(U256::from_be_slice)
                        .unwrap_or_default();
                    if token
                        == crate::types::NameLabel::parse(REGISTERED_NAME)
                            .unwrap()
                            .token_id()
                    {
                        return Json(ok(json!(format!("0x{}{}", "0".repeat(24), &VICTIM[2..]))));
                    }
                    return Json(revert);
                }
                if selector == <chain_abi::ownerOfCall as SolCall>::SELECTOR {
                    let token = bytes
                        .get(4..36)
                        .map(U256::from_be_slice)
                        .unwrap_or_default();
                    if token
                        == crate::types::NameLabel::parse(REGISTERED_NAME)
                            .unwrap()
                            .token_id()
                    {
                        return Json(ok(json!(format!("0x{}{}", "0".repeat(24), &VICTIM[2..]))));
                    }
                    return Json(revert);
                }
                if selector == <chain_abi::ownerOfCall as SolCall>::SELECTOR {
                    let token = bytes
                        .get(4..36)
                        .map(U256::from_be_slice)
                        .unwrap_or_default();
                    if token
                        == crate::types::NameLabel::parse(REGISTERED_NAME)
                            .unwrap()
                            .token_id()
                    {
                        return Json(ok(json!(format!("0x{}{}", "0".repeat(24), &VICTIM[2..]))));
                    }
                    return Json(revert);
                }
                if selector == <chain_abi::ownerOfCall as SolCall>::SELECTOR {
                    let token = bytes
                        .get(4..36)
                        .map(U256::from_be_slice)
                        .unwrap_or_default();
                    if token
                        == crate::types::NameLabel::parse(REGISTERED_NAME)
                            .unwrap()
                            .token_id()
                    {
                        return Json(ok(json!(format!("0x{}{}", "0".repeat(24), &VICTIM[2..]))));
                    }
                    return Json(revert);
                }
                if selector == <chain_abi::domainSeparatorCall as SolCall>::SELECTOR {
                    if to == CHAIN_UNVERIFIABLE_CONTRACT.to_lowercase() {
                        return Json(revert);
                    }
                    return Json(ok(json!(format!(
                        "0x{}",
                        hex::encode(TEST_DOMAIN_SEPARATOR)
                    ))));
                }
                Json(revert)
            }
            _ => Json(
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "method not found" } }),
            ),
        }
    }
    let app = axum::Router::new()
        .route("/", post(handle))
        .with_state(seen);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake chain");
    let addr = listener.local_addr().expect("chain addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), raws)
}

async fn spawn_fake_relayer() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let app = axum::Router::new().route(
        "/api/v1/relayers/{relayer_id}/transactions",
        post(|| async {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            Json(json!({ "success": true, "data": { "id": format!("oz-{n}"), "hash": format!("0x{:064x}", n), "status": "submitted" } }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake relayer");
    let addr = listener.local_addr().expect("relayer addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

/// Unsigned split-overload calldata: r/s/v are constants, so the signature
/// never recovers to `user_address`.
fn split_sig_calldata(user_address: &str) -> String {
    let call = executeMetaTransactionCall {
        userAddress: user_address.parse::<Address>().expect("address"),
        functionSignature: Bytes::from_static(FS),
        sigR: FixedBytes::<32>::repeat_byte(0x11),
        sigS: FixedBytes::<32>::repeat_byte(0x22),
        sigV: 27,
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

fn sign_meta_tx_for(
    user: Address,
    key: &PrivateKeySigner,
    function_signature: &[u8],
) -> ([u8; 32], [u8; 32], u8) {
    let digest = meta_tx_digest(
        TEST_DOMAIN_SEPARATOR,
        U256::ZERO,
        user,
        function_signature,
        MetaTxStruct::FunctionSignature,
    );
    let sig = key.sign_hash_sync(&digest).expect("sign digest");
    let raw = sig.as_bytes();
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&raw[0..32]);
    s.copy_from_slice(&raw[32..64]);
    let v = if raw[64] >= 27 { raw[64] } else { raw[64] + 27 };
    (r, s, v)
}

fn signed_split_calldata_as(
    user: Address,
    key: &PrivateKeySigner,
    function_signature: &[u8],
) -> String {
    let (r, s, v) = sign_meta_tx_for(user, key, function_signature);
    let call = executeMetaTransactionCall {
        userAddress: user,
        functionSignature: Bytes::copy_from_slice(function_signature),
        sigR: FixedBytes::<32>::from_slice(&r),
        sigS: FixedBytes::<32>::from_slice(&s),
        sigV: v,
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

fn signed_split_calldata(key: &PrivateKeySigner, function_signature: &[u8]) -> String {
    signed_split_calldata_as(key.address(), key, function_signature)
}

fn signed_combined_calldata(key: &PrivateKeySigner, function_signature: &[u8]) -> String {
    let user = key.address();
    let (r, s, v) = sign_meta_tx_for(user, key, function_signature);
    let mut signature = Vec::with_capacity(65);
    signature.extend_from_slice(&r);
    signature.extend_from_slice(&s);
    signature.push(v);
    let call = super::abi::combined_sig::executeMetaTransactionCall {
        userAddress: user,
        functionData: Bytes::copy_from_slice(function_signature),
        signature: Bytes::from(signature),
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

fn lazy_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(500))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap()
}

async fn scratch_pool() -> Option<PgPool> {
    crate::db::scratch::pool(6).await
}

fn state(pool: PgPool, settings: Settings) -> SharedState {
    Arc::new(State::new(pool, Outbound::for_tests(), settings).expect("state builds"))
}

/// A mirrored collection on matic plus the mirror's complete-pass marker
/// (the collections mirror stands in for the old squid `collection` table).
async fn seed_collection(pool: &PgPool, address: &str) {
    sqlx::query(
        "INSERT INTO shop.collections \
         (network, contract_address, chain_id, name, symbol, creator, owner, urn, is_completed, is_approved, is_editable, \
          created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'test', 'TST', $2, $2, $3, true, true, false, 0, 0, 0, $4) \
         ON CONFLICT (network, contract_address) DO NOTHING",
    )
    .bind(address.to_lowercase())
    .bind(VICTIM)
    .bind(format!("urn:decentraland:matic:collections-v2:{}", address.to_lowercase()))
    .bind(now())
    .execute(pool)
    .await
    .expect("seed collection");
    mark_mirror_pass(pool).await;
}

async fn mark_mirror_pass(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         VALUES ('collections', 'matic', 'test://collections', $1) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at",
    )
    .bind(now())
    .execute(pool)
    .await
    .expect("mirror pass marker");
}

async fn clear_mirror_pass(pool: &PgPool) {
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror = 'collections' AND network = 'matic'")
        .execute(pool)
        .await
        .expect("clear mirror marker");
}

async fn row_count(pool: &PgPool, address: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM economy.transactions WHERE user_address = $1")
        .bind(address.to_lowercase())
        .fetch_one(pool)
        .await
        .expect("count")
}

async fn confirmed_hash(pool: &PgPool, addr: &str) -> Option<String> {
    sqlx::query_scalar("SELECT tx_hash FROM economy.transactions WHERE user_address = $1")
        .bind(addr.to_lowercase())
        .fetch_one(pool)
        .await
        .expect("tx_hash row")
}

async fn cleanup_user(pool: &PgPool, addr: &str) {
    sqlx::query("DELETE FROM economy.transactions WHERE user_address = $1")
        .bind(addr.to_lowercase())
        .execute(pool)
        .await
        .expect("cleanup");
}

fn tx_body(from: &str, contract: &str, calldata: &str) -> String {
    json!({ "transactionData": { "from": from, "params": [contract, calldata] } }).to_string()
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<String>,
    headers: &[(&str, &str)],
) -> (u16, String) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn post_transaction(
    app: &axum::Router,
    from: &str,
    contract: &str,
    calldata: &str,
) -> (u16, Value) {
    let (status, text) = send(
        app,
        "POST",
        "/v1/transactions",
        Some(tx_body(from, contract, calldata)),
        &[],
    )
    .await;
    (status, serde_json::from_str(&text).unwrap_or_default())
}

fn sender(addr: &str) -> MetaTxSender {
    MetaTxSender::from_meta_tx_calldata(addr, &split_sig_calldata(addr))
        .expect("calldata signed by the same address it is posted for")
}

fn component(pool: PgPool) -> TransactionComponent {
    TransactionComponent::new(pool, None, None, None, None, RuntimeConfig::new())
}

fn unique_address() -> String {
    format!("{:#x}", PrivateKeySigner::random().address())
}

#[tokio::test]
async fn reserve_then_confirm_promotes_and_is_user_visible() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let tc = component(pool.clone());
    let addr = unique_address();
    let session = Uuid::new_v4().to_string();

    tc.reserve_quota(10, &sender(&addr), &session, 137, COLLECTION, now())
        .await
        .expect("reserve");

    let (tx_hash, sid, chain_id, contract): (Option<String>, Option<String>, i32, String) = sqlx::query_as(
        "SELECT tx_hash, session_id, chain_id, contract_address FROM economy.transactions WHERE user_address = $1",
    )
    .bind(addr.to_lowercase())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(tx_hash.is_none(), "a fresh reservation carries no tx_hash");
    assert_eq!(sid.as_deref(), Some(session.as_str()));
    assert_eq!(
        (chain_id, contract.as_str()),
        (137, COLLECTION),
        "the validated target is recorded"
    );

    assert!(
        tc.get_by_user_address(&addr).await.unwrap().is_empty(),
        "pending reservation must not appear in getByUserAddress"
    );

    tc.confirm_reservation(&session, "0xdeadbeef")
        .await
        .expect("confirm");
    let rows = tc.get_by_user_address(&addr).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tx_hash, "0xdeadbeef");
    let wire = serde_json::to_value(&rows[0]).unwrap();
    assert!(
        wire["created_at"].as_str().unwrap().ends_with('Z'),
        "{wire}"
    );

    let sid_after: Option<String> =
        sqlx::query_scalar("SELECT session_id FROM economy.transactions WHERE user_address = $1")
            .bind(addr.to_lowercase())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(sid_after.is_none(), "confirm clears session_id");
    cleanup_user(&pool, &addr).await;
}

#[tokio::test]
async fn reserve_enforces_daily_limit_and_release_refunds_a_slot() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let tc = component(pool.clone());
    let addr = unique_address();
    const MAX: i64 = 3;

    let mut sessions = Vec::new();
    for _ in 0..MAX {
        let s = Uuid::new_v4().to_string();
        tc.reserve_quota(MAX, &sender(&addr), &s, 137, COLLECTION, now())
            .await
            .expect("reserve within budget");
        sessions.push(s);
    }
    let over = Uuid::new_v4().to_string();
    let err = tc
        .reserve_quota(MAX, &sender(&addr), &over, 137, COLLECTION, now())
        .await
        .expect_err("over quota");
    assert!(
        matches!(err, ApiError::QuotaReached(_)),
        "expected QuotaReached, got {err:?}"
    );
    assert_eq!(row_count(&pool, &addr).await, MAX);

    tc.release_reservation(&sessions[0]).await.expect("release");
    assert_eq!(row_count(&pool, &addr).await, MAX - 1);

    let refunded = Uuid::new_v4().to_string();
    tc.reserve_quota(MAX, &sender(&addr), &refunded, 137, COLLECTION, now())
        .await
        .expect("reserve after release");
    assert_eq!(row_count(&pool, &addr).await, MAX);

    // The window is the request clock's: a day later the same address is free again.
    let tomorrow = now() + chrono::Duration::seconds(super::QUOTA_WINDOW_SECS + 1);
    tc.reserve_quota(
        MAX,
        &sender(&addr),
        &Uuid::new_v4().to_string(),
        137,
        COLLECTION,
        tomorrow,
    )
    .await
    .expect("the quota counts the day before the bound clock, not now()");
    cleanup_user(&pool, &addr).await;
}

#[tokio::test]
async fn concurrent_reservations_never_exceed_the_limit() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let tc = Arc::new(component(pool.clone()));
    let addr = Arc::new(unique_address());
    const MAX: i64 = 3;
    const ATTEMPTS: i64 = 8;

    let tasks: Vec<_> = (0..ATTEMPTS)
        .map(|_| {
            let (tc, addr) = (tc.clone(), addr.clone());
            tokio::spawn(async move {
                tc.reserve_quota(
                    MAX,
                    &sender(&addr),
                    &Uuid::new_v4().to_string(),
                    137,
                    COLLECTION,
                    now(),
                )
                .await
            })
        })
        .collect();
    let (mut reserved, mut refused) = (0, 0);
    for task in tasks {
        match task.await.expect("task") {
            Ok(()) => reserved += 1,
            Err(ApiError::QuotaReached(msg)) => {
                assert!(msg.ends_with(&format!("Quota: {MAX}")), "{msg}");
                refused += 1;
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!(
        reserved, MAX,
        "exactly MAX reservations land under contention"
    );
    assert_eq!(refused, ATTEMPTS - MAX);
    assert_eq!(row_count(&pool, &addr).await, MAX);
    cleanup_user(&pool, &addr).await;
}

#[tokio::test]
async fn reservation_refuses_a_non_canonical_session_id() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let tc = component(pool.clone());
    let addr = unique_address();
    let err = tc
        .reserve_quota(
            10,
            &sender(&addr),
            "x'; DELETE FROM transactions; --",
            137,
            COLLECTION,
            now(),
        )
        .await
        .expect_err("a session id outside [A-Za-z0-9-] is refused before any SQL runs");
    assert!(matches!(err, ApiError::Internal(_)), "{err:?}");
    assert_eq!(row_count(&pool, &addr).await, 0);
}

#[tokio::test]
async fn reservations_are_isolated_per_user() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let tc = component(pool.clone());
    let alice = unique_address();
    let bob = unique_address();
    const MAX: i64 = 2;
    for _ in 0..MAX {
        tc.reserve_quota(
            MAX,
            &sender(&alice),
            &Uuid::new_v4().to_string(),
            137,
            COLLECTION,
            now(),
        )
        .await
        .expect("alice reserve");
    }
    assert!(matches!(
        tc.reserve_quota(
            MAX,
            &sender(&alice),
            &Uuid::new_v4().to_string(),
            137,
            COLLECTION,
            now()
        )
        .await,
        Err(ApiError::QuotaReached(_))
    ));
    for _ in 0..MAX {
        tc.reserve_quota(
            MAX,
            &sender(&bob),
            &Uuid::new_v4().to_string(),
            137,
            COLLECTION,
            now(),
        )
        .await
        .expect("bob reserve");
    }
    assert_eq!(row_count(&pool, &alice).await, MAX);
    assert_eq!(row_count(&pool, &bob).await, MAX);
    cleanup_user(&pool, &alice).await;
    cleanup_user(&pool, &bob).await;
}

const ESCROW: &str = "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4";
const CLAIM_COLLECTION: &str = "0x214ffc0f0103735728dc66b61a22e4f163e275ae";

async fn set_state(pool: &PgPool, table: &str, key: &str, status: &str, tx_hash: Option<&str>) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE shop.{table} SET status = $2, tx_hash = $3, updated_at = timezone('UTC', now()) - INTERVAL '1 hour' \
         WHERE idempotency_key = $1"
    )))
    .bind(key)
    .bind(status)
    .bind(tx_hash)
    .execute(pool)
    .await
    .expect("seed state");
}

async fn stored(pool: &PgPool, table: &str, key: &str) -> (String, bool) {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT status, updated_at > timezone('UTC', now()) - INTERVAL '1 minute' FROM shop.{table} WHERE idempotency_key = $1"
    )))
    .bind(key)
    .fetch_one(pool)
    .await
    .expect("stored row")
}

/// Every keyed broadcast claims its key the same way, so the three statements
/// are driven through one script: fresh claim, in-flight replay, settled
/// replay, error re-arm, and a second re-arm attempt that must find the row
/// already 'pending'.
async fn exercise<F, Fut>(pool: &PgPool, table: &str, claim: F) -> String
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Claim>,
{
    let key = Uuid::new_v4().to_string();
    let fresh = claim(key.clone()).await;
    assert_eq!(
        fresh,
        Claim {
            claimed: true,
            rearmed: false,
            status: None,
            tx_hash: None
        },
        "{table}: a fresh key inserts the pending row"
    );

    let replay = claim(key.clone()).await;
    assert!(!replay.claimed && !replay.rearmed, "{table}: {replay:?}");
    assert_eq!(replay.status(), "pending");
    assert_eq!(replay.tx_hash, None);

    set_state(pool, table, &key, "sent", Some("0xabc")).await;
    let settled = claim(key.clone()).await;
    assert_eq!(
        settled,
        Claim {
            claimed: false,
            rearmed: false,
            status: Some("sent".into()),
            tx_hash: Some("0xabc".into())
        },
        "{table}: a settled row replays its hash"
    );
    let (status, touched) = stored(pool, table, &key).await;
    assert_eq!(status, "sent");
    assert!(
        !touched,
        "{table}: a settled row is not rewritten by a replay"
    );

    set_state(pool, table, &key, "error", None).await;
    let rearmed = claim(key.clone()).await;
    assert_eq!(
        rearmed,
        Claim {
            claimed: false,
            rearmed: true,
            status: Some("error".into()),
            tx_hash: None
        },
        "{table}: an errored row is re-armed and reports its prior status"
    );
    let (status, touched) = stored(pool, table, &key).await;
    assert_eq!(status, "pending");
    assert!(touched, "{table}: the re-arm bumps updated_at");

    let again = claim(key.clone()).await;
    assert!(!again.claimed && !again.rearmed, "{table}: {again:?}");
    assert_eq!(again.status(), "pending");
    key
}

async fn delete_key(pool: &PgPool, table: &str, key: &str) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM shop.{table} WHERE idempotency_key = $1"
    )))
    .bind(key)
    .execute(pool)
    .await
    .expect("cleanup");
}

#[tokio::test]
async fn escrow_action_claims_cover_every_replay_state() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let key = exercise(&pool, "escrow_actions", |key| {
        let pool = pool.clone();
        async move {
            claim_escrow_action(
                &pool,
                &EscrowActionClaim {
                    key: &key,
                    action: "release",
                    collection: CLAIM_COLLECTION,
                    token_id: "7",
                    buyer: Some(BUYER),
                    escrow_address: ESCROW,
                    chain_id: 137,
                },
                now(),
            )
            .await
            .expect("claim")
        }
    })
    .await;
    delete_key(&pool, "escrow_actions", &key).await;
}

#[tokio::test]
async fn name_purchase_claims_cover_every_replay_state() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let key = exercise(&pool, "broker_purchases", |key| {
        let pool = pool.clone();
        async move {
            claim_name_purchase(
                &pool,
                &NamePurchaseClaim {
                    key: &key,
                    registrar: ESCROW,
                    name: Some("alice"),
                    token_id: None,
                    buyer_address: BUYER,
                    custody_address: BUYER,
                    price_wei: "100000000000000000000",
                    chain_id: 1,
                    mode: "name-mint",
                },
                now(),
            )
            .await
            .expect("claim")
        }
    })
    .await;
    delete_key(&pool, "broker_purchases", &key).await;
}

#[tokio::test]
async fn name_transfer_claims_cover_every_replay_state() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let key = exercise(&pool, "name_transfers", |key| {
        let pool = pool.clone();
        async move {
            claim_name_transfer(
                &pool,
                &NameTransferClaim {
                    key: &key,
                    registrar: ESCROW,
                    token_id: "7",
                    from_address: BUYER,
                    to_address: CLAIM_COLLECTION,
                    chain_id: 1,
                },
                now(),
            )
            .await
            .expect("claim")
        }
    })
    .await;
    delete_key(&pool, "name_transfers", &key).await;
}

/// The relay over the fake OZ relayer and the fake chain (the collections
/// mirror seeded with `COLLECTION`).
async fn oz_app(pool: &PgPool) -> axum::Router {
    let (chain_url, _) = spawn_fake_chain().await;
    let settings = Settings {
        oz_relayer_url: Some(spawn_fake_relayer().await),
        oz_relayer_id: Some("relayer-under-test".into()),
        oz_relayer_api_key: Some("api-key-under-test".into()),
        polygon_rpc_url: Some(chain_url),
        ..Settings::for_tests()
    };
    assert!(settings.has_relayer());
    router(state(pool.clone(), settings))
}

#[tokio::test]
async fn honestly_signed_meta_transactions_relay_for_both_overloads() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let app = oz_app(&pool).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());

    let (split_status, split_body) =
        post_transaction(&app, &user, COLLECTION, &signed_split_calldata(&key, FS)).await;
    let (combined_status, combined_body) =
        post_transaction(&app, &user, COLLECTION, &signed_combined_calldata(&key, FS)).await;
    let rows = row_count(&pool, &user).await;
    cleanup_user(&pool, &user).await;

    assert_eq!(
        split_status, 200,
        "an honestly-signed split-sig meta-transaction must relay, got {split_body}"
    );
    assert!(
        split_body["txHash"].as_str().unwrap().starts_with("0x"),
        "{split_body}"
    );
    assert_eq!(
        combined_status, 200,
        "an honestly-signed combined-sig meta-transaction must relay, got {combined_body}"
    );
    assert_eq!(
        rows, 2,
        "both honest relays are charged to the signing address"
    );
}

#[tokio::test]
async fn a_garbage_signature_is_rejected_and_charges_no_one() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let app = oz_app(&pool).await;
    let (status, body) =
        post_transaction(&app, VICTIM, COLLECTION, &split_sig_calldata(VICTIM)).await;
    assert_eq!(
        row_count(&pool, VICTIM).await,
        0,
        "a garbage-signed meta-transaction must consume no quota (status {status}, body {body})"
    );
    assert_eq!(status, 400, "body {body}");
    assert_eq!(body["error"], "invalid_transaction", "body {body}");
}

#[tokio::test]
async fn a_signature_from_another_key_cannot_burn_a_victims_quota() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let app = oz_app(&pool).await;
    let attacker = PrivateKeySigner::random();
    let victim: Address = VICTIM.parse().expect("victim address");
    let (status, body) = post_transaction(
        &app,
        VICTIM,
        COLLECTION,
        &signed_split_calldata_as(victim, &attacker, FS),
    )
    .await;
    assert_eq!(
        row_count(&pool, VICTIM).await,
        0,
        "a signature by another key must never consume {VICTIM}'s quota (status {status}, body {body})"
    );
    assert_eq!(
        row_count(&pool, &format!("{:#x}", attacker.address())).await,
        0,
        "the forged request must not be relayed at all"
    );
    assert_eq!(status, 400, "body {body}");
    assert_eq!(body["error"], "invalid_transaction", "body {body}");
}

#[tokio::test]
async fn a_from_that_disagrees_with_the_signed_user_address_is_rejected() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let app = oz_app(&pool).await;
    let attacker = PrivateKeySigner::random();
    let (status, body) = post_transaction(
        &app,
        VICTIM,
        COLLECTION,
        &signed_split_calldata(&attacker, FS),
    )
    .await;
    assert_eq!(
        row_count(&pool, VICTIM).await,
        0,
        "a `from` that disagrees with the signed userAddress must charge no one (status {status}, body {body})"
    );
    assert_eq!(
        row_count(&pool, &format!("{:#x}", attacker.address())).await,
        0,
        "the mismatched request must not be relayed"
    );
    assert_eq!(status, 400, "body {body}");
    assert_eq!(body["error"], "invalid_transaction", "body {body}");
}

#[tokio::test]
async fn undecodable_meta_tx_calldata_is_rejected() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let app = oz_app(&pool).await;
    let (status, body) =
        post_transaction(&app, VICTIM, COLLECTION, "0x0c53c51cffffffffffffffff").await;
    assert_eq!(
        row_count(&pool, VICTIM).await,
        0,
        "calldata whose userAddress cannot be recovered must never charge an address (status {status}, body {body})"
    );
    assert_eq!(status, 400, "body {body}");
    assert_eq!(body["error"], "invalid_transaction", "body {body}");
}

#[tokio::test]
async fn a_target_without_a_verifiable_domain_is_refused() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, CHAIN_UNVERIFIABLE_CONTRACT).await;
    let app = oz_app(&pool).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());
    let (status, body) = post_transaction(
        &app,
        &user,
        CHAIN_UNVERIFIABLE_CONTRACT,
        &signed_split_calldata(&key, FS),
    )
    .await;
    assert_eq!(
        row_count(&pool, &user).await,
        0,
        "an unverifiable target relays nothing (status {status}, body {body})"
    );
    assert_eq!(status, 400, "body {body}");
    assert_eq!(body["error"], "invalid_contract_address", "body {body}");
}

#[tokio::test]
async fn a_mirrored_collection_not_running_decentraland_code_is_refused() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, CHAIN_FOREIGN_CODE_CONTRACT).await;
    seed_collection(&pool, COLLECTION).await;
    let app = oz_app(&pool).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());
    let (status, body) = post_transaction(
        &app,
        &user,
        CHAIN_FOREIGN_CODE_CONTRACT,
        &signed_split_calldata(&key, FS),
    )
    .await;
    assert_eq!(
        row_count(&pool, &user).await,
        0,
        "unreviewed code relays nothing (status {status}, body {body})"
    );
    assert_eq!(status, 400, "body {body}");
    assert_eq!(body["error"], "invalid_contract_address", "body {body}");
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/contracts/{CHAIN_FOREIGN_CODE_CONTRACT}"),
        None,
        &[],
    )
    .await;
    assert_eq!(status, 404, "the mirror alone does not vouch for code");
    let (status, text) = send(
        &app,
        "GET",
        &format!("/v1/contracts/{COLLECTION}"),
        None,
        &[],
    )
    .await;
    assert_eq!(
        status, 200,
        "a minimal proxy over the recorded implementation is a collection: {text}"
    );
}

#[tokio::test]
async fn a_target_outside_the_allowlist_is_refused_only_after_a_complete_mirror_pass() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let unknown = unique_address();
    let app = oz_app(&pool).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());
    let calldata = signed_split_calldata(&key, FS);

    // The shared marker may already be set by a sibling test; the "no pass"
    // half runs only when this test finds it absent.
    let marker: Option<bool> = sqlx::query_scalar(
        "SELECT last_complete_pass_at IS NOT NULL FROM shop.mirror_state WHERE mirror = 'collections' AND network = 'matic'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    if marker != Some(true) {
        clear_mirror_pass(&pool).await;
        let (status, body) = post_transaction(&app, &user, &unknown, &calldata).await;
        assert_eq!(
            status, 503,
            "without a complete pass the mirror cannot say 'not a collection': {body}"
        );
        assert_eq!(body["error"], "unknown", "{body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("collections mirror"),
            "{body}"
        );
        let (status, text) =
            send(&app, "GET", &format!("/v1/contracts/{unknown}"), None, &[]).await;
        assert_eq!(status, 503, "{text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["source"], "collections-mirror", "{text}");
        assert_eq!(v["configure"], "MARKETPLACE_MIRRORS_ENABLED", "{text}");
    }
    mark_mirror_pass(&pool).await;
    let (status, body) = post_transaction(&app, &user, &unknown, &calldata).await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"], "invalid_contract_address", "{body}");
    assert_eq!(row_count(&pool, &user).await, 0);
    let (status, _) = send(&app, "GET", &format!("/v1/contracts/{unknown}"), None, &[]).await;
    assert_eq!(status, 404);
    seed_collection(&pool, COLLECTION).await;
    let (status, text) = send(
        &app,
        "GET",
        &format!("/v1/contracts/{COLLECTION}"),
        None,
        &[],
    )
    .await;
    assert_eq!((status, text.as_str()), (200, r#"{"ok":true}"#));
}

#[derive(Clone, Default)]
struct Captured {
    body: Vec<u8>,
    content_type: Option<String>,
    authorization: Option<String>,
}

#[derive(Clone)]
struct MockBehaviour {
    status: u16,
    body: &'static str,
    /// Set: the mock signals it once the request is recorded and never answers.
    stall: Option<Arc<tokio::sync::Notify>>,
}

type Seen = Arc<Mutex<Option<Captured>>>;

/// A stand-in for transactions-api.decentraland.org: one canned response on
/// POST /v1/transactions, recording what the forwarder actually sent.
async fn spawn_mock_upstream(behaviour: MockBehaviour, seen: Seen) -> String {
    async fn handle(
        axum::extract::State((behaviour, seen)): axum::extract::State<(MockBehaviour, Seen)>,
        headers: axum::http::HeaderMap,
        body: axum::body::Bytes,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        *seen.lock().unwrap() = Some(Captured {
            body: body.to_vec(),
            content_type: headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            authorization: headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(String::from),
        });
        if let Some(received) = &behaviour.stall {
            received.notify_one();
            std::future::pending::<()>().await;
        }
        (
            StatusCode::from_u16(behaviour.status).unwrap(),
            [("content-type", "application/json")],
            behaviour.body,
        )
            .into_response()
    }
    let app = axum::Router::new()
        .route("/v1/transactions", post(handle))
        .with_state((behaviour, seen));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock upstream");
    let addr = listener.local_addr().expect("mock upstream addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

/// No local broadcast provider and no chain, so the only route is the upstream forward.
fn upstream_app(pool: &PgPool, upstream_base: &str, timeout: Duration) -> axum::Router {
    let settings = Settings {
        transactions_upstream_url: Some(upstream_base.to_string()),
        transactions_upstream_timeout: timeout,
        ..Settings::for_tests()
    };
    router(state(pool.clone(), settings))
}

async fn post_with_auth(app: &axum::Router, body: &str) -> (u16, String) {
    send(
        app,
        "POST",
        "/v1/transactions",
        Some(body.to_string()),
        &[("authorization", "Bearer loki-local-secret")],
    )
    .await
}

#[tokio::test]
async fn upstream_200_passes_through_verbatim_and_confirms_the_slot() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    const UPSTREAM_BODY: &str = r#"{"ok":true,"txHash":"0x00000000000000000000000000000000000000000000000000000000000000aa","upstreamOnlyKey":true}"#;
    let seen: Seen = Arc::new(Mutex::new(None));
    let mock = spawn_mock_upstream(
        MockBehaviour {
            status: 200,
            body: UPSTREAM_BODY,
            stall: None,
        },
        seen.clone(),
    )
    .await;
    let app = upstream_app(&pool, &mock, Duration::from_secs(5));
    let buyer = unique_address();
    let body = tx_body(&buyer, COLLECTION, &split_sig_calldata(&buyer));
    let (status, text) = post_with_auth(&app, &body).await;
    let rows = row_count(&pool, &buyer).await;
    let hash = confirmed_hash(&pool, &buyer).await;
    let captured = seen.lock().unwrap().clone().expect("upstream was called");
    cleanup_user(&pool, &buyer).await;

    assert_eq!(status, 200, "body {text}");
    assert_eq!(
        text, UPSTREAM_BODY,
        "the upstream body must reach the client byte-for-byte, not re-wrapped"
    );
    assert_eq!(
        captured.body,
        body.as_bytes(),
        "the validated request body must be forwarded verbatim"
    );
    assert_eq!(
        captured.content_type.as_deref(),
        Some("application/json"),
        "the inbound content-type is preserved"
    );
    assert_eq!(
        captured.authorization, None,
        "node-local auth headers must never be forwarded upstream"
    );
    assert_eq!(rows, 1, "the relay consumed one quota slot");
    assert_eq!(
        hash.as_deref(),
        Some("0x00000000000000000000000000000000000000000000000000000000000000aa"),
        "the reservation is confirmed with the upstream txHash"
    );
}

#[tokio::test]
async fn upstream_400_passes_through_verbatim_and_refunds_the_slot() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    const UPSTREAM_BODY: &str = r#"{"ok":false,"message":"Invalid transaction data. Errors: [upstream-shaped]","code":"invalid_schema"}"#;
    let seen: Seen = Arc::new(Mutex::new(None));
    let mock = spawn_mock_upstream(
        MockBehaviour {
            status: 400,
            body: UPSTREAM_BODY,
            stall: None,
        },
        seen.clone(),
    )
    .await;
    let app = upstream_app(&pool, &mock, Duration::from_secs(5));
    let buyer = unique_address();
    let (status, text) = post_with_auth(
        &app,
        &tx_body(&buyer, COLLECTION, &split_sig_calldata(&buyer)),
    )
    .await;
    assert_eq!(status, 400, "body {text}");
    assert_eq!(
        text, UPSTREAM_BODY,
        "the upstream rejection must reach the client byte-for-byte"
    );
    assert_eq!(
        row_count(&pool, &buyer).await,
        0,
        "an upstream rejection broadcast nothing, so the quota slot is refunded"
    );
}

#[tokio::test]
async fn upstream_timeout_keeps_the_slot_even_when_the_broadcast_lands_late() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let seen: Seen = Arc::new(Mutex::new(None));
    let received = Arc::new(tokio::sync::Notify::new());
    let mock = spawn_mock_upstream(
        MockBehaviour {
            status: 200,
            body: r#"{"ok":true,"txHash":"0xtoolate"}"#,
            stall: Some(received.clone()),
        },
        seen.clone(),
    )
    .await;
    // The forward times out only once the test moves the clock past it, after the
    // upstream holds the request; on real time the bound is a hang guard.
    let upstream_timeout = Duration::from_secs(60);
    let app = upstream_app(&pool, &mock, upstream_timeout);
    let buyer = unique_address();
    let body = tx_body(&buyer, COLLECTION, &split_sig_calldata(&buyer));
    let mut post = std::pin::pin!(post_with_auth(&app, &body));
    tokio::select! {
        answered = &mut post => panic!("answered before the upstream held the request: {answered:?}"),
        _ = received.notified() => {}
    }
    tokio::time::pause();
    tokio::time::advance(upstream_timeout).await;
    tokio::time::resume();
    let (status, text) = post.await;
    let rows = row_count(&pool, &buyer).await;
    let hash = confirmed_hash(&pool, &buyer).await;
    let request_reached_upstream = seen.lock().unwrap().is_some();
    cleanup_user(&pool, &buyer).await;

    assert!(
        request_reached_upstream,
        "the request body went out, so the broadcast outcome is indeterminate"
    );
    assert_eq!(status, 504, "body {text}");
    let parsed: Value = serde_json::from_str(&text).expect("error envelope is JSON");
    assert_eq!(parsed["ok"], false, "body {text}");
    assert_eq!(parsed["error"], "unknown", "body {text}");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap_or_default()
            .contains("broadcast outcome is unknown"),
        "body {text}"
    );
    assert_eq!(
        rows, 1,
        "a timed-out forward may still have broadcast, so the quota slot stays consumed"
    );
    assert_eq!(
        hash, None,
        "the slot is kept but unconfirmed: no txHash was observed"
    );
}

#[tokio::test]
async fn upstream_connection_refused_refunds_the_slot() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind throwaway port");
    let refused = format!("http://{}", listener.local_addr().expect("addr"));
    drop(listener);
    let app = upstream_app(&pool, &refused, Duration::from_secs(5));
    let buyer = unique_address();
    let (status, text) = post_with_auth(
        &app,
        &tx_body(&buyer, COLLECTION, &split_sig_calldata(&buyer)),
    )
    .await;
    assert_eq!(status, 503, "body {text}");
    let parsed: Value = serde_json::from_str(&text).expect("error envelope is JSON");
    assert_eq!(parsed["ok"], false, "body {text}");
    assert_eq!(parsed["error"], "unknown", "body {text}");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap_or_default()
            .contains("could not be reached"),
        "body {text}"
    );
    assert_eq!(
        row_count(&pool, &buyer).await,
        0,
        "a refused connection provably sent nothing, so the quota slot is refunded"
    );
}

#[tokio::test]
async fn upstream_429_passes_through_verbatim_and_refunds_the_slot() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    const UPSTREAM_BODY: &str = r#"{"ok":false,"message":"Max amount of transactions reached for address","code":"quota_reached"}"#;
    let seen: Seen = Arc::new(Mutex::new(None));
    let mock = spawn_mock_upstream(
        MockBehaviour {
            status: 429,
            body: UPSTREAM_BODY,
            stall: None,
        },
        seen.clone(),
    )
    .await;
    let app = upstream_app(&pool, &mock, Duration::from_secs(5));
    let buyer = unique_address();
    let (status, text) = post_with_auth(
        &app,
        &tx_body(&buyer, COLLECTION, &split_sig_calldata(&buyer)),
    )
    .await;
    assert_eq!(status, 429, "body {text}");
    assert_eq!(
        text, UPSTREAM_BODY,
        "the upstream rate-limit rejection must reach the client byte-for-byte"
    );
    assert_eq!(
        row_count(&pool, &buyer).await,
        0,
        "an upstream 429 rejects before any broadcast, so the quota slot is refunded"
    );
}

#[tokio::test]
async fn upstream_524_keeps_the_slot_as_indeterminate() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    const UPSTREAM_BODY: &str = "<html>524: a timeout occurred</html>";
    let seen: Seen = Arc::new(Mutex::new(None));
    let mock = spawn_mock_upstream(
        MockBehaviour {
            status: 524,
            body: UPSTREAM_BODY,
            stall: None,
        },
        seen.clone(),
    )
    .await;
    let app = upstream_app(&pool, &mock, Duration::from_secs(5));
    let buyer = unique_address();
    let (status, text) = post_with_auth(
        &app,
        &tx_body(&buyer, COLLECTION, &split_sig_calldata(&buyer)),
    )
    .await;
    let rows = row_count(&pool, &buyer).await;
    let hash = confirmed_hash(&pool, &buyer).await;
    cleanup_user(&pool, &buyer).await;
    assert_eq!(status, 524, "body {text}");
    assert_eq!(
        text, UPSTREAM_BODY,
        "the intermediary error must reach the client byte-for-byte"
    );
    assert_eq!(
        rows, 1,
        "a Cloudflare origin-timeout is broadcast-indeterminate, so the slot stays consumed"
    );
    assert_eq!(
        hash, None,
        "the slot is kept but unconfirmed: no txHash was observed"
    );
}

/// An app whose only chain access is a direct signer against the fake chain
/// and whose pool never connects: it serves the endpoints that read the chain
/// without touching Postgres (payments config, nonce, balance).
async fn chain_only_app(with_signer: bool) -> (axum::Router, Option<PrivateKeySigner>, RawTxs) {
    let (chain_url, raws) = spawn_fake_chain().await;
    let mut settings = Settings {
        polygon_rpc_url: Some(chain_url),
        admin_token: Some(ADMIN.into()),
        ..Settings::for_tests()
    };
    let key = with_signer.then(PrivateKeySigner::random);
    if let Some(key) = &key {
        settings.meta_tx_broadcast_enabled = true;
        settings.relayer_private_key = Some(hex::encode(key.to_bytes()));
    }
    (router(state(lazy_pool(), settings)), key, raws)
}

async fn get_json(app: &axum::Router, path: &str) -> (u16, Value) {
    let (status, text) = send(app, "GET", path, None, &[]).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

#[tokio::test]
async fn balance_endpoint_reads_the_mana_contract_through_the_relayer_rpc() {
    let (app, key, _) = chain_only_app(true).await;
    let wallet = format!("{:#x}", PrivateKeySigner::random().address());
    let (status, body) = get_json(&app, &format!("/v1/payments/balance/{wallet}")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body,
        json!({ "balance": TEST_MANA_BALANCE_WEI.to_string() })
    );
    let (status, body) =
        get_json(&app, &format!("/v1/payments/balance/{CHAIN_EMPTY_WALLET}")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, json!({ "balance": "0" }));
    let (status, body) = get_json(&app, &format!("/v1/payments/nonce/{wallet}")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, json!({ "nonce": "0" }));
    let (status, body) = get_json(&app, "/v1/payments/config").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["enabled"], true);
    assert_eq!(body["payTo"], format!("{:#x}", key.unwrap().address()));
    assert_eq!(body["chainId"], 137);
}

#[tokio::test]
async fn balance_endpoint_rejects_a_malformed_address() {
    let (app, _, _) = chain_only_app(true).await;
    let (status, body) = get_json(&app, "/v1/payments/balance/not-an-address").await;
    assert_eq!(status, 400, "{body}");
}

#[tokio::test]
async fn balance_endpoint_is_unavailable_without_an_rpc_provider() {
    let (app, _, _) = chain_only_app(false).await;
    let (status, body) = get_json(
        &app,
        "/v1/payments/balance/0x951bb66ce4a5d4b1c667e386af5313753d14ba2e",
    )
    .await;
    assert_eq!(status, 503, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or("")
            .contains("cannot read the MANA balance"),
        "{body}"
    );
    let (status, body) = get_json(&app, "/v1/payments/config").await;
    assert_eq!(status, 200);
    assert_eq!(body["enabled"], false, "{body}");
}

#[tokio::test]
async fn the_write_gate_refuses_every_write_but_payments_verify() {
    let (chain_url, _) = spawn_fake_chain().await;
    let settings = Settings {
        polygon_rpc_url: Some(chain_url),
        data_writes_enabled: false,
        ..Settings::for_tests()
    };
    let app = router(state(lazy_pool(), settings));
    for path in [
        "/v1/transactions",
        "/v1/broker/buy",
        "/v1/escrow/reclaim",
        "/v1/admin/collections/approve-mana",
        "/v1/broker/names/transfer",
    ] {
        let (status, text) = send(&app, "POST", path, Some("{}".into()), &[]).await;
        assert_eq!(status, 503, "{path}: {text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("DATA_WRITES_ENABLED=false"),
            "{path}: {text}"
        );
    }
    // The relayer controls change process-local atomics only: they pass the
    // gate and stop at the admin bearer, so an operator can pause
    // broadcasting or pick the signer before writes are turned on.
    for path in ["/v1/admin/relayer/toggle", "/v1/admin/relayer/signer"] {
        let (status, text) = send(&app, "POST", path, Some("{}".into()), &[]).await;
        assert_eq!(status, 403, "{path}: {text}");
        assert!(
            text.contains("CATALYRST_ECONOMY_ADMIN_TOKEN is unset"),
            "{path}: {text}"
        );
    }
    // A read passes the gate and reaches the route (a chain read, no Postgres).
    let (status, _) = get_json(&app, "/v1/payments/config").await;
    assert_eq!(status, 200);
    // verify passes the gate; the admin bearer is the next door.
    let (status, text) = send(
        &app,
        "POST",
        "/v1/payments/verify",
        Some(json!({ "txHash": format!("0x{}", "ab".repeat(32)) }).to_string()),
        &[],
    )
    .await;
    assert_eq!(status, 403, "{text}");
    assert!(
        text.contains("CATALYRST_ECONOMY_ADMIN_TOKEN is unset"),
        "{text}"
    );
}

#[tokio::test]
async fn money_routes_need_the_admin_bearer() {
    let (app, _, _) = chain_only_app(true).await;
    let body = json!({ "collection": COLLECTION, "tokenId": "7" }).to_string();
    let (status, text) = send(&app, "POST", "/v1/escrow/reclaim", Some(body.clone()), &[]).await;
    assert_eq!(status, 403, "{text}");
    let (status, text) = send(
        &app,
        "POST",
        "/v1/escrow/reclaim",
        Some(body.clone()),
        &[("authorization", "Bearer wrong")],
    )
    .await;
    assert_eq!(status, 403, "{text}");
    let (status, text) = send(
        &app,
        "GET",
        "/v1/admin/relayer",
        None,
        &[("authorization", &format!("Bearer {ADMIN}"))],
    )
    .await;
    assert_eq!(status, 200, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["provisioned"]["direct"], true);
    assert_eq!(v["signerPreference"], "auto");
    // No LANDILER_ESCROW_ADDRESS: the escrow routes are unavailable, nothing is broadcast.
    let (status, text) = send(
        &app,
        "POST",
        "/v1/escrow/reclaim",
        Some(body),
        &[("authorization", &format!("Bearer {ADMIN}"))],
    )
    .await;
    assert_eq!(status, 503, "{text}");
}

/// The direct signer over the fake chain: the escrow reclaim broadcasts one
/// raw EIP-1559 transaction signed by the relayer key, targeted at the
/// pinned escrow with the reclaim calldata, and records the action; a replay
/// under the same idempotency key returns the recorded hash without a
/// second broadcast.
#[tokio::test]
async fn escrow_reclaim_signs_and_broadcasts_through_the_shared_client_and_replays_idempotently() {
    use alloy_consensus::{Transaction, TxEnvelope};
    use alloy_eips::eip2718::Decodable2718;

    let Some(pool) = scratch_pool().await else {
        return;
    };
    let (chain_url, raws) = spawn_fake_chain().await;
    let key = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(chain_url),
        admin_token: Some(ADMIN.into()),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(format!("0x{}", hex::encode(key.to_bytes()))),
        landiler_escrow_address: Some(ESCROW.into()),
        ..Settings::for_tests()
    };
    let app = router(state(pool.clone(), settings));
    let idem = Uuid::new_v4().to_string();
    let body = json!({ "collection": CLAIM_COLLECTION, "tokenId": "7", "idempotencyKey": idem })
        .to_string();
    let auth = format!("Bearer {ADMIN}");

    let (status, text) = send(
        &app,
        "POST",
        "/v1/escrow/reclaim",
        Some(body.clone()),
        &[("authorization", &auth)],
    )
    .await;
    assert_eq!(status, 200, "{text}");
    let first: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(first["ok"], true);
    assert_eq!(first["status"], "sent");
    let tx_hash = first["txHash"].as_str().unwrap().to_string();

    let sent = raws.lock().unwrap().clone();
    assert_eq!(
        sent.len(),
        1,
        "exactly one raw transaction reached the node"
    );
    let envelope = TxEnvelope::decode_2718(&mut sent[0].as_slice()).expect("a 2718 envelope");
    assert!(
        envelope.is_eip1559(),
        "the fake chain reports a base fee, so the signer builds a 1559 transaction"
    );
    assert_eq!(envelope.chain_id(), Some(137));
    assert_eq!(envelope.nonce(), 5, "the pending nonce the node reported");
    assert_eq!(
        envelope.gas_limit(),
        0x5208,
        "the estimate, under MAX_GAS_LIMIT"
    );
    assert_eq!(
        envelope.to(),
        Some(ESCROW.parse().unwrap()),
        "the reclaim targets the pinned escrow"
    );
    let calldata = super::abi::reclaimCall::abi_decode(envelope.input()).expect("reclaim calldata");
    assert_eq!(format!("{:#x}", calldata.collection), CLAIM_COLLECTION);
    assert_eq!(calldata.tokenId, U256::from(7u64));
    let signed = envelope.as_eip1559().expect("a 1559 envelope");
    let recovered = signed
        .signature()
        .recover_address_from_prehash(&signed.signature_hash())
        .unwrap();
    assert_eq!(recovered, key.address(), "signed by the relayer key");
    assert_eq!(
        format!("{:#x}", envelope.tx_hash()),
        tx_hash,
        "the answered hash is the signed transaction's"
    );

    let (status, text) = send(
        &app,
        "POST",
        "/v1/escrow/reclaim",
        Some(body),
        &[("authorization", &auth)],
    )
    .await;
    assert_eq!(status, 200, "{text}");
    let replay: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        replay["txHash"], tx_hash,
        "the replay returns the recorded hash"
    );
    assert_eq!(
        raws.lock().unwrap().len(),
        1,
        "a replay never re-broadcasts"
    );

    let (status, tx, chain_id): (String, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT status, tx_hash, chain_id FROM shop.escrow_actions WHERE idempotency_key = $1",
    )
    .bind(&idem)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (status.as_str(), tx.as_deref(), chain_id),
        ("sent", Some(tx_hash.as_str()), Some(137))
    );
    delete_key(&pool, "escrow_actions", &idem).await;
}

#[tokio::test]
async fn a_direct_relay_is_signed_locally_and_recorded_with_its_target() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let (chain_url, raws) = spawn_fake_chain().await;
    let relayer_key = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(chain_url),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(relayer_key.to_bytes())),
        ..Settings::for_tests()
    };
    let app = router(state(pool.clone(), settings));
    let user_key = PrivateKeySigner::random();
    let user = format!("{:#x}", user_key.address());
    let (status, body) = post_transaction(
        &app,
        &user,
        COLLECTION,
        &signed_split_calldata(&user_key, FS),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        raws.lock().unwrap().len(),
        1,
        "one raw transaction went out"
    );
    let hash = confirmed_hash(&pool, &user).await;
    assert_eq!(
        hash.as_deref(),
        body["txHash"].as_str(),
        "the reservation is confirmed with the broadcast hash"
    );
    let rows = super::transaction::TransactionComponent::new(
        pool.clone(),
        None,
        None,
        None,
        None,
        RuntimeConfig::new(),
    )
    .get_by_user_address(&user)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    // The relayer's own address can never be relayed for.
    let relayer = format!("{:#x}", relayer_key.address());
    let (status, body) = post_transaction(
        &app,
        &relayer,
        COLLECTION,
        &signed_split_calldata(&relayer_key, FS),
    )
    .await;
    assert_eq!(status, 400, "{body}");
    assert_eq!(row_count(&pool, &relayer).await, 0);
    cleanup_user(&pool, &user).await;
}

/// A real read over a real endpoint, only when `ECONOMY_LIVE_RPC_URL` names
/// a public Polygon JSON-RPC: the chain id and MANA's `getNonce` for a fresh
/// wallet, through the strict guard (no loopback admitted).
#[tokio::test]
async fn live_polygon_rpc_answers_the_chain_id_and_a_mana_nonce() {
    let Ok(url) = std::env::var("ECONOMY_LIVE_RPC_URL") else {
        eprintln!("ECONOMY_LIVE_RPC_URL unset: skipping the live RPC check");
        return;
    };
    let http = Outbound::new(crate::http::HttpClientCfg::default()).unwrap();
    let chain = super::chain::ChainComponent::new(http, &url, Duration::from_secs(20));
    assert_eq!(chain.get_chain_id().await.expect("eth_chainId"), 137);
    let mana = super::contracts_addrs::DclContracts::for_chain(137)
        .unwrap()
        .mana_token;
    let nonce = chain
        .get_meta_transaction_nonce(mana, PrivateKeySigner::random().address())
        .await
        .expect("getNonce");
    assert_eq!(
        nonce,
        Some(U256::ZERO),
        "a fresh wallet has meta-tx nonce 0 on MANA"
    );
    assert!(
        chain
            .get_domain_separator(mana)
            .await
            .expect("domainSeparator")
            .is_some(),
        "MANA reports its separator"
    );
}

const MANAGER: &str = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5";
const RULE_ESCROW: &str = "0x00000000000000000000000000000000000000e5";
const MANA: &str = "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4";

/// A mirrored item of `collection` plus the items mirror's complete-pass marker.
async fn seed_item(pool: &PgPool, collection: &str, item_id: u64, item_type: &str, creator: &str) {
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, $2::numeric, 137, $3, $4, $5, 'epic', 1, 10, 9, 2500000000000000000, 'Hat', 'hat', 1, 1, 1, $6) \
         ON CONFLICT (network, contract_address, item_id) DO NOTHING",
    )
    .bind(collection.to_lowercase())
    .bind(item_id.to_string())
    .bind(item_type)
    .bind(format!("urn:decentraland:matic:collections-v2:{}:{item_id}", collection.to_lowercase()))
    .bind(creator.to_lowercase())
    .bind(now())
    .execute(pool)
    .await
    .expect("seed item");
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) VALUES ('items', 'matic', 'test://items', $1) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at",
    )
    .bind(now())
    .execute(pool)
    .await
    .expect("items mirror pass marker");
}

async fn clear_items_pass(pool: &PgPool) {
    sqlx::query("DELETE FROM shop.mirror_state WHERE mirror = 'items' AND network = 'matic'")
        .execute(pool)
        .await
        .expect("clear items marker");
}

/// A trade signed by `key` over the Polygon off-chain marketplace's domain,
/// as the trades read serializes it: an ERC-721 order (3) or an item order (4).
fn signed_trade_json(
    key: &PrivateKeySigner,
    sent_type: u64,
    collection: Address,
    value: u64,
    amount: u128,
) -> Value {
    use crate::checkout::trade::{
        MANA_POLYGON, OFFCHAIN_MARKETPLACE_POLYGON, SolAsset, SolChecks, SolTrade, trade_digest,
    };
    let expiration_ms = chrono::Utc::now().timestamp_millis() + 3_600_000;
    let onchain = SolTrade {
        signer: key.address(),
        signature: Bytes::new(),
        checks: SolChecks {
            uses: U256::from(1),
            expiration: U256::from((expiration_ms / 1000) as u64),
            effective: U256::ZERO,
            salt: B256::ZERO,
            contractSignatureIndex: U256::ZERO,
            signerSignatureIndex: U256::ZERO,
            allowedRoot: B256::ZERO,
            allowedProof: vec![],
            externalChecks: vec![],
        },
        sent: vec![SolAsset {
            assetType: U256::from(sent_type),
            contractAddress: collection,
            value: U256::from(value),
            beneficiary: Address::ZERO,
            extra: Bytes::new(),
        }],
        received: vec![SolAsset {
            assetType: U256::from(1),
            contractAddress: MANA_POLYGON,
            value: U256::from(amount),
            beneficiary: key.address(),
            extra: Bytes::new(),
        }],
    };
    let signature = key
        .sign_hash_sync(&trade_digest(&onchain, 137, OFFCHAIN_MARKETPLACE_POLYGON))
        .expect("sign trade");
    let (kind, key_name) = if sent_type == 3 {
        ("public_nft_order", "tokenId")
    } else {
        ("public_item_order", "itemId")
    };
    json!({
        "id": Uuid::new_v4().to_string(), "signer": format!("{:#x}", key.address()), "signature": format!("0x{}", hex::encode(signature.as_bytes())), "type": kind,
        "checks": { "uses": 1, "expiration": expiration_ms, "effective": 0, "salt": "0x", "contractSignatureIndex": 0, "signerSignatureIndex": 0 },
        "sent": [{ "assetType": sent_type, "contractAddress": format!("{collection:#x}"), key_name: value.to_string() }],
        "received": [{ "assetType": 1, "contractAddress": MANA, "amount": amount.to_string(), "beneficiary": format!("{:#x}", key.address()) }],
    })
}

fn offchain_trade(
    signer: Address,
    sent_type: u64,
    collection: Address,
    value: u64,
) -> super::abi::offchain::Trade {
    use super::abi::offchain::{Asset, Checks, Trade};
    Trade {
        signer,
        signature: Bytes::new(),
        checks: Checks {
            uses: U256::from(1),
            expiration: U256::from(4_102_444_800u64),
            effective: U256::ZERO,
            salt: B256::ZERO,
            contractSignatureIndex: U256::ZERO,
            signerSignatureIndex: U256::ZERO,
            allowedRoot: B256::ZERO,
            allowedProof: vec![],
            externalChecks: vec![],
        },
        sent: vec![Asset {
            assetType: U256::from(sent_type),
            contractAddress: collection,
            value: U256::from(value),
            beneficiary: Address::ZERO,
            extra: Bytes::new(),
        }],
        received: vec![Asset {
            assetType: U256::from(2),
            contractAddress: MANA.parse().unwrap(),
            value: U256::from(5_000_000_000_000_000_000u128),
            beneficiary: signer,
            extra: Bytes::new(),
        }],
    }
}

fn use_credits(target: Address, selector: [u8; 4], data: Vec<u8>, max_uncredited: U256) -> Vec<u8> {
    let ten_mana = U256::from(10_000_000_000_000_000_000u128);
    crate::policy_entry::encode_use_credits(
        target,
        selector,
        data,
        ten_mana,
        max_uncredited,
        ten_mana,
    )
}

async fn broker_buy(app: &axum::Router, pool: &PgPool, body: Value) -> (u16, Value, i64) {
    let bearer = format!("Bearer {ADMIN}");
    let key = format!("guard-{}", Uuid::new_v4());
    let (status, text) = send(
        app,
        "POST",
        "/v1/broker/buy",
        Some(body.to_string()),
        &[("authorization", &bearer), ("idempotency-key", &key)],
    )
    .await;
    let claimed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM shop.broker_purchases WHERE idempotency_key = $1")
            .bind(&key)
            .fetch_one(pool)
            .await
            .unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or_default(),
        claimed,
    )
}

fn refused(status: u16, body: &Value, code: &str, what: &str) {
    assert_eq!(status, 422, "{what}: {body}");
    assert_eq!(body["error"], "payment_rule_refused", "{what}: {body}");
    assert_eq!(body["refusal"]["code"], code, "{what}: {body}");
}

/// `POST /v1/transactions` decodes a `useCredits` to a CreditsManager and
/// judges the call it executes: every resale spelling is refused before the
/// quota is touched, a mint of a mirrored wearable relays.
#[tokio::test]
async fn the_relay_refuses_use_credits_on_a_resale_a_crafted_batch_a_hybrid_resale_and_a_custom_call()
 {
    use super::abi::{ItemToBuy, buyCall, executeOrderCall, offchain};
    use crate::policy_entry::{
        EXECUTOR_EXECUTE_SELECTOR, call_args, manager_routes, passthrough_executors,
    };
    use crate::types::ChainId;
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    seed_collection(&pool, MANAGER).await;
    seed_item(&pool, COLLECTION, 4, "wearable_v2", VICTIM).await;
    let app = oz_app(&pool).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());
    let collection: Address = COLLECTION.parse().unwrap();
    let creator: Address = VICTIM.parse().unwrap();
    let routes = manager_routes(ChainId::POLYGON).unwrap();
    let one_mana = U256::from(1_000_000_000_000_000_000u128);
    let relay = |inner: Vec<u8>| {
        let app = &app;
        let key = &key;
        let user = user.clone();
        async move { post_transaction(app, &user, MANAGER, &signed_split_calldata(key, &inner)).await }
    };
    let accept =
        |trades: Vec<offchain::Trade>| call_args(&offchain::acceptCall { _trades: trades });

    let (status, body) = relay(use_credits(
        routes.marketplaces[0],
        offchain::acceptCall::SELECTOR,
        accept(vec![offchain_trade(key.address(), 3, collection, 42)]),
        U256::ZERO,
    ))
    .await;
    refused(
        status,
        &body,
        "creditsNotAllowed",
        "an ERC-721 resale through accept",
    );
    let (status, body) = relay(use_credits(
        routes.marketplaces[0],
        offchain::acceptCall::SELECTOR,
        accept(vec![offchain_trade(creator, 3, collection, 42)]),
        U256::ZERO,
    ))
    .await;
    refused(
        status,
        &body,
        "creditsNotAllowed",
        "the creator reselling a token already minted",
    );
    let (status, body) = relay(use_credits(
        routes.marketplaces[0],
        offchain::acceptCall::SELECTOR,
        accept(vec![offchain_trade(creator, 3, collection, 42)]),
        one_mana,
    ))
    .await;
    refused(
        status,
        &body,
        "creditsNotAllowed",
        "credits plus a MANA remainder on a resale",
    );
    let (status, body) = relay(use_credits(
        routes.marketplaces[2],
        offchain::acceptCall::SELECTOR,
        accept(vec![
            offchain_trade(creator, 4, collection, 4),
            offchain_trade(creator, 3, collection, 42),
        ]),
        U256::ZERO,
    ))
    .await;
    refused(
        status,
        &body,
        "creditsNotAllowed",
        "a crafted batch: a resale beside an item order",
    );
    let order = executeOrderCall {
        nftAddress: collection,
        assetId: U256::from(42),
        price: one_mana,
    };
    let (status, body) = relay(use_credits(
        routes.legacy_marketplace,
        executeOrderCall::SELECTOR,
        call_args(&order),
        U256::ZERO,
    ))
    .await;
    refused(status, &body, "creditsNotAllowed", "a legacy executeOrder");
    let (status, body) = relay(use_credits(
        passthrough_executors(ChainId::POLYGON)[0],
        EXECUTOR_EXECUTE_SELECTOR,
        vec![0xde, 0xad, 0xbe, 0xef],
        U256::ZERO,
    ))
    .await;
    refused(
        status,
        &body,
        "undecodedCustomCall",
        "a custom call to the NAME executor (a route to LAND or a resale)",
    );
    assert_eq!(
        row_count(&pool, &user).await,
        0,
        "a refused useCredits consumes no relay quota"
    );

    let buy = buyCall {
        itemsToBuy: vec![ItemToBuy {
            collection,
            ids: vec![U256::from(4)],
            prices: vec![one_mana],
            beneficiaries: vec![key.address()],
        }],
    };
    let (status, body) = relay(use_credits(
        routes.collection_store,
        buyCall::SELECTOR,
        call_args(&buy),
        U256::ZERO,
    ))
    .await;
    assert_eq!(
        status, 200,
        "a CollectionStore mint of a mirrored wearable relays: {body}"
    );
    let (status, body) = relay(use_credits(
        routes.marketplaces[2],
        offchain::acceptCall::SELECTOR,
        accept(vec![offchain_trade(creator, 4, collection, 4)]),
        one_mana,
    ))
    .await;
    assert_eq!(
        status, 200,
        "a creator item order paid credits plus MANA from the wallet relays: {body}"
    );
    assert_eq!(row_count(&pool, &user).await, 2);

    clear_items_pass(&pool).await;
    let (status, body) = relay(use_credits(
        routes.collection_store,
        buyCall::SELECTOR,
        call_args(&buy),
        U256::ZERO,
    ))
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (503, Some("shop-collection-items")),
        "no items mirror pass: unavailable, never re-priced: {body}"
    );
    assert_eq!(row_count(&pool, &user).await, 2);
    cleanup_user(&pool, &user).await;
}

/// `POST /v1/broker/buy` is the credit-funded lane: a resale (an order-book
/// token, an ERC-721 trade) is refused before the claim and the broadcast;
/// a mint is admitted once the mirrors describe it.
#[tokio::test]
async fn the_broker_refuses_a_resale_before_any_broadcast_and_admits_a_mirrored_mint() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let (chain_url, raws) = spawn_fake_chain().await;
    let relayer_key = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(chain_url),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(relayer_key.to_bytes())),
        admin_token: Some(ADMIN.into()),
        ..Settings::for_tests()
    };
    let app = router(state(pool.clone(), settings));
    let base = json!({ "collection": COLLECTION, "priceWei": "1000000000000000000", "escrowAddress": RULE_ESCROW, "buyerAddress": BUYER });
    let with = |extra: Value| {
        let mut body = base.clone();
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        body
    };
    let broadcasts = || raws.lock().unwrap().len();

    let (status, body, claimed) = broker_buy(
        &app,
        &pool,
        with(json!({ "mode": "secondary", "tokenId": "7" })),
    )
    .await;
    refused(status, &body, "creditsNotAllowed", "an order-book resale");
    assert_eq!(
        (claimed, broadcasts()),
        (0, 0),
        "nothing claimed, nothing broadcast"
    );

    let seller = PrivateKeySigner::random();
    let collection: Address = COLLECTION.parse().unwrap();
    let resale = signed_trade_json(&seller, 3, collection, 7, 1_000_000_000_000_000_000);
    let (status, body, claimed) = broker_buy(
        &app,
        &pool,
        with(json!({ "mode": "trade", "trade": resale })),
    )
    .await;
    refused(
        status,
        &body,
        "creditsNotAllowed",
        "an ERC-721 trade, whoever signed it",
    );
    assert_eq!((claimed, broadcasts()), (0, 0));

    let (status, body, claimed) = broker_buy(
        &app,
        &pool,
        with(json!({ "mode": "primary", "itemId": "4" })),
    )
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (503, Some("shop-collection-items")),
        "no items mirror pass: the mint is unavailable, never assumed: {body}"
    );
    assert_eq!((claimed, broadcasts()), (0, 0));

    let creator = PrivateKeySigner::random();
    seed_item(
        &pool,
        COLLECTION,
        4,
        "wearable_v2",
        &format!("{:#x}", creator.address()),
    )
    .await;
    let (status, body, claimed) = broker_buy(
        &app,
        &pool,
        with(json!({ "mode": "primary", "itemId": "4" })),
    )
    .await;
    assert_ne!(status, 422, "{body}");
    assert_eq!(
        (claimed, broadcasts()),
        (1, 1),
        "the rule admitted the mint: claimed and broadcast ({status} {body})"
    );
    let order = signed_trade_json(&creator, 4, collection, 4, 1_000_000_000_000_000_000);
    let (status, body, claimed) = broker_buy(
        &app,
        &pool,
        with(json!({ "mode": "trade", "trade": order })),
    )
    .await;
    assert_ne!(status, 422, "{body}");
    assert_eq!(
        (claimed, broadcasts()),
        (1, 2),
        "a creator item order is a primary: claimed and broadcast ({status} {body})"
    );
    clear_items_pass(&pool).await;
}

/// `POST /v1/broker/names/buy`: a registered NAME is a resale (MANA only),
/// the registration of an available NAME a primary the ledger may fund.
#[tokio::test]
async fn the_names_broker_refuses_a_registered_name_resale_and_registers_an_available_name() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let (chain_url, raws) = spawn_fake_chain().await;
    let relayer_key = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(chain_url.clone()),
        eth_rpc_url: Some(chain_url),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(relayer_key.to_bytes())),
        admin_token: Some(ADMIN.into()),
        ..Settings::for_tests()
    };
    let app = router(state(pool.clone(), settings));
    let bearer = format!("Bearer {ADMIN}");
    let name_buy = |body: Value| {
        let app = &app;
        let bearer = bearer.clone();
        async move {
            let key = format!("names-{}", Uuid::new_v4());
            let (status, text) = send(
                app,
                "POST",
                "/v1/broker/names/buy",
                Some(body.to_string()),
                &[("authorization", &bearer), ("idempotency-key", &key)],
            )
            .await;
            (
                status,
                serde_json::from_str::<Value>(&text).unwrap_or_default(),
            )
        }
    };
    let taken = crate::types::NameLabel::parse(REGISTERED_NAME)
        .unwrap()
        .token_id()
        .to_string();

    let (status, body) = name_buy(json!({ "mode": "secondary", "tokenId": taken, "priceWei": "5000000000000000000", "buyerAddress": BUYER })).await;
    refused(
        status,
        &body,
        "creditsNotAllowed",
        "a NAME resale on the legacy marketplace",
    );
    let (status, body) =
        name_buy(json!({ "mode": "mint", "name": REGISTERED_NAME, "buyerAddress": BUYER })).await;
    refused(
        status,
        &body,
        "nameTaken",
        "registering a NAME the registrar already holds",
    );
    assert_eq!(raws.lock().unwrap().len(), 0, "nothing was broadcast");

    let (status, body) =
        name_buy(json!({ "mode": "mint", "name": "nacho", "buyerAddress": BUYER })).await;
    assert_eq!(status, 200, "an available NAME registers: {body}");
    assert_eq!(raws.lock().unwrap().len(), 1);
}
