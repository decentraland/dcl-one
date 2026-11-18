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
    BrokerPurchaseClaim, Claim, EscrowActionClaim, NameTransferClaim, claim_broker_purchase,
    claim_escrow_action, claim_name_transfer,
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

/// MANA payments the fake chain reports by transaction hash: `(from, to)`.
static PAYMENTS: std::sync::LazyLock<Mutex<std::collections::HashMap<String, (Address, Address)>>> =
    std::sync::LazyLock::new(Default::default);

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
                json!({ "number": "0x1", "baseFeePerGas": "0x3b9aca00", "timestamp": "0x6553f100" }),
            )),
            "eth_blockNumber" => Json(ok(json!("0x50"))),
            "eth_sendRawTransaction" => {
                let raw = req["params"][0].as_str().unwrap_or("");
                let bytes = hex::decode(raw.trim_start_matches("0x")).unwrap_or_default();
                let hash = alloy_primitives::keccak256(&bytes);
                seen.lock().unwrap().push(bytes);
                Json(ok(json!(format!("{hash:#x}"))))
            }
            "eth_getTransactionReceipt" => {
                let hash = req["params"][0].as_str().unwrap_or("").to_lowercase();
                let paid = PAYMENTS.lock().unwrap().get(&hash).copied();
                match paid {
                    Some((from, to)) => Json(ok(json!({
                        "status": "0x1",
                        "blockNumber": "0x10",
                        "logs": [{
                            "address": MANA,
                            "topics": [
                                format!("0x{}", hex::encode(alloy_primitives::keccak256("Transfer(address,address,uint256)"))),
                                format!("0x{:0>64}", hex::encode(from)),
                                format!("0x{:0>64}", hex::encode(to)),
                            ],
                            "data": format!("0x{:064x}", 7_000_000_000_000_000_000u128),
                        }],
                    }))),
                    None => Json(ok(json!({ "status": "0x1", "logs": [] }))),
                }
            }
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
async fn the_free_call_budget_never_holds_back_a_sale() {
    use axum::response::IntoResponse;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let tc = component(pool.clone());
    // A day no other test reserves in.
    let at = chrono::NaiveDate::from_ymd_opt(2100, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let addrs = [unique_address(), unique_address(), unique_address()];
    let free = |a: &str, budget: i64| {
        let (tc, a) = (&tc, a.to_string());
        async move {
            tc.reserve_free_call(
                10,
                budget,
                &sender(&a),
                &Uuid::new_v4().to_string(),
                137,
                COLLECTION,
                at,
            )
            .await
        }
    };
    free(&addrs[0], 2).await.expect("first free call");
    free(&addrs[1], 2).await.expect("second free call");
    let err = free(&addrs[2], 2).await.unwrap_err();
    assert!(matches!(err, ApiError::RelayBudgetSpent(_)), "got {err:?}");
    assert_eq!(
        err.into_response().status(),
        axum::http::StatusCode::TOO_MANY_REQUESTS
    );
    tc.reserve_quota(
        10,
        &sender(&addrs[2]),
        &Uuid::new_v4().to_string(),
        137,
        COLLECTION,
        at,
    )
    .await
    .expect("a sale still relays when the free-call budget is spent");
    free(&addrs[2], 0).await.expect("0 is no limit");
    sqlx::query("DELETE FROM economy.transactions WHERE user_address = ANY($1)")
        .bind(addrs.iter().map(|a| a.to_lowercase()).collect::<Vec<_>>())
        .execute(&pool)
        .await
        .unwrap();
}

#[test]
fn sales_and_credits_spends_are_paid_calls() {
    let settings = Settings::for_tests();
    let call = |to: &str, data: String| super::transaction::TransactionData {
        from: VICTIM.to_string(),
        params: vec![to.to_string(), data],
    };
    let credits_manager = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5";
    let meta = |inner: Vec<u8>| {
        format!(
            "0x{}",
            hex::encode(
                executeMetaTransactionCall {
                    userAddress: VICTIM.parse().unwrap(),
                    functionSignature: inner.into(),
                    sigR: Default::default(),
                    sigS: Default::default(),
                    sigV: 27,
                }
                .abi_encode()
            )
        )
    };
    let mut use_credits = crate::policy_entry::USE_CREDITS_SELECTOR.to_vec();
    use_credits.extend([0u8; 32]);
    assert!(super::transaction::is_paid_call(
        &settings,
        &call(credits_manager, meta(use_credits.clone()))
    ));
    // The checkout client relays credits spends in the combined-signature form.
    let combined = format!(
        "0x{}",
        hex::encode(
            super::abi::combined_sig::executeMetaTransactionCall {
                userAddress: VICTIM.parse().unwrap(),
                functionData: use_credits.into(),
                signature: vec![0u8; 65].into(),
            }
            .abi_encode()
        )
    );
    assert!(
        super::transaction::is_paid_call(&settings, &call(credits_manager, combined)),
        "a combined-signature useCredits is a credits spend"
    );
    assert!(
        !super::transaction::is_paid_call(
            &settings,
            &call(credits_manager, meta(vec![0xde, 0xad, 0xbe, 0xef]))
        ),
        "the manager's other functions are free calls"
    );
    assert!(
        !super::transaction::is_paid_call(&settings, &call(COLLECTION, "0xa22cb465".into())),
        "an approval on a collection is a free call"
    );
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
async fn broker_purchase_claims_cover_every_replay_state() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let key = exercise(&pool, "broker_purchases", |key| {
        let pool = pool.clone();
        async move {
            claim_broker_purchase(
                &pool,
                &BrokerPurchaseClaim {
                    key: &key,
                    collection: ESCROW,
                    item_id: Some("alice"),
                    token_id: None,
                    buyer_address: BUYER,
                    escrow_address: BUYER,
                    price_wei: "100000000000000000000",
                    chain_id: 1,
                    mode: "name-mint",
                    trade_hashed_signature: None,
                    usd_amount_wei: None,
                    mana_usd_rate_wei: None,
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
    assert_eq!(
        parsed["not_sent"], true,
        "the forward never reached the upstream: nothing was sent"
    );
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

/// An upstream's 500 or 503 can follow its send. It passes through as it is, never marked as
/// evidence that nothing was sent, and the slot is kept.
#[tokio::test]
async fn an_upstream_500_or_503_passes_through_unmarked_and_keeps_the_slot() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    const UPSTREAM_BODY: &str = r#"{"ok":false,"message":"The relayer service is temporarily unavailable.","code":"unknown"}"#;
    for upstream_status in [500u16, 503] {
        let seen: Seen = Arc::new(Mutex::new(None));
        let mock = spawn_mock_upstream(
            MockBehaviour {
                status: upstream_status,
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
        cleanup_user(&pool, &buyer).await;
        assert_eq!(status, upstream_status, "{text}");
        assert_eq!(
            text, UPSTREAM_BODY,
            "the body passes through byte-for-byte, unmarked"
        );
        assert_eq!(
            rows, 1,
            "upstream {upstream_status} is indeterminate: the slot stays consumed"
        );
    }
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
/// A confirmed payment reports where and how deep it was mined.
#[tokio::test]
async fn verify_reports_the_payment_block_and_depth() {
    let (app, key, _) = chain_only_app(true).await;
    let relayer = key.unwrap().address();
    let payer = Address::repeat_byte(0x22);
    let hash = format!("0x{}", "c4".repeat(32));
    PAYMENTS
        .lock()
        .unwrap()
        .insert(hash.clone(), (payer, relayer));
    let (status, text) = send(
        &app,
        "POST",
        "/v1/payments/verify",
        Some(json!({ "txHash": hash }).to_string()),
        &[("authorization", &format!("Bearer {ADMIN}"))],
    )
    .await;
    assert_eq!(status, 200, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "confirmed");
    assert_eq!(v["from"], format!("{payer:#x}"));
    assert_eq!(v["valueWei"], "7000000000000000000");
    assert_eq!(v["blockNumber"], 16);
    assert_eq!(v["blockTimestamp"], 0x6553f100);
    assert_eq!(v["confirmations"], 0x50 - 16);
}

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

/// A relay that broadcast but could not record the hash on the buyer's quota reservation still
/// answers the hash: an error would tell the client nothing was sent, and it would sign again
/// while this transaction lands. The reservation stays and still counts against the quota.
#[tokio::test]
async fn a_broadcast_whose_hash_cannot_be_recorded_still_answers_the_hash() {
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
    // Recording a hash fails for this buyer only: the scratch database is shared.
    let name = format!("refuse_hash_{}", &user[2..14]);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION economy.{name}() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.tx_hash IS NOT NULL AND lower(NEW.user_address) = '{user}' THEN \
         RAISE EXCEPTION 'recording refused'; END IF; RETURN NEW; END $$; \
         CREATE TRIGGER {name} BEFORE UPDATE ON economy.transactions \
         FOR EACH ROW EXECUTE FUNCTION economy.{name}();"
    )))
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = post_transaction(
        &app,
        &user,
        COLLECTION,
        &signed_split_calldata(&user_key, FS),
    )
    .await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {name} ON economy.transactions; DROP FUNCTION economy.{name}();"
    )))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["ok"], true, "{body}");
    let sent = raws.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "one raw transaction went out");
    assert_eq!(
        body["txHash"].as_str(),
        Some(format!("{:#x}", alloy_primitives::keccak256(&sent[0])).as_str()),
        "the answer carries the broadcast hash"
    );
    assert_eq!(confirmed_hash(&pool, &user).await, None);
    assert_eq!(
        row_count(&pool, &user).await,
        1,
        "the reservation still counts against the quota"
    );
    cleanup_user(&pool, &user).await;
}

/// What a JSON-RPC endpoint in front of the fake chain does to one call; the rest pass through.
#[derive(Clone, Copy)]
enum RpcFault {
    /// The send reaches the node (the fake chain records it), and the caller gets a gateway 502.
    LoseTheSendAnswer,
    /// The send reaches the node, and the caller gets a JSON-RPC revert error, as from a gateway
    /// that retried a send the node had already taken, or a node that simulates.
    AnswerTheSendWithARevert,
    /// The relayer's own gas estimation reverts (the buyer's simulation passes): the relay has
    /// reserved the slot and refuses before it signs or sends anything.
    RevertTheRelayersEstimate,
}

async fn spawn_faulty_rpc(chain_url: String, fault: RpcFault, relayer: Address) -> String {
    async fn handle(
        axum::extract::State((chain, fault, relayer)): axum::extract::State<(
            String,
            RpcFault,
            String,
        )>,
        body: axum::body::Bytes,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        let req = serde_json::from_slice::<Value>(&body).unwrap_or_default();
        let revert = Json(json!({
            "jsonrpc": "2.0",
            "id": req["id"],
            "error": { "code": 3, "message": "execution reverted" }
        }));
        let method = req["method"].as_str().unwrap_or("");
        let from = req["params"][0]["from"]
            .as_str()
            .unwrap_or("")
            .to_lowercase();
        if method == "eth_estimateGas"
            && matches!(fault, RpcFault::RevertTheRelayersEstimate)
            && from == relayer
        {
            return revert.into_response();
        }
        let answer = reqwest::Client::new()
            .post(&chain)
            .header("content-type", "application/json")
            .body(body.to_vec())
            .send()
            .await
            .expect("the fake chain answers");
        if method == "eth_sendRawTransaction" {
            match fault {
                RpcFault::LoseTheSendAnswer => {
                    return (StatusCode::BAD_GATEWAY, "<html>502 Bad Gateway</html>")
                        .into_response();
                }
                RpcFault::AnswerTheSendWithARevert => return revert.into_response(),
                RpcFault::RevertTheRelayersEstimate => {}
            }
        }
        let status = StatusCode::from_u16(answer.status().as_u16()).unwrap();
        let text = answer.bytes().await.expect("the fake chain's body");
        (status, [("content-type", "application/json")], text).into_response()
    }
    let app = axum::Router::new().route("/", post(handle)).with_state((
        chain_url,
        fault,
        format!("{relayer:#x}"),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind faulty rpc");
    let addr = listener.local_addr().expect("faulty rpc addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

/// One relayed purchase through a direct signer whose RPC has `fault`: the answer, how many raw
/// transactions reached the node, and the buyer's reservation (rows, recorded hash).
async fn relay_through(fault: RpcFault) -> ((u16, Value), usize, i64, Option<String>) {
    let pool = scratch_pool().await.expect("a scratch database");
    seed_collection(&pool, COLLECTION).await;
    let (chain_url, raws) = spawn_fake_chain().await;
    let relayer_key = PrivateKeySigner::random();
    let rpc = spawn_faulty_rpc(chain_url, fault, relayer_key.address()).await;
    let settings = Settings {
        polygon_rpc_url: Some(rpc),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(relayer_key.to_bytes())),
        ..Settings::for_tests()
    };
    let app = router(state(pool.clone(), settings));
    let user_key = PrivateKeySigner::random();
    let user = format!("{:#x}", user_key.address());
    let answer = post_transaction(
        &app,
        &user,
        COLLECTION,
        &signed_split_calldata(&user_key, FS),
    )
    .await;
    let sent = raws.lock().unwrap().len();
    let rows = row_count(&pool, &user).await;
    let hash = if rows > 0 {
        confirmed_hash(&pool, &user).await
    } else {
        None
    };
    cleanup_user(&pool, &user).await;
    (answer, sent, rows, hash)
}

/// The node took the transaction, but its answer to the send was lost. That is no evidence the
/// transaction was never sent: the relay answers 504 with no hash, never a refusal (a 4xx, which
/// the client reads as "sign again"), and the buyer's quota slot stays consumed.
#[tokio::test]
async fn a_send_the_node_took_but_whose_answer_was_lost_is_never_a_refusal() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    if scratch_pool().await.is_none() {
        return;
    }
    let ((status, body), sent, rows, hash) = relay_through(RpcFault::LoseTheSendAnswer).await;
    assert_eq!(sent, 1, "the node holds the transaction");
    assert_eq!(status, 504, "{body}");
    assert_eq!(body["ok"], false, "{body}");
    assert!(body.get("txHash").is_none(), "{body}");
    assert!(body.get("not_sent").is_none(), "{body}");
    assert_eq!(rows, 1, "the reservation still counts against the quota");
    assert_eq!(hash, None, "no hash was observed");
}

/// A JSON-RPC error answered to the send (here a revert) comes after the send was attempted, and
/// the node may hold the transaction: it is a 504 that keeps the slot, never the 400 a client reads
/// as "never sent".
#[tokio::test]
async fn an_error_answered_to_the_send_is_never_a_refusal() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    if scratch_pool().await.is_none() {
        return;
    }
    let ((status, body), sent, rows, _) = relay_through(RpcFault::AnswerTheSendWithARevert).await;
    assert_eq!(sent, 1, "the node holds the transaction");
    assert_eq!(status, 504, "{body}");
    assert_eq!(body["ok"], false, "{body}");
    assert!(body.get("not_sent").is_none(), "{body}");
    assert_eq!(rows, 1, "the reservation still counts against the quota");
}

/// The control: a refusal proven before any send (the relayer's gas estimation reverts, after the
/// slot was reserved) is still a 400 that releases the slot, and nothing reaches the node.
#[tokio::test]
async fn a_refusal_before_any_send_is_still_a_4xx_that_frees_the_slot() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    if scratch_pool().await.is_none() {
        return;
    }
    let ((status, body), sent, rows, _) = relay_through(RpcFault::RevertTheRelayersEstimate).await;
    assert_eq!(sent, 0, "nothing reached the node");
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["not_sent"], true, "{body}");
    assert_eq!(rows, 0, "the slot is released");
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

/// dcl-one's own Amoy manager is guarded like the shared ones: a relayed
/// `executeMetaTransaction(useCredits)` to it is decoded on Amoy's routes and
/// judged, so a resale is refused and a mint of a mirrored wearable passes. On
/// Polygon the same address is no CreditsManager, so nothing is judged there.
#[tokio::test]
async fn the_relay_guards_use_credits_to_dcl_ones_amoy_manager() {
    use super::abi::{ItemToBuy, buyCall, offchain};
    use crate::guards::relay::guard_meta_transaction;
    use crate::policy_entry::{
        DCL_ONE_AMOY_MANAGER, call_args, is_credits_manager, manager_routes,
    };
    use crate::types::ChainId;
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    seed_item(&pool, COLLECTION, 4, "wearable_v2", VICTIM).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());
    let manager = format!("{DCL_ONE_AMOY_MANAGER:#x}");
    let collection: Address = COLLECTION.parse().unwrap();
    let routes = manager_routes(ChainId::AMOY).unwrap();
    let one_mana = U256::from(1_000_000_000_000_000_000u128);

    let resale = signed_split_calldata(
        &key,
        &use_credits(
            routes.marketplaces[2],
            offchain::acceptCall::SELECTOR,
            call_args(&offchain::acceptCall {
                _trades: vec![offchain_trade(key.address(), 3, collection, 42)],
            }),
            U256::ZERO,
        ),
    );
    match guard_meta_transaction(&pool, 80002, &user, &manager, &resale).await {
        Err(ApiError::PaymentRule(refused)) => {
            assert_eq!(refused.detail()["code"], "creditsNotAllowed", "{refused}")
        }
        other => panic!(
            "a resale paid with credits through dcl-one's manager must be refused: {other:?}"
        ),
    }

    let mint = signed_split_calldata(
        &key,
        &use_credits(
            routes.collection_store,
            buyCall::SELECTOR,
            call_args(&buyCall {
                itemsToBuy: vec![ItemToBuy {
                    collection,
                    ids: vec![U256::from(4)],
                    prices: vec![one_mana],
                    beneficiaries: vec![key.address()],
                }],
            }),
            U256::ZERO,
        ),
    );
    guard_meta_transaction(&pool, 80002, &user, &manager, &mint)
        .await
        .expect("a mint of a mirrored wearable through dcl-one's manager is allowed");

    assert!(!is_credits_manager(ChainId::POLYGON, DCL_ONE_AMOY_MANAGER));
    guard_meta_transaction(&pool, 137, &user, &manager, &resale)
        .await
        .expect("on Polygon the address is no CreditsManager: the guard judges nothing");
}

