//! The old crate's contract tests over the module as `serve` mounts it
//! (nested under `/credits`), driven with a fake catalogue/order book/oracle
//! (`FakeMarket`) and a fake economy broker (`MockBroker`). DB-backed tests
//! use the process's scratch database (`CATALYST_TEST_DATABASE_URL`,
//! `db::scratch`), rows scoped by random wallets and uuid-suffixed keys,
//! deleted at the end; without the variable they skip. The old
//! `sql_capture` statement budgets are not carried (no capture hook in v2);
//! the observable results they guarded are.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use async_trait::async_trait;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::admin_store::{GrantRequest, PackSpec};
use super::authorize::NewAuthorization;
use super::checkout::{CartLine, RepricedLine};
use super::errors::ApiError;
use super::escrow::{Broker, BrokerError, BuyRequest, ManaPaymentVerification};
use super::packs::{PackRow, ReversalOutcome};
use super::pricing::{ItemInfo, ListingRow, MARKETPLACE_V2_POLYGON, Market, TradeLookup};
use super::store::Credits;
use super::{RawSettings, Settings, SharedState, State, router};
use crate::auth::{AuthLink, AuthLinkType};
use crate::http::Outbound;
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use crate::types::{ManaWei, UsdCents};

const COLLECTION: &str = "0xeede64bfaf8055492aa500846ec7c6e6a9f533d5";
/// The fulfilment tick claims every pending row of the shared scratch
/// database, so the tests that create outbox rows or drive the worker run
/// one at a time.
static OUTBOX_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// The creator the fake catalogue's items carry.
const CREATOR: &str = "0x00000000000000000000000000000000000000c1";
/// `4 << 216`: the token id a collections-v2 NFT minted from item 4 carries.
const ITEM_TOKEN_ID: &str = "421249166674228746791672110734681729275580381602196445017243910144";

/// A catalogue of `(collection, item_id) -> item`, an order book, a trade
/// book and a fixed MANA/USD spot; every read counted.
#[derive(Default)]
pub(crate) struct FakeMarket {
    pub items: HashMap<(String, String), ItemInfo>,
    pub orders: Vec<ListingRow>,
    pub trades: HashMap<String, TradeLookup>,
    pub mana_usd: Option<String>,
    pub item_reads: AtomicUsize,
    pub order_reads: AtomicUsize,
    pub oracle_reads: AtomicUsize,
}

impl FakeMarket {
    pub(super) fn with_item(
        mut self,
        collection: &str,
        item_id: &str,
        price_wei: &str,
        store_mintable: bool,
    ) -> Self {
        self.items.insert(
            (collection.to_string(), item_id.to_string()),
            ItemInfo {
                item_id: item_id.to_string(),
                urn: format!("urn:decentraland:matic:collections-v2:{collection}:{item_id}"),
                category: "wearable".to_string(),
                price_wei: price_wei.to_string(),
                contract_address: collection.to_string(),
                store_mintable,
                creator: CREATOR.to_string(),
                chain_id: 137,
            },
        );
        self
    }

    fn with_listing(mut self, collection: &str, price_wei: &str) -> Self {
        self.orders.push(ListingRow {
            contract_address: collection.to_string(),
            marketplace_address: MARKETPLACE_V2_POLYGON.to_string(),
            status: "open".to_string(),
            expires_at_ms: 0,
            token_id: Some(ITEM_TOKEN_ID.to_string()),
            trade_id: None,
            price_wei: price_wei.to_string(),
        });
        self
    }

    pub(super) fn with_mana_usd(mut self, usd: &str) -> Self {
        self.mana_usd = Some(usd.to_string());
        self
    }

    fn with_trade(mut self, id: &str, trade: Value) -> Self {
        self.trades.insert(id.to_string(), TradeLookup::Open(trade));
        self
    }
}

#[async_trait]
impl Market for FakeMarket {
    async fn items(
        &self,
        pairs: &[(String, String)],
    ) -> Result<Vec<Result<ItemInfo, ApiError>>, ApiError> {
        self.item_reads.fetch_add(1, Ordering::SeqCst);
        Ok(pairs
            .iter()
            .map(|(c, i)| {
                self.items
                    .get(&(c.to_ascii_lowercase(), i.clone()))
                    .cloned()
                    .ok_or_else(|| ApiError::not_found("item not found in catalog"))
            })
            .collect())
    }

    async fn open_orders(
        &self,
        _pairs: &[(String, String)],
        _per_contract: usize,
    ) -> Result<Vec<ListingRow>, ApiError> {
        self.order_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.orders.clone())
    }

    async fn trade(&self, trade_id: &str) -> Result<TradeLookup, ApiError> {
        Ok(self
            .trades
            .get(trade_id)
            .cloned()
            .unwrap_or(TradeLookup::NotFound))
    }

    async fn mana_usd(&self) -> Result<String, ApiError> {
        self.oracle_reads.fetch_add(1, Ordering::SeqCst);
        self.mana_usd
            .clone()
            .ok_or_else(|| ApiError::service_unavailable("no MANA price on this node"))
    }
}

/// A broker that confirms every buy with a synthetic hash (or refuses),
/// verifies one known top-up transaction and records what it was asked.
pub(crate) struct MockBroker {
    pub configured: bool,
    pub buys: Arc<parking_lot::Mutex<Vec<BuyRequest>>>,
    pub buy_answer: parking_lot::Mutex<Result<(), BrokerError>>,
    pub verifications: HashMap<String, ManaPaymentVerification>,
}

impl MockBroker {
    pub(super) fn configured() -> Self {
        Self {
            configured: true,
            buys: Arc::new(parking_lot::Mutex::new(Vec::new())),
            buy_answer: parking_lot::Mutex::new(Ok(())),
            verifications: HashMap::new(),
        }
    }

    fn unconfigured() -> Self {
        Self {
            configured: false,
            ..Self::configured()
        }
    }
}

#[async_trait]
impl Broker for MockBroker {
    fn configured(&self) -> bool {
        self.configured
    }

    fn escrow_address(&self) -> Option<String> {
        self.configured
            .then(|| "0x0000000000000000000000000000000000000001".to_string())
    }

    async fn buy(&self, req: &BuyRequest) -> Result<String, BrokerError> {
        self.buys.lock().push(req.clone());
        self.buy_answer
            .lock()
            .clone()
            .map(|_| format!("0x{}", req.idempotency_key.replace(':', "")))
    }

    async fn reclaim(
        &self,
        _collection: &str,
        _token_id: &str,
        idempotency_key: &str,
    ) -> Result<String, ApiError> {
        Ok(format!("0xreclaim-{idempotency_key}"))
    }

    async fn release(
        &self,
        _collection: &str,
        _token_id: &str,
        _buyer: &str,
        idempotency_key: &str,
    ) -> Result<String, ApiError> {
        Ok(format!("0xrelease-{idempotency_key}"))
    }

    async fn verify_mana_payment(
        &self,
        tx_hash: &str,
    ) -> Result<ManaPaymentVerification, ApiError> {
        self.verifications
            .get(tx_hash)
            .cloned()
            .ok_or_else(|| ApiError::bad_request("economy rejected the txHash"))
    }
}

fn lazy_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://nobody:nowhere@127.0.0.1:1/never")
        .expect("lazy pool")
}

async fn pool() -> Option<PgPool> {
    crate::db::scratch::pool(8).await
}

/// A database of this test's own, for the statements that walk EVERY wallet
/// (the realignment): on the shared scratch database they would adjust the
/// half-seeded wallets of the tests running beside them. Dropped at the end.
pub(super) struct OwnDatabase {
    pub(super) pool: PgPool,
    admin: PgPool,
    name: String,
}

impl OwnDatabase {
    pub(super) async fn create(label: &str) -> Option<Self> {
        let url = crate::db::scratch::url().await?;
        let opts: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_test_lib_{}_{label}_{}",
            std::process::id(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut own = url::Url::parse(&url).unwrap();
        own.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            own.as_str(),
            &crate::db::PoolSettings {
                max_connections: 4,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        Some(Self { pool, admin, name })
    }

    pub(super) async fn drop(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

struct StateCfg {
    mock_card: bool,
    fulfillment_mode: &'static str,
    stripe_webhook_secret: Option<&'static str>,
    onchain_authorize_enabled: bool,
    require_purchase_intent: bool,
    admin_token: Option<&'static str>,
    writes_enabled: bool,
}

impl Default for StateCfg {
    fn default() -> Self {
        Self {
            mock_card: false,
            fulfillment_mode: "secondary",
            stripe_webhook_secret: None,
            onchain_authorize_enabled: false,
            require_purchase_intent: false,
            admin_token: None,
            writes_enabled: true,
        }
    }
}

fn state_with(pool: PgPool, cfg: StateCfg, market: FakeMarket, broker: MockBroker) -> SharedState {
    let settings = Settings::parse(RawSettings {
        writes_enabled: cfg.writes_enabled,
        mock_card: cfg.mock_card,
        checkout_fulfillment_mode: cfg.fulfillment_mode.to_string(),
        stripe_webhook_secret: cfg.stripe_webhook_secret.map(str::to_string),
        credits_onchain_authorize_enabled: cfg.onchain_authorize_enabled,
        require_purchase_intent: cfg.require_purchase_intent,
        admin_token: cfg.admin_token.map(str::to_string),
        marketplace_markup_bps: 0,
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    Arc::new(State::with_sources(
        pool,
        Outbound::for_tests(),
        settings,
        Arc::new(market),
        Arc::new(broker),
        None,
    ))
}

fn test_state(pool: PgPool, cfg: StateCfg) -> SharedState {
    state_with(pool, cfg, FakeMarket::default(), MockBroker::configured())
}

pub(super) struct Wallet {
    signer: PrivateKeySigner,
    pub(super) address: String,
}

pub(super) fn scratch_wallet() -> Wallet {
    let signer = PrivateKeySigner::random();
    let address = format!("{:#x}", signer.address());
    Wallet { signer, address }
}

pub(super) async fn signed_headers(w: &Wallet, method: &str, path: &str) -> HeaderMap {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = build_payload_v6(method, path, &timestamp, "{}");
    let signature = w
        .signer
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
    headers.insert(AUTH_METADATA_HEADER, HeaderValue::from_static("{}"));
    for (i, link) in chain.iter().enumerate() {
        headers.insert(
            axum::http::HeaderName::from_bytes(format!("{AUTH_CHAIN_HEADER_PREFIX}{i}").as_bytes())
                .unwrap(),
            HeaderValue::from_str(&serde_json::to_string(link).unwrap()).unwrap(),
        );
    }
    headers
}

pub(super) fn app(state: SharedState) -> Router {
    Router::new().nest("/credits", router(state))
}

pub(super) async fn send(
    app: &Router,
    method: &str,
    path: &str,
    headers: HeaderMap,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers.iter() {
        req = req.header(k, v);
    }
    let req = match body {
        Some(v) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, headers, body)
}

/// A signed call over the PUBLIC path (`/credits/...`), the way the explorer signs it.
pub(super) async fn call(
    app: &Router,
    w: &Wallet,
    method: &str,
    path: &str,
    extra: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut headers = signed_headers(w, method, path).await;
    for (k, v) in extra {
        headers.insert(
            axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
            HeaderValue::from_str(v).unwrap(),
        );
    }
    let (status, _, body) = send(app, &method.to_uppercase(), path, headers, body).await;
    (status, body)
}

fn adr44() -> Value {
    json!({ "error": "Invalid Auth Chain", "message": "This endpoint requires a signed fetch request. See ADR-44." })
}

pub(super) async fn seed_wallet(pool: &PgPool, addr: &str, earned: &str, paid: &str) {
    sqlx::query(
        "INSERT INTO credits.wallets (address, available, earned_available, has_started_program) \
         VALUES ($1, $2::numeric + $3::numeric, $2::numeric, FALSE)",
    )
    .bind(addr)
    .bind(earned)
    .bind(paid)
    .execute(pool)
    .await
    .unwrap();
    for (kind, bucket, amt) in [("claim", "earned", earned), ("grant", "paid", paid)] {
        sqlx::query(
            "INSERT INTO credits.ledger (address, kind, amount, bucket, captcha_ok) \
             SELECT $1, $2, $3::numeric, $4, FALSE WHERE $3::numeric > 0",
        )
        .bind(addr)
        .bind(kind)
        .bind(amt)
        .bind(bucket)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn seed_paid_credits(pool: &PgPool, addr: &str, amount: &str) {
    sqlx::query("INSERT INTO credits.wallets (address, available, has_started_program) VALUES ($1, $2::numeric, FALSE)")
        .bind(addr)
        .bind(amount)
        .execute(pool)
        .await
        .unwrap();
}

/// Balance as exact NUMERIC text -- never f64.
pub(super) async fn available(pool: &PgPool, addr: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT COALESCE((SELECT available::text FROM credits.wallets WHERE address = $1), '0')",
    )
    .bind(addr)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn balances(pool: &PgPool, addr: &str) -> (String, String) {
    sqlx::query_as(
        "SELECT available::text, earned_available::text FROM credits.wallets WHERE address = $1",
    )
    .bind(addr)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// `true` when the signed ledger sum equals the balance, for BOTH the total
/// and the earned bucket -- evaluated by PostgreSQL in NUMERIC.
async fn ledger_matches_balance(pool: &PgPool, addr: &str) -> (bool, String, String) {
    sqlx::query_as(
        "WITH l AS ( \
             SELECT COALESCE(SUM(CASE \
                        WHEN kind IN ('grant','refund','purchase','claim') THEN amount \
                        WHEN kind IN ('spend','consume','expire') THEN -amount \
                        ELSE 0 END), 0) AS total, \
                    COALESCE(SUM(CASE \
                        WHEN bucket <> 'earned' THEN 0 \
                        WHEN kind IN ('grant','refund','purchase','claim') THEN amount \
                        WHEN kind IN ('spend','consume','expire') THEN -amount \
                        ELSE 0 END), 0) AS earned \
             FROM credits.ledger WHERE address = $1 \
         ), u AS ( \
             SELECT COALESCE((SELECT available FROM credits.wallets WHERE address = $1), 0) AS av, \
                    COALESCE((SELECT earned_available FROM credits.wallets WHERE address = $1), 0) AS ea \
         ) \
         SELECT (l.total = u.av AND l.earned = u.ea), (l.total - u.av)::text, (l.earned - u.ea)::text \
         FROM l, u",
    )
    .bind(addr)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub(super) async fn assert_reconciles(pool: &PgPool, addr: &str, ctx: &str) {
    let (ok, total_drift, earned_drift) = ledger_matches_balance(pool, addr).await;
    assert!(
        ok,
        "{ctx}: ledger replay must reproduce the balance (total drift {total_drift}, earned drift {earned_drift})"
    );
}

async fn ledger_kinds(pool: &PgPool, addr: &str) -> Vec<(String, String, String)> {
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT kind, bucket, amount::text FROM credits.ledger WHERE address = $1 ORDER BY id",
    )
    .bind(addr)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn ledger(pool: &PgPool, addr: &str, kind: &str) -> Vec<(String, String)> {
    sqlx::query_as::<_, (String, String)>("SELECT bucket, amount::text FROM credits.ledger WHERE address = $1 AND kind = $2 ORDER BY bucket")
        .bind(addr)
        .bind(kind)
        .fetch_all(pool)
        .await
        .unwrap()
}

/// The settlement pass writes money (it retires stale pack orders and sends
/// revocations): held by the restore stamps it writes nothing, and resumes
/// once they vouch for the node.
#[tokio::test]
async fn the_settlement_pass_skips_while_the_restore_stamps_hold() {
    use crate::shared::writes::PassGate;
    use crate::shared::writes::restore::{lab::StampLab, scoped};
    let Some(own) = OwnDatabase::create("settle_hold").await else {
        return;
    };
    let wallet = scratch_wallet();
    sqlx::query(
        "INSERT INTO credits.purchases (address, sku, credits, amount_cents, currency, stripe_checkout_session, method, status, created_at) \
         VALUES ($1, 'test-pack', 10, 100, 'usd', 'cs_stale', 'card', 'pending', timezone('UTC', now()) - interval '27 hours')",
    )
    .bind(&wallet.address)
    .execute(&own.pool)
    .await
    .unwrap();
    let status = || async {
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM credits.purchases WHERE stripe_checkout_session = 'cs_stale'",
        )
        .fetch_one(&own.pool)
        .await
        .unwrap()
    };
    let credits = Credits::new(own.pool.clone());
    let gate = PassGate::new("credits-settlement", crate::shared::writes::ops_refusal);
    let held = StampLab::new();
    let ran = scoped(
        held.stamps.clone(),
        super::settlement::settlement_pass(&credits, None, &gate, true, None),
    )
    .await
    .unwrap();
    assert!(!ran);
    assert_eq!(status().await, "pending", "held: nothing retired");
    let fresh = StampLab::fresh();
    let ran = scoped(
        fresh.stamps.clone(),
        super::settlement::settlement_pass(&credits, None, &gate, true, None),
    )
    .await
    .unwrap();
    assert!(ran);
    assert_eq!(status().await, "abandoned");
    own.drop().await;
}

async fn seed_paid_purchase(pool: &PgPool, addr: &str, pi: &str, credits: &str, amount_cents: i64) {
    sqlx::query(
        "INSERT INTO credits.purchases (address, sku, credits, amount_cents, currency, stripe_payment_intent, method, status) \
         VALUES ($1, 'test-pack', $2::numeric, $3, 'usd', $4, 'card', 'paid')",
    )
    .bind(addr)
    .bind(credits)
    .bind(amount_cents)
    .bind(pi)
    .execute(pool)
    .await
    .unwrap();
}

async fn purchase_revoked(pool: &PgPool, pi: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT revoked_credits::text FROM credits.purchases WHERE stripe_payment_intent = $1",
    )
    .bind(pi)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn cleanup(pool: &PgPool, addr: &str) {
    for sql in [
        "DELETE FROM credits.fulfillment_outbox WHERE checkout_id IN (SELECT id FROM credits.checkouts WHERE address = $1)",
        "DELETE FROM credits.checkouts WHERE address = $1",
        "DELETE FROM credits.cart_items WHERE cart_id IN (SELECT id FROM credits.carts WHERE address = $1)",
        "DELETE FROM credits.carts WHERE address = $1",
        "DELETE FROM credits.authorizations WHERE address = $1",
        "DELETE FROM credits.captcha_challenges WHERE address = $1",
        "DELETE FROM credits.ledger WHERE address = $1",
        "DELETE FROM credits.purchases WHERE address = $1",
        "DELETE FROM credits.idempotency WHERE address = $1",
        "DELETE FROM ops.admin_audit WHERE domain = 'credits' AND target = $1",
        "DELETE FROM shop.usage_grants WHERE grantee_address = $1",
        "DELETE FROM credits.wallets WHERE address = $1",
    ] {
        sqlx::query(sql).bind(addr).execute(pool).await.unwrap();
    }
}

fn unique(tag: &str) -> String {
    format!("{tag}-{}", uuid::Uuid::new_v4().simple())
}

fn line(item_id: &str, unit_price_credits: &str) -> RepricedLine {
    RepricedLine {
        item_id: item_id.into(),
        collection: COLLECTION.into(),
        urn: format!("urn:decentraland:matic:collections-v2:{COLLECTION}:{item_id}"),
        category: "wearable".into(),
        qty: 1,
        unit_price_credits: unit_price_credits.into(),
        token_id: None,
        trade_id: None,
        basis_wei: Some("0".into()),
        mode: "primary".into(),
    }
}

fn cart_line<'a>(address: &'a str, item_id: &'a str, urn: &'a str, qty: i32) -> CartLine<'a> {
    CartLine {
        address,
        item_id,
        collection: COLLECTION,
        urn,
        category: "wearable",
        qty,
    }
}

#[tokio::test]
async fn spend_debits_earned_first_and_refund_restores_split() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "20", "15").await;
    let credits = Credits::new(pool.clone());

    credits
        .spend(&addr, "25", "test:bucket-spend", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("10".to_string(), "0".to_string())
    );
    assert_eq!(
        ledger(&pool, &addr, "spend").await,
        vec![
            ("earned".to_string(), "20".to_string()),
            ("paid".to_string(), "5".to_string())
        ]
    );

    credits
        .refund(&addr, "25", "test:bucket-spend", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string())
    );
    assert_eq!(
        ledger(&pool, &addr, "refund").await,
        vec![
            ("earned".to_string(), "20".to_string()),
            ("paid".to_string(), "5".to_string())
        ]
    );

    let second = credits
        .refund(&addr, "25", "test:bucket-spend", None)
        .await
        .unwrap();
    assert_eq!(second.applied.parse::<f64>().unwrap(), 0.0);
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string())
    );
    assert_eq!(
        ledger(&pool, &addr, "refund").await.len(),
        2,
        "a refund past the window writes no row"
    );
    assert_reconciles(&pool, &addr, "after the round trip").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn partial_refund_does_not_over_restore_earned() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "20", "15").await;
    let credits = Credits::new(pool.clone());

    credits
        .spend(&addr, "25", "test:bucket-partial", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("10".to_string(), "0".to_string())
    );

    credits
        .refund(&addr, "3", "test:bucket-partial", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("13".to_string(), "3".to_string())
    );

    credits
        .refund(&addr, "22", "test:bucket-partial", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string())
    );

