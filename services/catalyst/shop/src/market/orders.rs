//! `/v1/orders` and `/v1/orders/open-by-items`: the off-chain v3 leg from the
//! trades mirror (an open `public_nft_order` is one that is effective, not
//! expired and not in `trade_cancellations`) merged with the legacy
//! on-chain orders of the Marketplace contracts.
//!
//! A node that walks the nft index reads both legs on one read-only
//! snapshot ([`book_snapshot`]) on which the index is published and recent
//! on every network the request reads, and the land-authz index ready when
//! an open LAND or estate order could be answered; otherwise the route
//! answers the unavailable envelope naming the index that is not. The
//! legacy orders come from `shop.legacy_order` (`legacy_orders`): an open
//! order reads `open` only while the token's owner source (`shop.nft`, or
//! `land_authz.token_right` for LAND and estates) proves the seller holds
//! it, and `transferred` otherwise (another owner, none, or no row), as the
//! subgraphs reported a moved token; only the orders of the collections,
//! names, LAND and estates are served. The v3 listings pass
//! the "signer still owns the nft" guard on the same index, so a listing
//! whose signer no longer holds that exact token drops out, while a token
//! the index does not hold (LAND, an estate) passes.
//!
//! A node that does not walk the index (switch off, or chain settings off
//! its catalogue, `Settings::walks_catalogue`) serves
//! the legacy `orders` of the collections subgraph reader (first ≤ 100,
//! 30 s; past the graph's 5,000 skip by keyset up to `readers::DEEP_WINDOW`)
//! and the v3 listings without the guard; the `x-listings-owner-check`
//! header names which.
//!
//! The two legs are sorted together in memory over the `first + skip`
//! window (a deeper window than `readers::DEEP_WINDOW` is a 400). The
//! legacy leg's total is a count capped at 5,000, or at the deep window for
//! a deeper page. `nftName` is parsed and not applied, as the old query
//! builder did (it selected the name and never filtered on it).

use std::collections::HashSet;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, Row, Transaction};

use super::catalog::{ITEM_ID_SHIFT, unified_trades_cte};
use super::chain_walk::ChainNet;
use super::graph;
use super::owned_items::{self, Rule};
use super::readers::{self, GraphPage, Ttl, seconds_to_ms};
use super::wire::{
    ApiError, ChainId, DataTotalString, InvalidParameterError, Network, Params, clamp_first,
    clamp_skip, network_and_chain,
};
use super::{MarketplaceState, SharedState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSortBy {
    Oldest,
    RecentlyListed,
    RecentlyUpdated,
    Cheapest,
    IssuedIdAsc,
    IssuedIdDesc,
}

#[derive(Debug, Clone, Default)]
pub struct OrderFilters {
    pub first: Option<i64>,
    pub skip: Option<i64>,
    pub sort_by: Option<OrderSortBy>,
    pub marketplace_address: Option<String>,
    pub owner: Option<String>,
    pub buyer: Option<String>,
    pub contract_address: Option<String>,
    pub token_id: Option<String>,
    pub status: Option<String>,
    pub network: Option<Network>,
    pub item_id: Option<String>,
    pub nft_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct Order {
    pub id: String,
    #[serde(rename = "marketplaceAddress")]
    pub marketplace_address: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: Option<String>,
    pub owner: String,
    pub buyer: Option<String>,
    pub price: String,
    pub status: String,
    #[serde(rename = "expiresAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_at: i64,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated_at: i64,
    pub network: Network,
    #[serde(rename = "chainId")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub chain_id: ChainId,
    #[serde(rename = "issuedId")]
    pub issued_id: Option<String>,
    #[serde(rename = "tradeId")]
    pub trade_id: Option<String>,
    /// Who a v3 listing pays (its received asset's beneficiary); not on the wire.
    #[serde(skip)]
    pub payee: Option<String>,
}

/// The sort keys both legs carry so one comparison orders the merged list.
#[derive(Debug, Clone)]
pub struct Keyed {
    pub order: Order,
    pub sort_price: u128,
    pub sort_token_id: String,
}

pub fn from_seconds_to_milliseconds(time: f64) -> i64 {
    if time < 1e11_f64 {
        (time * 1000.0).round() as i64
    } else {
        time.round() as i64
    }
}

pub fn parse_filters(pairs: &[(String, String)]) -> Result<OrderFilters, InvalidParameterError> {
    let p = Params::new(pairs);
    let sort_by = p
        .get_value(
            "sortBy",
            &[
                "oldest",
                "recently_listed",
                "recently_updated",
                "cheapest",
                "issued_id_asc",
                "issued_id_desc",
            ],
            None,
        )
        .map(|s| match s.as_str() {
            "oldest" => OrderSortBy::Oldest,
            "recently_updated" => OrderSortBy::RecentlyUpdated,
            "cheapest" => OrderSortBy::Cheapest,
            "issued_id_asc" => OrderSortBy::IssuedIdAsc,
            "issued_id_desc" => OrderSortBy::IssuedIdDesc,
            _ => OrderSortBy::RecentlyListed,
        });
    let network = p
        .get_value("network", &["ETHEREUM", "MATIC"], None)
        .map(|s| {
            if s == "ETHEREUM" {
                Network::Ethereum
            } else {
                Network::Matic
            }
        });
    Ok(OrderFilters {
        first: p.get_number("first", None).map(|v| v as i64),
        skip: p.get_number("skip", None).map(|v| v as i64),
        sort_by,
        marketplace_address: p.get_string("marketplaceAddress", None),
        owner: p.get_string("owner", None),
        buyer: p.get_string("buyer", None),
        contract_address: p.get_string("contractAddress", None),
        token_id: p.get_string("tokenId", None),
        status: p.get_string("status", None),
        network,
        item_id: p.get_string("itemId", None),
        nft_name: p.get_string("nftName", None),
    })
}

pub const OPEN_BY_ITEMS_MAX_ITEMS: usize = 200;
pub const OPEN_BY_ITEMS_DEFAULT_PER_CONTRACT: i64 = 500;
pub const OPEN_BY_ITEMS_MAX_PER_CONTRACT: i64 = 5000;

pub(crate) fn parse_open_by_items(
    pairs: &[(String, String)],
) -> Result<(Vec<(String, String)>, i64), InvalidParameterError> {
    let p = Params::new(pairs);
    let raw = p.get_list("item", &[]);
    if raw.is_empty() {
        return Err(InvalidParameterError::new("item", ""));
    }
    if raw.len() > OPEN_BY_ITEMS_MAX_ITEMS {
        return Err(InvalidParameterError::new(
            "item",
            format!("{} items (max {OPEN_BY_ITEMS_MAX_ITEMS})", raw.len()),
        ));
    }
    let mut items = Vec::with_capacity(raw.len());
    for entry in &raw {
        let Some(item) = super::wire::split_item_wire_id(entry) else {
            return Err(InvalidParameterError::new("item", entry.clone()));
        };
        items.push(item);
    }
    let per_contract = p
        .get_number("perContract", None)
        .map(|v| v as i64)
        .unwrap_or(OPEN_BY_ITEMS_DEFAULT_PER_CONTRACT)
        .clamp(1, OPEN_BY_ITEMS_MAX_PER_CONTRACT);
    Ok((items, per_contract))
}

/// Open `public_nft_order` trades with the filters that apply to the leg;
/// `$1` binds `now`. [`OWNER_GUARD`] follows it wherever the nft index is
/// read.
const TRADES_LEG_SQL: &str = "SELECT u.id::text AS id, u.trade_contract AS marketplace_address, u.contract_address_sent AS nft_address, u.token_id, \
        u.amount_received::text AS price, u.signer AS owner, u.beneficiary AS payee, \
        (EXTRACT(EPOCH FROM u.created_at) * 1000)::bigint AS created_at, (EXTRACT(EPOCH FROM u.expires_at) * 1000)::bigint AS expires_at, \
        upper(u.network) AS network, u.chain_id::bigint AS chain_id, mod(u.token_id::numeric, SHIFT::numeric)::text AS issued_id, u.item_id::text AS item_id \
 FROM unified_trades u \
 WHERE u.type = 'public_nft_order' AND u.token_id IS NOT NULL";

/// The signer still holds the token: drops a listing only when the `nft`
/// index holds the token and another address (or nobody: burned) owns it.
const OWNER_GUARD: &str = " AND NOT EXISTS (SELECT 1 FROM shop.nft n WHERE n.network = lower(u.network) AND n.contract_address = u.contract_address_sent \
                   AND n.token_id = u.token_id::numeric AND n.owner IS DISTINCT FROM u.signer)";

/// The response header naming how the v3 listings were checked.
pub const OWNER_CHECK_HEADER: &str = "x-listings-owner-check";

/// How the v3 listings leg checked that each signer still holds its token.
/// Ordered by how much it leaves unchecked, so the worst of several reads is
/// their `max`. A node that walks the index never serves a listing it could
/// not check: while the index is not published and recent the route
/// answers the unavailable envelope instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnerCheck {
    /// Every listing served passed [`OWNER_GUARD`] on a published and recent
    /// index, or the request reads no listing.
    Checked,
    /// The node does not walk the index: every open listing is served
    /// without the guard, one whose signer no longer holds the token
    /// included (buying it fails on chain).
    Unchecked,
}

impl OwnerCheck {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Checked => "checked",
            Self::Unchecked => "unchecked",
        }
    }

    /// `response` with [`OWNER_CHECK_HEADER`] naming this check.
    fn stamp(self, mut response: Response) -> Response {
        response.headers_mut().insert(
            HeaderName::from_static(OWNER_CHECK_HEADER),
            HeaderValue::from_static(self.as_str()),
        );
        response
    }

    /// The ownership predicate of the statement.
    fn predicate(self) -> &'static str {
        match self {
            Self::Checked => OWNER_GUARD,
            Self::Unchecked => "",
        }
    }
}