/// dcl-one's own Polygon manager is guarded like the shared one: a relayed
/// `executeMetaTransaction(useCredits)` to it is decoded on Polygon's routes and
/// judged, so a resale through any of its marketplaces is refused and a mint of a
/// mirrored wearable passes. On Amoy the same address is no CreditsManager, so this
/// guard judges nothing there; what keeps it off Amoy is the relay's whitelist and
/// the audited domain, which name it on Polygon only.
#[tokio::test]
async fn the_relay_guards_use_credits_to_dcl_ones_polygon_manager() {
    use super::abi::{ItemToBuy, buyCall, offchain};
    use crate::guards::relay::guard_meta_transaction;
    use crate::policy_entry::{
        DCL_ONE_POLYGON_MANAGER, call_args, is_credits_manager, manager_routes,
    };
    use crate::types::ChainId;
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    seed_item(&pool, COLLECTION, 4, "wearable_v2", VICTIM).await;
    let key = PrivateKeySigner::random();
    let user = format!("{:#x}", key.address());
    let manager = format!("{DCL_ONE_POLYGON_MANAGER:#x}");
    let collection: Address = COLLECTION.parse().unwrap();
    let routes = manager_routes(ChainId::POLYGON).unwrap();
    let one_mana = U256::from(1_000_000_000_000_000_000u128);

    let resale_through = |marketplace: Address| {
        signed_split_calldata(
            &key,
            &use_credits(
                marketplace,
                offchain::acceptCall::SELECTOR,
                call_args(&offchain::acceptCall {
                    _trades: vec![offchain_trade(key.address(), 3, collection, 42)],
                }),
                U256::ZERO,
            ),
        )
    };
    for marketplace in routes.marketplaces {
        match guard_meta_transaction(&pool, 137, &user, &manager, &resale_through(*marketplace))
            .await
        {
            Err(ApiError::PaymentRule(refused)) => {
                assert_eq!(refused.detail()["code"], "creditsNotAllowed", "{refused}")
            }
            other => panic!(
                "a resale through {marketplace:#x} paid with credits through dcl-one's Polygon manager must be refused: {other:?}"
            ),
        }
    }

    let mint = signed_split_calldata(
        &key,
        &use_credits(
            routes.collection_store,
            buyCall::SELECTOR,
            call_args(&buyCall {
                itemsToBuy: vec![ItemToBuy {
                    collection,
                    ids: vec![U256::from(4)],
                    prices: vec![one_mana],
                    beneficiaries: vec![key.address()],
                }],
            }),
            U256::ZERO,
        ),
    );
    guard_meta_transaction(&pool, 137, &user, &manager, &mint)
        .await
        .expect("a mint of a mirrored wearable through dcl-one's Polygon manager is allowed");

    assert!(!is_credits_manager(ChainId::AMOY, DCL_ONE_POLYGON_MANAGER));
    guard_meta_transaction(
        &pool,
        80002,
        &user,
        &manager,
        &resale_through(routes.marketplaces[2]),
    )
    .await
    .expect("on Amoy the address is no CreditsManager: the guard judges nothing");
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

/// `checkout::broker::resume` drives a claim from its row alone, with no
/// request body: a sent trade buy whose payload is gone forwards the token
/// the row recorded, a forwarding claim settles on its receipt without a new
/// broadcast, and a key with no claim is not found.
#[tokio::test]
async fn a_claimed_broker_buy_resumes_from_its_row_alone() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let (chain_url, raws) = spawn_fake_chain().await;
    let relayer_key = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(chain_url),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(relayer_key.to_bytes())),
        ..Settings::for_tests()
    };
    let state = state(pool.clone(), settings);
    let claim = |key: String,
                 mode: &'static str,
                 status: &'static str,
                 token: Option<&'static str>,
                 forward: Option<String>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO shop.broker_purchases \
                 (idempotency_key, tx_hash, forward_tx_hash, collection, token_id, buyer_address, \
                  escrow_address, price_wei, chain_id, mode, status) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 1, 137, $8, $9)",
            )
            .bind(&key)
            .bind(format!("{:#x}", B256::repeat_byte(0xaa)))
            .bind(forward)
            .bind(COLLECTION)
            .bind(token)
            .bind(BUYER)
            .bind(RULE_ESCROW)
            .bind(mode)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
            key
        }
    };

    let sent = claim(
        format!("resume-sent-{}", Uuid::new_v4()),
        "trade",
        "sent",
        Some("7"),
        None,
    )
    .await;
    let forward = crate::checkout::broker::resume(&state, &sent)
        .await
        .unwrap();
    assert_eq!(
        raws.lock().unwrap().len(),
        1,
        "only the escrow forward is broadcast"
    );
    let (status, token, recorded): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, minted_token_id, forward_tx_hash FROM shop.broker_purchases \
         WHERE idempotency_key = $1",
    )
    .bind(&sent)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "confirmed");
    assert_eq!(
        token.as_deref(),
        Some("7"),
        "the recorded token is forwarded"
    );
    assert_eq!(recorded.as_deref(), Some(forward.as_str()));

    let hash = format!("{:#x}", B256::repeat_byte(0xbb));
    let forwarding = claim(
        format!("resume-forwarding-{}", Uuid::new_v4()),
        "primary",
        "forwarding",
        None,
        Some(hash.clone()),
    )
    .await;
    assert_eq!(
        crate::checkout::broker::resume(&state, &forwarding)
            .await
            .unwrap(),
        hash
    );
    assert_eq!(raws.lock().unwrap().len(), 1, "nothing new is broadcast");

    let missing = crate::checkout::broker::resume(&state, "resume-never-claimed").await;
    assert!(matches!(missing, Err(ApiError::NotFound(_))), "{missing:?}");

    sqlx::query("DELETE FROM shop.broker_purchases WHERE idempotency_key = ANY($1)")
        .bind(vec![sent, forwarding])
        .execute(&pool)
        .await
        .unwrap();
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