    let sums: Vec<(String, String)> = sqlx::query_as(
        "SELECT bucket, SUM(amount)::text FROM credits.ledger WHERE address = $1 AND kind = 'refund' GROUP BY bucket ORDER BY bucket",
    )
    .bind(&addr)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        sums,
        vec![
            ("earned".to_string(), "20".to_string()),
            ("paid".to_string(), "5".to_string())
        ]
    );

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn expired_history_wallet_refunds_normally() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "0", "15").await;
    for (kind, amt) in [("claim", "20"), ("expire", "20")] {
        sqlx::query("INSERT INTO credits.ledger (address, kind, amount, tx_ref, bucket, captcha_ok) VALUES ($1, $2, $3::numeric, 'season-expiry', 'earned', FALSE)")
            .bind(&addr)
            .bind(kind)
            .bind(amt)
            .execute(&pool)
            .await
            .unwrap();
    }
    let credits = Credits::new(pool.clone());

    let outcome = credits
        .refund(&addr, "5", "test:bucket-no-spend", None)
        .await
        .unwrap();
    assert_eq!(
        outcome.applied.parse::<f64>().unwrap(),
        0.0,
        "a refund is bounded by what was spent under its tx_ref"
    );
    assert_eq!(
        balances(&pool, &addr).await,
        ("15".to_string(), "0".to_string())
    );
    assert!(ledger(&pool, &addr, "refund").await.is_empty());

    credits
        .spend(&addr, "5", "test:bucket-historic-spend", None)
        .await
        .unwrap();
    let outcome = credits
        .refund(&addr, "5", "test:bucket-historic-spend", None)
        .await
        .unwrap();
    assert_eq!(outcome.applied.parse::<f64>().unwrap(), 5.0);
    assert_eq!(
        balances(&pool, &addr).await,
        ("15".to_string(), "0".to_string())
    );
    assert_eq!(
        ledger(&pool, &addr, "refund").await,
        vec![("paid".to_string(), "5".to_string())]
    );

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn refund_rejects_invalid_amounts() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    for bad in ["-5", "NaN", "1e400", "0"] {
        let err = credits
            .refund(&addr, bad, "test:bucket-bad-amount", None)
            .await
            .expect_err("invalid refund amount must be rejected");
        assert_eq!(err.status().as_u16(), 400, "for amount {bad:?}");
    }
}

#[tokio::test]
async fn manual_checkout_refund_closes_fulfilling_checkout() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "20", "15").await;
    let credits = Credits::new(pool.clone());

    let checkout_id: i64 = sqlx::query_scalar(
        "INSERT INTO credits.checkouts (idempotency_key, address, total_credits, status) VALUES ($1, $2, 25, 'fulfilling') RETURNING id",
    )
    .bind(unique("test-close"))
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    let tx_ref = format!("checkout:{}", checkout_id);
    credits.spend(&addr, "25", &tx_ref, None).await.unwrap();

    let (outcome, closed) = credits
        .refund_checkout_manual(checkout_id, &addr, "25")
        .await
        .unwrap();
    assert!(closed, "fulfilling checkout must transition on refund");
    assert_eq!(outcome.applied.parse::<f64>().unwrap(), 25.0);
    let status: String = sqlx::query_scalar("SELECT status FROM credits.checkouts WHERE id = $1")
        .bind(checkout_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string())
    );

    let (replay, closed_again) = credits
        .refund_checkout_manual(checkout_id, &addr, "25")
        .await
        .unwrap();
    assert!(replay.replayed);
    assert!(!closed_again);
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string())
    );

    // The audited form writes the operator row in the closing statement.
    let checkout2: i64 = sqlx::query_scalar(
        "INSERT INTO credits.checkouts (idempotency_key, address, total_credits, status) VALUES ($1, $2, 6, 'fulfilling') RETURNING id",
    )
    .bind(unique("test-close-audited"))
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    credits
        .spend(&addr, "6", &format!("checkout:{checkout2}"), None)
        .await
        .unwrap();
    let (refund, closed) = credits
        .refund_checkout_manual_audited(
            checkout2,
            &addr,
            "6",
            "token:qc-admin",
            |o| json!({ "applied": o.applied, "replayed": o.replayed, "reason": "qc" }),
        )
        .await
        .unwrap();
    assert!(closed && refund.applied == "6" && !refund.replayed);
    let (status, detail, actor): (String, Value, String) = sqlx::query_as(
        "SELECT c.status, a.detail, a.actor FROM credits.checkouts c \
         JOIN ops.admin_audit a ON a.target = c.id::text AND a.domain = 'credits' AND a.action = 'checkout.refund' \
         WHERE c.id = $1 ORDER BY a.id DESC LIMIT 1",
    )
    .bind(checkout2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(actor, "token:qc-admin");
    assert_eq!(detail["closed"], true);
    assert_eq!(detail["applied"], "6");
    assert_eq!(detail["reason"], "qc");
    sqlx::query("DELETE FROM ops.admin_audit WHERE domain = 'credits' AND action = 'checkout.refund' AND target = $1").bind(checkout2.to_string()).execute(&pool).await.unwrap();

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn reclaim_line_lookup_resolves_checkout_and_matches_earned_window() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "20", "15").await;
    let credits = Credits::new(pool.clone());

    let checkout_id: i64 = sqlx::query_scalar(
        "INSERT INTO credits.checkouts (idempotency_key, address, total_credits, status) VALUES ($1, $2, 25, 'fulfilled') RETURNING id",
    )
    .bind(unique("test-reclaim"))
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    let escrow_ref = unique("escrow");
    sqlx::query(
        "INSERT INTO credits.fulfillment_outbox (checkout_id, item_id, collection, urn, unit_price_credits, mode, status, external_ref) \
         VALUES ($1, 'item-1', $3, 'urn:test', 25, 'secondary', 'confirmed', $2)",
    )
    .bind(checkout_id)
    .bind(&escrow_ref)
    .bind(COLLECTION)
    .execute(&pool)
    .await
    .unwrap();
    let tx_ref = format!("checkout:{}", checkout_id);
    credits.spend(&addr, "25", &tx_ref, None).await.unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("10".to_string(), "0".to_string())
    );

    let (found_id, found_addr, found_amount) = credits
        .find_confirmed_line_by_ref(&escrow_ref)
        .await
        .unwrap()
        .expect("confirmed line must resolve");
    assert_eq!(found_id, checkout_id);
    assert_eq!(found_addr, addr);
    assert_eq!(found_amount.parse::<f64>().unwrap(), 25.0);

    credits
        .refund(
            &found_addr,
            &found_amount,
            &format!("checkout:{}", found_id),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string())
    );
    assert_eq!(
        ledger(&pool, &addr, "refund").await,
        vec![
            ("earned".to_string(), "20".to_string()),
            ("paid".to_string(), "5".to_string())
        ]
    );

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn dispute_revokes_granted_credits_instead_of_paying_the_buyer_twice() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let pi = unique("pi_dispute");
    let credits = Credits::new(pool.clone());

    seed_paid_purchase(&pool, &addr, &pi, "100", 1000).await;
    credits
        .admin_grant_credits(GrantRequest {
            address: &addr,
            amount: "100",
            kind: "purchase",
            reason: Some("stripe purchase"),
            actor: Some("stripe"),
            idempotency_key: None,
            detail: &json!({}),
        })
        .await
        .unwrap();
    assert_eq!(available(&pool, &addr).await, "100");

    let outcome = credits
        .record_full_reversal(&pi, "disputed", &unique("evt_dispute"))
        .await
        .unwrap();
    match outcome {
        ReversalOutcome::Reversed {
            charged_back,
            removed,
            has_shortfall,
            ..
        } => {
            assert_eq!(charged_back.parse::<f64>().unwrap(), 100.0);
            assert_eq!(removed.parse::<f64>().unwrap(), 100.0);
            assert!(!has_shortfall, "the buyer had not spent anything");
        }
        other => panic!("expected a reversal, got {other:?}"),
    }

    assert_eq!(
        available(&pool, &addr).await.parse::<f64>().unwrap(),
        0.0,
        "a chargeback must REMOVE the granted credits, never add them"
    );
    let rows = ledger_kinds(&pool, &addr).await;
    assert!(
        !rows.iter().any(|(kind, _, _)| kind == "refund"),
        "a fiat reversal is not a credits refund: {rows:?}"
    );
    assert!(
        rows.iter().any(|(kind, _, _)| kind == "consume"),
        "the reversal must appear as a debit row: {rows:?}"
    );
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.admin_audit WHERE domain = 'credits' AND action = 'revoke' AND target = $1")
        .bind(&addr)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audited, 1, "the reversal is audited");
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id WHERE l.address = $1 AND l.kind = 'consume'",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 1, "the consume row names its audit row");
    assert_reconciles(&pool, &addr, "after dispute").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn charge_refund_revokes_and_reports_the_unrecoverable_shortfall() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let pi = unique("pi_short");
    let credits = Credits::new(pool.clone());

    seed_paid_purchase(&pool, &addr, &pi, "100", 1000).await;
    credits
        .admin_grant_credits(GrantRequest {
            address: &addr,
            amount: "100",
            kind: "purchase",
            reason: None,
            actor: Some("stripe"),
            idempotency_key: None,
            detail: &json!({}),
        })
        .await
        .unwrap();
    credits
        .spend(&addr, "70", "checkout:formal-shortfall", None)
        .await
        .unwrap();
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 30.0);

    let outcome = credits
        .record_charge_refund(&pi, 1000, &unique("evt_short"))
        .await
        .unwrap();
    match outcome {
        ReversalOutcome::Reversed {
            charged_back,
            removed,
            shortfall,
            has_shortfall,
            ..
        } => {
            assert_eq!(charged_back.parse::<f64>().unwrap(), 100.0);
            assert_eq!(removed.parse::<f64>().unwrap(), 30.0);
            assert_eq!(shortfall.parse::<f64>().unwrap(), 70.0);
            assert!(has_shortfall);
        }
        other => panic!("expected a reversal, got {other:?}"),
    }
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 0.0);
    assert_reconciles(&pool, &addr, "after partial-clawback reversal").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn revoke_debits_the_paid_bucket_first() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let pi = unique("pi_paidfirst");
    let credits = Credits::new(pool.clone());

    seed_wallet(&pool, &addr, "40", "60").await;
    seed_paid_purchase(&pool, &addr, &pi, "50", 500).await;

    credits
        .record_full_reversal(&pi, "disputed", &unique("evt_pf"))
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("50".to_string(), "40".to_string()),
        "a chargeback takes PAID credits back first"
    );

    let debits: Vec<(String, String)> = sqlx::query_as("SELECT bucket, amount::text FROM credits.ledger WHERE address = $1 AND kind = 'consume' ORDER BY id")
        .bind(&addr)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(debits, vec![("paid".to_string(), "50".to_string())]);

    let pi2 = unique("pi_paidfirst2");
    seed_paid_purchase(&pool, &addr, &pi2, "30", 300).await;
    credits
        .record_full_reversal(&pi2, "disputed", &unique("evt_pf2"))
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("20".to_string(), "20".to_string()),
        "paid 10 absorbed first, the remaining 20 came out of earned"
    );
    assert_reconciles(&pool, &addr, "after paid-first revocations").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn revoke_without_a_wallet_row_is_still_audited_as_a_total_loss() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let pi = unique("pi_nowallet");
    let credits = Credits::new(pool.clone());

    seed_paid_purchase(&pool, &addr, &pi, "100", 1000).await;
    assert_eq!(available(&pool, &addr).await, "0");

    let outcome = credits
        .record_full_reversal(&pi, "disputed", &unique("evt_nw"))
        .await
        .unwrap();
    match outcome {
        ReversalOutcome::Reversed {
            removed,
            shortfall,
            has_shortfall,
            ..
        } => {
            assert_eq!(removed.parse::<f64>().unwrap(), 0.0);
            assert_eq!(shortfall.parse::<f64>().unwrap(), 100.0);
            assert!(has_shortfall, "nothing could be clawed back at all");
        }
        other => panic!("expected a reversal, got {other:?}"),
    }

    let audits: Vec<(String, Value)> =
        sqlx::query_as("SELECT action, detail FROM ops.admin_audit WHERE domain = 'credits' AND target = $1 AND action = 'revoke' ORDER BY id")
            .bind(&addr)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        audits.len(),
        1,
        "the no-wallet chargeback must leave exactly one audit row: {audits:?}"
    );
    let (_, detail) = &audits[0];
    assert_eq!(detail["amount"], json!("0"), "nothing was removed");
    assert_eq!(detail["removed"], json!("0"));
    assert_eq!(detail["shortfall"], json!("100"));
    assert_eq!(detail["chargedBack"], json!("100"));
    assert_eq!(
        detail["walletRow"],
        json!("missing"),
        "finance must be able to tell this apart from a partial clawback"
    );

    assert!(ledger_kinds(&pool, &addr).await.is_empty());
    assert_reconciles(&pool, &addr, "after a no-wallet chargeback").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn refund_under_a_tx_ref_with_no_spend_applies_nothing() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_wallet(&pool, &addr, "0", "15").await;

    let outcome = credits
        .refund(&addr, "1000000", "evt_1Abc_stripe_event_id", None)
        .await
        .unwrap();
    assert_eq!(
        outcome.applied.parse::<f64>().unwrap(),
        0.0,
        "an unmatched tx_ref must not mint credits"
    );
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 15.0);
    assert_reconciles(&pool, &addr, "after unmatched refund").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn pack_reversal_is_bounded_by_the_purchase_across_replays() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let pi = unique("pi_bound");
    let credits = Credits::new(pool.clone());

    seed_paid_purchase(&pool, &addr, &pi, "100", 1000).await;
    credits
        .admin_grant_credits(GrantRequest {
            address: &addr,
            amount: "100",
            kind: "purchase",
            reason: None,
            actor: Some("stripe"),
            idempotency_key: None,
            detail: &json!({}),
        })
        .await
        .unwrap();

    credits
        .record_charge_refund(&pi, 300, &unique("evt_b1"))
        .await
        .unwrap();
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 70.0);

    assert!(matches!(
        credits
            .record_charge_refund(&pi, 300, &unique("evt_b1r"))
            .await
            .unwrap(),
        ReversalOutcome::NothingToReverse
    ));
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 70.0);

    credits
        .record_charge_refund(&pi, 1000, &unique("evt_b2"))
        .await
        .unwrap();
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 0.0);
    assert_eq!(
        purchase_revoked(&pool, &pi).await.parse::<f64>().unwrap(),
        100.0,
        "a fully reversed purchase lands on exactly its granted credits"
    );

    let after = credits
        .record_full_reversal(&pi, "disputed", &unique("evt_b3"))
        .await
        .unwrap();
    assert!(
        matches!(after, ReversalOutcome::NoPaidPurchase),
        "a fully-refunded purchase is no longer 'paid': {after:?}"
    );
    assert_eq!(
        purchase_revoked(&pool, &pi).await.parse::<f64>().unwrap(),
        100.0
    );
    assert_reconciles(&pool, &addr, "after bounded reversals").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn over_revocation_is_rejected_by_the_database() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let pi = unique("pi_check");
    seed_paid_purchase(&pool, &addr, &pi, "100", 1000).await;

    let err = sqlx::query("UPDATE credits.purchases SET revoked_credits = 100.0000001 WHERE stripe_payment_intent = $1")
        .bind(&pi)
        .execute(&pool)
        .await
        .expect_err("revoking more than the purchase granted must be rejected");
    assert!(
        err.to_string().contains("purchases_revoked_bound_check"),
        "expected the revocation bound to fire, got: {err}"
    );

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn spend_rejects_non_positive_amounts_and_never_mints() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_wallet(&pool, &addr, "20", "80").await;

    for bad in [
        "-5",
        "-0.000001",
        "-1e-9",
        "-1e400",
        "NaN",
        "Infinity",
        "1e400",
        "abc",
        "",
    ] {
        let err = credits
            .spend(&addr, bad, "checkout:formal-guard", None)
            .await
            .expect_err("spend must reject");
        assert_eq!(err.status().as_u16(), 400, "for amount {bad:?}");
        assert_eq!(
            available(&pool, &addr).await.parse::<f64>().unwrap(),
            100.0,
            "rejected spend {bad:?} must not move the balance"
        );
    }
    assert_reconciles(&pool, &addr, "after rejected spends").await;

    for bad in ["-5", "0"] {
        assert_eq!(
            credits
                .admin_revoke_credits(&addr, bad, None, Some("t"), &json!({}))
                .await
                .expect_err("admin revoke must reject")
                .status()
                .as_u16(),
            400
        );
        assert_eq!(
            credits
                .admin_grant_credits(GrantRequest {
                    address: &addr,
                    amount: bad,
                    kind: "grant",
                    reason: None,
                    actor: Some("t"),
                    idempotency_key: None,
                    detail: &json!({})
                })
                .await
                .expect_err("admin grant must reject")
                .status()
                .as_u16(),
            400
        );
    }
    assert_eq!(available(&pool, &addr).await.parse::<f64>().unwrap(), 100.0);

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn zero_spend_is_a_no_op_not_an_error() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_wallet(&pool, &addr, "20", "80").await;
    let ledger_before = ledger_kinds(&pool, &addr).await;

    for zero in ["0", "0.00", "0e10", "-0", "+0", ".0"] {
        let outcome = credits
            .spend(&addr, zero, "checkout:formal-zero", None)
            .await
            .unwrap_or_else(|e| panic!("zero spend {zero:?} must succeed, got {e:?}"));
        assert!(!outcome.replayed);
        assert_eq!(outcome.applied.parse::<f64>().unwrap(), 0.0);
        assert_eq!(outcome.available, "100");
        assert_eq!(available(&pool, &addr).await, "100");
    }
    assert_eq!(
        ledger_kinds(&pool, &addr).await,
        ledger_before,
        "a zero spend must write NO ledger row"
    );
    assert_reconciles(&pool, &addr, "after zero spends").await;

    let fresh = scratch_wallet().address;
    let outcome = credits
        .spend(&fresh, "0", "checkout:formal-zero-fresh", None)
        .await
        .expect("a zero spend needs no wallet row");
    assert_eq!(outcome.available, "0");
    assert!(ledger_kinds(&pool, &fresh).await.is_empty());

    let key = unique("t:zero");
    credits
        .spend(&addr, "0", "checkout:formal-zero", Some(&key))
        .await
        .unwrap();
    let real = credits
        .spend(&addr, "25", "checkout:formal-zero", Some(&key))
        .await
        .expect("a zero spend must not consume the key for a later real spend");
    assert_eq!(real.applied, "25");
    assert_eq!(available(&pool, &addr).await, "75");
    assert_reconciles(&pool, &addr, "after zero-then-real spend").await;

    cleanup(&pool, &addr).await;
    cleanup(&pool, &fresh).await;
}