/// `$1` is now, `$2` the trades floor; `extra` binds from `$3`. `check`
/// picks the ownership predicate ([`OwnerCheck::predicate`]).
fn trades_leg_sql(extra: &str, check: OwnerCheck) -> String {
    format!(
        "WITH {} {}{} {extra} ORDER BY u.created_at DESC, u.id::text ASC LIMIT 1000",
        unified_trades_cte("$1", "$2"),
        TRADES_LEG_SQL.replace("SHIFT", ITEM_ID_SHIFT),
        check.predicate()
    )
}

/// An open v3 listing ([`TRADES_LEG_SQL`]'s row) as an order: its id is the
/// trade's, and `tradeId` names it so a client cancels that trade.
pub(super) fn trade_row_order(r: &sqlx::postgres::PgRow) -> Order {
    let (network, _) = network_and_chain(&r.get::<String, _>("network"));
    // The trade's own chain, which the leg keeps to the served one.
    let chain_id = wire_chain(r.get("chain_id"), network);
    let created_at: i64 = r.get("created_at");
    Order {
        id: r.get("id"),
        trade_id: Some(r.get("id")),
        marketplace_address: r
            .get::<Option<String>, _>("marketplace_address")
            .unwrap_or_default(),
        contract_address: r.get("nft_address"),
        token_id: r.get("token_id"),
        owner: r.get("owner"),
        buyer: Some(String::new()),
        price: r.get("price"),
        status: "open".to_string(),
        expires_at: r.get("expires_at"),
        created_at,
        updated_at: created_at,
        network,
        chain_id,
        issued_id: r.get("issued_id"),
        payee: r.get("payee"),
    }
}

/// The newest open v3 listing of each `(network, contract, token, owner)`,
/// signed by that owner: the `order` an nft page carries when it has no
/// legacy one. One statement for the whole page; `network` is the
/// database name (`matic`, `ethereum`), the rest lowercase.
pub async fn open_trade_orders(
    state: &MarketplaceState,
    wanted: &[(String, String, String, String)],
) -> Result<std::collections::HashMap<(String, String, String), Order>, ApiError> {
    Ok(open_trade_orders_on(state, &state.pool, wanted).await?)
}

pub(super) async fn open_trade_orders_on<'e>(
    state: &MarketplaceState,
    conn: impl sqlx::PgExecutor<'e>,
    wanted: &[(String, String, String, String)],
) -> Result<std::collections::HashMap<(String, String, String), Order>, sqlx::Error> {
    let mut out = std::collections::HashMap::new();
    if wanted.is_empty() {
        return Ok(out);
    }
    let sql = format!(
        "WITH {} SELECT DISTINCT ON (x.network, x.nft_address, x.token_id) x.* FROM ({}{OWNER_GUARD} \
           AND (lower(u.network), u.contract_address_sent, u.token_id, u.signer) IN \
               (SELECT * FROM unnest($3::text[], $4::text[], $5::text[], $6::text[]))) x \
         ORDER BY x.network, x.nft_address, x.token_id, x.created_at DESC, x.id ASC",
        unified_trades_cte("$1", "$2"),
        TRADES_LEG_SQL.replace("SHIFT", ITEM_ID_SHIFT)
    );
    let column = |i: usize| {
        wanted
            .iter()
            .map(|w| match i {
                0 => w.0.clone(),
                1 => w.1.clone(),
                2 => w.2.clone(),
                _ => w.3.clone(),
            })
            .collect::<Vec<String>>()
    };
    let now = Utc::now().naive_utc();
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now)
        .bind(super::trades::mirrored_since(&state.settings, now))
        .bind(column(0))
        .bind(column(1))
        .bind(column(2))
        .bind(column(3))
        .fetch_all(conn)
        .await?;
    for r in &rows {
        let order = trade_row_order(r);
        out.insert(
            (
                order.network.db_name().to_string(),
                order.contract_address.clone(),
                order.token_id.clone().unwrap_or_default(),
            ),
            order,
        );
    }
    Ok(out)
}

fn keyed(order: Order, sort_token_id: String) -> Keyed {
    let sort_price = order.price.parse::<u128>().unwrap_or(0);
    Keyed {
        order,
        sort_price,
        sort_token_id,
    }
}

/// The networks a read of `network` names (both without a filter).
fn nets_of(network: Option<Network>) -> &'static [ChainNet] {
    match network {
        Some(Network::Ethereum) => &[ChainNet::Ethereum],
        Some(Network::Matic) => &[ChainNet::Matic],
        None => &ChainNet::ALL,
    }
}

/// Whether `contract` is one of the node's LAND or Estate registries.
fn is_land_registry(state: &MarketplaceState, contract: &str) -> bool {
    let r = served(state);
    !r.land.is_empty()
        && (contract.eq_ignore_ascii_case(&r.land) || contract.eq_ignore_ascii_case(&r.estate))
}

/// Whether a read of `f` could answer an open (or `transferred`) LAND or
/// estate order, whose seller is checked against the land-authz index: the
/// filters leave Ethereum and the registries in, and an order that is open
/// (a sold or cancelled status, or a buyer, which only a sale sets, keeps
/// none).
fn reads_land(state: &MarketplaceState, f: &OrderFilters) -> bool {
    let open_possible = f.buyer.is_none()
        && !matches!(f.status.as_deref(), Some(s) if s != "open" && s != "transferred");
    let land_possible = match &f.contract_address {
        Some(c) => is_land_registry(state, c),
        None => !served(state).land.is_empty(),
    };
    open_possible && land_possible && f.network != Some(Network::Matic)
}