/// The direct signer's fee cap bounds the fee it signs, `max_fee = 2 * base + tip` (3.5 gwei on
/// the fake chain), not the node's `eth_gasPrice` (1 gwei): at the cap it sends, one wei under it
/// nothing reaches `eth_sendRawTransaction`, and with no cap it sends as before.
#[tokio::test]
async fn the_direct_signer_refuses_a_fee_over_its_cap_before_sending() {
    use super::chain::ChainComponent;
    use super::signer::DirectSigner;
    let (url, raws) = spawn_fake_chain().await;
    let chain = Arc::new(ChainComponent::new(
        Outbound::for_tests(),
        &url,
        Duration::from_secs(5),
    ));
    let key = hex::encode(PrivateKeySigner::random().to_bytes());
    let signed_fee: u128 = 2 * 1_000_000_000 + 1_500_000_000;
    let signer = |cap: Option<u128>| {
        DirectSigner::new(
            chain.clone(),
            &key,
            137,
            1_500_000,
            Duration::from_millis(1),
            Duration::from_millis(50),
        )
        .unwrap()
        .with_max_fee_per_gas(cap)
    };
    let to = Address::repeat_byte(0x11);

    let err = signer(Some(signed_fee - 1))
        .send_direct_call(to, Bytes::new())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, ApiError::HighCongestion(why) if why.contains("RELAYER_MAX_FEE_PER_GAS_WEI")),
        "a fee over the cap is a retryable 503 HIGH_CONGESTION: {err:?}"
    );
    assert!(
        raws.lock().unwrap().is_empty(),
        "a fee over the cap is never sent, though eth_gasPrice is under it"
    );

    signer(Some(signed_fee))
        .send_direct_call(to, Bytes::new())
        .await
        .expect("a fee at the cap is sent");
    assert_eq!(raws.lock().unwrap().len(), 1);
    signer(None)
        .send_direct_call(to, Bytes::new())
        .await
        .expect("no cap, no refusal");
    assert_eq!(raws.lock().unwrap().len(), 2);
}

fn trial_relay() -> Settings {
    Settings {
        trial_mode: true,
        trial_buyers: Some(crate::credits::trial::TrialBuyers::default()),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(PrivateKeySigner::random().to_bytes())),
        polygon_rpc_url: Some("http://127.0.0.1:1/".into()),
        relayer_max_fee_per_gas_wei: Some(500_000_000_000),
        ..Settings::for_tests()
    }
}

/// Under CREDITS_TRIAL_MODE the relay is how the trial's buyers spend their credits, so its
/// writes open with OPS_WRITES_ENABLED, not DATA_WRITES_ENABLED: with the data plane off and the
/// operator plane on, a relayed meta-transaction passes the gate and reaches the relay's own
/// checks.
#[tokio::test]
async fn a_trial_relay_opens_with_the_operator_plane_while_data_writes_stay_off() {
    let app = super::router(state(
        lazy_pool(),
        Settings {
            data_writes_enabled: false,
            ops_writes_enabled: true,
            ..trial_relay()
        },
    ));
    for path in ["/v1/transactions"] {
        let (status, text) = send(&app, "POST", path, Some("{}".into()), &[]).await;
        assert_ne!(status, 503, "{path} passes the gate: {text}");
    }
}

/// With the operator plane off, a trial's relay answers the credits 503 (naming
/// OPS_WRITES_ENABLED) even while DATA_WRITES_ENABLED is on, and the relayer controls still
/// pass the gate and stop at the admin bearer.
#[tokio::test]
async fn a_trial_relay_is_held_by_the_operator_plane_even_with_data_writes_on() {
    let app = super::router(state(
        lazy_pool(),
        Settings {
            data_writes_enabled: true,
            ops_writes_enabled: false,
            ..trial_relay()
        },
    ));
    let (status, text) = send(&app, "POST", "/v1/transactions", Some("{}".into()), &[]).await;
    assert_eq!(status, 503, "{text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        v["message"],
        crate::shared::writes::WRITES_DISABLED_MESSAGE,
        "{text}"
    );
    for path in ["/v1/admin/relayer/toggle", "/v1/admin/relayer/signer"] {
        let (status, text) = send(&app, "POST", path, Some("{}".into()), &[]).await;
        assert_eq!(status, 403, "{path}: {text}");
        assert!(
            text.contains("CATALYRST_ECONOMY_ADMIN_TOKEN is unset"),
            "{path}: {text}"
        );
    }
    let (status, _) = send(&app, "GET", "/v1/admin/relayer", None, &[]).await;
    assert_ne!(status, 503, "a read passes the gate");
}

/// Outside a trial the relay keeps the data plane's gate: OPS_WRITES_ENABLED alone does not
/// open it.
#[tokio::test]
async fn outside_a_trial_the_relay_keeps_the_data_plane_gate() {
    let app = super::router(state(
        lazy_pool(),
        Settings {
            data_writes_enabled: false,
            ops_writes_enabled: true,
            ..Settings::for_tests()
        },
    ));
    let (status, text) = send(&app, "POST", "/v1/transactions", Some("{}".into()), &[]).await;
    assert_eq!(status, 503, "{text}");
    assert!(text.contains("DATA_WRITES_ENABLED=false"), "{text}");
}