#[tokio::test]
async fn zero_total_checkout_completes_and_moves_nothing() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let credits = Credits::new(pool.clone());
    let urn = format!("urn:decentraland:matic:collections-v2:{COLLECTION}:1");

    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "20", "80").await;
    let before = ledger_kinds(&pool, &addr).await;
    credits
        .add_item(cart_line(&addr, "1", &urn, 1), "0")
        .await
        .unwrap();
    let out = credits
        .run_checkout(&addr, &unique("t:zero-co"), &[line("1", "0")])
        .await
        .expect("an all-free cart must check out");
    assert!(!out.replayed);
    assert_eq!(out.status, "fulfilling");
    assert_eq!(
        available(&pool, &addr).await,
        "100",
        "a zero-total checkout must not move the balance"
    );
    assert_eq!(
        ledger_kinds(&pool, &addr).await,
        before,
        "a zero-total checkout must write no ledger row"
    );
    assert_reconciles(&pool, &addr, "after a zero-total checkout").await;

    let fresh = scratch_wallet().address;
    credits
        .add_item(cart_line(&fresh, "1", &urn, 1), "0")
        .await
        .unwrap();
    let out = credits
        .run_checkout(&fresh, &unique("t:zero-co"), &[line("1", "0")])
        .await
        .expect("a zero total is affordable without a wallet row");
    assert_eq!(out.status, "fulfilling");
    assert!(ledger_kinds(&pool, &fresh).await.is_empty());

    cleanup(&pool, &addr).await;
    cleanup(&pool, &fresh).await;
}

#[tokio::test]
async fn sub_f64_amounts_keep_ledger_and_balance_in_lockstep() {
    let Some(pool) = pool().await else { return };
    let credits = Credits::new(pool.clone());
    let dust = format!("0.{}1", "0".repeat(399));
    assert_eq!(
        dust.parse::<f64>().unwrap(),
        0.0,
        "the witness must be invisible to f64"
    );

    let addr = scratch_wallet().address;
    seed_wallet(&pool, &addr, "20", "15").await;
    credits
        .spend(&addr, &dust, "checkout:formal-dust", None)
        .await
        .unwrap();
    assert_reconciles(&pool, &addr, "after sub-f64 spend").await;
    let spends: Vec<(String, String)> = sqlx::query_as(
        "SELECT bucket, amount::text FROM credits.ledger WHERE address = $1 AND kind = 'spend'",
    )
    .bind(&addr)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(spends.len(), 1, "exactly one debit row: {spends:?}");
    assert_eq!(spends[0].0, "earned", "the debit came out of earned");

    credits
        .refund(&addr, &dust, "checkout:formal-dust", None)
        .await
        .unwrap();
    assert_reconciles(&pool, &addr, "after sub-f64 refund").await;

    credits
        .admin_revoke_credits(&addr, &dust, None, Some("t"), &json!({}))
        .await
        .unwrap();
    assert_reconciles(&pool, &addr, "after sub-f64 revoke").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn clamped_origin_is_recorded_as_an_adjustment_row_and_the_walk_is_batched() {
    let Some(db) = OwnDatabase::create("realign").await else {
        return;
    };
    let pool = db.pool.clone();
    let credits = Credits::new(pool.clone());

    let addrs: Vec<String> = (0..3).map(|_| scratch_wallet().address).collect();
    for (i, addr) in addrs.iter().enumerate() {
        sqlx::query("INSERT INTO credits.wallets (address, available, earned_available, has_started_program) VALUES ($1, $2, $3, FALSE)")
            .bind(addr)
            .bind(10 + i as i32)
            .bind(10 + i as i32)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO credits.ledger (address, kind, amount, bucket, captcha_ok) VALUES ($1, 'claim', $2, 'earned', FALSE)")
            .bind(addr)
            .bind(50 + i as i32)
            .execute(&pool)
            .await
            .unwrap();
        let (ok, _, _) = ledger_matches_balance(&pool, addr).await;
        assert!(!ok, "wallet {addr} must start out divergent");
    }
    let report = credits.reconcile().await.unwrap();
    assert!(!report.ok);
    assert_eq!(report.ledger_balance_mismatches.len(), 3);
    assert_eq!(report.earned_balance_mismatches.len(), 3);

    let applied = credits
        .realign_ledger_to_balances_in_batches("test: batched walk", 1)
        .await
        .unwrap();
    for addr in &addrs {
        assert!(
            applied.iter().any(|a| &a.address == addr),
            "batching must not skip {addr}: {applied:?}"
        );
        assert_reconciles(&pool, addr, "after batched realignment").await;
    }
    let adjustments: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT kind, bucket, amount::text FROM credits.ledger WHERE address = $1 AND tx_ref = $2 ORDER BY id",
    )
    .bind(&addrs[0])
    .bind(super::reconcile::REALIGN_TX_REF)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        adjustments,
        vec![("consume".into(), "earned".into(), "40".into())],
        "earned ledger sum 50 vs stored 10 -> a -40 earned adjustment"
    );
    let audited: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.admin_audit WHERE domain = 'credits' AND action = 'ledger_realign'").fetch_one(&pool).await.unwrap();
    assert_eq!(audited, 3, "one audit row per adjustment");

    let second = credits
        .realign_ledger_to_balances("test: rerun")
        .await
        .unwrap();
    assert!(
        second.is_empty(),
        "realignment must be idempotent: {second:?}"
    );
    let report = credits.reconcile().await.unwrap();
    assert!(report.ok, "{report:?}");

    db.drop().await;
}

#[tokio::test]
async fn refund_restores_earned_first_by_design() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_wallet(&pool, &addr, "20", "15").await;

    credits
        .spend(&addr, "25", "checkout:formal-earned-first", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("10".to_string(), "0".to_string())
    );
    credits
        .refund(&addr, "5", "checkout:formal-earned-first", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("15".to_string(), "5".to_string()),
        "earned-first restore mirrors the earned-first debit rule"
    );
    credits
        .refund(&addr, "20", "checkout:formal-earned-first", None)
        .await
        .unwrap();
    assert_eq!(
        balances(&pool, &addr).await,
        ("35".to_string(), "20".to_string()),
        "spend then full refund is the identity"
    );
    assert_reconciles(&pool, &addr, "after earned-first round trip").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn cumulative_refund_never_exceeds_cumulative_spend_in_any_order() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_wallet(&pool, &addr, "0", "1000").await;
    let tx_ref = unique("checkout:formal-window");

    let early = credits
        .refund(&addr, "100", &tx_ref, Some(&unique("t:w1")))
        .await
        .unwrap();
    assert_eq!(
        early.applied, "0",
        "a refund with no spend to refund must apply nothing"
    );
    assert_eq!(available(&pool, &addr).await, "1000");

    credits.spend(&addr, "50", &tx_ref, None).await.unwrap();
    assert_eq!(available(&pool, &addr).await, "950");

    let honest = credits
        .refund(&addr, "50", &tx_ref, Some(&unique("t:w2")))
        .await
        .unwrap();
    assert_eq!(honest.applied, "50");
    assert_eq!(
        available(&pool, &addr).await,
        "1000",
        "the early refund must not have consumed the window"
    );

    let over = credits
        .refund(&addr, "50", &tx_ref, Some(&unique("t:w3")))
        .await
        .unwrap();
    assert_eq!(over.applied, "0");
    assert_eq!(available(&pool, &addr).await, "1000");
    assert_reconciles(&pool, &addr, "after the refund-window round trip").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn concurrent_refunds_under_one_tx_ref_cannot_double_pay() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_wallet(&pool, &addr, "0", "100").await;
    let tx_ref = unique("checkout:formal-race");

    credits.spend(&addr, "100", &tx_ref, None).await.unwrap();
    assert_eq!(available(&pool, &addr).await, "0");

    let (c1, c2) = (credits.clone(), credits.clone());
    let (a1, a2) = (addr.clone(), addr.clone());
    let (t1, t2) = (tx_ref.clone(), tx_ref.clone());
    let (k1, k2) = (unique("reversal"), unique("admin:refund"));
    let (r1, r2) = tokio::join!(
        tokio::spawn(async move { c1.refund(&a1, "100", &t1, Some(&k1)).await }),
        tokio::spawn(async move { c2.refund(&a2, "100", &t2, Some(&k2)).await }),
    );
    r1.unwrap().unwrap();
    r2.unwrap().unwrap();

    assert_eq!(
        available(&pool, &addr).await,
        "100",
        "two concurrent refunds of one 100-credit spend must not pay out twice"
    );
    assert_reconciles(&pool, &addr, "after concurrent refunds").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn conversion_floors_and_the_quote_covers_the_grant() {
    let Some(pool) = pool().await else { return };
    use super::handlers::topup::{credits_for_wei, quote_wei};

    assert_eq!(
        credits_for_wei(&pool, "100000000000000000000", "0.25")
            .await
            .unwrap(),
        "250"
    );
    assert_eq!(
        credits_for_wei(&pool, "3999900000000000000", "0.25")
            .await
            .unwrap(),
        "9"
    );
    assert_eq!(credits_for_wei(&pool, "1", "0.25").await.unwrap(), "0");
    assert_eq!(credits_for_wei(&pool, "0", "0.25").await.unwrap(), "0");

    assert_eq!(
        quote_wei(&pool, 250, "0.25").await.unwrap(),
        "102000000000000000000"
    );
    for (credits, usd) in [
        (1i64, "0.25"),
        (7, "0.333333"),
        (250, "0.25"),
        (99_999, "0.17"),
    ] {
        let wei = quote_wei(&pool, credits, usd).await.unwrap();
        let granted: i64 = credits_for_wei(&pool, &wei, usd)
            .await
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            granted >= credits,
            "quote {wei} wei for {credits} credits at {usd} grants only {granted}"
        );
    }
    let wei = quote_wei(&pool, 250, "0.25").await.unwrap();
    let granted: i64 = credits_for_wei(&pool, &wei, "0.245250")
        .await
        .unwrap()
        .parse()
        .unwrap();
    assert!(granted >= 250, "granted {granted} after 1.9% drift");
}