/// The read-only snapshot a walking node's order book reads on: the
/// land-authz index ready on it when `land` (its own snapshot, which the
/// LAND/estate reads share), the nft index published and recent on `nets`
/// at `now` ([`Rule::Fresh`]) on the chain the node serves on each (its
/// catalogue, `Settings::catalogue`), with no write to its owners (or, with
/// `book`, to the book) past that publication by any writer
/// (`chain_walk::rows_published_on`) and, with `book`, the legacy order book caught
/// up with that publication on each (`legacy_orders::caught_up`) and the
/// trades mirror and its verdicts ready (`trades::ready_on`), each statement
/// bounded by `nft_reads::READ_STATEMENT_TIMEOUT`, and the request's instant:
/// taken once the snapshot is established, so a fold the snapshot sees was
/// published before it (a fold published while the request waited for a
/// connection is not read as published in the future), and shared by the
/// checks, the rows and the totals. `Err` is the unavailable envelope naming
/// the source that is not ready, or a 500.
pub(crate) async fn book_snapshot(
    state: &MarketplaceState,
    nets: &[ChainNet],
    land: bool,
    book: bool,
) -> Result<(Transaction<'static, Postgres>, DateTime<Utc>), Response> {
    let db = |e: sqlx::Error| ApiError::internal(e.to_string()).into_response();
    let mut tx = if land {
        let snapshot = match state.land_network() {
            Some(net) => super::nft_reads::land_snapshot(&state.pool, net).await,
            None => None,
        };
        snapshot.ok_or_else(super::nft_reads::land_unavailable)?.tx
    } else {
        let mut tx = state.pool.begin().await.map_err(db)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx
    };
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(format!(
            "{}ms",
            super::nft_reads::READ_STATEMENT_TIMEOUT.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    // The statement above established the snapshot (the land path's already
    // had); every publication it sees was committed before this instant.
    let now = Utc::now();
    owned_items::check(
        &mut tx,
        state.settings.catalogue(),
        nets,
        Rule::Fresh,
        false,
        now,
    )
    .await
    .map_err(|e| e.into_response(|m| ApiError::internal(m.to_string()).into_response()))?;
    // The owners the book's statuses and the listings' guard read (and the
    // book itself) as the published fold left them: a fold committed past it
    // but not published yet, whichever binary wrote it, is not read.
    let projections: &[&str] = if book {
        &["nft", "legacy_order"]
    } else {
        &["nft"]
    };
    for &net in nets {
        if !super::chain_walk::rows_published_on(
            &mut tx,
            super::chain_walk::INDEXER_NFT,
            net,
            projections,
        )
        .await
        .map_err(db)?
        {
            return Err(owned_items::not_ready(&format!(
                "the {} nft index holds a fold it has not published yet",
                net.db_name()
            )));
        }
    }
    if book {
        for &net in nets {
            if !super::legacy_orders::caught_up(&mut tx, net)
                .await
                .map_err(db)?
            {
                return Err(owned_items::not_ready(&format!(
                    "the {} legacy order book has not caught up with the published nft fold",
                    net.db_name()
                )));
            }
        }
        super::trades::ready_on(state, &mut tx, now).await?;
    }
    Ok((tx, now))
}

/// A statement of an order-book read failed: one Postgres cancelled at the
/// statement bound is the 503 of `nft_reads::timed_out`, anything else a 500.
fn read_error(e: sqlx::Error) -> Response {
    let cancelled = e
        .as_database_error()
        .is_some_and(|d| d.code().as_deref() == Some("57014"));
    if cancelled {
        super::nft_reads::timed_out()
    } else {
        ApiError::from(e).into_response()
    }
}

/// The v3 leg's predicates for `f` and their binds (from `$3`); `None` when
/// the leg reads nothing (a status other than open, or a buyer).
fn trades_filters(f: &OrderFilters) -> Option<(String, Vec<String>)> {
    if f.status.as_deref().is_some_and(|s| s != "open") || f.buyer.is_some() {
        return None;
    }
    let mut extra = String::new();
    let mut binds: Vec<String> = Vec::new();
    let next = |binds: &mut Vec<String>, v: String| {
        binds.push(v);
        format!("${}", binds.len() + 2)
    };
    if let Some(v) = &f.marketplace_address {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND lower(u.trade_contract) = {p}"));
    }
    if let Some(v) = &f.owner {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND u.signer = {p}"));
    }
    if let Some(v) = &f.contract_address {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND u.contract_address_sent = {p}"));
    }
    if let Some(v) = &f.token_id {
        let p = next(&mut binds, v.clone());
        extra.push_str(&format!(" AND u.token_id = {p}"));
    }
    if let Some(v) = &f.item_id {
        let bare = v
            .rsplit_once('-')
            .map(|(_, id)| id.to_string())
            .unwrap_or_else(|| v.clone());
        let p = next(&mut binds, bare);
        extra.push_str(&format!(" AND u.item_id::text = {p}"));
    }
    if let Some(n) = f.network {
        let p = next(&mut binds, n.db_name().to_string());
        extra.push_str(&format!(" AND lower(u.network) = {p}"));
    }
    Some((extra, binds))
}

/// The open v3 listings matching `f` on `conn`, behind `check`'s ownership
/// predicate (which reads the nft index of `conn`'s snapshot).
pub(super) async fn read_trades(
    conn: &mut PgConnection,
    state: &MarketplaceState,
    f: &OrderFilters,
    check: OwnerCheck,
    now: DateTime<Utc>,
) -> Result<Vec<Keyed>, sqlx::Error> {
    let Some((extra, binds)) = trades_filters(f) else {
        return Ok(Vec::new());
    };
    let now = now.naive_utc();
    let sql = trades_leg_sql(&extra, check);
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now)
        .bind(super::trades::mirrored_since(&state.settings, now));
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(conn).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let order = trade_row_order(r);
            let token_id = order.token_id.clone().unwrap_or_default();
            keyed(order, token_id)
        })
        .collect())
}

/// The open v3 listings matching `f`, and how they were checked. A node
/// that walks the nft index reads them behind the guard on a
/// [`book_snapshot`] of the networks `f` names (the unavailable envelope
/// while the index is not published and recent there); a node that does
/// not has no ownership source and serves them unguarded.
pub async fn trades_leg(
    state: &MarketplaceState,
    f: &OrderFilters,
) -> Result<(Vec<Keyed>, OwnerCheck), Response> {
    if owned_items::walks_here(state) {
        let (mut tx, now) = book_snapshot(state, nets_of(f.network), false, false).await?;
        let rows = read_trades(&mut tx, state, f, OwnerCheck::Checked, now)
            .await
            .map_err(read_error)?;
        tx.rollback().await.ok();
        Ok((rows, OwnerCheck::Checked))
    } else {
        let mut conn = state
            .pool
            .acquire()
            .await
            .map_err(|e| ApiError::from(e).into_response())?;
        let rows = read_trades(&mut conn, state, f, OwnerCheck::Unchecked, Utc::now())
            .await
            .map_err(read_error)?;
        Ok((rows, OwnerCheck::Unchecked))
    }
}

/// The v3 listings of each of `fs` on a node that does not walk the nft
/// index (unguarded), and the instant they were read at: the trades
/// mirror's readiness and the rows on one short read-only snapshot, taken
/// once it is established and released before the subgraph is awaited, so
/// a trade-events rewind between the check and the rows cannot empty a
/// leg the check passed. The subgraph's filter takes the same instant.
async fn unguarded_trades(
    state: &MarketplaceState,
    fs: &[&OrderFilters],
) -> Result<(Vec<Vec<Keyed>>, DateTime<Utc>), Response> {
    let (mut tx, now) = trades_snapshot(state).await?;
    let mut out = Vec::with_capacity(fs.len());
    for f in fs {
        out.push(
            read_trades(&mut tx, state, f, OwnerCheck::Unchecked, now)
                .await
                .map_err(read_error)?,
        );
    }
    tx.rollback().await.ok();
    Ok((out, now))
}