/// Under CREDITS_TRIAL_MODE the relay's one route is its own capped direct signer: any OZ or
/// upstream setting, even an incomplete one, refuses the start, and so does a missing signer
/// piece, an uncapped gas limit or a missing or zero fee cap. The refusal names variables only.
#[test]
fn a_trial_relay_has_one_capped_route() {
    assert!(trial_relay().refuse_an_open_trial_relay().is_ok());
    for (name, open) in [
        (
            "OZ_RELAYER_URL",
            Settings {
                oz_relayer_url: Some("https://oz.example".into()),
                ..trial_relay()
            },
        ),
        (
            "OZ_RELAYER_ID",
            Settings {
                oz_relayer_id: Some("relayer".into()),
                ..trial_relay()
            },
        ),
        (
            "OZ_RELAYER_API_KEY",
            Settings {
                oz_relayer_api_key: Some("key-SECRET".into()),
                ..trial_relay()
            },
        ),
        (
            "TRANSACTIONS_UPSTREAM_URL",
            Settings {
                transactions_upstream_url: Some("https://transactions.example".into()),
                ..trial_relay()
            },
        ),
    ] {
        let why = open.refuse_an_open_trial_relay().unwrap_err().to_string();
        assert!(why.contains(name) && !why.contains("SECRET"), "{why}");
    }
    for (needs, closed) in [
        (
            "META_TX_BROADCAST_ENABLED",
            Settings {
                meta_tx_broadcast_enabled: false,
                ..trial_relay()
            },
        ),
        (
            "RELAYER_PRIVATE_KEY_FILE",
            Settings {
                relayer_private_key: None,
                ..trial_relay()
            },
        ),
        (
            "RPC_ENDPOINT_POLYGON",
            Settings {
                polygon_rpc_url: None,
                ..trial_relay()
            },
        ),
        (
            "MAX_GAS_LIMIT",
            Settings {
                max_gas_limit: 0,
                ..trial_relay()
            },
        ),
        (
            "RELAYER_MAX_FEE_PER_GAS_WEI",
            Settings {
                relayer_max_fee_per_gas_wei: None,
                ..trial_relay()
            },
        ),
        (
            "RELAYER_MAX_FEE_PER_GAS_WEI",
            Settings {
                relayer_max_fee_per_gas_wei: Some(0),
                ..trial_relay()
            },
        ),
        (
            "CREDITS_TRIAL_BUYERS",
            Settings {
                trial_buyers: None,
                ..trial_relay()
            },
        ),
    ] {
        let why = closed.refuse_an_open_trial_relay().unwrap_err().to_string();
        assert!(why.contains(needs), "{why}");
    }
}

/// A trial's relayer key signs on the collections chain alone: with ETH_RPC_URL set, as a mirror
/// always has it, the NAMEs chain signer is not built from it; outside a trial it still is.
#[tokio::test]
async fn a_trial_never_reuses_the_relayer_key_on_the_names_chain() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://127.0.0.1:1/none")
        .unwrap();
    let with_eth = |settings: Settings| Settings {
        eth_rpc_url: Some("http://127.0.0.1:1/".into()),
        ..settings
    };
    let trial = State::new(pool.clone(), Outbound::for_tests(), with_eth(trial_relay()))
        .expect("a complete trial relay starts");
    assert!(trial.eth_signer.is_none());
    let open = State::new(
        pool,
        Outbound::for_tests(),
        with_eth(Settings {
            trial_mode: false,
            ..trial_relay()
        }),
    )
    .expect("outside a trial the relay starts as before");
    assert!(open.eth_signer.is_some());
    // The fee cap is the collections-chain signer's alone: the NAMEs chain signer keeps its fees.
    assert_eq!(
        open.transaction
            .direct_signer()
            .and_then(|s| s.max_fee_per_gas()),
        Some(500_000_000_000)
    );
    assert_eq!(open.eth_signer.as_ref().unwrap().max_fee_per_gas(), None);
}

/// Under a trial the MANA payment routes refuse before any work: `config` would hand a client the
/// relay's own signer as the pay-to address of a top-up the credits lane then refuses.
#[tokio::test]
async fn a_trial_refuses_the_mana_payment_routes() {
    let (chain_url, _raws) = spawn_fake_chain().await;
    let key = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(chain_url),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(key.to_bytes())),
        relayer_max_fee_per_gas_wei: Some(500_000_000_000),
        trial_mode: true,
        trial_buyers: Some(crate::credits::trial::TrialBuyers::default()),
        ..Settings::for_tests()
    };
    let app = router(state(lazy_pool(), settings));
    let wallet = format!("{:#x}", key.address());
    for path in [
        "/v1/payments/config".to_string(),
        format!("/v1/payments/nonce/{wallet}"),
        format!("/v1/payments/balance/{wallet}"),
    ] {
        let (status, body) = get_json(&app, &path).await;
        assert_eq!(status, 403, "{path}: {body}");
        assert!(body.to_string().contains("CREDITS_TRIAL_MODE"), "{body}");
    }
    let (status, text) = send(
        &app,
        "POST",
        "/v1/payments/verify",
        Some("{}".to_string()),
        &[],
    )
    .await;
    assert_eq!(status, 403, "{text}");
}

/// A trial's relayer pays gas for the trial's buyers only: a meta-transaction signed for any other
/// wallet is refused with 403 before the daily quota is charged or anything is broadcast, while a
/// listed buyer's relays as before.
#[tokio::test]
async fn a_trial_relays_meta_transactions_for_its_buyers_only() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let (chain_url, raws) = spawn_fake_chain().await;
    let buyer = PrivateKeySigner::random();
    let stranger = PrivateKeySigner::random();
    let buyer_address = format!("{:#x}", buyer.address());
    let stranger_address = format!("{:#x}", stranger.address());
    let settings = Settings {
        trial_mode: true,
        trial_buyers: Some(
            crate::credits::trial::TrialBuyers::parse(Some(
                &buyer_address.to_uppercase().replace("0X", "0x"),
            ))
            .unwrap(),
        ),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(PrivateKeySigner::random().to_bytes())),
        polygon_rpc_url: Some(chain_url),
        relayer_max_fee_per_gas_wei: Some(500_000_000_000),
        ..Settings::for_tests()
    };
    let app = router(state(pool.clone(), settings));

    let (status, body) = post_transaction(
        &app,
        &stranger_address,
        COLLECTION,
        &signed_split_calldata(&stranger, FS),
    )
    .await;
    assert_eq!(status, 403, "{body}");
    assert!(body.to_string().contains("CREDITS_TRIAL_MODE"), "{body}");
    assert_eq!(
        row_count(&pool, &stranger_address).await,
        0,
        "no quota charged"
    );
    assert!(
        raws.lock().unwrap().is_empty(),
        "nothing broadcast for a stranger"
    );

    let (status, body) = post_transaction(
        &app,
        &buyer_address,
        COLLECTION,
        &signed_split_calldata(&buyer, FS),
    )
    .await;
    assert_eq!(
        status, 200,
        "a listed buyer's meta-transaction relays: {body}"
    );
    assert_eq!(raws.lock().unwrap().len(), 1);
    cleanup_user(&pool, &buyer_address).await;
    cleanup_user(&pool, &stranger_address).await;
}

/// A JSON-RPC chain whose failures a test scripts: the next `fail_nonce_reads`
/// nonce reads fail (a failure before signing); after `pass_sends` accepted
/// sends, the next `drop_sends` `eth_sendRawTransaction` calls fail without
/// the node taking the transaction (as a crash between the record and the
/// send leaves it); `taken_nonces` counts transactions of other senders that
/// used the relayer's nonces, `finalized_taken_nonces` those of them already
/// final (the `finalized` count). An accepted transaction mines at once; its
/// receipt carries a mint of [`SCRIPTED_TOKEN`] on `mint`'s collection to
/// its recipient when set.
#[derive(Default)]
struct ScriptedChain {
    fail_nonce_reads: std::sync::atomic::AtomicU32,
    pass_sends: std::sync::atomic::AtomicU32,
    drop_sends: std::sync::atomic::AtomicU32,
    taken_nonces: std::sync::atomic::AtomicU32,
    finalized_taken_nonces: std::sync::atomic::AtomicU32,
    /// Hashes the node knows (`eth_getTransactionByHash`) without a receipt.
    known: Mutex<Vec<String>>,
    /// `eth_getTransactionByHash` replies carry neither `result` nor `error`.
    blank_tx_lookups: std::sync::atomic::AtomicBool,
    /// Hashes with a receipt though this node never took them (another
    /// backend did).
    mined_elsewhere: Mutex<Vec<String>>,
    /// Hashes whose receipt reports a revert.
    reverted: Mutex<Vec<String>>,
    accepted: Mutex<Vec<Vec<u8>>>,
    mint: Mutex<Option<(Address, Address)>>,
}

const SCRIPTED_TOKEN: u64 = 42;

impl ScriptedChain {
    fn take(counter: &std::sync::atomic::AtomicU32) -> bool {
        use std::sync::atomic::Ordering;
        let mut n = counter.load(Ordering::SeqCst);
        while n > 0 {
            match counter.compare_exchange(n, n - 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return true,
                Err(now) => n = now,
            }
        }
        false
    }

    fn accepted_hashes(&self) -> Vec<String> {
        self.accepted
            .lock()
            .unwrap()
            .iter()
            .map(|raw| format!("{:#x}", alloy_primitives::keccak256(raw)))
            .collect()
    }
}