#[tokio::test]
async fn double_post_grants_once_and_replays_original_amounts() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let tx_hash = format!("0x{}", uuid::Uuid::new_v4().simple().to_string().repeat(2));
    let idem = format!("mana-topup:{tx_hash}");
    let credits = Credits::new(pool.clone());
    let detail = json!({ "source": "mana-topup", "txHash": tx_hash });
    let grant = |amount: &'static str| GrantRequest {
        address: &addr,
        amount,
        kind: "purchase",
        reason: Some("MANA top-up"),
        actor: Some("mana-topup"),
        idempotency_key: Some(&idem),
        detail: &detail,
    };

    let first = credits.admin_grant_credits(grant("250")).await.unwrap();
    assert!(!first.replayed);
    assert_eq!(first.applied, "250");

    let prior = credits
        .find_grant_by_idempotency_key(&idem)
        .await
        .unwrap()
        .expect("committed grant visible");
    assert_eq!(prior.address, addr);
    assert_eq!(prior.amount, "250");
    assert_eq!(prior.available, first.available);

    let second = credits.admin_grant_credits(grant("250")).await.unwrap();
    assert!(second.replayed);
    assert_eq!(second.applied, "250");
    assert_eq!(second.available, first.available);

    let ledger_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.ledger WHERE address = $1 AND kind = 'purchase'",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger_rows, 1);

    assert!(
        credits.admin_grant_credits(grant("251")).await.is_err(),
        "drifted amount under a used key must not grant"
    );
    assert_eq!(available(&pool, &addr).await, "250");

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn mana_topup_route_verifies_through_the_broker_and_grants_once() {
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let tx_hash = format!("0x{}", uuid::Uuid::new_v4().simple().to_string().repeat(2));
    let pending = format!("0x{}", uuid::Uuid::new_v4().simple().to_string().repeat(2));
    let mut broker = MockBroker::configured();
    broker.verifications.insert(
        tx_hash.clone(),
        ManaPaymentVerification::Confirmed {
            from: wallet.address.clone(),
            to: "0x1111111111111111111111111111111111111111".into(),
            value_wei: "100000000000000000000".into(),
        },
    );
    broker
        .verifications
        .insert(pending.clone(), ManaPaymentVerification::Pending);
    let state = state_with(
        pool.clone(),
        StateCfg::default(),
        FakeMarket::default().with_mana_usd("0.25"),
        broker,
    );
    let app = app(state);

    let (status, body) = call(
        &app,
        &wallet,
        "post",
        "/credits/topup/mana",
        &[],
        Some(json!({ "txHash": pending })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["status"], "pending");

    let (status, body) = call(
        &app,
        &wallet,
        "post",
        "/credits/topup/mana",
        &[],
        Some(json!({ "txHash": tx_hash.to_uppercase().replace("0X", "0x") })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["creditsGranted"], "250");
    assert_eq!(body["available"], "250");
    assert_eq!(body["txHash"], tx_hash);

    let (status, body) = call(
        &app,
        &wallet,
        "post",
        "/credits/topup/mana",
        &[],
        Some(json!({ "txHash": tx_hash })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["creditsGranted"], "250",
        "a replay serves the original grant"
    );
    assert_eq!(available(&pool, &wallet.address).await, "250");

    let stranger = scratch_wallet();
    let (status, _) = call(
        &app,
        &stranger,
        "post",
        "/credits/topup/mana",
        &[],
        Some(json!({ "txHash": tx_hash })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "another wallet cannot claim the same transaction"
    );

    let (status, body) = send(
        &app,
        "GET",
        "/credits/topup/mana/quote?credits=250",
        HeaderMap::new(),
        None,
    )
    .await
    .pipe(|(s, _, b)| (s, b));
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["weiSuggested"], "102000000000000000000");
    assert_eq!(body["manaUsd"], "0.25");

    cleanup(&pool, &wallet.address).await;
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

#[tokio::test]
async fn progress_keeps_missing_rows_and_balances_in_one_query() {
    let Some(pool) = pool().await else { return };
    let credits = Credits::new(pool.clone());
    let missing = scratch_wallet().address;
    let program = scratch_wallet().address;
    let credit = scratch_wallet().address;
    let both = scratch_wallet().address;
    credits.mark_started(&program).await.unwrap();
    sqlx::query("INSERT INTO credits.wallets (address, available, earned_available, is_blocked_for_claiming, has_started_program) VALUES ($1, 12.5, 2.5, TRUE, FALSE), ($2, 10, 4, FALSE, FALSE)")
        .bind(&credit)
        .bind(&both)
        .execute(&pool)
        .await
        .unwrap();
    for (address, started, values) in [
        (&missing, false, None),
        (&program, true, Some((0, 0, false))),
        (&credit, false, Some((125, 25, true))),
        (&both, false, Some((100, 40, false))),
    ] {
        let (actual_started, actual_credits) = credits.user_progress(address).await.unwrap();
        assert_eq!(actual_started, started, "{address}");
        assert_eq!(
            actual_credits.map(|c| (c.available_cents, c.earned_cents, c.is_blocked_for_claiming)),
            values,
            "{address}: integer cents at the peg"
        );
    }
    for a in [&missing, &program, &credit, &both] {
        cleanup(&pool, a).await;
    }
}

fn future() -> chrono::NaiveDateTime {
    super::store::now() + chrono::Duration::minutes(10)
}

fn na<'a>(
    id: &'a str,
    addr: &'a str,
    usd_cents: i64,
    expires: chrono::NaiveDateTime,
) -> NewAuthorization<'a> {
    NewAuthorization {
        id,
        address: addr,
        usd_cents,
        amount_wei: "1000000000000000000",
        trade_id: Some("trade-x"),
        contract_address: None,
        item_id: None,
        source: None,
        expires_at: expires,
    }
}

async fn authorized_sum_cents(pool: &PgPool, addr: &str) -> i64 {
    sqlx::query_scalar("SELECT COALESCE(SUM(usd_cents), 0)::bigint FROM credits.authorizations WHERE address = $1 AND status IN ('authorized', 'retired')")
        .bind(addr)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn authorized_count(pool: &PgPool, addr: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM credits.authorizations WHERE address = $1 AND status IN ('authorized', 'retired')").bind(addr).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn sequential_reservations_never_over_pledge_the_balance() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_paid_credits(&pool, &addr, "100").await;

    credits
        .reserve_authorization(
            &na(&format!("{addr}-a"), &addr, 600, future()),
            &unique("k1"),
        )
        .await
        .expect("first $6 reservation fits in 100 credits ($10)");
    let over = credits
        .reserve_authorization(
            &na(&format!("{addr}-b"), &addr, 600, future()),
            &unique("k2"),
        )
        .await
        .expect_err("second $6 overshoots the remaining $4");
    assert!(matches!(over, ApiError::PaymentRequired(_)), "got {over:?}");
    credits
        .reserve_authorization(
            &na(&format!("{addr}-c"), &addr, 400, future()),
            &unique("k3"),
        )
        .await
        .expect("the remaining $4 is still reservable");
    let broke = credits
        .reserve_authorization(&na(&format!("{addr}-d"), &addr, 1, future()), &unique("k4"))
        .await
        .expect_err("nothing left, not even a cent");
    assert!(
        matches!(broke, ApiError::PaymentRequired(_)),
        "got {broke:?}"
    );

    assert_eq!(
        authorized_sum_cents(&pool, &addr).await,
        1000,
        "pledged total equals the balance and never exceeds it"
    );
    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn concurrent_full_balance_reservations_admit_exactly_one() {
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    seed_paid_credits(&pool, &addr, "100").await;

    let mut handles = Vec::new();
    for i in 0..8 {
        let credits = Credits::new(pool.clone());
        let addr = addr.clone();
        handles.push(tokio::spawn(async move {
            let id = format!("{addr}-c{i}");
            let key = format!("{addr}:c{i}");
            credits
                .reserve_authorization(&na(&id, &addr, 1000, future()), &key)
                .await
                .is_ok()
        }));
    }
    let mut wins = 0;
    for h in handles {
        if h.await.unwrap() {
            wins += 1;
        }
    }
    assert_eq!(wins, 1, "exactly one concurrent full-balance claim may win");
    assert_eq!(authorized_count(&pool, &addr).await, 1);
    assert_eq!(authorized_sum_cents(&pool, &addr).await, 1000);
    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn replay_cancel_and_expiry_of_reservations() {
    // Its own database: the expiry sweep walks every wallet, and a neighbour's stale
    // authorization on the shared one would be counted in this test's sweep.
    let Some(db) = OwnDatabase::create("expiry").await else {
        return;
    };
    let pool = db.pool.clone();
    let addr = scratch_wallet().address;
    let credits = Credits::new(pool.clone());
    seed_paid_credits(&pool, &addr, "100").await;

    let key = unique("same");
    let id_first = format!("{addr}-first");
    let first = credits
        .reserve_authorization(&na(&id_first, &addr, 500, future()), &key)
        .await
        .unwrap();
    assert!(!first.replayed);
    let second = credits
        .reserve_authorization(&na(&format!("{addr}-second"), &addr, 500, future()), &key)
        .await
        .unwrap();
    assert!(second.replayed, "same key must replay");
    assert_eq!(second.id, id_first, "replay returns the ORIGINAL id");
    assert_eq!(authorized_count(&pool, &addr).await, 1);

    // a cancel retires the pledge but frees no budget: the signed credit
    // stays spendable on chain until it expires
    credits
        .reserve_authorization(
            &na(&format!("{addr}-full"), &addr, 500, future()),
            &unique("k"),
        )
        .await
        .expect("the rest of the balance");
    let broke = credits
        .reserve_authorization(&na(&format!("{addr}-x"), &addr, 1, future()), &unique("k"))
        .await
        .expect_err("no budget left");
    assert!(matches!(broke, ApiError::PaymentRequired(_)));
    let retired = credits
        .retire_intents(std::slice::from_ref(&id_first), &addr)
        .await
        .unwrap();
    assert_eq!(
        (retired.retired, retired.ids.as_slice()),
        (1, std::slice::from_ref(&id_first))
    );
    assert_eq!(
        credits
            .get_authorization(&id_first)
            .await
            .unwrap()
            .unwrap()
            .status,
        "retired"
    );
    let still = credits
        .reserve_authorization(
            &na(&format!("{addr}-again"), &addr, 500, future()),
            &unique("k"),
        )
        .await
        .expect_err("a retired pledge still counts until the chain has been read past its expiry");
    assert!(matches!(still, ApiError::PaymentRequired(_)), "{still:?}");
    assert_eq!(
        authorized_sum_cents(&pool, &addr).await,
        1000,
        "authorized and retired pledges both count"
    );
    assert_eq!(credits.spendable(&addr).await.unwrap().spendable_cents(), 0);

    // a live reservation blocks a spend of the pledged balance
    let blocked = credits
        .spend(&addr, "1", "checkout:blocked", None)
        .await
        .expect_err("balance pledged to a live authorization is unspendable");
    assert!(matches!(blocked, ApiError::PaymentRequired(_)));

    // the expiry sweep frees stale ones: the retired one to 'released', the live one to 'expired'
    sqlx::query("UPDATE credits.authorizations SET expires_at = $2 WHERE address = $1")
        .bind(&addr)
        .bind(super::store::now() - chrono::Duration::minutes(1))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(credits.expire_stale_authorizations().await.unwrap(), 2);
    assert_eq!(authorized_count(&pool, &addr).await, 0);
    assert_eq!(
        credits
            .get_authorization(&id_first)
            .await
            .unwrap()
            .unwrap()
            .status,
        "released"
    );
    assert_eq!(
        credits
            .get_authorization(&format!("{addr}-full"))
            .await
            .unwrap()
            .unwrap()
            .status,
        "expired"
    );
    credits
        .reserve_authorization(
            &na(&format!("{addr}-again"), &addr, 500, future()),
            &unique("k"),
        )
        .await
        .expect("swept pledges free the budget");
    credits
        .spend(&addr, "1", "checkout:freed", None)
        .await
        .expect("the rest is spendable again");

    db.drop().await;
}

#[tokio::test]
async fn priced_authorization_derives_the_wei_from_the_oracle() {
    let Some(pool) = pool().await else { return };
    let credits = Credits::new(pool.clone());
    let addr = scratch_wallet().address;
    seed_paid_credits(&pool, &addr, "100").await;
    let expires = future();
    let priced = |id: String, cents: i64| {
        NewAuthorization {
            amount_wei: "",
            source: Some("qc"),
            ..na("", "", cents, expires)
        }
        .with_id_and_address(id, &addr)
    };

    let key = unique("qc-auth");
    let (first, rate) = credits
        .reserve_authorization_priced(&priced(format!("{addr}-a"), 600), "0.5", &key)
        .await
        .unwrap();
    assert!(!first.replayed);
    assert_eq!(first.usd_cents, 600);
    assert_eq!(
        first.amount_wei, "12000000000000000000",
        "6.00 USD at 0.5 USD/MANA is 12 MANA"
    );
    assert_eq!(rate, "500000000000000000");

    let (replay, rate) = credits
        .reserve_authorization_priced(&priced(format!("{addr}-b"), 900), "0.25", &key)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.id, format!("{addr}-a"));
    assert_eq!(replay.amount_wei, "12000000000000000000");
    assert_eq!(rate, "250000000000000000");

    let err = credits
        .reserve_authorization_priced(&priced(format!("{addr}-c"), 401), "0.5", &unique("c"))
        .await
        .expect_err("over budget");
    assert!(matches!(err, ApiError::PaymentRequired(_)), "{err:?}");
    let (third, _) = credits
        .reserve_authorization_priced(&priced(format!("{addr}-d"), 399), "0.5", &unique("d"))
        .await
        .unwrap();
    assert_eq!(third.amount_wei, "7980000000000000000");

    let stranger = scratch_wallet().address;
    let err = credits
        .reserve_authorization_priced(
            &priced(format!("{stranger}-e"), 1)
                .with_id_and_address(format!("{stranger}-e"), &stranger),
            "0.5",
            &unique("e"),
        )
        .await
        .expect_err("no wallet");
    assert!(matches!(err, ApiError::PaymentRequired(_)), "{err:?}");

    let given = credits
        .reserve_authorization(
            &NewAuthorization {
                amount_wei: "123",
                ..na(&format!("{addr}-f"), &addr, 1, expires)
            },
            &unique("f"),
        )
        .await
        .unwrap();
    assert_eq!(
        given.amount_wei, "123",
        "the given-wei path keeps the caller's amount verbatim"
    );
    assert_eq!(authorized_count(&pool, &addr).await, 3);

    cleanup(&pool, &addr).await;
}

#[test]
fn the_default_settings_sign_the_buffered_quote_and_an_explicit_headroom_is_kept() {
    let defaults = Settings::parse(RawSettings::default()).unwrap();
    assert_eq!(
        (defaults.credit_cap_headroom_pct, defaults.credit_buffer_bps),
        (100, 200)
    );
    let explicit = Settings::parse(RawSettings {
        credit_cap_headroom_pct: 150,
        ..RawSettings::default()
    })
    .unwrap();
    assert_eq!(explicit.credit_cap_headroom_pct, 150);

    // 4 MANA quoted at 0.5 USD/MANA ($2): what each configuration signs.
    let rate = crate::types::ManaUsdRate::parse_usd_per_mana("0.5").unwrap();
    let signed = |buffer_bps, headroom_pct| {
        crate::types::CreditSizing::new(rate, buffer_bps, headroom_pct)
            .unwrap()
            .cap(UsdCents::new(200))
            .unwrap()
            .get()
            .to_string()
    };
    assert_eq!(
        signed(defaults.credit_buffer_bps, defaults.credit_cap_headroom_pct),
        "4080000000000000000",
        "1.02x by default"
    );
    assert_eq!(
        signed(200, 150),
        "6120000000000000000",
        "1.53x with headroom 150"
    );
    assert_eq!(
        signed(1_000, 100),
        "4400000000000000000",
        "1.10x with the largest buffer, no headroom"
    );
    assert_eq!(
        signed(1_000, 150),
        "6600000000000000000",
        "1.65x with both at their maximum"
    );
}

#[tokio::test]
async fn a_five_dollar_pack_signs_the_mana_of_five_hundred_cents_on_the_authorize_lane() {
    let Some(db) = OwnDatabase::create("peg").await else {
        return;
    };
    let pool = db.pool.clone();
    let contract = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5".to_string();
    let signer_key = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f2e3e8a5d4b8e3e3e3";
    let mut settings = Settings::parse(RawSettings {
        writes_enabled: true,
        mock_card: true,
        credits_onchain_authorize_enabled: true,
        credits_signer_key: Some(signer_key.into()),
        credits_manager_contract: Some(contract.clone()),
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    settings.usage = super::usage::Settings {
        enabled: true,
        rpc_url: Some("http://127.0.0.1:9/".into()),
        chain_id: 137,
        contract: Some(contract.clone()),
        ..super::usage::Settings::default()
    };
    sqlx::query("INSERT INTO credits.usage_index_cursor (chain_id, contract, last_block, folded_block, folded_at) VALUES (137, $1, 100, 100, timezone('UTC', now()))")
        .bind(&contract)
        .execute(&pool)
        .await
        .unwrap();
    // The rounding-vectors.json oracle case: 30000000 at 8 decimals is 0.30 USD/MANA.
    let vectors: Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../tests/fixtures/shop/rounding-vectors.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let case = &vectors["usdCentsToManaWei"]["cases"][0];
    assert_eq!(
        (
            case["cents"].as_i64(),
            case["rate"].as_str(),
            case["decimals"].as_u64(),
            case["bufferBps"].as_u64()
        ),
        (Some(200), Some("30000000"), Some(8), Some(200))
    );
    let rate = alloy_primitives::U256::from(30_000_000u64);
    let vector_wei = |cents: i64, buffer_bps: u32| {
        ManaWei::from_usd_cents(UsdCents::new(cents), rate, 8, buffer_bps)
            .unwrap()
            .get()
            .to_string()
    };
    assert_eq!(
        vector_wei(200, 200),
        case["manaWei"].as_str().unwrap(),
        "the fixture's buffered cap"
    );
    let market = FakeMarket::default()
        .with_item(COLLECTION, "200", "4000000000000000000", true)
        .with_item(COLLECTION, "95", "1900000000000000000", true)
        .with_item(COLLECTION, "201", "4020000000000000000", true)
        .with_item(COLLECTION, "1", "20000000000000000", true)
        .with_mana_usd("0.5");
    let state: SharedState = Arc::new(
        State::with_sources(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            Arc::new(market),
            Arc::new(MockBroker::configured()),
            None,
        )
        .with_gate(Arc::new(super::settlement::FixedGate(Ok(())))),
    );
    let lane = app(state);
    let wallet = scratch_wallet();

    let (status, v) = call(
        &lane,
        &wallet,
        "post",
        "/credits/packs/credits_usd_5/mock-purchase",
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["creditsGranted"], "50", "the $5 pack is 50 credits");
    assert_eq!(available(&pool, &wallet.address).await, "50");

    // The rule derives every line from the catalogue: store items priced
    // in MANA at 0.5 USD/MANA (200, 95, 201 and 1 cents); the client's
    // usdPriceCents must be the server's quote.
    let authorize = |cents: i64, item: &'static str| {
        let lane = &lane;
        let wallet = &wallet;
        async move {
            call(lane, wallet, "post", "/credits/credits/authorize", &[], Some(json!({ "usdPriceCents": cents, "contractAddress": COLLECTION, "itemId": item }))).await
        }
    };
    let rate = crate::types::ManaUsdRate::parse_usd_per_mana("0.5").unwrap();
    let sizing = crate::types::CreditSizing::new(rate, 200, 100).unwrap();
    let cap = |quoted: i64| sizing.cap(UsdCents::new(quoted)).unwrap().get().to_string();
    let (status, v) = authorize(200, "200").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["usdCents"], 200);
    assert_eq!(v["oracleRate"], "500000000000000000");
    assert_eq!(
        v["credit"]["amount"],
        cap(200),
        "the signed value is the quoted MANA need plus the oracle buffer, times the headroom"
    );
    assert_eq!(
        v["credit"]["amount"], "4080000000000000000",
        "4 MANA * 1.02, no headroom"
    );
    assert_eq!(v["maxCreditedValue"], v["credit"]["amount"]);
    assert_eq!(v["credit"]["availableAmount"], v["credit"]["amount"]);
    assert_eq!(v["credit"]["contract"], contract.as_str());
    let expires = v["credit"]["expiresAt"].as_i64().unwrap();
    let now = chrono::Utc::now().timestamp();
    assert!(
        (now + 290..=now + 300).contains(&expires),
        "unix seconds, 300 s TTL: {expires} vs {now}"
    );

    let (status, v) = authorize(95, "95").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v["usdCents"], 100,
        "a price rounds up to whole credits before it is pledged"
    );
    assert_eq!(
        v["credit"]["amount"],
        cap(95),
        "the cap follows the quote, not the rounded charge"
    );

    let (status, v) = authorize(200, "95").await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the client's price is not the catalogue's: {v}"
    );
    assert_eq!(v["error"], "Price changed");
    assert_eq!(
        v["lines"][0],
        json!({ "index": 0, "quotedCents": 95, "requestedCents": 200 })
    );

    let (status, v) = authorize(201, "201").await;
    assert_eq!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "201 cents is 21 credits; 20 remain: {v}"
    );
    assert_eq!(
        (
            v["error"].as_str(),
            v["balanceCents"].as_i64(),
            v["requiredCents"].as_i64()
        ),
        (Some("Insufficient credits"), Some(200), Some(210))
    );
    let (status, v) = authorize(200, "200").await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let (status, v) = authorize(1, "1").await;
    assert_eq!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "the pack is fully pledged: {v}"
    );
    assert_eq!(v["balanceCents"], 0);

    let (pledged_cents, signed_wei): (i64, String) = sqlx::query_as(
        "SELECT SUM(usd_cents)::bigint, SUM(amount_wei)::text FROM credits.authorizations WHERE address = $1 AND status = 'authorized'",
    )
    .bind(&wallet.address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pledged_cents, 500, "a $5 pack pledges $5");
    let signed = alloy_primitives::U256::from_str_radix(&signed_wei, 10).unwrap();
    let parse = |s: String| alloy_primitives::U256::from_str_radix(&s, 10).unwrap();
    assert_eq!(
        signed,
        parse(cap(200)) * alloy_primitives::U256::from(2) + parse(cap(95))
    );
    assert!(
        signed < ManaWei::from_mana(11).get(),
        "$5 at 0.5 USD/MANA is 10 MANA, 10.2 with the buffer, not the 153 the 100-cent peg signed: {signed}"
    );
    let lines: i64 = sqlx::query_scalar("SELECT count(*) FROM credits.authorization_lines l JOIN credits.authorizations a ON a.id = l.authorization_id WHERE a.address = $1 AND l.sale_kind = 'primary_mint' AND l.asset_kind = 'wearable'").bind(&wallet.address).fetch_one(&pool).await.unwrap();
    assert_eq!(
        lines, 3,
        "each credit records the rule's derivation of its line"
    );

    let spend = Credits::new(pool.clone())
        .spend(&wallet.address, "1", "checkout:held", None)
        .await
        .expect_err("the pledged pack is held at ten cents a credit");
    assert!(matches!(spend, ApiError::PaymentRequired(_)), "{spend:?}");

    // The balance route: the pledges are on hold, the spendable balance is what is left.
    let (status, v) = call(
        &lane,
        &wallet,
        "get",
        &format!("/credits/users/{}/credits", wallet.address),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        (
            v["usd"]["balanceCents"].as_i64(),
            v["usd"]["credits"].as_i64()
        ),
        (Some(0), Some(0))
    );
    assert_eq!(
        (
            v["usd"]["held"]["cents"].as_i64(),
            v["usd"]["held"]["credits"].as_i64()
        ),
        (Some(500), Some(50))
    );
    assert_eq!(v["usd"]["held"]["purchases"].as_array().unwrap().len(), 3);
    assert_eq!(
        v["usd"]["held"]["purchases"][0]["contractAddress"],
        COLLECTION
    );
    db.drop().await;
}

