//! The market federation: wallet-signed bid, order and trade actions
//! (`crate::fed` envelopes in the `DecentralandMarket` domain) appended to
//! ONE log, `shop.fed_actions`, keyed by the envelope's digest and
//! numbered by one identity `seq`, and served back as the old
//! `/v1/federation/{bids,orders,trades}` lists and the
//! `/federation/market/{snapshot,changes}` feeds peers pull. The log is its
//! own truth: `/v1/bids`, `/v1/orders` and `/v1/trades` never read it, as
//! before.
//!
//! A write passes the old preflight in its order: the request's signed-fetch
//! chain (401), the envelope (400), the envelope's signer = the chain's
//! signer inside the skew window (401), the domain (400), a new nonce for the
//! signer (409; `shop.fed_seen_nonces`, swept by
//! `market-fed-nonce-sweep`), 120 actions a minute per signer (429). Then the
//! old authority rules: a bid, an order's cancel only by its signer (403,
//! 404 unknown); a bid's accept only by a wallet owning a token of the bid's
//! item (the nft index; its envelope until published); an order only by an
//! owner of a token of the item who holds no active lease on it (403); a
//! trade record by anyone, for a known order (404). The writes sit behind
//! `DATA_WRITES_ENABLED` (the router's gate).
//!
//! What changed: the five old tables (`market_bids_local`, ...) are the
//! `kind` of one log, and their five BIGSERIALs are one `seq`, so the one
//! `since` cursor of `/federation/market/changes` means the same position
//! for every kind (with five sequences a cursor taken from one table
//! skipped or repeated the others' rows); a price that is not a uint256
//! decimal is a 400, where the old cast answered 500.

use std::sync::LazyLock;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

use super::{MarketplaceState, SharedState};
use crate::comms::limits::{RateLimitDecision, RateLimiter};
use crate::fed::{FedError, Signed, TypedMessage};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::signed_fetch::{self, PayloadPolicy};

pub const NONCES: &str = "shop.fed_seen_nonces";
const DOMAIN: &str = "DecentralandMarket";
const LIST_LIMIT: i64 = 500;
const DEFAULT_CHANGES: i64 = 500;
const MAX_CHANGES: i64 = 5000;

/// The old `RateLimiter::new(120, 60 s)`, per signer, process-wide.
static LIMITER: LazyLock<RateLimiter> =
    LazyLock::new(|| RateLimiter::new(120, Duration::from_secs(60)));