async fn spawn_scripted_chain(chain: Arc<ScriptedChain>) -> String {
    async fn handle(
        axum::extract::State(chain): axum::extract::State<Arc<ScriptedChain>>,
        Json(req): Json<Value>,
    ) -> Json<Value> {
        let id = req.get("id").cloned().unwrap_or(json!(1));
        let ok = |v: Value| Json(json!({ "jsonrpc": "2.0", "id": id, "result": v }));
        let fail = |message: &str| {
            Json(
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": message } }),
            )
        };
        match req.get("method").and_then(|m| m.as_str()).unwrap_or("") {
            "eth_chainId" => ok(json!("0x89")),
            "eth_estimateGas" => ok(json!("0x5208")),
            "eth_gasPrice" => ok(json!("0x3b9aca00")),
            "eth_maxPriorityFeePerGas" => ok(json!("0x59682f00")),
            "eth_getBlockByNumber" => ok(
                json!({ "number": "0x1", "baseFeePerGas": "0x3b9aca00", "timestamp": "0x6553f100" }),
            ),
            "eth_blockNumber" => ok(json!("0x50")),
            "eth_getTransactionCount" => {
                if ScriptedChain::take(&chain.fail_nonce_reads) {
                    return fail("header not found");
                }
                let taken = if req["params"][1] == "finalized" {
                    &chain.finalized_taken_nonces
                } else {
                    &chain.taken_nonces
                };
                let taken = taken.load(std::sync::atomic::Ordering::SeqCst) as usize;
                ok(json!(format!(
                    "{:#x}",
                    chain.accepted.lock().unwrap().len() + taken
                )))
            }
            "eth_sendRawTransaction" => {
                if !ScriptedChain::take(&chain.pass_sends) && ScriptedChain::take(&chain.drop_sends)
                {
                    return fail("upstream request timeout");
                }
                let raw = req["params"][0].as_str().unwrap_or("");
                let bytes = hex::decode(raw.trim_start_matches("0x")).unwrap_or_default();
                let mut accepted = chain.accepted.lock().unwrap();
                if accepted.contains(&bytes) {
                    return fail("already known");
                }
                let hash = alloy_primitives::keccak256(&bytes);
                accepted.push(bytes);
                ok(json!(format!("{hash:#x}")))
            }
            "eth_getTransactionByHash" => {
                if chain
                    .blank_tx_lookups
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    return Json(json!({ "jsonrpc": "2.0", "id": id }));
                }
                let hash = req["params"][0].as_str().unwrap_or("").to_lowercase();
                if chain.accepted_hashes().contains(&hash)
                    || chain.known.lock().unwrap().contains(&hash)
                    || chain.mined_elsewhere.lock().unwrap().contains(&hash)
                {
                    return ok(json!({ "hash": hash }));
                }
                ok(Value::Null)
            }
            "eth_getTransactionReceipt" => {
                let hash = req["params"][0].as_str().unwrap_or("").to_lowercase();
                if !chain.accepted_hashes().contains(&hash)
                    && !chain.mined_elsewhere.lock().unwrap().contains(&hash)
                {
                    return ok(Value::Null);
                }
                let logs = match *chain.mint.lock().unwrap() {
                    Some((collection, to)) => json!([{
                        "address": format!("{collection:#x}"),
                        "topics": [
                            format!("0x{}", hex::encode(alloy_primitives::keccak256("Transfer(address,address,uint256)"))),
                            format!("0x{}", "0".repeat(64)),
                            format!("0x{:0>64}", hex::encode(to)),
                            format!("0x{SCRIPTED_TOKEN:064x}"),
                        ],
                        "data": "0x",
                    }]),
                    None => json!([]),
                };
                let status = if chain.reverted.lock().unwrap().contains(&hash) {
                    "0x0"
                } else {
                    "0x1"
                };
                ok(json!({ "status": status, "blockNumber": "0x10", "logs": logs }))
            }
            _ => Json(
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": 3, "message": "execution reverted" } }),
            ),
        }
    }
    let app = axum::Router::new()
        .route("/", post(handle))
        .with_state(chain);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind scripted chain");
    let addr = listener.local_addr().expect("chain addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

/// The economy state over a scripted chain, serving both the Polygon and the
/// NAMEs signer with one relayer key.
async fn scripted_state(pool: &PgPool, chain: Arc<ScriptedChain>) -> (SharedState, Address) {
    let url = spawn_scripted_chain(chain).await;
    let relayer = PrivateKeySigner::random();
    let settings = Settings {
        polygon_rpc_url: Some(url.clone()),
        eth_rpc_url: Some(url),
        meta_tx_broadcast_enabled: true,
        relayer_private_key: Some(hex::encode(relayer.to_bytes())),
        admin_token: Some(ADMIN.into()),
        ..Settings::for_tests()
    };
    (state(pool.clone(), settings), relayer.address())
}

async fn keyed_post(app: &axum::Router, path: &str, key: &str, body: &Value) -> (u16, Value) {
    let bearer = format!("Bearer {ADMIN}");
    let (status, text) = send(
        app,
        "POST",
        path,
        Some(body.to_string()),
        &[("authorization", &bearer), ("idempotency-key", key)],
    )
    .await;
    (status, serde_json::from_str(&text).unwrap_or_default())
}

#[derive(Debug, sqlx::FromRow)]
struct RecordedPurchase {
    status: String,
    tx_hash: Option<String>,
    signed_tx: Option<String>,
    forward_tx_hash: Option<String>,
    forward_signed_tx: Option<String>,
    write_ahead: bool,
}

async fn recorded_purchase(pool: &PgPool, key: &str) -> RecordedPurchase {
    sqlx::query_as(
        "SELECT status, tx_hash, signed_tx, forward_tx_hash, forward_signed_tx, write_ahead \
         FROM shop.broker_purchases WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_one(pool)
    .await
    .expect("broker purchase row")
}

fn hash_of_hex(raw: &str) -> String {
    let bytes = hex::decode(raw.trim_start_matches("0x")).expect("recorded hex");
    format!("{:#x}", alloy_primitives::keccak256(bytes))
}

/// A crash between recording a NAME buy and broadcasting it (here: the send
/// never reaches the node) leaves the claim `sent` with the hash of the
/// recorded bytes. The retry re-sends those same bytes instead of signing a
/// new purchase, and one transaction is all the chain ever sees.
#[tokio::test]
async fn a_name_buy_recorded_but_not_broadcast_resumes_by_resending_the_same_transaction() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    chain
        .drop_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, _) = scripted_state(&pool, chain.clone()).await;
    let app = router(state);
    let key = format!("names-{}", Uuid::new_v4());
    let body = json!({ "mode": "mint", "name": "resent", "buyerAddress": BUYER });

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        status, 504,
        "a recorded transaction whose send failed is indeterminate: {reply}"
    );
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(row.status, "sent", "{row:?}");
    assert!(row.write_ahead);
    let recorded_hash = row
        .tx_hash
        .clone()
        .expect("the hash is recorded before the send");
    assert_eq!(
        hash_of_hex(
            row.signed_tx
                .as_deref()
                .expect("the signed bytes are recorded")
        ),
        recorded_hash
    );
    assert!(
        chain.accepted_hashes().is_empty(),
        "nothing reached the node"
    );

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 200, "the retry re-sends and confirms: {reply}");
    assert_eq!(reply["txHash"], recorded_hash.as_str());
    assert_eq!(chain.accepted_hashes(), vec![recorded_hash.clone()]);
    assert_eq!(recorded_purchase(&pool, &key).await.status, "confirmed");

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        (status, reply["txHash"].as_str()),
        (200, Some(recorded_hash.as_str()))
    );
    assert_eq!(chain.accepted_hashes().len(), 1, "a replay sends nothing");
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A NAME buy whose signing failed (nothing recorded, nothing sent) is
/// `error`, and the next request with the same key re-arms it and buys in
/// that same call instead of answering 409.
#[tokio::test]
async fn a_name_buy_that_failed_before_its_record_is_bought_by_the_next_request() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    chain
        .fail_nonce_reads
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, _) = scripted_state(&pool, chain.clone()).await;
    let app = router(state);
    let key = format!("names-{}", Uuid::new_v4());
    let body = json!({ "mode": "mint", "name": "rearmed", "buyerAddress": BUYER });

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 500, "{reply}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash.as_deref()),
        ("error", None)
    );

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        status, 200,
        "the re-armed claim buys in the same call: {reply}"
    );
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(row.status, "confirmed");
    assert_eq!(chain.accepted_hashes(), vec![row.tx_hash.unwrap()]);
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A claim still being signed when its credits checkout is frozen is fenced:
/// its caller's record then matches no row, so nothing is sent. A buy request
/// on a frozen checkout's line, claimed or not, answers 409 and says why.
#[tokio::test]
async fn a_frozen_credits_checkout_line_is_fenced_and_refused() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let creator = PrivateKeySigner::random();
    seed_item(
        &pool,
        COLLECTION,
        4,
        "wearable_v2",
        &format!("{:#x}", creator.address()),
    )
    .await;
    let chain = Arc::new(ScriptedChain::default());
    let (state, relayer) = scripted_state(&pool, chain.clone()).await;
    *chain.mint.lock().unwrap() = Some((COLLECTION.parse().unwrap(), relayer));
    let app = router(state.clone());
    let checkout: i64 = sqlx::query_scalar(
        "INSERT INTO credits.checkouts (idempotency_key, address, status) \
         VALUES ($1, $2, 'fulfilling') RETURNING id",
    )
    .bind(format!("frozen-{}", Uuid::new_v4()))
    .bind(BUYER)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (signing, unclaimed) = (
        format!("checkout:{checkout}:1"),
        format!("checkout:{checkout}:2"),
    );
    let claimed_at = now();
    let claim = claim_broker_purchase(
        &pool,
        &BrokerPurchaseClaim {
            key: &signing,
            collection: COLLECTION,
            item_id: Some("4"),
            token_id: None,
            buyer_address: BUYER,
            escrow_address: RULE_ESCROW,
            price_wei: "1000000000000000000",
            chain_id: 137,
            mode: "primary",
            trade_hashed_signature: None,
            usd_amount_wei: None,
            mana_usd_rate_wei: None,
        },
        claimed_at,
    )
    .await
    .unwrap();
    assert!(claim.claimed);

    sqlx::query("UPDATE credits.checkouts SET status = 'reversing' WHERE id = $1")
        .bind(checkout)
        .execute(&pool)
        .await
        .unwrap();
    let fenced = crate::checkout::claims::fence_unrecorded(
        &mut pool.acquire().await.unwrap(),
        &[signing.clone(), unclaimed.clone()],
    )
    .await
    .unwrap();
    assert_eq!(fenced, 1);

    let signer = state.eth_signer.as_ref().unwrap();
    let call = crate::checkout::calls::BrokerCall {
        to: Address::repeat_byte(0x11),
        data: Bytes::new(),
    };
    let late =
        crate::checkout::broker::send_recorded_buy(&state, signer, &signing, claimed_at, call)
            .await;
    assert!(matches!(late, Err(ApiError::Conflict(_))), "{late:?}");
    assert!(chain.accepted_hashes().is_empty(), "nothing was sent");
    assert_eq!(recorded_purchase(&pool, &signing).await.status, "error");

    let body = json!({ "collection": COLLECTION, "priceWei": "1000000000000000000", "escrowAddress": RULE_ESCROW, "buyerAddress": BUYER, "mode": "primary", "itemId": "4" });
    for key in [&signing, &unclaimed] {
        let (status, reply) = keyed_post(&app, "/v1/broker/buy", key, &body).await;
        assert_eq!(status, 409, "{reply}");
        assert!(reply.to_string().contains("frozen"), "{reply}");
    }
    assert!(chain.accepted_hashes().is_empty());

    delete_key(&pool, "broker_purchases", &signing).await;
    sqlx::query("DELETE FROM credits.checkouts WHERE id = $1")
        .bind(checkout)
        .execute(&pool)
        .await
        .unwrap();
    clear_items_pass(&pool).await;
}