/// [`unguarded_trades`]' snapshot: read-only, the trades mirror and its
/// verdicts ready on it, and its instant.
pub(super) async fn trades_snapshot(
    state: &MarketplaceState,
) -> Result<(Transaction<'static, Postgres>, DateTime<Utc>), Response> {
    let db = |e: sqlx::Error| ApiError::internal(e.to_string()).into_response();
    let mut tx = state.pool.begin().await.map_err(db)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(format!(
            "{}ms",
            super::nft_reads::READ_STATEMENT_TIMEOUT.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    // The statement above established the snapshot.
    let now = Utc::now();
    super::trades::ready_on(state, &mut tx, now).await?;
    Ok((tx, now))
}

/// What the book serves, on the chains the node serves (its catalogue,
/// `Settings::catalogue`, the chains its readiness checks): the
/// collections the nft index walks there ([`Served::catalogue_sql`]) and,
/// on Ethereum, that chain's LAND, Estate and names registrar, lowercase;
/// empty where the chain has none, so no order of theirs is served.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Served {
    ethereum: i64,
    matic: i64,
    land: String,
    estate: String,
    names: String,
}

impl Served {
    /// `c` (a `shop.collections` row) is in the catalogue the nft index
    /// walks on its network's served chain: a row of that chain and, on a
    /// testnet, verified there (`NftIndex::load`'s address set). The
    /// marketplace log also holds orders for assets outside it; those are
    /// not served.
    fn catalogue_sql(&self) -> String {
        let on = |net: ChainNet, chain: i64| {
            format!(
                "(c.network = '{}' AND c.chain_id = {chain}{})",
                net.db_name(),
                if chain == net.chain_id() {
                    ""
                } else {
                    " AND c.testnet_verified"
                }
            )
        };
        format!(
            "({} OR {})",
            on(ChainNet::Ethereum, self.ethereum),
            on(ChainNet::Matic, self.matic)
        )
    }

    /// `i` (a `shop.items` row an Ethereum item filter joins) is an item of
    /// the served Ethereum chain and, on a testnet, verified there, as
    /// `nft_reads::collection_source` requires of the item it shows.
    fn item_proof_sql(&self) -> String {
        format!(
            "i.chain_id = {}{}",
            self.ethereum,
            if self.ethereum == ChainNet::Ethereum.chain_id() {
                ""
            } else {
                " AND i.testnet_verified"
            }
        )
    }

    /// The wire `chainId` of a book row on `network`: the served chain.
    fn chain_id(&self, network: Network) -> ChainId {
        wire_chain(
            match network {
                Network::Ethereum => self.ethereum,
                Network::Matic => self.matic,
            },
            network,
        )
    }
}

/// `chain` as the wire's `ChainId`. The index settings admit only the
/// supported chains, so the network's default never stands in for a served one.
fn wire_chain(chain: i64, network: Network) -> ChainId {
    u64::try_from(chain)
        .ok()
        .and_then(|c| ChainId::try_from(c).ok())
        .unwrap_or_else(|| network.chain_id())
}

fn served(state: &MarketplaceState) -> Served {
    let catalogue = state.settings.catalogue();
    let ethereum = catalogue.chain_id(ChainNet::Ethereum);
    let deployments =
        super::nft_index::deployments::Deployments::for_chain(ChainNet::Ethereum, ethereum);
    let (land, estate) = deployments
        .as_ref()
        .and_then(|d| d.land.as_ref())
        .map(|n| {
            (
                n.land_registry.to_lowercase(),
                n.estate_registry.to_lowercase(),
            )
        })
        .unwrap_or_default();
    Served {
        ethereum,
        matic: catalogue.chain_id(ChainNet::Matic),
        land,
        estate,
        names: deployments
            .and_then(|d| d.registrar())
            .unwrap_or_default()
            .to_lowercase(),
    }
}

/// Whether `s` is a decimal `uint256` as the index stores token and item ids.
fn is_uint(s: &str) -> bool {
    !s.is_empty() && s.len() <= 78 && s.bytes().all(|b| b.is_ascii_digit())
}

/// The `shop.legacy_order` leg's `FROM ... WHERE` for `f` at `now_ms`, its
/// binds from `$1`, or `None` when no order can match (a token or item id
/// that is not a decimal). `land` joins the land-authz rights, whose owner
/// decides `transferred` for LAND and estate orders; without it no open
/// LAND or estate order may be read ([`reads_land`]).
fn book_from(
    served: &Served,
    f: &OrderFilters,
    now_ms: i64,
    land: bool,
) -> Option<(String, Vec<String>)> {
    let mut binds: Vec<String> = Vec::new();
    let next = |binds: &mut Vec<String>, v: String| {
        binds.push(v);
        format!("${}", binds.len())
    };
    let catalogue = served.catalogue_sql();
    let listed = format!(
        "(EXISTS (SELECT 1 FROM shop.collections c WHERE c.network = o.network AND c.contract_address = o.contract_address AND {catalogue}) \
          OR (o.network = 'ethereum' AND o.contract_address IN ({}, {}, {})))",
        next(&mut binds, served.land.clone()),
        next(&mut binds, served.estate.clone()),
        next(&mut binds, served.names.clone()),
    );
    let mut sql = format!(
        "FROM shop.legacy_order o \
         LEFT JOIN shop.nft n ON n.network = o.network AND n.contract_address = o.contract_address AND n.token_id = o.token_id{} \
         WHERE {listed}",
        if land {
            " LEFT JOIN land_authz.token_right tr ON o.network = 'ethereum' AND tr.token_address = o.contract_address AND tr.token_id = o.token_id"
        } else {
            ""
        }
    );
    let status = book_status(land);
    if let Some(v) = &f.marketplace_address {
        let p = next(&mut binds, v.to_lowercase());
        sql.push_str(&format!(" AND o.marketplace_address = {p}"));
    }
    if let Some(v) = &f.owner {
        let p = next(&mut binds, v.to_lowercase());
        sql.push_str(&format!(" AND o.seller = {p}"));
    }
    if let Some(v) = &f.buyer {
        let p = next(&mut binds, v.to_lowercase());
        sql.push_str(&format!(" AND o.buyer = {p}"));
    }
    if let Some(v) = &f.contract_address {
        let p = next(&mut binds, v.to_lowercase());
        sql.push_str(&format!(" AND o.contract_address = {p}"));
    }
    if let Some(v) = &f.token_id {
        if !is_uint(v) {
            return None;
        }
        let p = next(&mut binds, v.clone());
        sql.push_str(&format!(" AND o.token_id = {p}::numeric"));
    }
    // As the subgraph `where` read: open is open and unexpired, another
    // status that status at any expiry, no status any unexpired order.
    match f.status.as_deref() {
        Some("open") => {
            let now_p = next(&mut binds, now_ms.to_string());
            sql.push_str(&format!(
                " AND o.status = 'open' AND o.expires_at > {now_p}::bigint AND {status} = 'open'"
            ))
        }
        Some("transferred") => sql.push_str(&format!(
            " AND o.status = 'open' AND {status} = 'transferred'"
        )),
        Some(s) => {
            let p = next(&mut binds, s.to_string());
            sql.push_str(&format!(" AND o.status = {p}"));
        }
        None => {
            let now_p = next(&mut binds, now_ms.to_string());
            sql.push_str(&format!(" AND o.expires_at > {now_p}::bigint"))
        }
    }
    if let Some(n) = f.network {
        let p = next(&mut binds, n.db_name().to_string());
        sql.push_str(&format!(" AND o.network = {p}"));
    }
    if let Some(v) = &f.item_id {
        // The subgraph's item is `<contract>-<item id>`; a bare id takes the
        // contract filter.
        let (contract, item) = match v.split_once('-') {
            Some((c, i)) => (c.to_lowercase(), i.to_string()),
            None => (
                f.contract_address.as_deref().unwrap_or("").to_lowercase(),
                v.clone(),
            ),
        };
        if !is_uint(&item) {
            return None;
        }
        let contract = next(&mut binds, contract);
        let item = next(&mut binds, item);
        sql.push_str(&format!(
            " AND o.contract_address = {contract} AND ( \
                (o.network = 'matic' AND div(o.token_id, {ITEM_ID_SHIFT}::numeric) = {item}::numeric) \
                OR (o.network = 'ethereum' AND EXISTS ( \
                    SELECT 1 FROM shop.collections c \
                    JOIN shop.items i ON i.network = c.network AND i.contract_address = c.contract_address AND i.urn = c.urn || ':' || n.wearable_id \
                    WHERE c.network = 'ethereum' AND c.contract_address = o.contract_address AND {catalogue} AND {item_proof} \
                      AND i.item_id = {item}::numeric)))",
            item_proof = served.item_proof_sql()
        ));
    }
    Some((sql, binds))
}

/// The status a legacy order reads. An open order stays open only while its
/// owner source proves the seller holds the token: `shop.nft` for the
/// collections and names, `land_authz.token_right` for the LAND and Estate
/// registries (`$1`, `$2` of [`book_from`]'s binds, joined only with
/// `land`). Another owner, no owner (burned, dissolved) or no row at all
/// reads `transferred`, so an order whose ownership cannot be proven is
/// never served as open. Without `land` no open LAND or estate order is
/// read ([`reads_land`]); one would read `transferred`, having no
/// `shop.nft` row.
fn book_status(land: bool) -> String {
    let moved = if land {
        "(CASE WHEN o.network = 'ethereum' AND o.contract_address IN ($1, $2) \
               THEN tr.token_id IS NULL OR tr.owner IS DISTINCT FROM o.seller \
               ELSE n.token_id IS NULL OR n.owner IS DISTINCT FROM o.seller END)"
    } else {
        "(n.token_id IS NULL OR n.owner IS DISTINCT FROM o.seller)"
    };
    format!("(CASE WHEN o.status = 'open' AND {moved} THEN 'transferred' ELSE o.status END)")
}

/// The legacy leg's order for `sort_by`, ties by id as [`sort_merged`].
fn book_order_by(sort_by: Option<OrderSortBy>) -> &'static str {
    match sort_by {
        Some(OrderSortBy::Oldest) => "o.created_at ASC, o.order_id ASC",
        Some(OrderSortBy::RecentlyUpdated) => "o.updated_at DESC, o.order_id ASC",
        Some(OrderSortBy::Cheapest) => "o.price ASC, o.order_id ASC",
        Some(OrderSortBy::IssuedIdAsc) => "o.token_id ASC, o.order_id ASC",
        Some(OrderSortBy::IssuedIdDesc) => "o.token_id DESC, o.order_id ASC",
        _ => "o.created_at DESC, o.order_id ASC",
    }
}