/// The crafted-API suite for the authorize lane (SHOP.md backend 3; the
/// relay's is `economy::tests`, the broker's beside it, the cart's below):
/// a signed credit is never issued against a resale, whoever signed the
/// listing, nor against a LAND token, nor for a batch that hides a resale
/// beside an item order; the line's kind comes from the decoded trade and
/// the mirror, never from the request. Eligible primaries still pledge.
#[tokio::test]
async fn the_authorize_lane_refuses_every_resale_spelling_and_pledges_primaries_only() {
    let Some(db) = OwnDatabase::create("rule").await else {
        return;
    };
    let pool = db.pool.clone();
    let contract = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5".to_string();
    let signer_key = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f2e3e8a5d4b8e3e3e3";
    let mut settings = Settings::parse(RawSettings {
        writes_enabled: true,
        mock_card: true,
        credits_onchain_authorize_enabled: true,
        credits_signer_key: Some(signer_key.into()),
        credits_manager_contract: Some(contract.clone()),
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    settings.usage = super::usage::Settings {
        enabled: true,
        rpc_url: Some("http://127.0.0.1:9/".into()),
        chain_id: 137,
        contract: Some(contract.clone()),
        ..super::usage::Settings::default()
    };
    sqlx::query("INSERT INTO credits.usage_index_cursor (chain_id, contract, last_block, folded_block, folded_at) VALUES (137, $1, 100, 100, timezone('UTC', now()))")
        .bind(&contract)
        .execute(&pool)
        .await
        .unwrap();
    const MARKETPLACE_V3: &str = "0x540fb08edb56aae562864b390542c97f562825ba";
    const LAND_REGISTRY: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
    const STRANGER: &str = "0x00000000000000000000000000000000000000d1";
    const MANA_POLYGON: &str = "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4";
    let mana = |amount_wei: &str| json!({ "assetType": 1, "contractAddress": MANA_POLYGON, "amount": amount_wei, "beneficiary": CREATOR });
    let trade = |signer: &str, chain_id: u64, sent: Value, amount_wei: &str| {
        json!({ "id": "x", "signer": signer, "signature": "0x00", "type": if sent["tokenId"].is_string() { "public_nft_order" } else { "public_item_order" }, "network": "MATIC", "chainId": chain_id, "contract": MARKETPLACE_V3,
                "checks": { "uses": 1, "expiration": 4102444800000i64, "effective": 0, "salt": "0x", "contractSignatureIndex": 0, "signerSignatureIndex": 0 },
                "sent": [sent], "received": [mana(amount_wei)] })
    };
    let token = |contract: &str, token_id: &str| json!({ "assetType": 3, "contractAddress": contract, "tokenId": token_id });
    let item =
        |item_id: &str| json!({ "assetType": 4, "contractAddress": COLLECTION, "itemId": item_id });
    let market = FakeMarket::default()
        .with_item(COLLECTION, "4", "4000000000000000000", true)
        .with_item(COLLECTION, "5", "2000000000000000000", true)
        .with_trade(
            "de5a1e",
            trade(
                STRANGER,
                137,
                token(COLLECTION, ITEM_TOKEN_ID),
                "4000000000000000000",
            ),
        )
        .with_trade(
            "c0de-de5a1e",
            trade(
                CREATOR,
                137,
                token(COLLECTION, ITEM_TOKEN_ID),
                "4000000000000000000",
            ),
        )
        .with_trade(
            "1a4d",
            trade(
                STRANGER,
                137,
                token(LAND_REGISTRY, "1"),
                "4000000000000000000",
            ),
        )
        .with_trade(
            "0de4",
            trade(CREATOR, 137, item("4"), "4000000000000000000"),
        )
        .with_trade(
            "0de4-b",
            trade(CREATOR, 137, item("5"), "2000000000000000000"),
        )
        .with_trade(
            "11-0de4",
            trade(CREATOR, 1, item("4"), "4000000000000000000"),
        )
        .with_mana_usd("0.5");
    let state: SharedState = Arc::new(
        State::with_sources(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            Arc::new(market),
            Arc::new(MockBroker::configured()),
            None,
        )
        .with_gate(Arc::new(super::settlement::FixedGate(Ok(())))),
    );
    let lane = app(state);
    let wallet = scratch_wallet();
    let (status, v) = call(
        &lane,
        &wallet,
        "post",
        "/credits/packs/credits_usd_5/mock-purchase",
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let single = |body: Value| {
        let lane = &lane;
        let wallet = &wallet;
        async move {
            call(
                lane,
                wallet,
                "post",
                "/credits/credits/authorize",
                &[],
                Some(body),
            )
            .await
        }
    };
    let batch = |items: Vec<Value>| {
        let lane = &lane;
        let wallet = &wallet;
        async move {
            call(
                lane,
                wallet,
                "post",
                "/credits/credits/authorize/batch",
                &[],
                Some(json!({ "items": items })),
            )
            .await
        }
    };
    let authorizations = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM credits.authorizations WHERE address = $1",
        )
        .bind(&wallet.address)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let refused = |status: StatusCode, v: &Value, index: i64, what: &str| {
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {v}");
        assert_eq!(
            (
                v["ok"].as_bool(),
                v["refusal"].as_str(),
                v["index"].as_i64()
            ),
            (Some(false), Some("credits_not_allowed"), Some(index)),
            "{what}: {v}"
        );
        assert!(
            v["error"].as_str().unwrap().contains("MANA only"),
            "{what}: the buyer is told the line is MANA only: {v}"
        );
    };

    let (status, v) = single(json!({ "usdPriceCents": 200, "tradeId": "de5a1e" })).await;
    refused(status, &v, 0, "an ERC-721 resale trade");
    let (status, v) = single(json!({ "usdPriceCents": 200, "tradeId": "c0de-de5a1e" })).await;
    refused(
        status,
        &v,
        0,
        "the creator reselling a token already minted",
    );
    let (status, v) = single(json!({ "usdPriceCents": 200, "tradeId": "1a4d" })).await;
    refused(status, &v, 0, "a LAND token");
    let (status, v) = batch(vec![
        json!({ "usdPriceCents": 200, "tradeId": "0de4" }),
        json!({ "usdPriceCents": 200, "tradeId": "de5a1e" }),
    ])
    .await;
    refused(
        status,
        &v,
        1,
        "a crafted batch: a resale hidden behind an item order",
    );
    assert!(v["error"].as_str().unwrap().starts_with("items[1]:"), "{v}");
    let (status, v) = single(json!({ "usdPriceCents": 200, "tradeId": "11-0de4" })).await;
    assert_eq!(
        (status, v["refusal"].as_str()),
        (StatusCode::BAD_REQUEST, Some("chain")),
        "a trade on another chain never gets a Polygon credit: {v}"
    );
    let (status, v) = single(json!({ "usdPriceCents": 200, "name": "nacho" })).await;
    assert_eq!(
        (status, v["source"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some(super::batch::NAME_EXECUTOR_SOURCE)
        ),
        "a NAME with credits is unavailable, never quietly MANA: {v}"
    );
    assert_eq!(
        authorizations().await,
        0,
        "no refused line reserved anything"
    );
    assert_eq!(
        available(&pool, &wallet.address).await,
        "50",
        "the pack is untouched"
    );

    let (status, v) =
        single(json!({ "usdPriceCents": 200, "contractAddress": COLLECTION, "itemId": "4" })).await;
    assert_eq!(status, StatusCode::OK, "a store mint pledges: {v}");
    assert_eq!(v["usdCents"], 200);
    let (status, v) = batch(vec![
        json!({ "usdPriceCents": 200, "tradeId": "0de4" }),
        json!({ "usdPriceCents": 100, "tradeId": "0de4-b" }),
    ])
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "two creator item orders on one marketplace are one credit: {v}"
    );
    assert_eq!(v["usdCents"], 300, "{v}");
    assert_eq!(authorizations().await, 2);
    let kinds: Vec<(String, String)> = sqlx::query_as("SELECT DISTINCT l.sale_kind, l.asset_kind FROM credits.authorization_lines l JOIN credits.authorizations a ON a.id = l.authorization_id WHERE a.address = $1").bind(&wallet.address).fetch_all(&pool).await.unwrap();
    assert_eq!(
        kinds,
        vec![("primary_mint".to_string(), "wearable".to_string())],
        "every recorded line is the rule's primary derivation"
    );
    db.drop().await;
}

/// Every credits<->cents conversion reads the shop peg: `pricing::CREDIT_USD`
/// is the same string, and the rounding directions (ceil for a charge, floor
/// for a balance) match upstream's `Math.ceil`/`Math.floor(cents / 10)`.
#[test]
fn usd_cents_conversions_follow_the_shop_peg() {
    assert_eq!(UsdCents::CENTS_PER_CREDIT, 10);
    assert_eq!(super::pricing::CREDIT_USD, UsdCents::CREDIT_USD);
    assert_eq!(UsdCents::from_credits(50), Some(UsdCents::new(500)));
    assert_eq!(UsdCents::new(500).credits_floor(), 50);
    assert_eq!(UsdCents::new(501).credits_floor(), 50);
    assert_eq!(UsdCents::new(501).credits_ceil(), 51);
    assert_eq!(UsdCents::new(0).charge_for_quote(), None);
    assert_eq!(
        UsdCents::new(501).charge_for_quote(),
        Some(UsdCents::new(510)),
        "a charge rounds up to the next whole credit"
    );
}

impl<'a> NewAuthorization<'a> {
    fn with_id_and_address(self, _id: String, _addr: &'a str) -> Self {
        // leaked so the borrowed struct outlives the closure; test-only
        let id: &'a str = Box::leak(_id.into_boxed_str());
        NewAuthorization {
            id,
            address: _addr,
            ..self
        }
    }
}

#[tokio::test]
async fn authorize_route_refuses_while_the_flag_is_off_or_unconfigured() {
    let wallet = scratch_wallet();
    let body = json!({ "usdPriceCents": 500, "tradeId": "t1" });
    let off = app(test_state(lazy_pool(), StateCfg::default()));
    let (status, v) = call(
        &off,
        &wallet,
        "post",
        "/credits/credits/authorize",
        &[],
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{v}");
    assert!(
        v["message"]
            .as_str()
            .unwrap()
            .contains("CREDITS_ONCHAIN_AUTHORIZE_ENABLED")
    );

    let on = app(test_state(
        lazy_pool(),
        StateCfg {
            onchain_authorize_enabled: true,
            ..StateCfg::default()
        },
    ));
    let (status, v) = call(
        &on,
        &wallet,
        "post",
        "/credits/credits/authorize",
        &[],
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{v}");
    assert!(v["message"].as_str().unwrap().contains("signer key"));
}

#[tokio::test]
async fn cancel_releases_only_the_signers_authorized_intents() {
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let id = format!("0x{}", uuid::Uuid::new_v4().simple().to_string().repeat(2));
    let state = test_state(pool.clone(), StateCfg::default());
    state
        .credits
        .insert_authorization(&na(&id, &wallet.address, 500, future()))
        .await
        .unwrap();
    let app = app(state.clone());

    let stranger = scratch_wallet();
    let (status, v) = call(
        &app,
        &stranger,
        "post",
        "/credits/credits/authorize/cancel",
        &[],
        Some(json!({ "salts": [id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        state
            .credits
            .get_authorization(&id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "authorized",
        "another signer cannot release it"
    );

    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/credits/authorize/cancel",
        &[],
        Some(json!({ "salts": [id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (v["released"].as_i64(), v["retired"].as_i64()),
        (Some(1), Some(1)),
        "{v}"
    );
    let row = state.credits.get_authorization(&id).await.unwrap().unwrap();
    assert_eq!(
        row.status, "retired",
        "a cancel retires the pledge; the sweep releases it once the chain has been read past its expiry"
    );
    assert_eq!(
        v["spendableAtSeconds"].as_i64(),
        Some(super::history::releases_at_seconds(row.expires_at))
    );
    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/credits/authorize/cancel",
        &[],
        Some(json!({ "salts": [id] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        v,
        json!({ "released": 0, "retired": 0, "spendableAtSeconds": null }),
        "a second cancel finds nothing authorized"
    );
    let (status, v) = call(&app, &wallet, "post", "/credits/credits/authorize/submitted", &[], Some(json!({ "salts": [id], "txHash": "0xd4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4" }))).await;
    assert_eq!(
        (status, v["recorded"].as_i64()),
        (StatusCode::OK, Some(1)),
        "{v}"
    );
    assert_eq!(
        state
            .credits
            .get_authorization(&id)
            .await
            .unwrap()
            .unwrap()
            .submitted_tx_hash
            .as_deref(),
        Some("0xd4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4")
    );
    let (status, v) = call(&app, &stranger, "post", "/credits/credits/authorize/submitted", &[], Some(json!({ "salts": [id], "txHash": "0xd4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4" }))).await;
    assert_eq!(
        (status, v["recorded"].as_i64()),
        (StatusCode::OK, Some(0)),
        "another signer stamps nothing"
    );
    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/credits/authorize/submitted",
        &[],
        Some(json!({ "salts": [id], "txHash": "0xdeadbeef" })),
    )
    .await;
    assert_eq!(
        (status, v["message"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("A \"txHash\" bytes32 hex string is required")
        )
    );

    cleanup(&pool, &wallet.address).await;
}

#[tokio::test]
async fn every_signed_route_answers_the_adr44_envelope_unauthenticated() {
    let app = app(test_state(lazy_pool(), StateCfg::default()));
    for (method, path) in [
        (
            "GET",
            "/credits/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/credits",
        ),
        (
            "GET",
            "/credits/users/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/progress",
        ),
        (
            "GET",
            "/credits/wallet/0x8d6f63e382d73cf53858864f673f39e9ff915a1e/balance",
        ),
        ("POST", "/credits/credits/checkout"),
        ("GET", "/credits/credits/orders/ord_x"),
        ("POST", "/credits/credits/authorize"),
        ("POST", "/credits/credits/authorize/cancel"),
        ("GET", "/credits/cart"),
        ("POST", "/credits/checkout"),
        ("GET", "/credits/captcha"),
    ] {
        let (status, _, body) = send(&app, method, path, HeaderMap::new(), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {path}: {body}");
        assert_eq!(body, adr44(), "{method} {path}");
    }
}

#[tokio::test]
async fn signer_mismatch_is_a_403_and_a_signed_wallet_reads_its_credits() {
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let other = "0x0000000000000000000000000000000000000009";
    sqlx::query("INSERT INTO credits.wallets (address, available, earned_available, has_started_program) VALUES ($1, 7.5, 2.5, FALSE)")
        .bind(&wallet.address)
        .execute(&pool)
        .await
        .unwrap();
    let app = app(test_state(pool.clone(), StateCfg::default()));

    let (status, body) = call(
        &app,
        &wallet,
        "get",
        &format!("/credits/users/{other}/credits"),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["ok"], false);

    let (status, v) = call(
        &app,
        &wallet,
        "get",
        &format!("/credits/users/{}/credits", wallet.address),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v["totalCredits"], 0,
        "no season (MANA) credits: upstream's legacy fields are zero"
    );
    assert_eq!(v["totals"]["nonExpiring"], 0);
    assert_eq!(v["usd"]["credits"], 7);
    assert_eq!(
        v["usd"]["balanceCents"], 75,
        "7.5 credits at ten cents, spendable"
    );
    assert!(v["usd"].get("held").is_none(), "nothing pledged");
    assert!(v["credits"].as_array().unwrap().is_empty());

    // The old service behind nginx saw the route-local path: a signature over it verifies too.
    let route_signed = signed_headers(
        &wallet,
        "get",
        &format!("/users/{}/progress", wallet.address),
    )
    .await;
    let (status, _, v) = send(
        &app,
        "GET",
        &format!("/credits/users/{}/progress", wallet.address),
        route_signed,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    // Whole credits held, floored like upstream's Math.floor(cents / 10).
    assert_eq!(v["credits"]["available"], 7);
    assert_eq!(v["credits"]["earned"], 2);
    assert_eq!(v["credits"]["paid"], 5);
    assert_eq!(v["user"]["hasStartedProgram"], false);

    let (status, v) = call(&app, &wallet, "post", "/credits/users", &[], None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let (_, v) = call(
        &app,
        &wallet,
        "get",
        &format!("/credits/users/{}/progress", wallet.address),
        &[],
        None,
    )
    .await;
    assert_eq!(v["user"]["hasStartedProgram"], true);

    let (status, v) = call(
        &app,
        &wallet,
        "get",
        &format!(
            "/credits/wallet/{}/balance",
            wallet.address.to_uppercase().replace("0X", "0x")
        ),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v, json!({ "address": wallet.address, "available": "7.5" }));

    cleanup(&pool, &wallet.address).await;
}

#[tokio::test]
async fn pack_listings_serve_the_seeded_tiers_in_both_shapes() {
    let Some(pool) = pool().await else { return };
    let app = app(test_state(pool.clone(), StateCfg::default()));
    let (status, _, v) = send(
        &app,
        "GET",
        "/credits/credits/packs",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let packs = v["packs"].as_array().expect("packs array");
    assert!(packs.len() >= 4, "the four seeded tiers: {v}");
    for p in packs {
        for key in ["id", "usd", "credits", "recommended", "order"] {
            assert!(p.get(key).is_some(), "pack missing {key}: {p}");
        }
    }
    assert!(
        packs
            .iter()
            .any(|p| p["id"] == "credits_usd_5" && p["usd"] == 5.0 && p["credits"] == 50)
    );
    let (status, _, v) = send(&app, "GET", "/credits/packs", HeaderMap::new(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .any(|p| p["sku"] == "credits_usd_50"
                && p["priceCents"] == 5000
                && p["credits"] == "500")
    );
    let (status, _, v) = send(&app, "GET", "/credits/seasons", HeaderMap::new(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(v.get("currentSeason").is_some() && v.get("lastSeason").is_some());
}

#[tokio::test]
async fn mock_topup_is_gated_validated_capped_and_idempotent() {
    let wallet = scratch_wallet();
    let off = app(test_state(lazy_pool(), StateCfg::default()));
    let (status, _) = call(
        &off,
        &wallet,
        "post",
        "/credits/topup/mock-card",
        &[],
        Some(json!({ "credits": "5" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_IMPLEMENTED,
        "mock_card off must refuse"
    );

    let Some(pool) = pool().await else { return };
    let on = app(test_state(
        pool.clone(),
        StateCfg {
            mock_card: true,
            ..StateCfg::default()
        },
    ));
    for bad in ["abc", "", "  ", "0", "0.000", "-5", "1e3", "1.2.3"] {
        let (status, v) = call(
            &on,
            &wallet,
            "post",
            "/credits/topup/mock-card",
            &[],
            Some(json!({ "credits": bad })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad:?}: {v}");
    }
    for over in ["10000.01", "10001", "999999"] {
        let (status, v) = call(
            &on,
            &wallet,
            "post",
            "/credits/topup/mock-card",
            &[],
            Some(json!({ "credits": over })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{over:?}: {v}");
        assert!(v["message"].as_str().unwrap().contains("capped at 10000"));
    }

    let headers = signed_headers(&wallet, "post", "/credits/topup/mock-card").await;
    let (status, _, v) = send(
        &on,
        "POST",
        "/credits/topup/mock-card",
        headers.clone(),
        Some(json!({ "credits": "10000" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "creditsGranted": "10000", "available": "10000", "mock": true })
    );
    let (status, _, rv) = send(
        &on,
        "POST",
        "/credits/topup/mock-card",
        headers,
        Some(json!({ "credits": "10000" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rv}");
    assert_eq!(rv["creditsGranted"], "10000");
    assert_eq!(
        available(&pool, &wallet.address).await,
        "10000",
        "the same signed timestamp replays, never grants twice"
    );
    let purchases: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.ledger WHERE address = $1 AND kind = 'purchase'",
    )
    .bind(&wallet.address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(purchases, 1);

    let (status, v) = call(
        &on,
        &wallet,
        "post",
        "/credits/packs/credits_usd_5/mock-purchase",
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "creditsGranted": "50", "available": "10050", "mock": true })
    );

    cleanup(&pool, &wallet.address).await;
}

const WEBHOOK_SECRET: &str = "whsec_query_counts";

fn stripe_headers(body: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "stripe-signature",
        super::stripe::sign_for_tests(
            WEBHOOK_SECRET,
            chrono::Utc::now().timestamp(),
            body.as_bytes(),
        )
        .parse()
        .unwrap(),
    );
    headers
}

async fn deliver(app: &Router, body: &str) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/credits/stripe/webhook");
    for (k, v) in stripe_headers(body).iter() {
        req = req.header(k, v);
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1 << 16).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn webhook_purchase_and_refund_each_run_in_one_transaction() {
    let Some(pool) = pool().await else { return };
    let state = test_state(
        pool.clone(),
        StateCfg {
            stripe_webhook_secret: Some(WEBHOOK_SECRET),
            ..StateCfg::default()
        },
    );
    let addr = scratch_wallet().address;
    let pack = PackRow {
        sku: "qc-pack".into(),
        title: "QC".into(),
        credits: "50".into(),
        price_cents: 500,
        currency: "usd".into(),
        sort_order: 0,
    };
    let pi = unique("pi_qc");
    state
        .credits
        .insert_pending_purchase(&addr, &pack, &pi)
        .await
        .unwrap();
    let app = app(state.clone());

    // An unsigned or badly signed delivery touches no row.
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/stripe/webhook",
        HeaderMap::new(),
        Some(json!({ "id": "evt_x", "type": "payment_intent.succeeded" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    let mut forged = HeaderMap::new();
    forged.insert("stripe-signature", "t=1,v1=00".parse().unwrap());
    let (status, _, _) = send(
        &app,
        "POST",
        "/credits/stripe/webhook",
        forged,
        Some(json!({ "id": "evt_x", "type": "payment_intent.succeeded" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let evt = unique("evt_qc_paid");
    let paid = json!({ "id": evt, "type": "payment_intent.succeeded", "data": { "object": { "id": pi, "amount": 500, "metadata": { "sku": "qc-pack", "credits": "999999" } } } }).to_string();
    let (status, out) = deliver(&app, &paid).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["detail"], "processed");
    assert_eq!(
        available(&pool, &addr).await,
        "50",
        "granted from the purchase row, never the metadata"
    );
    let (status, event_id): (String, Option<String>) = sqlx::query_as(
        "SELECT status, stripe_event_id FROM credits.purchases WHERE stripe_payment_intent = $1",
    )
    .bind(&pi)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "paid");
    assert_eq!(event_id.as_deref(), Some(evt.as_str()));
    let processed: bool = sqlx::query_scalar(
        "SELECT processed_at IS NOT NULL FROM credits.stripe_events WHERE event_id = $1",
    )
    .bind(&evt)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(processed);
    let (ledger, audit): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM credits.ledger WHERE address = $1 AND kind = 'purchase'), \
                (SELECT count(*) FROM ops.admin_audit WHERE domain = 'credits' AND target = $1 AND action = 'grant' AND actor = 'stripe')",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((ledger, audit), (1, 1));

    let (_, out) = deliver(&app, &paid).await;
    assert_eq!(out["detail"], "already processed");
    assert_eq!(available(&pool, &addr).await, "50", "no double grant");

    let refund = |evt: &str, cents: i64| {
        json!({ "id": evt, "type": "charge.refunded", "data": { "object": { "payment_intent": pi, "amount_refunded": cents } } }).to_string()
    };
    let (_, out) = deliver(&app, &refund(&unique("evt_qc_ref1"), 250)).await;
    assert_eq!(out["detail"], "processed");
    let is_25: bool = sqlx::query_scalar(
        "SELECT available = 25::numeric FROM credits.wallets WHERE address = $1",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(is_25);
    let (refunded, revoked): (i64, bool) = sqlx::query_as("SELECT refunded_cents, revoked_credits = 25 FROM credits.purchases WHERE stripe_payment_intent = $1").bind(&pi).fetch_one(&pool).await.unwrap();
    assert_eq!(refunded, 250);
    assert!(revoked);

    let (_, out) = deliver(&app, &refund(&unique("evt_qc_ref2"), 250)).await;
    assert_eq!(out["detail"], "processed");
    let still_25: bool = sqlx::query_scalar(
        "SELECT available = 25::numeric FROM credits.wallets WHERE address = $1",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(still_25, "a repeated cumulative total reverses nothing");

    let dispute = json!({ "id": unique("evt_qc_disp"), "type": "charge.dispute.created", "data": { "object": { "payment_intent": pi } } }).to_string();
    let (_, out) = deliver(&app, &dispute).await;
    assert_eq!(out["detail"], "processed");
    let zero: bool =
        sqlx::query_scalar("SELECT available = 0::numeric FROM credits.wallets WHERE address = $1")
            .bind(&addr)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(zero);
    let status: String =
        sqlx::query_scalar("SELECT status FROM credits.purchases WHERE stripe_payment_intent = $1")
            .bind(&pi)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "disputed");
    assert_reconciles(&pool, &addr, "after the webhook sequence").await;

    // The order flow: a checkout session completes.
    let order_wallet = scratch_wallet();
    let order_id = unique("ord");
    let session = unique("cs");
    state
        .credits
        .insert_pending_order(
            &order_id,
            &order_wallet.address,
            &pack.order(1).unwrap(),
            &session,
        )
        .await
        .unwrap();
    let (status, v) = call(
        &app,
        &order_wallet,
        "get",
        &format!("/credits/credits/orders/{order_id}"),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "status": "processing", "creditsGranted": 0, "newBalance": 0, "error": "" })
    );
    let completed = json!({ "id": unique("evt_qc_sess"), "type": "checkout.session.completed", "data": { "object": { "id": session, "amount_total": 500, "metadata": { "orderId": order_id } } } }).to_string();
    let (_, out) = deliver(&app, &completed).await;
    assert_eq!(out["detail"], "processed");
    let (status, v) = call(
        &app,
        &order_wallet,
        "get",
        &format!("/credits/credits/orders/{order_id}"),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "status": "credited", "creditsGranted": 50, "newBalance": 50, "error": "" })
    );
    let stranger = scratch_wallet();
    let (status, _) = call(
        &app,
        &stranger,
        "get",
        &format!("/credits/credits/orders/{order_id}"),
        &[],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an order is only visible to its own signer"
    );

    // Three of the pack in one checkout: the row holds the totals, so the
    // webhook checks 3 x 500 cents and grants 3 x 50 credits; a charge of one
    // pack's price is a mismatch that grants nothing.
    let multi_wallet = scratch_wallet();
    let multi_order = unique("ord");
    let multi_session = unique("cs");
    state
        .credits
        .insert_pending_order(
            &multi_order,
            &multi_wallet.address,
            &pack.order(3).unwrap().from_source(Some("builder")),
            &multi_session,
        )
        .await
        .unwrap();
    let (quantity, source): (i32, Option<String>) =
        sqlx::query_as("SELECT quantity, source FROM credits.purchases WHERE order_id = $1")
            .bind(&multi_order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (quantity, source.as_deref()),
        (3, Some("builder")),
        "the order keeps the surface that opened it"
    );
    let underpaid = json!({ "id": unique("evt_qc_multi_under"), "type": "checkout.session.completed", "data": { "object": { "id": multi_session, "amount_total": 500, "metadata": { "orderId": multi_order } } } }).to_string();
    deliver(&app, &underpaid).await;
    let (_, v) = call(
        &app,
        &multi_wallet,
        "get",
        &format!("/credits/credits/orders/{multi_order}"),
        &[],
        None,
    )
    .await;
    assert_eq!(
        v["status"], "processing",
        "one pack's price does not pay for three: {v}"
    );
    let paid = json!({ "id": unique("evt_qc_multi"), "type": "checkout.session.completed", "data": { "object": { "id": multi_session, "amount_total": 1500, "metadata": { "orderId": multi_order } } } }).to_string();
    let (_, out) = deliver(&app, &paid).await;
    assert_eq!(out["detail"], "processed");
    let (_, v) = call(
        &app,
        &multi_wallet,
        "get",
        &format!("/credits/credits/orders/{multi_order}"),
        &[],
        None,
    )
    .await;
    assert_eq!(
        v,
        json!({ "status": "credited", "creditsGranted": 150, "newBalance": 150, "error": "" })
    );

    cleanup(&pool, &addr).await;
    cleanup(&pool, &order_wallet.address).await;
    cleanup(&pool, &multi_wallet.address).await;
    sqlx::query("DELETE FROM credits.stripe_events WHERE event_id LIKE 'evt_qc_%'")
        .execute(&pool)
        .await
        .unwrap();
}

fn market_for_checkout() -> FakeMarket {
    FakeMarket::default()
        .with_item(COLLECTION, "1", "2500000000000000000", true)
        .with_item(COLLECTION, "4", "2500000000000000000", true)
        .with_mana_usd("0.5")
}

#[tokio::test]
async fn replay_after_commit_returns_original_checkout_without_a_new_debit() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let addr = wallet.address.clone();
    let state = state_with(
        pool.clone(),
        StateCfg {
            fulfillment_mode: "auto",
            ..StateCfg::default()
        },
        market_for_checkout(),
        MockBroker::configured(),
    );
    let urn = format!("urn:decentraland:matic:collections-v2:{COLLECTION}:1");

    seed_paid_credits(&pool, &addr, "10").await;
    state
        .credits
        .add_item(cart_line(&addr, "1", &urn, 1), "3")
        .await
        .unwrap();

    let idem = unique("test-replay-commit");
    let first = state
        .credits
        .run_checkout(&addr, &idem, &[line("1", "3")])
        .await
        .unwrap();
    assert!(!first.replayed);
    assert_eq!(first.status, "fulfilling");
    assert!(
        state
            .credits
            .get_cart(&addr)
            .await
            .unwrap()
            .items
            .is_empty(),
        "a committed checkout consumes its cart lines"
    );
    assert_eq!(available(&pool, &addr).await, "7");

    let app = app(state.clone());
    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &idem)],
        Some(json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "retry must replay the committed checkout: {v}"
    );
    assert_eq!(
        v["id"].as_i64(),
        Some(first.id),
        "must be the ORIGINAL checkout"
    );
    assert_eq!(v["status"], "fulfilling");
    assert_eq!(v["replayed"], true);
    let spends: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.ledger WHERE address = $1 AND kind = 'spend'",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(spends, 1, "replay must not debit again");
    assert_eq!(available(&pool, &addr).await, "7");

    let intruder = scratch_wallet();
    let (status, v) = call(
        &app,
        &intruder,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &idem)],
        Some(json!({})),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "someone else's key must not replay: {v}"
    );

    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &unique("fresh"))],
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(v["message"].as_str().unwrap().contains("cart is empty"));

    cleanup(&pool, &addr).await;
}

async fn intent(w: &Wallet, key: &str, total: &str) -> Value {
    let intent = super::purchase_intent::PurchaseIntentIn {
        buyer: w.address.clone(),
        items: json!([[COLLECTION, "1", 1]]).to_string(),
        total_credits: total.into(),
        currency: "CREDITS".into(),
        nonce: key.into(),
        expires_at: chrono::Utc::now().timestamp() as u64 + 300,
    };
    let digest = super::purchase_intent::intent_digest(&intent).unwrap();
    let signature = w
        .signer
        .sign_hash(&alloy_primitives::B256::from(digest))
        .await
        .unwrap();
    json!({ "intent": intent, "intentSignature": signature.to_string() })
}

#[tokio::test]
async fn signed_checkout_binds_price_and_owner_debits_once_and_fulfils_through_the_broker() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let buyer = scratch_wallet();
    let stranger = scratch_wallet();
    let address = buyer.address.clone();
    let key = unique("signed-checkout");
    let state = state_with(
        pool.clone(),
        StateCfg {
            fulfillment_mode: "auto",
            require_purchase_intent: true,
            ..StateCfg::default()
        },
        market_for_checkout(),
        MockBroker::configured(),
    );
    let app = app(state.clone());
    sqlx::query("INSERT INTO credits.wallets (address, available, has_started_program) VALUES ($1, 20, FALSE)").bind(&address).execute(&pool).await.unwrap();

    let (status, cart) = call(
        &app,
        &buyer,
        "post",
        "/credits/cart/items",
        &[],
        Some(json!({ "collection": COLLECTION, "itemId": "1", "qty": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cart}");
    assert_eq!(
        cart["totalCredits"], "13",
        "2.5 MANA * 0.5 USD / 0.10 per credit, ceiled"
    );
    assert_eq!(cart["items"][0]["unitPriceCredits"], "13");

    let (status, v) = call(
        &app,
        &buyer,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &key)],
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no intent: {v}");
    let (status, v) = call(
        &app,
        &buyer,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &key)],
        Some(intent(&buyer, &key, "12").await),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "a stale total: {v}");
    assert_eq!(available(&pool, &address).await, "20");

    let signed = intent(&buyer, &key, "13").await;
    let (status, first) = call(
        &app,
        &buyer,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &key)],
        Some(signed.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "fulfilling");
    assert_eq!(first["replayed"], false);
    let (status, replay) = call(
        &app,
        &buyer,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &key)],
        Some(signed.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["id"], first["id"]);
    assert_eq!(replay["replayed"], true);
    let (status, _) = call(
        &app,
        &stranger,
        "post",
        "/credits/checkout",
        &[("idempotency-key", &key)],
        Some(signed),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let path = format!("/credits/checkout/{}", first["id"]);
    let (status, order) = call(&app, &buyer, "get", &path, &[], None).await;
    assert_eq!(status, StatusCode::OK, "{order}");
    assert_eq!(order["totalCredits"], "13");
    let (status, _) = call(&app, &stranger, "get", &path, &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, cart) = call(&app, &buyer, "get", "/credits/cart", &[], None).await;
    assert_eq!(cart["items"], json!([]));
    assert_eq!(available(&pool, &address).await, "7");
    let debits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.ledger WHERE address = $1 AND kind = 'spend'",
    )
    .bind(&address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(debits, 1);
    let checkout_id = first["id"].as_i64().unwrap();
    let jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.fulfillment_outbox WHERE checkout_id = $1",
    )
    .bind(checkout_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(jobs, 1);

    // The fulfilment tick delivers the line through the broker, confirms the checkout and writes the escrow lease.
    let worker = state.outbox_worker();
    assert_eq!(worker.run_once().await.unwrap(), 1);
    let (status, external_ref): (String, Option<String>) = sqlx::query_as(
        "SELECT status, external_ref FROM credits.fulfillment_outbox WHERE checkout_id = $1",
    )
    .bind(checkout_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "confirmed");
    assert!(
        external_ref
            .as_deref()
            .is_some_and(|r| r.starts_with("0xcheckout")),
        "{external_ref:?}"
    );
    let status: String = sqlx::query_scalar("SELECT status FROM credits.checkouts WHERE id = $1")
        .bind(checkout_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "fulfilled");
    let grants: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.usage_grants WHERE grantee_address = $1 AND status = 'active' AND escrow_ref = $2").bind(&address).bind(external_ref.as_deref()).fetch_one(&pool).await.unwrap();
    assert_eq!(grants, 1, "the escrow lease is recorded for the buyer");
    assert_eq!(
        worker.run_once().await.unwrap(),
        0,
        "nothing pending on the next tick"
    );
    let report = state.credits.reconcile().await.unwrap();
    assert!(
        report
            .checkout_fulfillment_mismatches
            .iter()
            .all(|d| d.checkout_id != checkout_id)
    );
    assert!(
        report
            .escrow_grant_mismatches
            .iter()
            .all(|d| d.checkout_id != checkout_id)
    );
    assert_eq!(report.credit_usage_index, "unavailable");

    cleanup(&pool, &address).await;
}

#[tokio::test]
async fn a_terminal_broker_failure_freezes_the_checkout_and_refunds_the_line() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let broker = MockBroker::configured();
    *broker.buy_answer.lock() = Err(BrokerError::Terminal(
        "broker status 400: item not mintable".into(),
    ));
    let state = state_with(
        pool.clone(),
        StateCfg {
            fulfillment_mode: "auto",
            ..StateCfg::default()
        },
        market_for_checkout(),
        broker,
    );
    let urn = format!("urn:decentraland:matic:collections-v2:{COLLECTION}:1");
    seed_wallet(&pool, &addr, "0", "10").await;
    state
        .credits
        .add_item(cart_line(&addr, "1", &urn, 1), "3")
        .await
        .unwrap();
    let out = state
        .credits
        .run_checkout(
            &addr,
            &unique("terminal"),
            &[RepricedLine {
                basis_wei: Some("2500000000000000000".into()),
                ..line("1", "3")
            }],
        )
        .await
        .unwrap();
    assert_eq!(available(&pool, &addr).await, "7");

    let worker = state.outbox_worker();
    assert_eq!(worker.run_once().await.unwrap(), 1);
    let status: String = sqlx::query_scalar("SELECT status FROM credits.checkouts WHERE id = $1")
        .bind(out.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "reversed",
        "nothing delivered: the checkout is reversed in the same pass"
    );
    assert_eq!(
        available(&pool, &addr).await,
        "10",
        "the undelivered Credits came back"
    );
    let (line_status, last_error): (String, Option<String>) = sqlx::query_as(
        "SELECT status, last_error FROM credits.fulfillment_outbox WHERE checkout_id = $1",
    )
    .bind(out.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(line_status, "failed");
    assert!(last_error.unwrap().contains("not mintable"));
    assert_reconciles(&pool, &addr, "after the reversal").await;

    // A retryable failure counts attempts and freezes at the cap.
    let retry_broker = MockBroker::configured();
    *retry_broker.buy_answer.lock() = Err(BrokerError::Retryable("broker status 503".into()));
    let state = state_with(
        pool.clone(),
        StateCfg {
            fulfillment_mode: "auto",
            ..StateCfg::default()
        },
        market_for_checkout(),
        retry_broker,
    );
    state
        .credits
        .add_item(cart_line(&addr, "1", &urn, 1), "3")
        .await
        .unwrap();
    let out = state
        .credits
        .run_checkout(
            &addr,
            &unique("retryable"),
            &[RepricedLine {
                basis_wei: Some("2500000000000000000".into()),
                ..line("1", "3")
            }],
        )
        .await
        .unwrap();
    let worker = state.outbox_worker();
    for expected_attempts in 1..5 {
        assert_eq!(worker.run_once().await.unwrap(), 1);
        let attempts: i32 = sqlx::query_scalar(
            "SELECT attempts FROM credits.fulfillment_outbox WHERE checkout_id = $1",
        )
        .bind(out.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(attempts, expected_attempts);
    }
    assert_eq!(
        worker.run_once().await.unwrap(),
        1,
        "the fifth attempt freezes"
    );
    let status: String = sqlx::query_scalar("SELECT status FROM credits.checkouts WHERE id = $1")
        .bind(out.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "reversed");
    assert_eq!(available(&pool, &addr).await, "10");

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn cart_reads_and_writes_keep_their_totals_and_the_checkout_records_its_ledger_row() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let credits = Credits::new(pool.clone());
    let addr = scratch_wallet().address;
    let urn_for =
        |item_id: &str| format!("urn:decentraland:matic:collections-v2:{COLLECTION}:{item_id}");

    for (item_id, qty, price) in [("1", 2, "1.50"), ("2", 1, "0.00"), ("3", 3, "2")] {
        credits
            .add_item(cart_line(&addr, item_id, &urn_for(item_id), qty), price)
            .await
            .unwrap();
    }
    let view = credits.get_cart(&addr).await.unwrap();
    let expected_total: String = sqlx::query_scalar(
        "SELECT COALESCE(SUM(ci.unit_price_credits * ci.qty), 0)::text FROM credits.cart_items ci JOIN credits.carts c ON c.id = ci.cart_id WHERE c.address = $1",
    )
    .bind(&addr)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        view.total_credits, expected_total,
        "folded window total must byte-equal the aggregate"
    );
    assert_eq!(
        view.items
            .iter()
            .map(|i| i.item_id.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    assert_eq!(
        view.items
            .iter()
            .map(|i| i.unit_price_credits.as_str())
            .collect::<Vec<_>>(),
        ["1.50", "0.00", "2"]
    );
    let empty = credits.get_cart(&scratch_wallet().address).await.unwrap();
    assert!(empty.items.is_empty());
    assert_eq!(empty.total_credits, "0");

    let view = credits.remove_item(&addr, COLLECTION, "2").await.unwrap();
    assert_eq!(
        view.items
            .iter()
            .map(|i| i.item_id.as_str())
            .collect::<Vec<_>>(),
        ["1", "3"]
    );
    assert_eq!(view.total_credits, "9.00");
    let same = credits.remove_item(&addr, COLLECTION, "404").await.unwrap();
    assert_eq!(same.items.len(), 2);

    // add_item_priced inlines the reprice and refuses a zero charge for a positive payment.
    let pricing = super::pricing::Pricing::new(Arc::new(FakeMarket::default()), 0);
    let expected = pricing
        .compute_credit_price(&pool, "2500000000000000000", "0.5")
        .await
        .unwrap();
    let price = credits
        .add_item_priced(
            cart_line(&addr, "4", &urn_for("4"), 1),
            "2500000000000000000",
            "0.5",
            0,
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        price, expected,
        "inline ceil must byte-equal compute_credit_price"
    );
    assert_eq!(price, "13");
    let zero = credits
        .add_item_priced(
            cart_line(&addr, "5", &urn_for("5"), 1),
            "2500000000000000000",
            "0",
            0,
            true,
        )
        .await
        .unwrap();
    assert!(super::pricing::ensure_charge_covers_payment("2500000000000000000", &zero).is_err());
    let cart = credits.get_cart(&addr).await.unwrap();
    assert!(
        !cart.items.iter().any(|i| i.item_id == "5"),
        "the unpriceable line must not be written"
    );

    // The checkout: total, ledger id, outbox rows, the admin listing, an unfunded checkout, the tick's sweep.
    let addr2 = scratch_wallet().address;
    sqlx::query("INSERT INTO credits.wallets (address, available, earned_available, has_started_program) VALUES ($1, 10, 4, FALSE)").bind(&addr2).execute(&pool).await.unwrap();
    for item_id in ["1", "2"] {
        credits
            .add_item(cart_line(&addr2, item_id, &urn_for(item_id), 1), "3")
            .await
            .unwrap();
    }
    let outcome = credits
        .run_checkout(
            &addr2,
            &unique("qc-checkout"),
            &[line("1", "3"), line("2", "3")],
        )
        .await
        .unwrap();
    assert_eq!(outcome.status, "fulfilling");
    let (total, status, ledger_id): (String, String, Option<i64>) = sqlx::query_as(
        "SELECT total_credits::text, status, ledger_id FROM credits.checkouts WHERE id = $1",
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((total.as_str(), status.as_str()), ("6", "fulfilling"));
    let expected_ledger: Option<i64> = sqlx::query_scalar(
        "SELECT max(id) FROM credits.ledger WHERE tx_ref = $1 AND kind = 'spend'",
    )
    .bind(format!("checkout:{}", outcome.id))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(expected_ledger.is_some());
    assert_eq!(ledger_id, expected_ledger);
    assert_eq!(
        balances(&pool, &addr2).await,
        ("4".to_string(), "0".to_string()),
        "earned first"
    );
    let outbox: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM credits.fulfillment_outbox WHERE checkout_id = $1",
    )
    .bind(outcome.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox, 2);
    assert!(credits.get_cart(&addr2).await.unwrap().items.is_empty());

    let listed = credits
        .admin_list_checkouts(Some(&addr2), None, 10, 0)
        .await
        .unwrap();
    let head = listed.iter().find(|c| c.id == outcome.id).expect("listed");
    assert_eq!(head.total_credits, "6");
    assert_eq!(head.lines.len(), 2);
    assert!(head.lines[0].id < head.lines[1].id);
    assert!(head.lines.iter().all(|l| l.status == "pending"
        && l.attempts == 0
        && l.last_error.is_none()
        && l.external_ref.is_none()));
    assert!(
        credits
            .admin_list_checkouts(Some(&addr2), Some("failed"), 10, 0)
            .await
            .unwrap()
            .is_empty()
    );

    let stale_auth = unique("qc-stale");
    sqlx::query("INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at) VALUES ($1, $2, 1, 1, 'authorized', $3)")
        .bind(&stale_auth)
        .bind(&addr2)
        .bind(super::store::now() - chrono::Duration::minutes(1))
        .execute(&pool)
        .await
        .unwrap();
    let stale_captcha: i64 = sqlx::query_scalar("INSERT INTO credits.captcha_challenges (address, answer_x, expires_at) VALUES ($1, 1, $2) RETURNING id")
        .bind(&addr2)
        .bind(super::store::now() - chrono::Duration::days(2))
        .fetch_one(&pool)
        .await
        .unwrap();
    let fresh_captcha: i64 = sqlx::query_scalar("INSERT INTO credits.captcha_challenges (address, answer_x, expires_at) VALUES ($1, 1, $2) RETURNING id")
        .bind(&addr2)
        .bind(super::store::now() - chrono::Duration::hours(1))
        .fetch_one(&pool)
        .await
        .unwrap();
    let idle = state_with(
        pool.clone(),
        StateCfg::default(),
        FakeMarket::default(),
        MockBroker::unconfigured(),
    );
    let processed = idle.outbox_worker().run_once().await.unwrap();
    assert_eq!(processed, 0, "no broker: the pending lines idle");
    let auth_status: String =
        sqlx::query_scalar("SELECT status FROM credits.authorizations WHERE id = $1")
            .bind(&stale_auth)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        auth_status, "expired",
        "the tick expires stale authorizations"
    );
    let captchas: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM credits.captcha_challenges WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![stale_captcha, fresh_captcha])
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        captchas,
        vec![fresh_captcha],
        "the tick purges challenges a day past their expiry and keeps the rest"
    );
    let still_pending: i64 = sqlx::query_scalar("SELECT count(*) FROM credits.fulfillment_outbox WHERE checkout_id = $1 AND status = 'pending'").bind(outcome.id).fetch_one(&pool).await.unwrap();
    assert_eq!(still_pending, 2);

    let broke = scratch_wallet().address;
    seed_paid_credits(&pool, &broke, "1").await;
    credits
        .add_item(cart_line(&broke, "1", &urn_for("1"), 1), "3")
        .await
        .unwrap();
    let idem = unique("qc-checkout-broke");
    let err = credits
        .run_checkout(&broke, &idem, &[line("1", "3")])
        .await
        .unwrap_err();
    assert_eq!(err.status().as_u16(), 402);
    let status: String =
        sqlx::query_scalar("SELECT status FROM credits.checkouts WHERE idempotency_key = $1")
            .bind(&idem)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        status, "failed",
        "an unfunded checkout is still recorded as failed"
    );
    assert_eq!(
        credits.get_cart(&broke).await.unwrap().items.len(),
        1,
        "a failed checkout leaves the cart alone"
    );

    for a in [&addr, &addr2, &broke] {
        cleanup(&pool, a).await;
    }
}

async fn quote(app: &Router, body: Value) -> Value {
    let (status, _, v) = send(
        app,
        "POST",
        "/credits/prices/quote",
        HeaderMap::new(),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    v
}

#[tokio::test]
async fn quotes_price_store_mints_only_and_memoise_the_misses() {
    let Some(pool) = pool().await else { return };
    let body = json!({ "items": [{ "itemId": "4", "collection": COLLECTION }], "amounts": ["1000000000000000000", "abc", "500000000000000000"] });

    let mint = app(state_with(
        pool.clone(),
        StateCfg::default(),
        FakeMarket::default()
            .with_item(COLLECTION, "4", "2500000000000000000", true)
            .with_mana_usd("0.5"),
        MockBroker::unconfigured(),
    ));
    let v = quote(&mint, body.clone()).await;
    assert_eq!(
        v["items"][0]["credits"], "13",
        "the store mint: 2.5 MANA at 0.5 USD is 13 credits: {v}"
    );
    assert_eq!(v["amounts"], json!(["5", null, "3"]));
    let again = quote(&mint, body.clone()).await;
    assert_eq!(again, v, "cached items and repriced amounts are stable");

    // The legacy modes priced the cheapest resale listing; a listing is never a credit price now.
    for legacy_mode in ["secondary", "auto"] {
        let listed_only = FakeMarket::default()
            .with_item(COLLECTION, "4", "2500000000000000000", false)
            .with_listing(COLLECTION, "1000000000000000000")
            .with_mana_usd("0.5");
        let app_listed = app(state_with(
            pool.clone(),
            StateCfg {
                fulfillment_mode: legacy_mode,
                ..StateCfg::default()
            },
            listed_only,
            MockBroker::unconfigured(),
        ));
        let v = quote(&app_listed, body.clone()).await;
        assert!(
            v["items"][0]["credits"].is_null(),
            "{legacy_mode}: an item sold only from resale listings has no credit price: {v}"
        );
        let undercut = FakeMarket::default()
            .with_item(COLLECTION, "4", "2500000000000000000", true)
            .with_listing(COLLECTION, "1000000000000000000")
            .with_mana_usd("0.5");
        let app_undercut = app(state_with(
            pool.clone(),
            StateCfg {
                fulfillment_mode: legacy_mode,
                ..StateCfg::default()
            },
            undercut,
            MockBroker::unconfigured(),
        ));
        let v = quote(
            &app_undercut,
            json!({ "items": [{ "itemId": "4", "collection": COLLECTION }] }),
        )
        .await;
        assert_eq!(
            v["items"][0]["credits"], "13",
            "{legacy_mode}: the 1 MANA listing never undercuts the mint's credit price: {v}"
        );
    }

    let (status, _, v) = send(
        &mint,
        "POST",
        "/credits/prices/quote",
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    let no_oracle = app(state_with(
        pool.clone(),
        StateCfg::default(),
        FakeMarket::default().with_item(COLLECTION, "4", "1", true),
        MockBroker::unconfigured(),
    ));
    let (status, _, v) = send(
        &no_oracle,
        "POST",
        "/credits/prices/quote",
        HeaderMap::new(),
        Some(json!({ "amounts": ["1"] })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "no MANA price is a 503, never a quote: {v}"
    );
}

/// A trade-book row as the trades read serializes it: an ERC-721 order (3)
/// over `ITEM_TOKEN_ID`, or the creator's item order (4) over item `value`.
fn trade_book_row(id: &str, sent_type: u64, value: &str) -> Value {
    let (kind, key) = if sent_type == 3 {
        ("public_nft_order", "tokenId")
    } else {
        ("public_item_order", "itemId")
    };
    json!({
        "id": id, "signer": CREATOR, "signature": "0x00", "type": kind, "network": "MATIC", "chainId": 137, "contract": "0x540fb08edb56aae562864b390542c97f562825ba",
        "checks": { "uses": 1, "expiration": 4102444800000i64, "effective": 0, "salt": "0x", "contractSignatureIndex": 0, "signerSignatureIndex": 0 },
        "sent": [{ "assetType": sent_type, "contractAddress": COLLECTION, key: value, "extra": "0x" }],
        "received": [{ "assetType": 2, "contractAddress": "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "amount": "5000000000000000000", "beneficiary": CREATOR, "extra": "0x" }],
        "createdAt": 0,
    })
}

/// The fulfilment job asks the rule before the broker: a legacy `secondary`
/// row and an ERC-721 trade row are refused with the documented reason and
/// refunded whatever their mode says; a creator item order is fulfilled with
/// its charge; a refused checkout cannot be force-fulfilled.
#[tokio::test]
async fn the_outbox_refuses_a_resale_row_whatever_its_mode_says_and_refunds_it() {
    let _serial = OUTBOX_SERIAL.lock().await;
    let Some(pool) = pool().await else { return };
    let addr = scratch_wallet().address;
    let market = market_for_checkout()
        .with_trade("4f7-de5a1e", trade_book_row("4f7-de5a1e", 3, ITEM_TOKEN_ID))
        .with_trade("17e4-0de4", trade_book_row("17e4-0de4", 4, "1"));
    let broker = MockBroker::configured();
    let buys = broker.buys.clone();
    let state = state_with(pool.clone(), StateCfg::default(), market, broker);
    let worker = state.outbox_worker();
    let urn = format!("urn:decentraland:matic:collections-v2:{COLLECTION}:1");
    seed_wallet(&pool, &addr, "0", "100").await;
    let checkout_status = |id: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT status FROM credits.checkouts WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let last_error = |id: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT last_error FROM credits.fulfillment_outbox WHERE checkout_id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .unwrap_or_default()
        }
    };

    state
        .credits
        .add_item(cart_line(&addr, "1", &urn, 1), "5")
        .await
        .unwrap();
    let legacy = RepricedLine {
        token_id: Some(ITEM_TOKEN_ID.into()),
        basis_wei: Some("1000000000000000000".into()),
        mode: "secondary".into(),
        ..line("1", "5")
    };
    let out = state
        .credits
        .run_checkout(&addr, &unique("resale-row"), &[legacy])
        .await
        .unwrap();
    assert_eq!(available(&pool, &addr).await, "95");
    assert_eq!(worker.run_once().await.unwrap(), 1);
    assert_eq!(
        checkout_status(out.id).await,
        "reversed",
        "a resale row is refused terminally and reversed in the same pass"
    );
    assert_eq!(
        available(&pool, &addr).await,
        "100",
        "the credits came back"
    );
    let error = last_error(out.id).await;
    assert!(
        error.contains("payment rule") && error.contains("MANA only"),
        "{error}"
    );
    assert!(buys.lock().is_empty(), "the broker was never asked");
    let force = state.credits.admin_force_fulfill(out.id).await.unwrap_err();
    assert!(
        matches!(force, ApiError::Conflict(_)),
        "a refused checkout is not re-armable: {force:?}"
    );

    state
        .credits
        .add_item(cart_line(&addr, "1", &urn, 1), "25")
        .await
        .unwrap();
    let nft_trade = RepricedLine {
        trade_id: Some("4f7-de5a1e".into()),
        basis_wei: Some("5000000000000000000".into()),
        mode: "trade".into(),
        ..line("1", "25")
    };
    let out = state
        .credits
        .run_checkout(&addr, &unique("resale-trade"), &[nft_trade])
        .await
        .unwrap();
    assert_eq!(worker.run_once().await.unwrap(), 1);
    assert_eq!(checkout_status(out.id).await, "reversed");
    assert_eq!(available(&pool, &addr).await, "100");
    assert!(last_error(out.id).await.contains("payment rule"));
    assert!(buys.lock().is_empty());

    state
        .credits
        .add_item(cart_line(&addr, "1", &urn, 1), "25")
        .await
        .unwrap();
    let item_trade = RepricedLine {
        trade_id: Some("17e4-0de4".into()),
        basis_wei: Some("5000000000000000000".into()),
        mode: "trade".into(),
        ..line("1", "25")
    };
    let out = state
        .credits
        .run_checkout(&addr, &unique("creator-order"), &[item_trade])
        .await
        .unwrap();
    assert_eq!(worker.run_once().await.unwrap(), 1);
    assert_eq!(
        checkout_status(out.id).await,
        "fulfilled",
        "a creator item order mints: the broker settles it"
    );
    let asked = buys.lock().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(
        (asked[0].mode.as_str(), asked[0].charge_cents),
        ("trade", Some(250)),
        "the broker is told the charge it settles"
    );
    assert_eq!(available(&pool, &addr).await, "75");
    assert_reconciles(&pool, &addr, "after the three checkouts").await;
    cleanup(&pool, &addr).await;
}

/// A cart line is priced from the store or not at all: an item sold only
/// from resale listings gets no credit price and no cart row, under the
/// legacy modes too.
#[tokio::test]
async fn a_cart_line_is_never_priced_from_a_resale_listing() {
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    for legacy_mode in ["secondary", "auto", "primary"] {
        let market = FakeMarket::default()
            .with_item(COLLECTION, "4", "2500000000000000000", false)
            .with_listing(COLLECTION, "1000000000000000000")
            .with_mana_usd("0.5");
        let state = state_with(
            pool.clone(),
            StateCfg {
                fulfillment_mode: legacy_mode,
                ..StateCfg::default()
            },
            market,
            MockBroker::configured(),
        );
        let app = app(state.clone());
        let (status, v) = call(
            &app,
            &wallet,
            "post",
            "/credits/cart/items",
            &[],
            Some(json!({ "collection": COLLECTION, "itemId": "4", "qty": 1 })),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "{legacy_mode}: no mint, nothing to price in credits; the listing is not it: {v}"
        );
        assert!(
            state
                .credits
                .get_cart(&wallet.address)
                .await
                .unwrap()
                .items
                .is_empty(),
            "{legacy_mode}"
        );
    }
    cleanup(&pool, &wallet.address).await;
}

#[tokio::test]
async fn packs_are_served_from_a_memo_until_an_admin_write() {
    let Some(pool) = pool().await else { return };
    let credits = Credits::new(pool.clone());
    let sku = unique("qc");
    let detail = json!({ "actor": "qc" });
    let mut spec = PackSpec {
        sku: &sku,
        title: "qc",
        credits: "10",
        price_cents: 100,
        currency: "usd",
        active: true,
        sort_order: 999,
    };
    credits
        .admin_create_pack(spec, "token:qc", &detail)
        .await
        .unwrap();
    assert!(matches!(
        credits.admin_create_pack(spec, "token:qc", &detail).await,
        Err(ApiError::Conflict(_))
    ));

    let listed = credits.list_active_packs().await.unwrap();
    assert!(listed.iter().any(|p| p.sku == sku));
    let pack = credits.get_pack(&sku).await.unwrap().expect("active pack");
    assert_eq!((pack.credits.as_str(), pack.price_cents), ("10", 100));

    sqlx::query("UPDATE credits.packs SET credits = 11 WHERE sku = $1")
        .bind(&sku)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        credits.get_pack(&sku).await.unwrap().unwrap().credits,
        "10",
        "a direct SQL edit is invisible for the memo's lifetime"
    );

    spec.active = false;
    credits
        .admin_update_pack(spec, "token:qc", &detail)
        .await
        .unwrap();
    assert!(
        credits.get_pack(&sku).await.unwrap().is_none(),
        "an admin write invalidates the memo; deactivated packs are not served"
    );
    assert!(
        credits
            .admin_list_packs()
            .await
            .unwrap()
            .iter()
            .any(|p| p.sku == sku && !p.active)
    );

    credits
        .admin_delete_pack(&sku, "token:qc", &detail)
        .await
        .unwrap();
    assert!(matches!(
        credits.admin_delete_pack(&sku, "token:qc", &detail).await,
        Err(ApiError::NotFound(_))
    ));
    let audits: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM ops.admin_audit WHERE domain = 'credits' AND target = $1 ORDER BY id",
    )
    .bind(&sku)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audits, ["pack.create", "pack.update", "pack.delete"]);
    sqlx::query("DELETE FROM ops.admin_audit WHERE domain = 'credits' AND target = $1")
        .bind(&sku)
        .execute(&pool)
        .await
        .unwrap();
}

/// `GET /captcha` writes a challenge: off or held it answers the credits
/// 503 and writes nothing.
#[tokio::test]
async fn captcha_generate_waits_for_the_credits_switch_and_the_restore_stamps() {
    use crate::shared::writes::restore::{
        lab::{HOUR, StampLab},
        scoped,
    };
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let (w, p) = (&wallet, &pool);
    let challenges = move || async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM credits.captcha_challenges WHERE address = $1",
        )
        .bind(&w.address)
        .fetch_one(p)
        .await
        .unwrap()
    };
    let get = move |app: Router| async move {
        let headers = signed_headers(w, "get", "/credits/captcha").await;
        let mut req = Request::builder().method("GET").uri("/credits/captcha");
        for (k, v) in headers.iter() {
            req = req.header(k, v);
        }
        let resp = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let status = resp.status();
        let body = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, body)
    };
    let off = app(test_state(
        pool.clone(),
        StateCfg {
            writes_enabled: false,
            ..StateCfg::default()
        },
    ));
    let (status, body) = get(off).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["message"], crate::shared::writes::WRITES_DISABLED_MESSAGE);
    let on = app(test_state(pool.clone(), StateCfg::default()));
    let lab = StampLab::fresh();
    lab.advance(30 * HOUR);
    let (status, body) = scoped(lab.stamps.clone(), get(on.clone())).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        v["message"],
        "operator-plane writes are held on this node: the last complete backup (backup.json) is 31 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
    );
    assert_eq!(challenges().await, 0);
    let (status, _) = get(on).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(challenges().await, 1);
}

#[tokio::test]
async fn captcha_generate_consumes_the_prior_challenge_and_claim_reads_the_block_flag() {
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let addr = wallet.address.clone();
    let state = test_state(pool.clone(), StateCfg::default());
    let app = app(state.clone());

    let seed_id: i64 = sqlx::query_scalar("INSERT INTO credits.captcha_challenges (address, answer_x, expires_at) VALUES ($1, 10, $2) RETURNING id")
        .bind(&addr)
        .bind(future())
        .fetch_one(&pool)
        .await
        .unwrap();
    let headers = signed_headers(&wallet, "get", "/credits/captcha").await;
    let resp = app
        .clone()
        .oneshot({
            let mut req = Request::builder().method("GET").uri("/credits/captcha");
            for (k, v) in headers.iter() {
                req = req.header(k, v);
            }
            req.body(Body::empty()).unwrap()
        })
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/png"
    );
    let png = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let seeded_consumed: bool = sqlx::query_scalar(
        "SELECT consumed_at IS NOT NULL FROM credits.captcha_challenges WHERE id = $1",
    )
    .bind(seed_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(seeded_consumed, "the prior open challenge must be consumed");
    let open_count: i64 = sqlx::query_scalar("SELECT count(*) FROM credits.captcha_challenges WHERE address = $1 AND consumed_at IS NULL").bind(&addr).fetch_one(&pool).await.unwrap();
    assert_eq!(open_count, 1);

    let seed_challenge = |pool: PgPool, addr: String| async move {
        sqlx::query("INSERT INTO credits.captcha_challenges (address, answer_x, expires_at) VALUES ($1, 10, $2)").bind(&addr).bind(future()).execute(&pool).await.unwrap();
    };
    sqlx::query("UPDATE credits.captcha_challenges SET consumed_at = $2 WHERE address = $1")
        .bind(&addr)
        .bind(super::store::now())
        .execute(&pool)
        .await
        .unwrap();
    seed_challenge(pool.clone(), addr.clone()).await;
    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/captcha",
        &[],
        Some(json!({ "x": 10.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "ok": true, "credits_granted": 0.0, "isBlockedForClaiming": false })
    );

    state
        .credits
        .admin_set_blocked(&addr, true, Some("qc"), Some("token:qc"), &json!({}))
        .await
        .unwrap();
    seed_challenge(pool.clone(), addr.clone()).await;
    let (_, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/captcha",
        &[],
        Some(json!({ "x": 10.0 })),
    )
    .await;
    assert_eq!(v["ok"], false);
    assert_eq!(v["isBlockedForClaiming"], true);

    seed_challenge(pool.clone(), addr.clone()).await;
    let (_, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/captcha",
        &[],
        Some(json!({ "x": 99.0 })),
    )
    .await;
    assert_eq!(v["ok"], false, "a wrong slider answer is rejected");

    let (status, v) = call(
        &app,
        &wallet,
        "post",
        "/credits/captcha",
        &[],
        Some(json!({ "x": 10.0 })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "no open challenge left: {v}"
    );

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn the_hcaptcha_provider_verifies_through_the_guarded_client() {
    use axum::extract::Form;
    use axum::routing::post;
    const SECRET: &str = "test-secret";
    const GOOD_TOKEN: &str = "valid-token";
    async fn siteverify(Form(form): Form<HashMap<String, String>>) -> axum::Json<Value> {
        if form.get("secret").map(String::as_str) != Some(SECRET) {
            return axum::Json(
                json!({ "success": false, "error-codes": ["invalid-input-secret"] }),
            );
        }
        axum::Json(
            json!({ "success": form.get("response").map(String::as_str) == Some(GOOD_TOKEN) }),
        )
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/siteverify", post(siteverify)),
        )
        .await
        .unwrap()
    });
    let client = Outbound::for_tests().allowing_private_hosts();
    let provider = super::captcha::CaptchaProvider::new(
        SECRET.to_string(),
        format!("http://{addr}/siteverify"),
        client.clone(),
    );
    assert!(provider.verify(GOOD_TOKEN, None).await.unwrap());
    assert!(
        !provider
            .verify("wrong-token", Some("1.2.3.4"))
            .await
            .unwrap()
    );
    let bad = super::captcha::CaptchaProvider::new(
        "bad-secret".to_string(),
        format!("http://{addr}/siteverify"),
        client.clone(),
    );
    assert!(!bad.verify(GOOD_TOKEN, None).await.unwrap());
    let unreachable = super::captcha::CaptchaProvider::new(
        SECRET.to_string(),
        "http://127.0.0.1:1/siteverify".to_string(),
        client,
    );
    assert!(unreachable.verify(GOOD_TOKEN, None).await.is_err());
    let production = Outbound::new(crate::http::HttpClientCfg::default()).unwrap();
    let guarded = super::captcha::CaptchaProvider::new(
        SECRET.to_string(),
        format!("http://{addr}/siteverify"),
        production,
    );
    assert!(
        guarded.verify(GOOD_TOKEN, None).await.is_err(),
        "the production client refuses a loopback verify URL"
    );
}

#[tokio::test]
async fn admin_routes_fail_closed_without_the_bearer_and_audit_with_it() {
    let Some(pool) = pool().await else { return };
    let unset = app(test_state(pool.clone(), StateCfg::default()));
    let (status, _, v) = send(
        &unset,
        "GET",
        "/credits/admin/packs",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        v["error"],
        "admin controls are disabled (CATALYRST_CREDITS_ADMIN_TOKEN unset)"
    );

    let state = test_state(
        pool.clone(),
        StateCfg {
            admin_token: Some("s3cret"),
            ..StateCfg::default()
        },
    );
    let app = app(state.clone());
    let mut wrong = HeaderMap::new();
    wrong.insert(header::AUTHORIZATION, "Bearer nope".parse().unwrap());
    let (status, _, v) = send(
        &app,
        "GET",
        "/credits/admin/ledger?address=0x0000000000000000000000000000000000000001",
        wrong,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(v["error"], "invalid admin token");

    let mut bearer = HeaderMap::new();
    bearer.insert(header::AUTHORIZATION, "Bearer s3cret".parse().unwrap());
    let addr = scratch_wallet().address;
    let (status, _, v) = send(&app, "POST", "/credits/admin/credits/grant", bearer.clone(), Some(json!({ "address": addr.to_uppercase().replace("0X", "0x"), "amount": "12.5", "reason": "qc", "idempotencyKey": unique("g") }))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "address": addr, "applied": "12.5", "available": "12.5", "replayed": false })
    );
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/credits/revoke",
        bearer.clone(),
        Some(json!({ "address": addr, "amount": "20" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["applied"], "12.5", "a revoke floors at zero");
    assert_eq!(v["available"], "0");
    let (status, _, v) = send(
        &app,
        "POST",
        &format!("/credits/admin/users/{addr}/block"),
        bearer.clone(),
        Some(json!({ "blocked": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let (status, _, v) = send(
        &app,
        "GET",
        &format!("/credits/admin/ledger?address={addr}"),
        bearer.clone(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().unwrap().len(), 2);
    assert_eq!(v[0]["kind"], "consume");
    assert_eq!(v[1]["kind"], "grant");
    let (status, _, v) = send(
        &app,
        "GET",
        "/credits/admin/reconcile",
        bearer.clone(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["creditUsageIndex"], "unavailable");
    assert!(v["escrowHoldings"]["available"].as_bool().unwrap());
    let (status, _, v) = send(
        &app,
        "GET",
        &format!("/credits/admin/purchases?address={addr}"),
        bearer,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v, json!([]));

    let audits: Vec<(String, String)> = sqlx::query_as("SELECT action, actor FROM ops.admin_audit WHERE domain = 'credits' AND target = $1 ORDER BY id").bind(&addr).fetch_all(&pool).await.unwrap();
    assert_eq!(
        audits,
        vec![
            (
                "grant".to_string(),
                "token:CATALYRST_CREDITS_ADMIN_TOKEN".to_string()
            ),
            (
                "revoke".to_string(),
                "token:CATALYRST_CREDITS_ADMIN_TOKEN".to_string()
            ),
            (
                "user.block".to_string(),
                "token:CATALYRST_CREDITS_ADMIN_TOKEN".to_string()
            ),
        ]
    );
    let linked: i64 = sqlx::query_scalar("SELECT count(*) FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id WHERE l.address = $1").bind(&addr).fetch_one(&pool).await.unwrap();
    assert_eq!(linked, 2, "every operator ledger row names its audit row");
    assert_reconciles(&pool, &addr, "after the operator sequence").await;

    cleanup(&pool, &addr).await;
}

#[tokio::test]
async fn the_write_gate_refuses_every_credits_write_and_serves_reads_and_quotes() {
    let Some(pool) = pool().await else { return };
    let wallet = scratch_wallet();
    let app = app(state_with(
        pool.clone(),
        StateCfg {
            writes_enabled: false,
            mock_card: true,
            ..StateCfg::default()
        },
        FakeMarket::default().with_mana_usd("0.5"),
        MockBroker::configured(),
    ));

    for (method, path, body) in [
        ("post", "/credits/users", None),
        (
            "post",
            "/credits/cart/items",
            Some(json!({ "collection": COLLECTION, "itemId": "1" })),
        ),
        ("post", "/credits/checkout", Some(json!({}))),
        (
            "post",
            "/credits/topup/mock-card",
            Some(json!({ "credits": "5" })),
        ),
        ("post", "/credits/captcha", Some(json!({ "x": 1 }))),
    ] {
        let (status, v) = call(&app, &wallet, method, path, &[], body).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {v}"
        );
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("OPS_WRITES_ENABLED=false")
        );
    }
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/stripe/webhook",
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{v}");
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/credits/grant",
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{v}");

    let (status, _, v) = send(&app, "GET", "/credits/packs", HeaderMap::new(), None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let (status, v) = call(&app, &wallet, "get", "/credits/cart", &[], None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["items"], json!([]));
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/prices/quote",
        HeaderMap::new(),
        Some(json!({ "amounts": ["1000000000000000000"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a quote is a read: {v}");
    assert_eq!(v["amounts"], json!(["5"]));
}