/// A wearable broker buy whose signing failed is re-armed and bought by the
/// next request, and both of its legs record their signed transaction before
/// it is sent.
#[tokio::test]
async fn a_broker_buy_that_failed_before_its_record_is_bought_by_the_next_request() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let creator = PrivateKeySigner::random();
    seed_item(
        &pool,
        COLLECTION,
        4,
        "wearable_v2",
        &format!("{:#x}", creator.address()),
    )
    .await;
    let chain = Arc::new(ScriptedChain::default());
    chain
        .fail_nonce_reads
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, relayer) = scripted_state(&pool, chain.clone()).await;
    *chain.mint.lock().unwrap() = Some((COLLECTION.parse().unwrap(), relayer));
    let app = router(state);
    let key = format!("broker-{}", Uuid::new_v4());
    let body = json!({ "collection": COLLECTION, "priceWei": "1000000000000000000", "escrowAddress": RULE_ESCROW, "buyerAddress": BUYER, "mode": "primary", "itemId": "4" });

    let (status, reply) = keyed_post(&app, "/v1/broker/buy", &key, &body).await;
    assert_eq!(status, 500, "{reply}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash.as_deref()),
        ("error", None)
    );
    assert!(chain.accepted_hashes().is_empty());

    let (status, reply) = keyed_post(&app, "/v1/broker/buy", &key, &body).await;
    assert_eq!(status, 200, "the re-armed claim buys and forwards: {reply}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(row.status, "confirmed", "{row:?}");
    let buy = row.tx_hash.unwrap();
    let forward = row.forward_tx_hash.unwrap();
    assert_eq!(hash_of_hex(row.signed_tx.as_deref().unwrap()), buy);
    assert_eq!(
        hash_of_hex(row.forward_signed_tx.as_deref().unwrap()),
        forward
    );
    assert_eq!(reply["txHash"], forward.as_str());
    assert_eq!(chain.accepted_hashes(), vec![buy, forward]);
    delete_key(&pool, "broker_purchases", &key).await;
    clear_items_pass(&pool).await;
}

/// A crash between the claim and the record leaves the claim `pending` with
/// nothing recorded, so nothing was sent. While it is fresh another request
/// is refused (the first may still be signing); once stale, a request takes
/// it over and buys. The stalled caller's record is fenced on its own claim,
/// so it can never send a second transaction, and a claim kept from before
/// transactions were recorded is never bought again.
#[tokio::test]
async fn a_claim_left_pending_before_its_record_is_taken_over_once_stale() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (state, relayer) = scripted_state(&pool, chain.clone()).await;
    let app = router(state.clone());
    let key = format!("names-{}", Uuid::new_v4());
    let body = json!({ "mode": "mint", "name": "takenover", "buyerAddress": BUYER });
    let registrar = format!(
        "{:#x}",
        super::contracts_addrs::NameContracts::for_chain(state.settings.names_chain_id)
            .unwrap()
            .registrar
    );
    let stalled_at = now() - chrono::Duration::minutes(10);
    let stalled = claim_broker_purchase(
        &pool,
        &BrokerPurchaseClaim {
            key: &key,
            collection: &registrar,
            item_id: Some("takenover"),
            token_id: None,
            buyer_address: BUYER,
            escrow_address: &format!("{relayer:#x}"),
            price_wei: "100000000000000000000",
            chain_id: state.settings.names_chain_id as i64,
            mode: "name-mint",
            trade_hashed_signature: None,
            usd_amount_wei: None,
            mana_usd_rate_wei: None,
        },
        now(),
    )
    .await
    .unwrap();
    assert!(stalled.claimed);

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        status, 409,
        "a fresh pending claim may still be signing: {reply}"
    );
    assert!(chain.accepted_hashes().is_empty());

    sqlx::query("UPDATE shop.broker_purchases SET updated_at = $2 WHERE idempotency_key = $1")
        .bind(&key)
        .bind(stalled_at)
        .execute(&pool)
        .await
        .unwrap();
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        status, 200,
        "a stale claim with nothing recorded is taken over: {reply}"
    );
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(chain.accepted_hashes(), vec![row.tx_hash.clone().unwrap()]);

    let signer = state.eth_signer.as_ref().unwrap();
    let call = crate::checkout::calls::BrokerCall {
        to: Address::repeat_byte(0x11),
        data: Bytes::new(),
    };
    let late =
        crate::checkout::broker::send_recorded_buy(&state, signer, &key, stalled_at, call).await;
    assert!(
        matches!(late, Err(ApiError::Conflict(_))),
        "the stalled caller's record is fenced: {late:?}"
    );
    assert_eq!(
        chain.accepted_hashes().len(),
        1,
        "one transaction for the claim"
    );
    assert_eq!(recorded_purchase(&pool, &key).await.tx_hash, row.tx_hash);

    sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'error', tx_hash = NULL, write_ahead = false \
         WHERE idempotency_key = $1",
    )
    .bind(&key)
    .execute(&pool)
    .await
    .unwrap();
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        status, 409,
        "an earlier claim's failure is not proof nothing was sent: {reply}"
    );
    assert_eq!(chain.accepted_hashes().len(), 1);
    delete_key(&pool, "broker_purchases", &key).await;
}

/// The key-only resume (what credits fulfilment calls) cannot build a buy,
/// so a claim left `pending` with nothing recorded is released to `error`
/// once stale, for the next buy request to re-arm; a fresh one is left to
/// its caller. Nothing is sent either way.
#[tokio::test]
async fn the_key_only_resume_releases_a_stale_pending_claim_without_sending() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (state, _) = scripted_state(&pool, chain.clone()).await;
    let key = format!("broker-{}", Uuid::new_v4());
    let claim = claim_broker_purchase(
        &pool,
        &BrokerPurchaseClaim {
            key: &key,
            collection: COLLECTION,
            item_id: Some("4"),
            token_id: None,
            buyer_address: BUYER,
            escrow_address: RULE_ESCROW,
            price_wei: "1000000000000000000",
            chain_id: 137,
            mode: "primary",
            trade_hashed_signature: None,
            usd_amount_wei: None,
            mana_usd_rate_wei: None,
        },
        now(),
    )
    .await
    .unwrap();
    assert!(claim.claimed);

    let fresh = crate::checkout::broker::resume(&state, &key).await;
    assert!(matches!(fresh, Err(ApiError::Conflict(_))), "{fresh:?}");
    assert_eq!(recorded_purchase(&pool, &key).await.status, "pending");

    sqlx::query("UPDATE shop.broker_purchases SET updated_at = $2 WHERE idempotency_key = $1")
        .bind(&key)
        .bind(now() - chrono::Duration::minutes(10))
        .execute(&pool)
        .await
        .unwrap();
    let stale = crate::checkout::broker::resume(&state, &key).await;
    assert!(matches!(stale, Err(ApiError::Conflict(_))), "{stale:?}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash.as_deref()),
        ("error", None)
    );
    assert!(chain.accepted_hashes().is_empty(), "nothing was sent");
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A broker buy whose forward was recorded but never reached the node is
/// settled by the key-only resume re-sending the recorded forward: the
/// chain sees the buy and that one forward, nothing else.
#[tokio::test]
async fn a_recorded_forward_that_was_not_broadcast_is_resent_by_the_key_only_resume() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let creator = PrivateKeySigner::random();
    seed_item(
        &pool,
        COLLECTION,
        4,
        "wearable_v2",
        &format!("{:#x}", creator.address()),
    )
    .await;
    let chain = Arc::new(ScriptedChain::default());
    chain
        .pass_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .drop_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, relayer) = scripted_state(&pool, chain.clone()).await;
    *chain.mint.lock().unwrap() = Some((COLLECTION.parse().unwrap(), relayer));
    let app = router(state.clone());
    let key = format!("broker-{}", Uuid::new_v4());
    let body = json!({ "collection": COLLECTION, "priceWei": "1000000000000000000", "escrowAddress": RULE_ESCROW, "buyerAddress": BUYER, "mode": "primary", "itemId": "4" });

    let (status, reply) = keyed_post(&app, "/v1/broker/buy", &key, &body).await;
    assert_eq!(
        status, 504,
        "the forward's send failed after its record: {reply}"
    );
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(row.status, "forwarding", "{row:?}");
    let buy = row.tx_hash.clone().unwrap();
    let forward = row.forward_tx_hash.clone().unwrap();
    assert_eq!(
        hash_of_hex(row.forward_signed_tx.as_deref().unwrap()),
        forward
    );
    assert_eq!(chain.accepted_hashes(), vec![buy.clone()]);

    let resumed = crate::checkout::broker::resume(&state, &key).await.unwrap();
    assert_eq!(
        resumed, forward,
        "the recorded forward is re-sent, not re-signed"
    );
    assert_eq!(chain.accepted_hashes(), vec![buy, forward]);
    assert_eq!(recorded_purchase(&pool, &key).await.status, "confirmed");
    delete_key(&pool, "broker_purchases", &key).await;
    clear_items_pass(&pool).await;
}

/// Moves a claim's first dead look (and, with `age`, its claim time) into
/// the past, as a later retry would find them.
async fn age_dead_look(pool: &PgPool, key: &str, minutes: i64, claim_days: Option<i64>) {
    sqlx::query(
        "UPDATE shop.broker_purchases SET dead_seen_at = $2 - make_interval(mins => $3), \
         created_at = COALESCE($2 - make_interval(days => $4), created_at) \
         WHERE idempotency_key = $1",
    )
    .bind(key)
    .bind(now())
    .bind(minutes as i32)
    .bind(claim_days.map(|d| d as i32))
    .execute(pool)
    .await
    .unwrap();
}

async fn dead_seen(pool: &PgPool, key: &str) -> (bool, Option<Vec<String>>) {
    sqlx::query_as(
        "SELECT dead_seen_at IS NOT NULL, released_tx_hashes FROM shop.broker_purchases \
         WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A NAME buy recorded under `key` whose send never reached the node.
async fn recorded_unsent_name_buy(
    pool: &PgPool,
    chain: &Arc<ScriptedChain>,
    name: &str,
) -> (axum::Router, String, Value, String) {
    chain
        .drop_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, _) = scripted_state(pool, chain.clone()).await;
    let app = router(state);
    let key = format!("names-{}", Uuid::new_v4());
    let body = json!({ "mode": "mint", "name": name, "buyerAddress": BUYER });
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "{reply}");
    let recorded = recorded_purchase(pool, &key).await.tx_hash.unwrap();
    (app, key, body, recorded)
}