fn ec(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidPlace {
    pub item_id: String,
    pub price: String,
    pub expires_at: i64,
    #[serde(default)]
    pub fingerprint: String,
    pub signed_at: i64,
}

impl TypedMessage for BidPlace {
    const PRIMARY_TYPE: &'static str = "BidPlace";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.item_id);
        ec(&mut b, &self.price);
        b.extend_from_slice(&self.expires_at.to_be_bytes());
        ec(&mut b, &self.fingerprint);
        b.extend_from_slice(&self.signed_at.to_be_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidCancel {
    pub bid_signature_hash: String,
    pub signed_at: i64,
}

impl TypedMessage for BidCancel {
    const PRIMARY_TYPE: &'static str = "BidCancel";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.bid_signature_hash);
        b.extend_from_slice(&self.signed_at.to_be_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidAccept {
    pub bid_signature_hash: String,
    pub signed_at: i64,
}

impl TypedMessage for BidAccept {
    const PRIMARY_TYPE: &'static str = "BidAccept";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.bid_signature_hash);
        b.extend_from_slice(&self.signed_at.to_be_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreate {
    pub item_id: String,
    pub price: String,
    pub expires_at: i64,
    pub signed_at: i64,
}

impl TypedMessage for OrderCreate {
    const PRIMARY_TYPE: &'static str = "OrderCreate";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.item_id);
        ec(&mut b, &self.price);
        b.extend_from_slice(&self.expires_at.to_be_bytes());
        b.extend_from_slice(&self.signed_at.to_be_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancel {
    pub order_signature_hash: String,
    pub signed_at: i64,
}

impl TypedMessage for OrderCancel {
    const PRIMARY_TYPE: &'static str = "OrderCancel";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.order_signature_hash);
        b.extend_from_slice(&self.signed_at.to_be_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub order_signature_hash: String,
    pub buyer: String,
    pub taken_at: i64,
    pub tx_hash: String,
    pub signed_at: i64,
}

impl TypedMessage for TradeRecord {
    const PRIMARY_TYPE: &'static str = "TradeRecord";
    fn encode_struct(&self) -> Vec<u8> {
        let mut b = Vec::new();
        ec(&mut b, Self::PRIMARY_TYPE);
        ec(&mut b, &self.order_signature_hash);
        ec(&mut b, &self.buyer);
        b.extend_from_slice(&self.taken_at.to_be_bytes());
        ec(&mut b, &self.tx_hash);
        b.extend_from_slice(&self.signed_at.to_be_bytes());
        b
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FedAck {
    pub ok: bool,
    pub signature_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FedErrorBody {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MarketSnapshot {
    pub latest_bids_seq: i64,
    pub latest_orders_seq: i64,
    pub latest_trades_seq: i64,
    pub latest_cancellations_seq: i64,
    pub latest_acceptances_seq: i64,
    pub log_hash: String,
    pub domain: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketChanges {
    pub bids: Vec<BidChange>,
    pub orders: Vec<OrderChange>,
    pub trades: Vec<TradeChange>,
    pub cancellations: Vec<CancelChange>,
    pub acceptances: Vec<AcceptChange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BidChange {
    pub kind: &'static str,
    #[serde(flatten)]
    pub entry: FedBidEntry,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderChange {
    pub kind: &'static str,
    #[serde(flatten)]
    pub entry: FedOrderEntry,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeChange {
    pub kind: &'static str,
    #[serde(flatten)]
    pub entry: FedTradeEntry,
}

#[derive(Debug, Clone, Serialize)]
pub struct CancelChange {
    pub kind: &'static str,
    pub signature_hash: String,
    pub target_signature_hash: String,
    pub target_kind: String,
    pub signer: String,
    pub signed_at: i64,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcceptChange {
    pub kind: &'static str,
    pub signature_hash: String,
    pub bid_signature_hash: String,
    pub signer: String,
    pub signed_at: i64,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FedList<T> {
    pub data: Vec<T>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FedBidEntry {
    pub signature_hash: String,
    pub item_id: String,
    pub signer: String,
    pub price: String,
    pub expires_at: i64,
    pub fingerprint: String,
    pub signed_at: i64,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FedOrderEntry {
    pub signature_hash: String,
    pub item_id: String,
    pub signer: String,
    pub price: String,
    pub expires_at: i64,
    pub signed_at: i64,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FedTradeEntry {
    pub signature_hash: String,
    pub order_signature_hash: String,
    pub buyer: String,
    pub tx_hash: String,
    pub taken_at: i64,
    pub signed_at: i64,
    pub seq: i64,
}

fn err_json(code: StatusCode, message: impl Into<String>) -> Response {
    (
        code,
        Json(FedErrorBody {
            ok: false,
            message: message.into(),
        }),
    )
        .into_response()
}

fn ok_json(signature_hash: String) -> Response {
    (
        StatusCode::OK,
        Json(FedAck {
            ok: true,
            signature_hash,
        }),
    )
        .into_response()
}

fn db_err(e: sqlx::Error) -> Response {
    tracing::error!(error = %e, "federation apply database error");
    err_json(StatusCode::INTERNAL_SERVER_ERROR, "database error")
}

/// The old preflight, stage by stage; the envelope and its lowercased signer.
async fn preflight<T: TypedMessage + DeserializeOwned>(
    state: &MarketplaceState,
    headers: &HeaderMap,
    path: &str,
    body: &[u8],
) -> Result<(Signed<T>, String), Response> {
    let outer = signed_fetch::require_signer(
        headers,
        "post",
        path,
        PayloadPolicy::V6ThenLegacy,
        state.eip1654.as_deref(),
    )
    .await
    .map_err(|e| err_json(StatusCode::UNAUTHORIZED, format!("auth chain: {e}")))?
    .into_string();
    let signed: Signed<T> = serde_json::from_slice(body).map_err(|e| {
        err_json(
            StatusCode::BAD_REQUEST,
            format!("invalid Signed<{}>: {e}", T::PRIMARY_TYPE),
        )
    })?;
    signed
        .verify(&outer, chrono::Utc::now().timestamp())
        .map_err(|e| err_json(StatusCode::UNAUTHORIZED, format!("signature verify: {e}")))?;
    if !signed.domain.name.eq_ignore_ascii_case(DOMAIN) {
        return Err(err_json(
            StatusCode::BAD_REQUEST,
            format!("domain mismatch: expected {DOMAIN}"),
        ));
    }
    if let Err(e) =
        crate::fed::check_and_record(&state.pool, NONCES, &outer, &signed.nonce, signed.signed_at)
            .await
    {
        return Err(match e {
            FedError::DuplicateNonce { .. } => err_json(StatusCode::CONFLICT, e.to_string()),
            other => err_json(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        });
    }
    if LIMITER.check(&outer) == RateLimitDecision::Deny {
        return Err(err_json(
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
        ));
    }
    Ok((signed, outer.to_lowercase()))
}

/// A uint256 decimal: the log's `numeric(78,0)` holds it exactly.
fn valid_price(price: &str) -> Result<(), Response> {
    if crate::rentals::db::is_decimal(price) {
        Ok(())
    } else {
        Err(err_json(
            StatusCode::BAD_REQUEST,
            "price must be a non-negative integer (wei) of at most 78 digits",
        ))
    }
}

#[derive(Default)]
struct Row0<'a> {
    kind: &'a str,
    item_id: Option<&'a str>,
    target_hash: Option<&'a str>,
    target_kind: Option<&'a str>,
    price: Option<&'a str>,
    expires_at: Option<i64>,
    fingerprint: Option<&'a str>,
    buyer: Option<String>,
    tx_hash: Option<&'a str>,
    taken_at: Option<i64>,
}

/// Appends the action; a digest already in the log is left as it is (the
/// old `ON CONFLICT DO NOTHING`), and the ack names it either way.
async fn append<T: TypedMessage>(
    pool: &PgPool,
    signed: &Signed<T>,
    signer: &str,
    row: Row0<'_>,
) -> Result<String, sqlx::Error> {
    let signature_hash = signed.signature_hash();
    let payload = serde_json::to_value(&signed.message).unwrap_or(Value::Null);
    sqlx::query(
        "INSERT INTO shop.fed_actions \
           (signature_hash, kind, signer, item_id, target_hash, target_kind, price, expires_at, fingerprint, buyer, tx_hash, taken_at, signed_at, message_payload) \
         VALUES ($1, $2, $3, $4, $5, $6, $7::numeric, $8, $9, $10, $11, $12, $13, $14) \
         ON CONFLICT (signature_hash) DO NOTHING",
    )
    .bind(&signature_hash)
    .bind(row.kind)
    .bind(signer)
    .bind(row.item_id)
    .bind(row.target_hash)
    .bind(row.target_kind)
    .bind(row.price)
    .bind(row.expires_at)
    .bind(row.fingerprint)
    .bind(row.buyer)
    .bind(row.tx_hash)
    .bind(row.taken_at)
    .bind(signed.signed_at)
    .bind(&payload)
    .execute(pool)
    .await?;
    Ok(signature_hash)
}

/// `(signer, item_id)` of a logged action of `kind`.
async fn lookup(
    pool: &PgPool,
    kind: &str,
    signature_hash: &str,
) -> Result<Option<(String, Option<String>)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT signer, item_id FROM shop.fed_actions WHERE signature_hash = $1 AND kind = $2",
    )
    .bind(signature_hash)
    .bind(kind)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (r.get("signer"), r.get("item_id"))))
}

/// An item id is the collection subgraph's `<contract>-<item>`: a v2
/// collection's decimal item id, or a v1 collection's wearable id.
fn split_item(item_id: &str) -> Option<(String, String)> {
    let (contract, item) = item_id.rsplit_once('-')?;
    let contract = contract.to_lowercase();
    (super::is_valid_eth_address(&contract) && !item.is_empty())
        .then(|| (contract, item.to_string()))
}

/// Whether `signer` owns a token of `item_id` (the old squid `nft` probe,
/// now the nft index); the index's envelope while a network it may answer
/// for is not published and it found no token.
async fn owns_any_nft_for_item(
    state: &MarketplaceState,
    signer: &str,
    item_id: &str,
) -> Result<bool, Response> {
    let Some((contract, item)) = split_item(item_id) else {
        return Ok(false);
    };
    let ready = super::nft_index::ready_networks(&state.pool, chrono::Utc::now()).await;
    let networks: Vec<String> = ready.iter().map(|n| n.db_name().to_string()).collect();
    let numeric = item
        .bytes()
        .all(|b| b.is_ascii_digit())
        .then_some(item.as_str());
    let found: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.nft WHERE owner = $1 AND network = ANY($2) AND contract_address = $3 \
           AND (item_id = $4::numeric OR ($4 IS NULL AND wearable_id = $5)))",
    )
    .bind(signer.to_lowercase())
    .bind(&networks)
    .bind(&contract)
    .bind(numeric)
    .bind(&item)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;
    if !found && ready.len() < super::chain_walk::ChainNet::ALL.len() {
        return Err(super::unavailable(
            super::Source::NftIndex,
            "item ownership reads the nft index, which is not published yet for every network",
        ));
    }
    Ok(found)
}

/// Whether `signer` holds an active usage grant on `item_id` (the old lease
/// probe: `status = 'active'`, whatever its unlock time).
async fn has_active_lease_for_item(
    pool: &PgPool,
    signer: &str,
    item_id: &str,
) -> Result<bool, Response> {
    let Some((contract, item)) = split_item(item_id) else {
        return Ok(false);
    };
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.usage_grants WHERE grantee_address = $1 AND status = 'active' \
           AND lower(split_part(urn, ':', 5)) = $2 AND split_part(urn, ':', 6) = $3)",
    )
    .bind(signer.to_lowercase())
    .bind(&contract)
    .bind(&item)
    .fetch_one(pool)
    .await
    .map_err(db_err)
}

pub async fn place_bid(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (signed, signer) =
        match preflight::<BidPlace>(&state, &headers, "/v1/federation/bid", &body).await {
            Ok(x) => x,
            Err(r) => return r,
        };
    if let Err(r) = valid_price(&signed.message.price) {
        return r;
    }
    let m = &signed.message;
    let row = Row0 {
        kind: "bid",
        item_id: Some(&m.item_id),
        price: Some(&m.price),
        expires_at: Some(m.expires_at),
        fingerprint: Some(&m.fingerprint),
        ..Default::default()
    };
    append(&state.pool, &signed, &signer, row)
        .await
        .map(ok_json)
        .unwrap_or_else(db_err)
}

pub async fn cancel_bid(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (signed, signer) =
        match preflight::<BidCancel>(&state, &headers, "/v1/federation/bid/cancel", &body).await {
            Ok(x) => x,
            Err(r) => return r,
        };
    let target = &signed.message.bid_signature_hash;
    match lookup(&state.pool, "bid", target).await {
        Ok(Some((original, _))) if !original.eq_ignore_ascii_case(&signer) => {
            return err_json(
                StatusCode::FORBIDDEN,
                "only the bid signer may cancel this bid",
            );
        }
        Ok(Some(_)) => {}
        Ok(None) => return err_json(StatusCode::NOT_FOUND, "bid not found"),
        Err(e) => return db_err(e),
    }
    let row = Row0 {
        kind: "cancel",
        target_hash: Some(target),
        target_kind: Some("bid"),
        ..Default::default()
    };
    append(&state.pool, &signed, &signer, row)
        .await
        .map(ok_json)
        .unwrap_or_else(db_err)
}

pub async fn accept_bid(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (signed, signer) =
        match preflight::<BidAccept>(&state, &headers, "/v1/federation/bid/accept", &body).await {
            Ok(x) => x,
            Err(r) => return r,
        };
    let target = &signed.message.bid_signature_hash;
    let item_id = match lookup(&state.pool, "bid", target).await {
        Ok(Some((_, Some(item)))) => item,
        Ok(_) => return err_json(StatusCode::NOT_FOUND, "bid not found"),
        Err(e) => return db_err(e),
    };
    match owns_any_nft_for_item(&state, &signer, &item_id).await {
        Ok(true) => {}
        Ok(false) => {
            return err_json(
                StatusCode::FORBIDDEN,
                "signer does not own any NFT for this bid's item_id",
            );
        }
        Err(r) => return r,
    }
    let row = Row0 {
        kind: "accept",
        target_hash: Some(target),
        ..Default::default()
    };
    append(&state.pool, &signed, &signer, row)
        .await
        .map(ok_json)
        .unwrap_or_else(db_err)
}

pub async fn create_order(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (signed, signer) =
        match preflight::<OrderCreate>(&state, &headers, "/v1/federation/order", &body).await {
            Ok(x) => x,
            Err(r) => return r,
        };
    let m = &signed.message;
    let (leased, owns) = tokio::join!(
        has_active_lease_for_item(&state.pool, &signer, &m.item_id),
        owns_any_nft_for_item(&state, &signer, &m.item_id)
    );
    match leased {
        Ok(true) => {
            return err_json(
                StatusCode::FORBIDDEN,
                "item is in the return window (leased); cannot be listed until it unlocks",
            );
        }
        Ok(false) => {}
        Err(r) => return r,
    }
    match owns {
        Ok(true) => {}
        Ok(false) => {
            return err_json(
                StatusCode::FORBIDDEN,
                "signer does not own any NFT for this order's item_id",
            );
        }
        Err(r) => return r,
    }
    if let Err(r) = valid_price(&m.price) {
        return r;
    }
    let row = Row0 {
        kind: "order",
        item_id: Some(&m.item_id),
        price: Some(&m.price),
        expires_at: Some(m.expires_at),
        ..Default::default()
    };
    append(&state.pool, &signed, &signer, row)
        .await
        .map(ok_json)
        .unwrap_or_else(db_err)
}

pub async fn cancel_order(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (signed, signer) = match preflight::<OrderCancel>(
        &state,
        &headers,
        "/v1/federation/order/cancel",
        &body,
    )
    .await
    {
        Ok(x) => x,
        Err(r) => return r,
    };
    let target = &signed.message.order_signature_hash;
    match lookup(&state.pool, "order", target).await {
        Ok(Some((original, _))) if !original.eq_ignore_ascii_case(&signer) => {
            return err_json(
                StatusCode::FORBIDDEN,
                "only the order signer may cancel this order",
            );
        }
        Ok(Some(_)) => {}
        Ok(None) => return err_json(StatusCode::NOT_FOUND, "order not found"),
        Err(e) => return db_err(e),
    }
    let row = Row0 {
        kind: "cancel",
        target_hash: Some(target),
        target_kind: Some("order"),
        ..Default::default()
    };
    append(&state.pool, &signed, &signer, row)
        .await
        .map(ok_json)
        .unwrap_or_else(db_err)
}

pub async fn record_trade(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (signed, signer) =
        match preflight::<TradeRecord>(&state, &headers, "/v1/federation/trade", &body).await {
            Ok(x) => x,
            Err(r) => return r,
        };
    let m = &signed.message;
    match lookup(&state.pool, "order", &m.order_signature_hash).await {
        Ok(Some(_)) => {}
        Ok(None) => return err_json(StatusCode::NOT_FOUND, "order not found"),
        Err(e) => return db_err(e),
    }
    let row = Row0 {
        kind: "trade",
        target_hash: Some(&m.order_signature_hash),
        buyer: Some(m.buyer.to_ascii_lowercase()),
        tx_hash: Some(&m.tx_hash),
        taken_at: Some(m.taken_at),
        ..Default::default()
    };
    append(&state.pool, &signed, &signer, row)
        .await
        .map(ok_json)
        .unwrap_or_else(db_err)
}

const BID_COLUMNS: &str = "signature_hash, item_id, signer, price::text AS price, expires_at, COALESCE(fingerprint, '') AS fingerprint, signed_at, seq";
const ORDER_COLUMNS: &str =
    "signature_hash, item_id, signer, price::text AS price, expires_at, signed_at, seq";
const TRADE_COLUMNS: &str = "signature_hash, target_hash, buyer, tx_hash, taken_at, signed_at, seq";

fn bid(r: &sqlx::postgres::PgRow) -> FedBidEntry {
    FedBidEntry {
        signature_hash: r.get("signature_hash"),
        item_id: r.get("item_id"),
        signer: r.get("signer"),
        price: r.get("price"),
        expires_at: r.get("expires_at"),
        fingerprint: r.get("fingerprint"),
        signed_at: r.get("signed_at"),
        seq: r.get("seq"),
    }
}

fn order(r: &sqlx::postgres::PgRow) -> FedOrderEntry {
    FedOrderEntry {
        signature_hash: r.get("signature_hash"),
        item_id: r.get("item_id"),
        signer: r.get("signer"),
        price: r.get("price"),
        expires_at: r.get("expires_at"),
        signed_at: r.get("signed_at"),
        seq: r.get("seq"),
    }
}

fn trade(r: &sqlx::postgres::PgRow) -> FedTradeEntry {
    FedTradeEntry {
        signature_hash: r.get("signature_hash"),
        order_signature_hash: r.get("target_hash"),
        buyer: r.get("buyer"),
        tx_hash: r.get("tx_hash"),
        taken_at: r.get("taken_at"),
        signed_at: r.get("signed_at"),
        seq: r.get("seq"),
    }
}

async fn newest(
    pool: &PgPool,
    kind: &str,
    columns: &str,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {columns} FROM shop.fed_actions WHERE kind = $1 ORDER BY seq DESC LIMIT $2"
    )))
    .bind(kind)
    .bind(LIST_LIMIT)
    .fetch_all(pool)
    .await
}

async fn after(
    pool: &PgPool,
    kind: &str,
    columns: &str,
    since: i64,
    limit: i64,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {columns} FROM shop.fed_actions WHERE kind = $1 AND seq > $2 ORDER BY seq ASC LIMIT $3")))
        .bind(kind)
        .bind(since)
        .bind(limit)
        .fetch_all(pool)
        .await
}

fn list<T: Serialize>(
    rows: Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>,
    f: fn(&sqlx::postgres::PgRow) -> T,
) -> Response {
    match rows {
        Ok(rows) => {
            let data: Vec<T> = rows.iter().map(f).collect();
            let total = data.len();
            Json(FedList { data, total }).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// `GET /v1/federation/bids`: the newest 500 bids of the log.
pub async fn list_bids(State(state): State<SharedState>) -> Response {
    list(newest(&state.pool, "bid", BID_COLUMNS).await, bid)
}

pub async fn list_orders(State(state): State<SharedState>) -> Response {
    list(newest(&state.pool, "order", ORDER_COLUMNS).await, order)
}

pub async fn list_trades(State(state): State<SharedState>) -> Response {
    list(newest(&state.pool, "trade", TRADE_COLUMNS).await, trade)
}

/// The old digest: sha256 over the bids', then the orders', then the trades'
/// digests, each kind in digest order, no separator.
pub fn log_hash(bids: &[String], orders: &[String], trades: &[String]) -> String {
    let mut h = Sha256::new();
    for s in bids.iter().chain(orders).chain(trades) {
        h.update(s.as_bytes());
    }
    hex::encode(h.finalize())
}

pub async fn snapshot_view(pool: &PgPool) -> Result<MarketSnapshot, sqlx::Error> {
    let mut latest = [0i64; 5];
    for (slot, kind) in latest
        .iter_mut()
        .zip(["bid", "order", "trade", "cancel", "accept"])
    {
        *slot = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT max(seq) FROM shop.fed_actions WHERE kind = $1",
        )
        .bind(kind)
        .fetch_one(pool)
        .await?
        .unwrap_or(0);
    }
    let mut hashes: Vec<Vec<String>> = Vec::new();
    for kind in ["bid", "order", "trade"] {
        hashes.push(sqlx::query_scalar("SELECT signature_hash FROM shop.fed_actions WHERE kind = $1 ORDER BY signature_hash").bind(kind).fetch_all(pool).await?);
    }
    Ok(MarketSnapshot {
        latest_bids_seq: latest[0],
        latest_orders_seq: latest[1],
        latest_trades_seq: latest[2],
        latest_cancellations_seq: latest[3],
        latest_acceptances_seq: latest[4],
        log_hash: log_hash(&hashes[0], &hashes[1], &hashes[2]),
        domain: DOMAIN,
    })
}

/// `GET /federation/market/snapshot`.
pub async fn snapshot(State(state): State<SharedState>) -> Response {
    match snapshot_view(&state.pool).await {
        Ok(s) => Json(s).into_response(),
        Err(e) => db_err(e),
    }
}

#[derive(Debug, Deserialize)]
pub struct ChangesQuery {
    #[serde(default)]
    pub since: i64,
    #[serde(default)]
    pub limit: Option<i64>,
}

pub async fn changes_view(
    pool: &PgPool,
    since: i64,
    limit: i64,
) -> Result<MarketChanges, sqlx::Error> {
    let limit = limit.clamp(1, MAX_CHANGES);
    let (bids, orders, trades, cancels, accepts) = tokio::try_join!(
        after(pool, "bid", BID_COLUMNS, since, limit),
        after(pool, "order", ORDER_COLUMNS, since, limit),
        after(pool, "trade", TRADE_COLUMNS, since, limit),
        after(
            pool,
            "cancel",
            "signature_hash, target_hash, target_kind, signer, signed_at, seq",
            since,
            limit
        ),
        after(
            pool,
            "accept",
            "signature_hash, target_hash, signer, signed_at, seq",
            since,
            limit
        ),
    )?;
    Ok(MarketChanges {
        bids: bids
            .iter()
            .map(|r| BidChange {
                kind: "bid",
                entry: bid(r),
            })
            .collect(),
        orders: orders
            .iter()
            .map(|r| OrderChange {
                kind: "order",
                entry: order(r),
            })
            .collect(),
        trades: trades
            .iter()
            .map(|r| TradeChange {
                kind: "trade",
                entry: trade(r),
            })
            .collect(),
        cancellations: cancels
            .iter()
            .map(|r| CancelChange {
                kind: "cancel",
                signature_hash: r.get("signature_hash"),
                target_signature_hash: r.get("target_hash"),
                target_kind: r.get("target_kind"),
                signer: r.get("signer"),
                signed_at: r.get("signed_at"),
                seq: r.get("seq"),
            })
            .collect(),
        acceptances: accepts
            .iter()
            .map(|r| AcceptChange {
                kind: "accept",
                signature_hash: r.get("signature_hash"),
                bid_signature_hash: r.get("target_hash"),
                signer: r.get("signer"),
                signed_at: r.get("signed_at"),
                seq: r.get("seq"),
            })
            .collect(),
    })
}

/// `GET /federation/market/changes?since=&limit=` (limit 500 by default, 1..=5000 per kind).
pub async fn changes(State(state): State<SharedState>, Query(q): Query<ChangesQuery>) -> Response {
    match changes_view(&state.pool, q.since, q.limit.unwrap_or(DEFAULT_CHANGES)).await {
        Ok(c) => Json(c).into_response(),
        Err(e) => db_err(e),
    }
}

/// `market-fed-nonce-sweep`: drops the nonces whose replay window closed.
pub fn spawn_job(state: &std::sync::Arc<MarketplaceState>, jobs: &Jobs) {
    let pool = state.pool.clone();
    let cfg = PeriodicCfg::every(Duration::from_secs(300))
        .with_jitter(Duration::from_secs(30))
        .with_error_backoff(Duration::from_secs(900));
    jobs.spawn_periodic_leased("market-fed-nonce-sweep", cfg, move |_ctx| {
        let pool = pool.clone();
        async move {
            crate::fed::sweep_nonces(&pool, NONCES, chrono::Utc::now().timestamp()).await?;
            Ok(())
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::local::PrivateKeySigner;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;

    use crate::fed::test_support::sign;
    use crate::shared::signed_fetch::test_support::signed_headers_by;

    fn to_json<T: Serialize>(v: &T) -> Value {
        serde_json::to_value(v).unwrap()
    }

    #[test]
    fn wire_identity_ack_and_error() {
        assert_eq!(
            to_json(&FedAck {
                ok: true,
                signature_hash: "ab".into()
            }),
            json!({ "ok": true, "signature_hash": "ab" })
        );
        assert_eq!(
            to_json(&FedErrorBody {
                ok: false,
                message: "bid not found".into()
            }),
            json!({ "ok": false, "message": "bid not found" })
        );
    }

    #[test]
    fn wire_identity_snapshot_and_changes() {
        let snap = MarketSnapshot {
            latest_bids_seq: 3,
            latest_orders_seq: 4,
            latest_trades_seq: 0,
            latest_cancellations_seq: 7,
            latest_acceptances_seq: 0,
            log_hash: log_hash(&[], &[], &[]),
            domain: DOMAIN,
        };
        assert_eq!(
            to_json(&snap),
            json!({ "latest_bids_seq": 3, "latest_orders_seq": 4, "latest_trades_seq": 0, "latest_cancellations_seq": 7, "latest_acceptances_seq": 0, "log_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "domain": "DecentralandMarket" })
        );
        let bid = FedBidEntry {
            signature_hash: "h".into(),
            item_id: "0xc-1".into(),
            signer: "0xs".into(),
            price: "10".into(),
            expires_at: 9,
            fingerprint: String::new(),
            signed_at: 5,
            seq: 1,
        };
        assert_eq!(
            to_json(&BidChange {
                kind: "bid",
                entry: bid.clone()
            }),
            json!({ "kind": "bid", "signature_hash": "h", "item_id": "0xc-1", "signer": "0xs", "price": "10", "expires_at": 9, "fingerprint": "", "signed_at": 5, "seq": 1 })
        );
        assert_eq!(
            to_json(&FedList {
                data: vec![bid],
                total: 1
            })["data"][0]["seq"],
            1
        );
        let trade = FedTradeEntry {
            signature_hash: "t".into(),
            order_signature_hash: "o".into(),
            buyer: "0xb".into(),
            tx_hash: "0xtx".into(),
            taken_at: 8,
            signed_at: 5,
            seq: 2,
        };
        assert_eq!(
            to_json(&TradeChange {
                kind: "trade",
                entry: trade
            }),
            json!({ "kind": "trade", "signature_hash": "t", "order_signature_hash": "o", "buyer": "0xb", "tx_hash": "0xtx", "taken_at": 8, "signed_at": 5, "seq": 2 })
        );
        assert_eq!(
            to_json(&CancelChange {
                kind: "cancel",
                signature_hash: "c".into(),
                target_signature_hash: "h".into(),
                target_kind: "bid".into(),
                signer: "0xs".into(),
                signed_at: 5,
                seq: 3
            }),
            json!({ "kind": "cancel", "signature_hash": "c", "target_signature_hash": "h", "target_kind": "bid", "signer": "0xs", "signed_at": 5, "seq": 3 })
        );
        assert_eq!(
            to_json(&AcceptChange {
                kind: "accept",
                signature_hash: "a".into(),
                bid_signature_hash: "h".into(),
                signer: "0xs".into(),
                signed_at: 5,
                seq: 4
            })["bid_signature_hash"],
            "h"
        );
        assert_eq!(
            to_json(&MarketChanges {
                bids: vec![],
                orders: vec![],
                trades: vec![],
                cancellations: vec![],
                acceptances: vec![]
            }),
            json!({ "bids": [], "orders": [], "trades": [], "cancellations": [], "acceptances": [] })
        );
    }

    #[test]
    fn items_split_into_contract_and_item() {
        assert_eq!(
            split_item("0x2222222222222222222222222222222222222222-0"),
            Some((
                "0x2222222222222222222222222222222222222222".into(),
                "0".into()
            ))
        );
        assert_eq!(
            split_item("0xABCDEFabcdefABCDEFabcdefABCDEFabcdefABCD-hat_1"),
            Some((
                "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".into(),
                "hat_1".into()
            ))
        );
        assert_eq!(split_item("nope"), None);
        assert_eq!(split_item("0x22-1"), None);
    }

    async fn post(
        app: &axum::Router,
        wallet: &PrivateKeySigner,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let headers = signed_headers_by(
            wallet,
            "post",
            path,
            "{}",
            true,
            &chrono::Utc::now().timestamp_millis().to_string(),
        )
        .await;
        let mut req = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        for (k, v) in headers.iter() {
            req = req.header(k, v);
        }
        call(app, req.body(Body::from(body.to_string())).unwrap()).await
    }

    async fn call(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn get(app: &axum::Router, path: &str) -> (StatusCode, Value) {
        call(
            app,
            Request::builder().uri(path).body(Body::empty()).unwrap(),
        )
        .await
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    /// The old contract, route by route: the preflight's stages and codes,
    /// the authority rules, the log and its feeds.
    #[tokio::test]
    async fn signed_actions_are_checked_logged_and_fed() {
        let settings = crate::market::Settings {
            data_writes_enabled: true,
            ..crate::market::Settings::for_tests()
        }
        .with_market(crate::market::MarketSettings {
            admin_token: Some("s3cr3t".to_string()),
            ..crate::market::MarketSettings::default()
        });
        let Some(state) = crate::market::tests::support::state_with_db(settings).await else {
            return;
        };
        let app = crate::market::market_router(state.clone());
        let bidder = PrivateKeySigner::random();
        let owner = PrivateKeySigner::random();
        let owner_addr = format!("{:#x}", owner.address());
        let market = crate::fed::domains::market();
        let hex = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let item = format!("0x{}-3", &hex[..40]);
        let (_, before) = get(&app, "/federation/market/snapshot").await;
        let since = [
            &before["latest_bids_seq"],
            &before["latest_orders_seq"],
            &before["latest_trades_seq"],
            &before["latest_cancellations_seq"],
            &before["latest_acceptances_seq"],
        ]
        .iter()
        .map(|v| v.as_i64().unwrap())
        .max()
        .unwrap();

        let bid = BidPlace {
            item_id: item.clone(),
            price: "1000000000000000000".into(),
            expires_at: now() + 86_400,
            fingerprint: String::new(),
            signed_at: now(),
        };
        let signed = sign(&bidder, market.clone(), bid.clone(), now());
        let (status, body) = post(&app, &bidder, "/v1/federation/bid", to_json(&signed)).await;
        assert_eq!(
            (status, body["signature_hash"].clone()),
            (StatusCode::OK, json!(signed.signature_hash())),
            "{body}"
        );
        let bid_hash = signed.signature_hash();
        let (status, body) = post(&app, &bidder, "/v1/federation/bid", to_json(&signed)).await;
        assert_eq!(status, StatusCode::CONFLICT, "a replayed nonce: {body}");

        let (status, body) = call(
            &app,
            Request::builder()
                .method("POST")
                .uri("/v1/federation/bid")
                .header("content-type", "application/json")
                .body(Body::from(to_json(&signed).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(
            (status, body["ok"].clone()),
            (StatusCode::UNAUTHORIZED, json!(false)),
            "no auth chain: {body}"
        );
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .starts_with("auth chain: ")
        );
        let (status, _) = post(&app, &bidder, "/v1/federation/bid", json!("not json")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, body) = post(
            &app,
            &owner,
            "/v1/federation/bid",
            to_json(&sign(&bidder, market.clone(), bid.clone(), now())),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "the envelope's signer is not the chain's: {body}"
        );
        let (status, body) = post(
            &app,
            &bidder,
            "/v1/federation/bid",
            to_json(&sign(
                &bidder,
                crate::fed::domains::places(),
                bid.clone(),
                now(),
            )),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::BAD_REQUEST,
                json!("domain mismatch: expected DecentralandMarket")
            )
        );
        let (status, _) = post(
            &app,
            &bidder,
            "/v1/federation/bid",
            to_json(&sign(
                &bidder,
                market.clone(),
                BidPlace {
                    price: "1.5".into(),
                    ..bid.clone()
                },
                now(),
            )),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "a non-integer price");

        let (status, body) = post(
            &app,
            &owner,
            "/v1/federation/bid/cancel",
            to_json(&sign(
                &owner,
                market.clone(),
                BidCancel {
                    bid_signature_hash: bid_hash.clone(),
                    signed_at: now(),
                },
                now(),
            )),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::FORBIDDEN,
                json!("only the bid signer may cancel this bid")
            )
        );
        let (status, _) = post(
            &app,
            &bidder,
            "/v1/federation/bid/cancel",
            to_json(&sign(
                &bidder,
                market.clone(),
                BidCancel {
                    bid_signature_hash: "ff".repeat(32),
                    signed_at: now(),
                },
                now(),
            )),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let cancel = sign(
            &bidder,
            market.clone(),
            BidCancel {
                bid_signature_hash: bid_hash.clone(),
                signed_at: now(),
            },
            now(),
        );
        let (status, _) = post(&app, &bidder, "/v1/federation/bid/cancel", to_json(&cancel)).await;
        assert_eq!(status, StatusCode::OK);

        // The ownership checks read the nft index: while it is not published
        // for every network (the scratch database), a wallet it cannot find
        // gets the index's envelope, never a 403 it cannot prove.
        let (status, body) = post(
            &app,
            &owner,
            "/v1/federation/bid/accept",
            to_json(&sign(
                &owner,
                market.clone(),
                BidAccept {
                    bid_signature_hash: bid_hash.clone(),
                    signed_at: now(),
                },
                now(),
            )),
        )
        .await;
        assert_eq!(
            (status, body["source"].clone()),
            (StatusCode::SERVICE_UNAVAILABLE, json!("nft-index")),
            "{body}"
        );
        let (contract, item_no) = split_item(&item).unwrap();
        sqlx::query("INSERT INTO shop.usage_grants (grantee_address, urn, token_id, collection, category, unlock_at) VALUES ($1, $2, '9', $3, 'wearable', timezone('UTC', now()) + interval '2 days')")
            .bind(&owner_addr)
            .bind(format!("urn:decentraland:matic:collections-v2:{contract}:{item_no}"))
            .bind(&contract)
            .execute(&state.pool)
            .await
            .unwrap();
        let order = OrderCreate {
            item_id: item.clone(),
            price: "3".into(),
            expires_at: now() + 86_400,
            signed_at: now(),
        };
        let (status, body) = post(
            &app,
            &owner,
            "/v1/federation/order",
            to_json(&sign(&owner, market.clone(), order.clone(), now())),
        )
        .await;
        assert_eq!(
            (status, body["message"].clone()),
            (
                StatusCode::FORBIDDEN,
                json!("item is in the return window (leased); cannot be listed until it unlocks")
            )
        );

        // An order already in the log (the ownership leg is the index's):
        // only its signer cancels it, anyone records its trade.
        let logged = sign(&owner, market.clone(), order, now());
        append(
            &state.pool,
            &logged,
            &owner_addr,
            Row0 {
                kind: "order",
                item_id: Some(&item),
                price: Some("3"),
                expires_at: Some(now() + 86_400),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let order_hash = logged.signature_hash();
        let (status, _) = post(
            &app,
            &bidder,
            "/v1/federation/order/cancel",
            to_json(&sign(
                &bidder,
                market.clone(),
                OrderCancel {
                    order_signature_hash: order_hash.clone(),
                    signed_at: now(),
                },
                now(),
            )),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let trade = TradeRecord {
            order_signature_hash: order_hash.clone(),
            buyer: "0xBBBB000000000000000000000000000000000000".into(),
            taken_at: now(),
            tx_hash: "0xtx".into(),
            signed_at: now(),
        };
        let (status, body) = post(
            &app,
            &bidder,
            "/v1/federation/trade",
            to_json(&sign(&bidder, market.clone(), trade.clone(), now())),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _) = post(
            &app,
            &bidder,
            "/v1/federation/trade",
            to_json(&sign(
                &bidder,
                market.clone(),
                TradeRecord {
                    order_signature_hash: "00".repeat(32),
                    ..trade
                },
                now(),
            )),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = post(
            &app,
            &owner,
            "/v1/federation/order/cancel",
            to_json(&sign(
                &owner,
                market.clone(),
                OrderCancel {
                    order_signature_hash: order_hash.clone(),
                    signed_at: now(),
                },
                now(),
            )),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (_, bids) = get(&app, "/v1/federation/bids").await;
        assert!(
            bids["data"]
                .as_array()
                .unwrap()
                .iter()
                .any(|b| b["signature_hash"] == json!(bid_hash)
                    && b["price"] == "1000000000000000000"
                    && b["item_id"] == json!(item))
        );
        let (_, trades) = get(&app, "/v1/federation/trades").await;
        assert!(
            trades["data"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["order_signature_hash"] == json!(order_hash)
                    && t["buyer"] == "0xbbbb000000000000000000000000000000000000")
        );
        let (_, changes) = get(&app, &format!("/federation/market/changes?since={since}")).await;
        let mine = |key: &str, field: &str, want: &str| {
            changes[key]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c[field] == want)
                .count()
        };
        assert_eq!(
            (
                mine("bids", "signature_hash", &bid_hash),
                mine("orders", "signature_hash", &order_hash),
                mine("trades", "order_signature_hash", &order_hash)
            ),
            (1, 1, 1)
        );
        assert_eq!(
            (
                mine("cancellations", "target_signature_hash", &bid_hash),
                mine("cancellations", "target_signature_hash", &order_hash)
            ),
            (1, 1)
        );
        assert_eq!(
            changes["cancellations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["target_signature_hash"] == json!(order_hash))
                .unwrap()["target_kind"],
            "order"
        );
        let seqs: Vec<i64> = ["bids", "orders", "trades", "cancellations"]
            .iter()
            .flat_map(|k| {
                changes[*k]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c["seq"].as_i64().unwrap())
            })
            .collect();
        let distinct: std::collections::HashSet<i64> = seqs.iter().copied().collect();
        assert_eq!(distinct.len(), seqs.len(), "one sequence across the kinds");
        let (_, snap) = get(&app, "/federation/market/snapshot").await;
        assert_eq!(
            (
                snap["domain"].clone(),
                snap["log_hash"].as_str().map(str::len)
            ),
            (json!("DecentralandMarket"), Some(64))
        );
        assert!(
            snap["latest_cancellations_seq"].as_i64().unwrap()
                > snap["latest_bids_seq"].as_i64().unwrap()
        );

        // The operator's reach into the log (the old admin routes' targets):
        // a flag on a logged bid, and a force-cancel appending the old
        // operator cancellation once.
        let spare = sign(
            &bidder,
            market.clone(),
            BidPlace {
                price: "7".into(),
                ..bid.clone()
            },
            now(),
        );
        append(
            &state.pool,
            &spare,
            &format!("{:#x}", bidder.address()),
            Row0 {
                kind: "bid",
                item_id: Some(&item),
                price: Some("7"),
                expires_at: Some(now() + 60),
                fingerprint: Some(""),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let spare_hash = spare.signature_hash();
        let admin = |method: &str, path: String| {
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", "Bearer s3cr3t")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap()
        };
        let (status, body) = call(
            &app,
            admin(
                "POST",
                format!("/v1/admin/moderation/bid/{spare_hash}/flag"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, first) = call(
            &app,
            admin(
                "POST",
                format!("/v1/admin/listings/bid/{spare_hash}/force-cancel"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{first}");
        assert!(
            first["cancellation_hash"]
                .as_str()
                .unwrap()
                .starts_with("operator:")
        );
        let (_, again) = call(
            &app,
            admin(
                "POST",
                format!("/v1/admin/listings/bid/{spare_hash}/force-cancel"),
            ),
        )
        .await;
        assert_eq!(
            (
                again["already_cancelled"].clone(),
                again["cancellation_hash"].clone()
            ),
            (json!(true), first["cancellation_hash"].clone())
        );
        let (_, feed) = get(&app, &format!("/federation/market/changes?since={since}")).await;
        let op = feed["cancellations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["target_signature_hash"] == json!(spare_hash))
            .unwrap()
            .clone();
        assert_eq!(
            (op["signer"].clone(), op["target_kind"].clone()),
            (json!("operator:admin-token"), json!("bid"))
        );

        let read_only = crate::market::market_router(
            crate::market::tests::support::state_with_db(crate::market::Settings::for_tests())
                .await
                .unwrap(),
        );
        let (status, _) = post(
            &read_only,
            &bidder,
            "/v1/federation/bid",
            to_json(&sign(&bidder, market, bid, now())),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "the write gate");
        assert_eq!(
            get(&read_only, "/v1/federation/orders").await.0,
            StatusCode::OK
        );
        sqlx::query("DELETE FROM shop.usage_grants WHERE grantee_address = $1")
            .bind(&owner_addr)
            .execute(&state.pool)
            .await
            .unwrap();
    }

    #[test]
    fn message_encodings_are_distinct_per_type() {
        let cancel = BidCancel {
            bid_signature_hash: "h".into(),
            signed_at: 1,
        };
        let accept = BidAccept {
            bid_signature_hash: "h".into(),
            signed_at: 1,
        };
        assert_ne!(cancel.encode_struct(), accept.encode_struct());
        let b: BidPlace = serde_json::from_value(
            json!({ "item_id": "i", "price": "1", "expires_at": 2, "signed_at": 3 }),
        )
        .unwrap();
        assert!(b.fingerprint.is_empty());
    }
}