/// The legacy orders of `f` over the merged `window` from
/// `shop.legacy_order` on a [`book_snapshot`].
async fn book_rows(
    conn: &mut PgConnection,
    state: &MarketplaceState,
    f: &OrderFilters,
    window: i64,
    land: bool,
    now_ms: i64,
) -> Result<Vec<Keyed>, sqlx::Error> {
    let served = served(state);
    let Some((from, binds)) = book_from(&served, f, now_ms, land) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT o.order_id, o.marketplace_address, o.contract_address, o.token_id::text AS token_id, o.seller, o.buyer, \
                o.price::text AS price, {} AS status, o.expires_at, o.created_at, o.updated_at, upper(o.network) AS network, \
                CASE WHEN o.network = 'matic' THEN mod(o.token_id, {ITEM_ID_SHIFT}::numeric)::text ELSE n.issued_id::text END AS issued_id \
         {from} ORDER BY {} LIMIT {}",
        book_status(land),
        book_order_by(f.sort_by),
        window.max(0)
    );
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(conn).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let (network, _) = network_and_chain(&r.get::<String, _>("network"));
            let order = Order {
                id: r.get("order_id"),
                marketplace_address: r.get("marketplace_address"),
                contract_address: r.get("contract_address"),
                token_id: Some(r.get("token_id")),
                owner: r.get("seller"),
                buyer: r.get("buyer"),
                price: r.get("price"),
                status: r.get("status"),
                expires_at: r.get("expires_at"),
                created_at: seconds_to_ms(r.get("created_at")),
                updated_at: seconds_to_ms(r.get("updated_at")),
                network,
                chain_id: served.chain_id(network),
                issued_id: r.get("issued_id"),
                trade_id: Some(String::new()),
                payee: None,
            };
            let token = order.token_id.clone().unwrap_or_default();
            keyed(order, token)
        })
        .collect())
}

/// How many legacy orders match `f` at `now_ms`: each network's count
/// capped at `cap`, summed, as the subgraph reader counts them.
async fn book_total(
    conn: &mut PgConnection,
    state: &MarketplaceState,
    f: &OrderFilters,
    cap: i64,
    land: bool,
    now_ms: i64,
) -> Result<i64, sqlx::Error> {
    let Some((from, binds)) = book_from(&served(state), f, now_ms, land) else {
        return Ok(0);
    };
    let mut total = 0;
    for net in nets_of(f.network) {
        let sql = format!(
            "SELECT count(*) FROM (SELECT 1 {from} AND o.network = '{}' LIMIT {}) x",
            net.db_name(),
            cap.max(0)
        );
        let mut q = sqlx::query_scalar(sqlx::AssertSqlSafe(sql));
        for b in &binds {
            q = q.bind(b);
        }
        let n: i64 = q.fetch_one(&mut *conn).await?;
        total += n;
    }
    Ok(total)
}

const ORDER_FIELDS: &str = "id marketplaceAddress nftAddress tokenId owner buyer price status expiresAt createdAt updatedAt nft { issuedId }";