/// A recorded transaction whose nonce another transaction took can never
/// mine. While that other transaction is not final, the retry neither
/// re-sends nor re-signs (a reorg could still mine ours) and answers 504.
/// Once it is final, a first dead look is only noted (504); a second look
/// ten minutes later agrees, so the claim is released (409) with the dead
/// hash kept, and the next request buys with a new transaction, the only
/// one the chain mines.
#[tokio::test]
async fn a_recorded_transaction_whose_nonce_was_finally_taken_is_released_and_bought_again() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, dead) = recorded_unsent_name_buy(&pool, &chain, "nonceless").await;

    chain
        .taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "not final yet: wait, never re-sign: {reply}");
    assert_eq!(dead_seen(&pool, &key).await, (false, None));

    chain
        .finalized_taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "one dead look is only noted: {reply}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash.as_deref()),
        ("sent", Some(dead.as_str()))
    );
    assert_eq!(dead_seen(&pool, &key).await, (true, None));
    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "a second look right after is not enough");

    age_dead_look(&pool, &key, 11, None).await;
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(
        status, 409,
        "two looks ten minutes apart release the claim: {reply}"
    );
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash, row.signed_tx),
        ("error", None, None)
    );
    assert_eq!(
        dead_seen(&pool, &key).await,
        (false, Some(vec![dead.clone()]))
    );
    assert!(chain.accepted_hashes().is_empty(), "nothing was sent");

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 200, "the released claim is bought again: {reply}");
    let row = recorded_purchase(&pool, &key).await;
    let bought = row.tx_hash.unwrap();
    assert_ne!(bought, dead);
    assert_eq!(row.status, "confirmed");
    assert_eq!(
        chain.accepted_hashes(),
        vec![bought],
        "one transaction mined for the claim"
    );
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A single lookup can be wrong. A node that still knows the transaction
/// contradicts a missing receipt, so nothing is noted; a first dead look
/// later contradicted by a receipt (a lagging backend) is forgotten, and the
/// mined buy is confirmed, never released or bought again.
#[tokio::test]
async fn a_dead_look_contradicted_by_a_later_receipt_releases_nothing() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, recorded) = recorded_unsent_name_buy(&pool, &chain, "lagging").await;
    chain
        .taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .finalized_taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);

    chain.known.lock().unwrap().push(recorded.clone());
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "{reply}");
    assert_eq!(
        dead_seen(&pool, &key).await,
        (false, None),
        "the node knows it"
    );

    chain.known.lock().unwrap().clear();
    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504);
    assert_eq!(dead_seen(&pool, &key).await, (true, None));

    age_dead_look(&pool, &key, 11, None).await;
    chain.mined_elsewhere.lock().unwrap().push(recorded.clone());
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 200, "the receipt settles it: {reply}");
    assert_eq!(reply["txHash"], recorded.as_str());
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash),
        ("confirmed", Some(recorded))
    );
    assert_eq!(
        dead_seen(&pool, &key).await,
        (false, None),
        "the dead look is forgotten"
    );
    assert!(
        chain.accepted_hashes().is_empty(),
        "nothing was bought again"
    );
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A claim older than the release window may have lost its receipt to a
/// pruned lookup index, so a dead look there is never released: it is left
/// `sent` for manual reconciliation.
#[tokio::test]
async fn an_old_claim_that_looks_dead_is_left_for_reconciliation() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, recorded) = recorded_unsent_name_buy(&pool, &chain, "pruned").await;
    chain
        .taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .finalized_taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    age_dead_look(&pool, &key, 11, Some(8)).await;

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 409, "{reply}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!((row.status.as_str(), row.tx_hash), ("sent", Some(recorded)));
    assert_eq!(dead_seen(&pool, &key).await.1, None, "nothing released");
    assert!(chain.accepted_hashes().is_empty());
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A recorded forward whose nonce a final transaction took can never mine;
/// the buy did. A first dead look is only noted; once a second look ten
/// minutes later agrees, the key-only resume signs one new forward in its
/// place and settles the claim.
#[tokio::test]
async fn a_recorded_forward_that_can_never_mine_is_replaced_by_one_new_forward() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let creator = PrivateKeySigner::random();
    seed_item(
        &pool,
        COLLECTION,
        4,
        "wearable_v2",
        &format!("{:#x}", creator.address()),
    )
    .await;
    let chain = Arc::new(ScriptedChain::default());
    chain
        .pass_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .drop_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, relayer) = scripted_state(&pool, chain.clone()).await;
    *chain.mint.lock().unwrap() = Some((COLLECTION.parse().unwrap(), relayer));
    let app = router(state.clone());
    let key = format!("broker-{}", Uuid::new_v4());
    let body = json!({ "collection": COLLECTION, "priceWei": "1000000000000000000", "escrowAddress": RULE_ESCROW, "buyerAddress": BUYER, "mode": "primary", "itemId": "4" });

    let (status, reply) = keyed_post(&app, "/v1/broker/buy", &key, &body).await;
    assert_eq!(status, 504, "{reply}");
    let row = recorded_purchase(&pool, &key).await;
    let (buy, dead) = (row.tx_hash.unwrap(), row.forward_tx_hash.unwrap());

    chain
        .taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .finalized_taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let first = crate::checkout::broker::resume(&state, &key).await;
    assert!(
        matches!(first, Err(ApiError::RelayerTimeout(_))),
        "{first:?}"
    );
    assert_eq!(recorded_purchase(&pool, &key).await.status, "forwarding");

    age_dead_look(&pool, &key, 11, None).await;
    let forward = crate::checkout::broker::resume(&state, &key).await.unwrap();
    assert_ne!(forward, dead);
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(row.status, "confirmed");
    assert_eq!(row.forward_tx_hash.as_deref(), Some(forward.as_str()));
    assert_eq!(dead_seen(&pool, &key).await, (false, None));
    let released: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT released_forward_tx_hashes FROM shop.broker_purchases WHERE idempotency_key = $1",
    )
    .bind(&key)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        released,
        Some(vec![dead]),
        "the dead forward is kept apart from buys"
    );
    assert_eq!(
        chain.accepted_hashes(),
        vec![buy, forward],
        "one buy, one forward mined"
    );
    delete_key(&pool, "broker_purchases", &key).await;
    clear_items_pass(&pool).await;
}

/// A NAME buy whose recorded transaction looks dead on this chain: its send
/// never reached the node and a finalized transaction holds its nonce.
async fn dead_looking_name_buy(
    pool: &PgPool,
    chain: &Arc<ScriptedChain>,
    name: &str,
) -> (axum::Router, String, Value, String) {
    let recorded = recorded_unsent_name_buy(pool, chain, name).await;
    chain
        .taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .finalized_taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    recorded
}

/// A look that finds the transaction known contradicts an earlier dead look
/// and clears it, so dead, known, then dead again ten minutes after the
/// first look is a fresh first look: nothing is released.
#[tokio::test]
async fn a_known_look_between_two_dead_looks_releases_nothing() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, recorded) = dead_looking_name_buy(&pool, &chain, "flicker").await;

    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504);
    assert_eq!(dead_seen(&pool, &key).await, (true, None));

    age_dead_look(&pool, &key, 11, None).await;
    chain.known.lock().unwrap().push(recorded.clone());
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "{reply}");
    assert_eq!(
        dead_seen(&pool, &key).await,
        (false, None),
        "the known look clears it"
    );

    chain.known.lock().unwrap().clear();
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "a fresh first look, not a release: {reply}");
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!((row.status.as_str(), row.tx_hash), ("sent", Some(recorded)));
    assert_eq!(dead_seen(&pool, &key).await, (true, None));
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A transaction lookup whose reply carries neither `result` nor `error`
/// proves nothing: it is not a dead look.
#[tokio::test]
async fn a_lookup_reply_without_a_result_is_not_a_dead_look() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, _) = dead_looking_name_buy(&pool, &chain, "blankreply").await;
    chain
        .blank_tx_lookups
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504, "{reply}");
    assert_eq!(dead_seen(&pool, &key).await, (false, None));
    assert_eq!(recorded_purchase(&pool, &key).await.status, "sent");
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A buy released as dead that mines after all is found when the claim is
/// re-armed: it is recorded again and settled, and nothing new is signed.
#[tokio::test]
async fn a_released_buy_that_mined_after_all_is_resumed_not_bought_again() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, released) = dead_looking_name_buy(&pool, &chain, "lateminer").await;
    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504);
    age_dead_look(&pool, &key, 11, None).await;
    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 409);
    assert_eq!(
        dead_seen(&pool, &key).await,
        (false, Some(vec![released.clone()]))
    );

    chain.mined_elsewhere.lock().unwrap().push(released.clone());
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 200, "{reply}");
    assert_eq!(reply["txHash"], released.as_str());
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.tx_hash),
        ("confirmed", Some(released))
    );
    assert_eq!(dead_seen(&pool, &key).await, (false, Some(vec![])));
    assert!(
        chain.accepted_hashes().is_empty(),
        "nothing new was signed or sent"
    );
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A released buy that mined but reverted spent nothing: the re-armed claim
/// signs a new buy instead of restoring the reverted one.
#[tokio::test]
async fn a_released_buy_that_reverted_is_bought_again() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (app, key, body, released) = dead_looking_name_buy(&pool, &chain, "lostbuy").await;
    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 504);
    age_dead_look(&pool, &key, 11, None).await;
    let (status, _) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 409);

    chain.mined_elsewhere.lock().unwrap().push(released.clone());
    chain.reverted.lock().unwrap().push(released.clone());
    let (status, reply) = keyed_post(&app, "/v1/broker/names/buy", &key, &body).await;
    assert_eq!(status, 200, "{reply}");
    let row = recorded_purchase(&pool, &key).await;
    let bought = row.tx_hash.unwrap();
    assert_ne!(bought, released);
    assert_eq!(row.status, "confirmed");
    assert_eq!(chain.accepted_hashes(), vec![bought]);
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A confirmed released hash anywhere in the list counts, even after one
/// that reverted.
#[tokio::test]
async fn a_confirmed_released_buy_wins_over_an_earlier_revert() {
    use crate::credits::escrow::Broker;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (state, _) = scripted_state(&pool, chain.clone()).await;
    let broker = crate::credits::escrow::EconomyBroker::new(state.clone());
    let (reverted, confirmed) = (
        format!("0x{}", "ab".repeat(32)),
        format!("0x{}", "cd".repeat(32)),
    );
    let key = broker_claim_in(&pool, "error", None).await;
    sqlx::query(
        "UPDATE shop.broker_purchases SET tx_hash = NULL, released_tx_hashes = ARRAY[$2, $3] \
         WHERE idempotency_key = $1",
    )
    .bind(&key)
    .bind(&reverted)
    .bind(&confirmed)
    .execute(&pool)
    .await
    .unwrap();
    chain
        .mined_elsewhere
        .lock()
        .unwrap()
        .extend([reverted.clone(), confirmed.clone()]);
    chain.reverted.lock().unwrap().push(reverted);

    assert!(broker.released_buy_mined(&key).await.unwrap());
    let signer = state.eth_signer.as_ref().unwrap();
    let mined = crate::checkout::broker::mined_released_tx(&state, signer, &key)
        .await
        .unwrap();
    assert_eq!(
        mined,
        Some((confirmed, crate::evm::signer::ReceiptOutcome::Confirmed))
    );
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A released forward that already mined settles a resumed claim before a
/// new forward is signed (`bought`) or the recorded one is waited on or sent
/// again (`forwarding`).
#[tokio::test]
async fn a_mined_released_forward_settles_before_any_forward_is_sent() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (state, _) = scripted_state(&pool, chain.clone()).await;
    let released = format!("0x{}", "ef".repeat(32));
    chain.mined_elsewhere.lock().unwrap().push(released.clone());
    for status in ["bought", "forwarding"] {
        let key = broker_claim_in(&pool, status, Some("0xstuckforward")).await;
        sqlx::query(
            "UPDATE shop.broker_purchases SET minted_token_id = '42', \
             released_forward_tx_hashes = ARRAY[$2] WHERE idempotency_key = $1",
        )
        .bind(&key)
        .bind(&released)
        .execute(&pool)
        .await
        .unwrap();

        let settled = crate::checkout::broker::resume(&state, &key).await.unwrap();
        assert_eq!(settled, released, "{status}");
        let row = recorded_purchase(&pool, &key).await;
        assert_eq!(
            (row.status.as_str(), row.forward_tx_hash.as_deref()),
            ("confirmed", Some(released.as_str())),
            "{status}"
        );
        assert!(chain.accepted_hashes().is_empty(), "{status}: nothing sent");
        delete_key(&pool, "broker_purchases", &key).await;
    }
}

/// A revert seen late never overwrites a claim another request settled.
#[tokio::test]
async fn a_late_revert_leaves_a_confirmed_claim_confirmed() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let (state, _) = scripted_state(&pool, Arc::new(ScriptedChain::default())).await;
    // another request settled the claim before this one saw its revert
    let key = broker_claim_in(&pool, "confirmed", None).await;
    crate::checkout::broker::set_reverted(&state, &key).await;
    assert_eq!(recorded_purchase(&pool, &key).await.status, "confirmed");
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A broker claim under a fresh key moved to `status` with `leg_hash` as its
/// recorded buy (`sent`, `error`) or forward (`forwarding`).
async fn broker_claim_in(pool: &PgPool, status: &str, leg_hash: Option<&str>) -> String {
    let key = format!("broker-{}", Uuid::new_v4());
    claim_broker_purchase(
        pool,
        &BrokerPurchaseClaim {
            key: &key,
            collection: COLLECTION,
            item_id: Some("4"),
            token_id: None,
            buyer_address: BUYER,
            escrow_address: RULE_ESCROW,
            price_wei: "1000000000000000000",
            chain_id: 137,
            mode: "primary",
            trade_hashed_signature: None,
            usd_amount_wei: None,
            mana_usd_rate_wei: None,
        },
        now(),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE shop.broker_purchases SET status = $2, \
         tx_hash = CASE WHEN $2 = 'sent' THEN $3 ELSE '0xbuy' END, \
         forward_tx_hash = CASE WHEN $2 = 'forwarding' THEN $3 END \
         WHERE idempotency_key = $1",
    )
    .bind(&key)
    .bind(status)
    .bind(leg_hash)
    .execute(pool)
    .await
    .unwrap();
    key
}

/// A release is fenced on its dead look still being ten minutes old: if a
/// look found the leg alive and a fresh dead look followed, nothing is
/// released, for the buy leg and the forward leg alike.
#[tokio::test]
async fn a_release_needs_a_dead_look_still_ten_minutes_old() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (state, _) = scripted_state(&pool, chain).await;
    let buy = broker_claim_in(&pool, "sent", Some("0xdeadbuy")).await;
    let forward = broker_claim_in(&pool, "forwarding", Some("0xdeadforward")).await;

    for key in [&buy, &forward] {
        age_dead_look(&pool, key, 0, None).await;
    }
    assert!(
        !crate::checkout::broker::release_dead_buy(&state, &buy, "0xdeadbuy")
            .await
            .unwrap()
    );
    assert!(
        !crate::checkout::broker::release_dead_forward(&state, &forward, "0xdeadforward")
            .await
            .unwrap()
    );
    assert_eq!(recorded_purchase(&pool, &buy).await.status, "sent");
    assert_eq!(
        recorded_purchase(&pool, &forward).await.status,
        "forwarding"
    );

    for key in [&buy, &forward] {
        age_dead_look(&pool, key, 11, None).await;
    }
    assert!(
        crate::checkout::broker::release_dead_buy(&state, &buy, "0xdeadbuy")
            .await
            .unwrap()
    );
    assert!(
        crate::checkout::broker::release_dead_forward(&state, &forward, "0xdeadforward")
            .await
            .unwrap()
    );
    assert_eq!(recorded_purchase(&pool, &buy).await.status, "error");
    assert_eq!(recorded_purchase(&pool, &forward).await.status, "bought");
    delete_key(&pool, "broker_purchases", &buy).await;
    delete_key(&pool, "broker_purchases", &forward).await;
}

/// Only a released buy that mined successfully counts as having spent MANA:
/// one whose receipt reverted spent nothing, so its line stays refundable.
#[tokio::test]
async fn only_a_confirmed_released_buy_counts_as_spent() {
    use crate::credits::escrow::Broker;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let chain = Arc::new(ScriptedChain::default());
    let (state, _) = scripted_state(&pool, chain.clone()).await;
    let broker = crate::credits::escrow::EconomyBroker::new(state);
    let released = format!("0x{}", "ab".repeat(32));
    let key = broker_claim_in(&pool, "error", None).await;
    sqlx::query(
        "UPDATE shop.broker_purchases SET tx_hash = NULL, released_tx_hashes = ARRAY[$2] \
         WHERE idempotency_key = $1",
    )
    .bind(&key)
    .bind(&released)
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        !broker.released_buy_mined(&key).await.unwrap(),
        "no receipt: not spent"
    );
    chain.mined_elsewhere.lock().unwrap().push(released.clone());
    chain.reverted.lock().unwrap().push(released.clone());
    assert!(
        !broker.released_buy_mined(&key).await.unwrap(),
        "a reverted buy spent nothing"
    );
    chain.reverted.lock().unwrap().clear();
    assert!(
        broker.released_buy_mined(&key).await.unwrap(),
        "a confirmed buy spent MANA"
    );
    delete_key(&pool, "broker_purchases", &key).await;
}

/// A forward released as dead that mines after all delivered the token, so
/// the replacement forward reverts. The claim is confirmed with the released
/// forward instead of ending `reverted`.
#[tokio::test]
async fn a_released_forward_that_mined_after_all_confirms_the_claim() {
    let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
        .lock()
        .await;
    let Some(pool) = scratch_pool().await else {
        return;
    };
    seed_collection(&pool, COLLECTION).await;
    let creator = PrivateKeySigner::random();
    seed_item(
        &pool,
        COLLECTION,
        4,
        "wearable_v2",
        &format!("{:#x}", creator.address()),
    )
    .await;
    let chain = Arc::new(ScriptedChain::default());
    chain
        .pass_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .drop_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let (state, relayer) = scripted_state(&pool, chain.clone()).await;
    *chain.mint.lock().unwrap() = Some((COLLECTION.parse().unwrap(), relayer));
    let app = router(state.clone());
    let key = format!("broker-{}", Uuid::new_v4());
    let body = json!({ "collection": COLLECTION, "priceWei": "1000000000000000000", "escrowAddress": RULE_ESCROW, "buyerAddress": BUYER, "mode": "primary", "itemId": "4" });

    let (status, reply) = keyed_post(&app, "/v1/broker/buy", &key, &body).await;
    assert_eq!(status, 504, "{reply}");
    let first = recorded_purchase(&pool, &key)
        .await
        .forward_tx_hash
        .unwrap();
    chain
        .taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    chain
        .finalized_taken_nonces
        .store(1, std::sync::atomic::Ordering::SeqCst);
    assert!(crate::checkout::broker::resume(&state, &key).await.is_err());
    age_dead_look(&pool, &key, 11, None).await;
    // The replacement forward is signed but does not reach the node.
    chain
        .drop_sends
        .store(1, std::sync::atomic::Ordering::SeqCst);
    assert!(crate::checkout::broker::resume(&state, &key).await.is_err());
    let replacement = recorded_purchase(&pool, &key)
        .await
        .forward_tx_hash
        .unwrap();
    assert_ne!(replacement, first);

    // The released forward mines after all; the replacement then reverts.
    chain.mined_elsewhere.lock().unwrap().push(first.clone());
    chain.reverted.lock().unwrap().push(replacement.clone());
    let settled = crate::checkout::broker::resume(&state, &key).await.unwrap();
    assert_eq!(settled, first);
    let row = recorded_purchase(&pool, &key).await;
    assert_eq!(
        (row.status.as_str(), row.forward_tx_hash),
        ("confirmed", Some(first))
    );
    delete_key(&pool, "broker_purchases", &key).await;
    clear_items_pass(&pool).await;
}