fn legacy_where(f: &OrderFilters, now_ms: i64) -> Value {
    let mut w = json!({});
    if let Some(v) = &f.marketplace_address {
        w["marketplaceAddress"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.owner {
        w["owner"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.buyer {
        w["buyer"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.contract_address {
        w["nftAddress"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.token_id {
        w["tokenId"] = json!(v);
    }
    match f.status.as_deref() {
        Some("open") => {
            w["status"] = json!("open");
            w["expiresAt_gt"] = json!(now_ms.to_string());
        }
        Some(s) => w["status"] = json!(s),
        None => w["expiresAt_gt"] = json!(now_ms.to_string()),
    }
    if let Some(v) = &f.item_id {
        let composite = if v.contains('-') {
            v.to_lowercase()
        } else {
            format!(
                "{}-{v}",
                f.contract_address.as_deref().unwrap_or("").to_lowercase()
            )
        };
        w["item"] = json!(composite);
    }
    w
}

fn legacy_order_by(sort_by: Option<OrderSortBy>) -> (&'static str, &'static str) {
    match sort_by {
        Some(OrderSortBy::Oldest) => ("createdAt", "asc"),
        Some(OrderSortBy::RecentlyUpdated) => ("updatedAt", "desc"),
        Some(OrderSortBy::Cheapest) => ("price", "asc"),
        Some(OrderSortBy::IssuedIdAsc) => ("tokenId", "asc"),
        Some(OrderSortBy::IssuedIdDesc) => ("tokenId", "desc"),
        _ => ("createdAt", "desc"),
    }
}

/// A subgraph `orders` row as the wire order.
pub fn order_from_graph(o: &Value, network: Network) -> Order {
    let ms = |key: &str| {
        let raw = readers::int_of(o, key);
        if raw != 0 && raw < 100_000_000_000 {
            seconds_to_ms(raw)
        } else {
            raw
        }
    };
    Order {
        id: readers::str_of(o, "id"),
        marketplace_address: readers::str_of(o, "marketplaceAddress"),
        contract_address: readers::str_of(o, "nftAddress"),
        token_id: readers::opt_str_of(o, "tokenId"),
        owner: readers::str_of(o, "owner"),
        buyer: readers::opt_str_of(o, "buyer"),
        price: readers::str_of(o, "price"),
        status: readers::str_of(o, "status"),
        expires_at: ms("expiresAt"),
        created_at: ms("createdAt"),
        updated_at: ms("updatedAt"),
        network,
        chain_id: network.chain_id(),
        issued_id: readers::at(o, "nft/issuedId").and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        }),
        trade_id: Some(String::new()),
        payee: None,
    }
}

/// The legacy rows over the merged window and their capped total.
pub async fn legacy_leg(
    state: &MarketplaceState,
    f: &OrderFilters,
    window: i64,
    now_ms: i64,
) -> Result<(Vec<Keyed>, i64), Response> {
    let networks = match f.network {
        Some(n) => vec![n.graph()],
        None => vec![graph::Network::Ethereum, graph::Network::Matic],
    };
    let where_ = legacy_where(f, now_ms);
    let (order_by, direction) = legacy_order_by(f.sort_by);
    let mut rows = Vec::new();
    let mut total = 0;
    for network in networks {
        let Some(url) = network.url(&state.index) else {
            continue;
        };
        let page = GraphPage {
            entity: "orders",
            filter_type: "Order_filter",
            fields: ORDER_FIELDS,
            where_: where_.clone(),
            order_by,
            direction,
            first: 0,
            skip: 0,
        };
        let got = readers::top_rows(state, network, url, Ttl::S30, &page, window)
            .await
            .map_err(|e| readers::upstream_failed("/v1/orders", &e))?;
        total += readers::count_to(
            state,
            network,
            url,
            "orders",
            "Order_filter",
            where_.clone(),
            readers::count_cap(0, window),
        )
        .await
        .map_err(|e| readers::upstream_failed("/v1/orders", &e))?;
        rows.extend(got.iter().map(|o| {
            let order = order_from_graph(o, Network::from(network));
            let token = order.token_id.clone().unwrap_or_default();
            keyed(order, token)
        }));
    }
    Ok((rows, total))
}

fn numeric_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

pub fn sort_merged(rows: &mut [Keyed], sort_by: Option<OrderSortBy>) {
    rows.sort_by(|a, b| {
        let id = a.order.id.cmp(&b.order.id);
        match sort_by {
            Some(OrderSortBy::Oldest) => a.order.created_at.cmp(&b.order.created_at).then(id),
            Some(OrderSortBy::RecentlyUpdated) => {
                b.order.updated_at.cmp(&a.order.updated_at).then(id)
            }
            Some(OrderSortBy::Cheapest) => a.sort_price.cmp(&b.sort_price).then(id),
            Some(OrderSortBy::IssuedIdAsc) => {
                numeric_cmp(&a.sort_token_id, &b.sort_token_id).then(id)
            }
            Some(OrderSortBy::IssuedIdDesc) => {
                numeric_cmp(&b.sort_token_id, &a.sort_token_id).then(id)
            }
            _ => b.order.created_at.cmp(&a.order.created_at).then(id),
        }
    });
}

pub async fn list_orders(
    state: &MarketplaceState,
    f: &OrderFilters,
) -> Result<(Vec<Order>, i64), Response> {
    list_orders_checked(state, f)
        .await
        .map(|(page, total, _)| (page, total))
}

/// [`list_orders`] and how its v3 listings were checked.
pub async fn list_orders_checked(
    state: &MarketplaceState,
    f: &OrderFilters,
) -> Result<(Vec<Order>, i64, OwnerCheck), Response> {
    let limit = clamp_first(f.first, 1000).min(readers::READER_MAX_FIRST);
    let offset = clamp_skip(f.skip);
    readers::check_window(limit, offset)?;
    let window = offset + limit;
    let (mut merged, legacy_total, trades, check) = if owned_items::walks_here(state) {
        let land = reads_land(state, f);
        let (mut tx, now) = book_snapshot(state, nets_of(f.network), land, true).await?;
        let now_ms = now.timestamp_millis();
        let (rows, total, trades) = async {
            let rows = book_rows(&mut tx, state, f, window, land, now_ms).await?;
            let cap = readers::count_cap(0, window);
            let total = book_total(&mut tx, state, f, cap, land, now_ms).await?;
            let trades = read_trades(&mut tx, state, f, OwnerCheck::Checked, now).await?;
            Ok::<_, sqlx::Error>((rows, total, trades))
        }
        .await
        .map_err(read_error)?;
        tx.rollback().await.ok();
        (rows, total, trades, OwnerCheck::Checked)
    } else {
        readers::require_networks(state, f.network.map(|n| n.graph()))?;
        let (mut trades, now) = unguarded_trades(state, &[f]).await?;
        let (rows, total) = legacy_leg(state, f, window, now.timestamp_millis()).await?;
        (rows, total, trades.remove(0), OwnerCheck::Unchecked)
    };
    let total = legacy_total + trades.len() as i64;
    merged.extend(trades);
    sort_merged(&mut merged, f.sort_by);
    let page = merged
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .map(|k| k.order)
        .collect();
    Ok((page, total, check))
}

pub async fn get_orders(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let filters = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return ApiError::from(e).into_response(),
    };
    match list_orders_checked(&state, &filters).await {
        Ok((data, total, check)) => check.stamp(
            Json(DataTotalString {
                data,
                total: total.to_string(),
            })
            .into_response(),
        ),
        Err(r) => r,
    }
}

/// The item id a collection token id encodes.
pub fn item_of_token(token_id: &str) -> Option<String> {
    let token = alloy_primitives::U256::from_str_radix(token_id, 10).ok()?;
    Some((token >> 216u32).to_string())
}

/// Open orders of `(contract, item)` pairs within each contract's
/// cheapest-first window: the mirror leg ranks in SQL, the reader leg reads
/// one cheapest page (≤ 1000) per contract and keeps the wanted items.
pub async fn open_orders_by_items(
    state: &MarketplaceState,
    items: &[(String, String)],
    per_contract: i64,
) -> Result<Vec<Order>, Response> {
    open_orders_by_items_checked(state, items, per_contract)
        .await
        .map(|(orders, _)| orders)
}

/// [`open_orders_by_items`] and how its v3 legs were checked. A node that
/// walks the nft index reads every contract's legs on one
/// [`book_snapshot`] of both networks (and the land-authz index when a
/// contract is a LAND or Estate registry); a node that does not reads the
/// subgraph reader's cheapest page (≤ 1000) per contract and the listings
/// unguarded.
pub async fn open_orders_by_items_checked(
    state: &MarketplaceState,
    items: &[(String, String)],
    per_contract: i64,
) -> Result<(Vec<Order>, OwnerCheck), Response> {
    let contracts: Vec<String> = items
        .iter()
        .map(|(c, _)| c.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let wanted: HashSet<(String, String)> = items.iter().cloned().collect();
    let filters = |contract: &String| OrderFilters {
        contract_address: Some(contract.clone()),
        status: Some("open".to_string()),
        sort_by: Some(OrderSortBy::Cheapest),
        ..Default::default()
    };
    let mut per: Vec<(String, Vec<Keyed>)> = Vec::with_capacity(contracts.len());
    let check = if owned_items::walks_here(state) {
        let land = contracts.iter().any(|c| is_land_registry(state, c));
        let (mut tx, now) = book_snapshot(state, &ChainNet::ALL, land, true).await?;
        for contract in &contracts {
            let f = filters(contract);
            let rows = async {
                let mut rows = book_rows(
                    &mut tx,
                    state,
                    &f,
                    per_contract,
                    land,
                    now.timestamp_millis(),
                )
                .await?;
                rows.extend(read_trades(&mut tx, state, &f, OwnerCheck::Checked, now).await?);
                Ok::<_, sqlx::Error>(rows)
            }
            .await
            .map_err(read_error)?;
            per.push((contract.clone(), rows));
        }
        tx.rollback().await.ok();
        OwnerCheck::Checked
    } else {
        readers::require_both_collections(state)?;
        let fs: Vec<OrderFilters> = contracts.iter().map(filters).collect();
        let (trades, now) = unguarded_trades(state, &fs.iter().collect::<Vec<_>>()).await?;
        for ((contract, f), trades) in contracts.iter().zip(&fs).zip(trades) {
            let (mut rows, _) = legacy_leg(
                state,
                f,
                per_contract.min(graph::THE_GRAPH_PAGE_SIZE),
                now.timestamp_millis(),
            )
            .await?;
            rows.extend(trades);
            per.push((contract.clone(), rows));
        }
        OwnerCheck::Unchecked
    };
    let mut all: Vec<Keyed> = Vec::new();
    for (contract, mut rows) in per {
        sort_merged(&mut rows, Some(OrderSortBy::Cheapest));
        rows.truncate(per_contract as usize);
        all.extend(rows.into_iter().filter(|k| {
            k.order
                .token_id
                .as_deref()
                .and_then(item_of_token)
                .is_some_and(|item| wanted.contains(&(contract.clone(), item)))
        }));
    }
    all.sort_by(|a, b| {
        a.order
            .contract_address
            .cmp(&b.order.contract_address)
            .then(a.sort_price.cmp(&b.sort_price))
            .then(a.order.id.cmp(&b.order.id))
    });
    Ok((all.into_iter().map(|k| k.order).collect(), check))
}

pub async fn get_open_orders_by_items(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (items, per_contract) = match parse_open_by_items(&pairs) {
        Ok(v) => v,
        Err(e) => return ApiError::from(e).into_response(),
    };
    match open_orders_by_items_checked(&state, &items, per_contract).await {
        Ok((data, check)) => check.stamp(
            Json(DataTotalString {
                total: data.len().to_string(),
                data,
            })
            .into_response(),
        ),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owner_check_header_names_the_least_checked_leg() {
        let worst = [OwnerCheck::Checked, OwnerCheck::Unchecked]
            .into_iter()
            .max();
        assert_eq!(worst, Some(OwnerCheck::Unchecked));
        for (check, value) in [
            (OwnerCheck::Checked, "checked"),
            (OwnerCheck::Unchecked, "unchecked"),
        ] {
            let response = check.stamp(axum::http::StatusCode::OK.into_response());
            assert_eq!(response.headers()[OWNER_CHECK_HEADER], value);
        }
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn parses_repeated_items_and_the_window() {
        let (items, per) = parse_open_by_items(&pairs(&[
            ("item", "0x016A61FEB6377239E34425B82E5C4B367E52457F-12"),
            ("item", "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-0"),
            ("perContract", "500"),
        ]))
        .unwrap();
        assert_eq!(
            items,
            vec![
                (
                    "0x016a61feb6377239e34425b82e5c4b367e52457f".to_string(),
                    "12".to_string()
                ),
                (
                    "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    "0".to_string()
                )
            ]
        );
        assert_eq!(per, 500);
        let one = pairs(&[("item", "0x016a61feb6377239e34425b82e5c4b367e52457f-1")]);
        assert_eq!(parse_open_by_items(&one).unwrap().1, 500);
        assert_eq!(
            parse_open_by_items(&pairs(&[
                ("item", "0x016a61feb6377239e34425b82e5c4b367e52457f-1"),
                ("perContract", "999999")
            ]))
            .unwrap()
            .1,
            5000
        );
        assert_eq!(
            parse_open_by_items(&pairs(&[
                ("item", "0x016a61feb6377239e34425b82e5c4b367e52457f-1"),
                ("perContract", "0")
            ]))
            .unwrap()
            .1,
            1
        );
        for bad in [
            "",
            "0x016a61feb6377239e34425b82e5c4b367e52457f",
            "0x016a61feb6377239e34425b82e5c4b367e52457f-",
            "0x016a61feb6377239e34425b82e5c4b367e52457f-abc",
            "0x016a-12",
            "016a61feb6377239e34425b82e5c4b367e52457f-12",
        ] {
            assert!(
                parse_open_by_items(&pairs(&[("item", bad)])).is_err(),
                "{bad:?}"
            );
        }
        assert!(parse_open_by_items(&[]).is_err());
        let max_id = alloy_primitives::U256::MAX.to_string();
        let (items, _) = parse_open_by_items(&pairs(&[(
            "item",
            &format!("0x016A61FEB6377239E34425B82E5C4B367E52457F-{max_id}"),
        )]))
        .unwrap();
        assert_eq!(
            items,
            vec![(
                "0x016a61feb6377239e34425b82e5c4b367e52457f".to_string(),
                max_id.clone()
            )]
        );
        assert!(
            parse_open_by_items(&pairs(&[(
                "item",
                &format!("0x016a61feb6377239e34425b82e5c4b367e52457f-{max_id}0"),
            )]))
            .is_err()
        );
    }

    #[test]
    fn seconds_are_scaled_to_milliseconds_and_millis_pass_through() {
        assert_eq!(
            from_seconds_to_milliseconds(1_700_000_000.0),
            1_700_000_000_000
        );
        assert_eq!(from_seconds_to_milliseconds(0.0), 0);
        assert_eq!(
            from_seconds_to_milliseconds(1_700_000_000_000.0),
            1_700_000_000_000
        );
        assert_eq!(from_seconds_to_milliseconds(1e11_f64), 100_000_000_000);
    }

    #[test]
    fn the_token_id_encodes_the_item_and_the_legacy_filter_names_the_graph_fields() {
        assert_eq!(
            item_of_token("105312291668557186697918027683670432318895095400549111254310977536")
                .as_deref(),
            Some("1")
        );
        assert_eq!(item_of_token("7").as_deref(), Some("0"));
        assert_eq!(item_of_token("x"), None);
        let f = parse_filters(&pairs(&[
            ("contractAddress", "0xABC"),
            ("itemId", "12"),
            ("status", "open"),
            ("sortBy", "cheapest"),
        ]))
        .unwrap();
        let w = legacy_where(&f, 1_000);
        assert_eq!(w["nftAddress"], "0xabc");
        assert_eq!(w["item"], "0xabc-12");
        assert_eq!(w["expiresAt_gt"], "1000");
        assert_eq!(legacy_order_by(f.sort_by), ("price", "asc"));
        let sql = trades_leg_sql(" AND u.signer = $3", OwnerCheck::Checked);
        assert!(
            !trades_leg_sql("", OwnerCheck::Unchecked).contains("shop.nft")
                && sql.contains("FROM shop.nft n")
                && sql.contains("u.type = 'public_nft_order'")
                && sql.contains("trade_cancellations")
                && sql.contains("LIMIT 1000"),
            "{sql}"
        );
    }

    #[test]
    fn the_book_reads_the_served_contracts_with_the_subgraph_status_rules() {
        let registries = Served {
            ethereum: 1,
            matic: 137,
            land: "0xland".to_string(),
            estate: "0xestate".to_string(),
            names: "0xnames".to_string(),
        };
        let open = parse_filters(&pairs(&[
            ("owner", "0xABC"),
            ("status", "open"),
            ("sortBy", "cheapest"),
        ]))
        .unwrap();
        let (sql, binds) = book_from(&registries, &open, 1_000, true).unwrap();
        assert!(
            sql.contains("FROM shop.legacy_order o")
                && sql.contains("shop.collections c WHERE c.network = o.network AND c.contract_address = o.contract_address AND ((c.network = 'ethereum' AND c.chain_id = 1) OR (c.network = 'matic' AND c.chain_id = 137)))")
                && sql.contains("o.contract_address IN ($1, $2, $3)")
                && sql.contains("LEFT JOIN land_authz.token_right tr")
                && sql.contains("o.seller = $4")
                && sql.contains("o.status = 'open' AND o.expires_at > $5::bigint")
                && sql.contains("THEN tr.token_id IS NULL OR tr.owner IS DISTINCT FROM o.seller")
                && sql
                    .contains("ELSE n.token_id IS NULL OR n.owner IS DISTINCT FROM o.seller END)"),
            "{sql}"
        );
        assert_eq!(
            binds,
            vec![
                "0xland".to_string(),
                "0xestate".to_string(),
                "0xnames".to_string(),
                "0xabc".to_string(),
                "1000".to_string()
            ]
        );
        assert_eq!(book_order_by(open.sort_by), "o.price ASC, o.order_id ASC");
        let (polygon, _) = book_from(&registries, &open, 1_000, false).unwrap();
        assert!(!polygon.contains("land_authz"), "{polygon}");
        // Another status is that status at any expiry; none is any unexpired.
        let sold = parse_filters(&pairs(&[("status", "sold")])).unwrap();
        let (sql, binds) = book_from(&registries, &sold, 1_000, false).unwrap();
        assert!(
            sql.ends_with("AND o.status = $4") && !sql.contains("expires_at"),
            "{sql}"
        );
        assert_eq!(binds.last().map(String::as_str), Some("sold"));
        let any = parse_filters(&[]).unwrap();
        let (sql, _) = book_from(&registries, &any, 1_000, false).unwrap();
        assert!(sql.ends_with("AND o.expires_at > $4::bigint"), "{sql}");
        let moved = parse_filters(&pairs(&[("status", "transferred")])).unwrap();
        let (sql, _) = book_from(&registries, &moved, 1_000, false).unwrap();
        assert!(sql.contains("= 'transferred'"), "{sql}");
        // The subgraph's composite item, or a bare id beside the contract.
        let item = parse_filters(&pairs(&[("itemId", "0xC-3")])).unwrap();
        let (sql, binds) = book_from(&registries, &item, 0, false).unwrap();
        assert!(sql.contains("div(o.token_id"), "{sql}");
        assert!(
            sql.contains("c.contract_address = o.contract_address AND ((c.network = 'ethereum' AND c.chain_id = 1) OR (c.network = 'matic' AND c.chain_id = 137)) AND i.chain_id = 1 AND i.item_id = $6::numeric"),
            "{sql}"
        );
        assert!(binds.ends_with(&["0xc".to_string(), "3".to_string()]));
        let bare = parse_filters(&pairs(&[("contractAddress", "0xD"), ("itemId", "4")])).unwrap();
        let (_, binds) = book_from(&registries, &bare, 0, false).unwrap();
        assert!(binds.ends_with(&["0xd".to_string(), "4".to_string()]));
        // A token or item no index could hold matches no order.
        for bad in [("tokenId", "abc"), ("itemId", "0xc-x"), ("tokenId", "")] {
            let f = parse_filters(&pairs(&[bad])).unwrap();
            assert!(book_from(&registries, &f, 0, false).is_none(), "{bad:?}");
        }
    }

    /// The served-contracts filter names the registries of the chain the
    /// node serves on Ethereum, never mainnet's by default: on the
    /// Sepolia/Amoy pair, Sepolia's LAND, Estate and names registrar. Its
    /// collections are those the nft index walks there, verified on a
    /// testnet, and its rows carry the served chains.
    #[tokio::test]
    async fn the_served_registries_follow_the_served_ethereum_chain() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres@127.0.0.1:1/unused")
            .unwrap();
        let state = |chain: i64| {
            let mut settings = super::super::Settings::for_tests();
            settings.market.indexes.chain_id_ethereum = chain;
            settings.market.indexes.chain_id_matic = if chain == ChainNet::Ethereum.chain_id() {
                ChainNet::Matic.chain_id()
            } else {
                ChainNet::Matic.testnet_chain_id()
            };
            MarketplaceState::new(pool.clone(), crate::http::Outbound::for_tests(), settings)
        };
        let deployments = |chain| {
            super::super::nft_index::deployments::Deployments::for_chain(ChainNet::Ethereum, chain)
                .unwrap()
        };
        for chain in [
            ChainNet::Ethereum.chain_id(),
            ChainNet::Ethereum.testnet_chain_id(),
        ] {
            let land = deployments(chain).land.unwrap();
            let served = served(&state(chain));
            assert_eq!(
                (
                    served.land.as_str(),
                    served.estate.as_str(),
                    served.names.as_str()
                ),
                (
                    land.land_registry.to_lowercase().as_str(),
                    land.estate_registry.to_lowercase().as_str(),
                    deployments(chain)
                        .registrar()
                        .unwrap()
                        .to_lowercase()
                        .as_str(),
                ),
                "{chain}"
            );
        }
        assert_ne!(
            served(&state(ChainNet::Ethereum.testnet_chain_id())).land,
            served(&state(ChainNet::Ethereum.chain_id())).land
        );
        let testnet = served(&state(ChainNet::Ethereum.testnet_chain_id()));
        assert_eq!(
            testnet.catalogue_sql(),
            "((c.network = 'ethereum' AND c.chain_id = 11155111 AND c.testnet_verified) \
             OR (c.network = 'matic' AND c.chain_id = 80002 AND c.testnet_verified))"
        );
        assert_eq!(
            (
                testnet.chain_id(Network::Ethereum),
                testnet.chain_id(Network::Matic)
            ),
            (ChainId::EthereumSepolia, ChainId::MaticAmoy)
        );
        let mainnet = served(&state(ChainNet::Ethereum.chain_id()));
        assert_eq!(
            (
                mainnet.chain_id(Network::Ethereum),
                mainnet.chain_id(Network::Matic)
            ),
            (ChainId::EthereumMainnet, ChainId::MaticMainnet)
        );
    }

    #[test]
    fn a_graph_order_keeps_the_wire_shape() {
        let o = json!({ "id": "0x1-1", "marketplaceAddress": "0xm", "nftAddress": "0xc", "tokenId": "5", "owner": "0xo", "buyer": null, "price": "10", "status": "open", "expiresAt": "1800000000000", "createdAt": "1700000000", "updatedAt": "1700000001", "nft": { "issuedId": "3" } });
        let order = order_from_graph(&o, Network::Matic);
        assert_eq!(
            serde_json::to_value(&order).unwrap(),
            json!({
                "id": "0x1-1", "marketplaceAddress": "0xm", "contractAddress": "0xc", "tokenId": "5", "owner": "0xo", "buyer": null, "price": "10", "status": "open",
                "expiresAt": 1_800_000_000_000i64, "createdAt": 1_700_000_000_000i64, "updatedAt": 1_700_000_001_000i64, "network": "MATIC", "chainId": 137, "issuedId": "3", "tradeId": ""
            })
        );
        let mut rows = vec![
            keyed(order.clone(), "5".into()),
            keyed(
                Order {
                    id: "b".into(),
                    price: "9".into(),
                    created_at: 1,
                    ..order.clone()
                },
                "12".into(),
            ),
        ];
        sort_merged(&mut rows, Some(OrderSortBy::Cheapest));
        assert_eq!(rows[0].order.id, "b");
        sort_merged(&mut rows, Some(OrderSortBy::IssuedIdDesc));
        assert_eq!(rows[0].sort_token_id, "12", "numeric, not lexicographic");
        sort_merged(&mut rows, None);
        assert_eq!(rows[0].order.id, "0x1-1", "newest first");
    }
}
