//! `/v1/bids`: off-chain bids from the trades mirror (`type = 'bid'`, open
//! unless expired or cancelled here; `sold` or `cancelled` by the chain's
//! verdict, `trade_events::chain_status_sql`; only while the network's
//! trade-events index is current, and never a V3 bid whose cancellation
//! digest could not be established, `trade_events::unverifiable_sql`) merged
//! with the legacy on-chain bids.
//! A mirror bid's seller is the item's creator when the bid is on an item and
//! the nft's current owner (the nft index, or land_authz for LAND and
//! estates) when it is on an nft; empty while no index holds the token.
//!
//! A node that walks the nft and sales indexes serves Polygon's legacy bids
//! (ERC721BidV2) from `shop.nft_bid` ([`polygon_rows`]) and the mirror's
//! bids on one read-only snapshot and one instant ([`bids_snapshot`]), on
//! which the trades mirror is ready, the sales fold published and recent,
//! the nft index fresh, and every bid in the request's scope has a seller:
//! the one the fold stored, or the token's latest transfer before the bid
//! when the nft walk's log reaches the bid's block and holds the token's
//! whole history. The scope is the catalogue the nft index walks, as the
//! collections subgraph only kept bids on assets it catalogued. A bid
//! neither proves fails the read closed rather than being served without
//! its seller or left out of a `seller` filter. Its statuses are the
//! subgraph's (`open`, `sold`, `cancelled`); a bid the contract dropped when
//! another bid on its token was accepted reads `cancelled`, since it can no
//! longer be accepted.
//! Ethereum's legacy bids, and every legacy bid on a node that does not walk
//! the indexes, come from the collections subgraph reader (first ≤ 100,
//! 30 s; a window past the graph's 5,000 skip is read by keyset up to
//! `readers::DEEP_WINDOW`), filtered at the same instant as the other legs.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, Row, Transaction};

use super::chain_walk::{self, ChainNet, Cursor, INDEXER_NFT, INDEXER_SALES};
use super::graph;
use super::owned_items;
use super::readers::{
    self, GraphPage, Ttl, index_read_error as read_error, index_source_error, seconds_to_ms,
};
use super::wire::{
    ApiError, ChainId, InvalidParameterError, Network, Params, get_pagination_params,
    get_parameter, network_and_chain, wire_chain,
};
use super::{MarketplaceState, SharedState};
use super::{Source, unavailable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidSortBy {
    RecentlyOffered,
    RecentlyUpdated,
    MostExpensive,
}

#[derive(Debug, Clone, Default)]
pub struct BidFilters {
    pub limit: i64,
    pub offset: i64,
    pub bidder: Option<String>,
    pub seller: Option<String>,
    pub sort_by: Option<BidSortBy>,
    pub contract_address: Option<String>,
    pub token_id: Option<String>,
    pub item_id: Option<String>,
    pub network: Option<Network>,
    pub network_db_filter: Option<Vec<String>>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct Bid {
    pub id: String,
    pub bidder: String,
    pub price: String,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated_at: i64,
    pub fingerprint: String,
    pub status: String,
    pub seller: String,
    pub network: Network,
    #[serde(rename = "chainId")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub chain_id: ChainId,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "expiresAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_at: i64,
    #[serde(rename = "tokenId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub token_id: Option<String>,
    #[serde(rename = "itemId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub item_id: Option<String>,
    #[serde(rename = "tradeId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub trade_id: Option<String>,
    #[serde(
        rename = "tradeContractAddress",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub trade_contract_address: Option<String>,
    #[serde(rename = "bidAddress", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bid_address: Option<String>,
    #[serde(rename = "blockchainId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub blockchain_id: Option<String>,
    #[serde(rename = "blockNumber", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub block_number: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct BidsPage {
    pub results: Vec<Bid>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub page: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub pages: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub limit: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct BidsEnvelope {
    pub ok: bool,
    pub data: BidsPage,
}

pub fn parse_filters(pairs: &[(String, String)]) -> Result<BidFilters, InvalidParameterError> {
    let pg = get_pagination_params(pairs);
    let p = Params::new(pairs);
    let sort_by = get_parameter(
        "sortBy",
        pairs,
        Some(&["recently_offered", "recently_updated", "most_expensive"]),
    )?
    .map(|s| match s.as_str() {
        "recently_updated" => BidSortBy::RecentlyUpdated,
        "most_expensive" => BidSortBy::MostExpensive,
        _ => BidSortBy::RecentlyOffered,
    });
    let network_raw = get_parameter(
        "network",
        pairs,
        Some(&[
            "ETHEREUM",
            "MATIC",
            "AVALANCHE",
            "BINANCE SMART CHAIN",
            "OPTIMISM",
            "ARBITRUM",
            "FANTOM",
        ]),
    )?;
    let network = network_raw.as_deref().and_then(|s| match s {
        "ETHEREUM" => Some(Network::Ethereum),
        "MATIC" => Some(Network::Matic),
        _ => None,
    });
    let network_db_filter = network_raw.as_deref().map(|s| match s {
        "ETHEREUM" => vec!["ETHEREUM".to_string()],
        "MATIC" => vec!["MATIC".to_string(), "POLYGON".to_string()],
        _ => Vec::new(),
    });
    let status = get_parameter("status", pairs, Some(&["open", "sold", "cancelled"]))?;
    Ok(BidFilters {
        limit: pg.limit,
        offset: pg.offset,
        bidder: p.get_string("bidder", None),
        seller: p.get_string("seller", None),
        sort_by,
        contract_address: p.get_string("contractAddress", None),
        token_id: p.get_string("tokenId", None),
        item_id: p.get_string("itemId", None),
        network,
        network_db_filter,
        status,
    })
}

/// Mirror bids with their status at `$1` (now); the received asset is the
/// nft or item bid on, the sent erc20 the price.
const TRADES_BIDS_SQL: &str = "SELECT t.id::text AS id, t.contract AS trade_contract_address, t.signer AS bidder, \
        (EXTRACT(EPOCH FROM t.created_at) * 1000)::bigint AS created_at, (EXTRACT(EPOCH FROM t.expires_at) * 1000)::bigint AS expires_at, \
        upper(t.network) AS network, t.chain_id::bigint AS chain_id, sent_erc20.amount::text AS price, recv_erc721.token_id, \
        COALESCE(recv_item.item_id, div(TOKEN_NUM, SHIFT::numeric)::text) AS item_id, \
        recv.contract_address, COALESCE(recv.extra, '') AS fingerprint, \
        (recv_erc721.token_id IS NOT NULL AND NOT LAND_CONTRACT) AS uses_nft_owner, \
        (recv_erc721.token_id IS NOT NULL AND LAND_CONTRACT) AS uses_land_owner, \
        CASE WHEN recv_erc721.token_id IS NULL THEN COALESCE(i.creator, '') \
             WHEN LAND_CONTRACT THEN COALESCE(tr.owner, '') ELSE COALESCE(n.owner, '') END AS seller, \
        CASE WHEN EXISTS (SELECT 1 FROM shop.trade_cancellations c WHERE c.target_hashed_signature = t.hashed_signature) THEN 'cancelled' \
             WHEN CHAIN_STATUS IS NOT NULL THEN CHAIN_STATUS \
             WHEN t.expires_at < $1 THEN 'cancelled' ELSE 'open' END AS status \
 FROM shop.trades t \
 JOIN shop.trade_assets sent ON sent.trade_id = t.id AND sent.direction = 'sent' AND sent.asset_type IN (1, 2) \
 JOIN shop.trade_assets_erc20 sent_erc20 ON sent_erc20.asset_id = sent.id \
 JOIN shop.trade_assets recv ON recv.trade_id = t.id AND recv.direction = 'received' \
 LEFT JOIN shop.trade_assets_erc721 recv_erc721 ON recv_erc721.asset_id = recv.id \
 LEFT JOIN shop.trade_assets_item recv_item ON recv_item.asset_id = recv.id \
 LEFT JOIN shop.items i ON i.contract_address = recv.contract_address AND i.network = lower(t.network) \
      AND i.item_id = COALESCE(ITEM_NUM, div(TOKEN_NUM, SHIFT::numeric)) \
      AND i.chain_id = t.chain_id AND (t.chain_id IN (1, 137) OR i.testnet_verified) \
 LEFT JOIN shop.nft n ON recv_erc721.token_id IS NOT NULL AND NOT LAND_CONTRACT AND n.network = lower(t.network) AND n.contract_address = recv.contract_address \
      AND n.token_id = TOKEN_NUM \
 LEFT JOIN land_authz.token_right tr ON recv_erc721.token_id IS NOT NULL AND LAND_CONTRACT AND tr.token_address = recv.contract_address AND tr.token_id = TOKEN_NUM \
 WHERE t.type = 'bid' AND t.expires_at > $1 AND COUNTED AND SERVED_CHAIN AND CHAIN_CURRENT \
   AND (recv_erc721.asset_id IS NULL OR TOKEN_NUM IS NOT NULL) AND (recv_item.asset_id IS NULL OR ITEM_NUM IS NOT NULL)";

/// The bids leg's `(id, status)` over every mirrored bid, for the index tests.
#[cfg(test)]
pub(crate) fn test_trades_bids_sql() -> String {
    format!(
        "SELECT q.id, q.status FROM ({}) q",
        trades_bids_scope(" ORDER BY b.created_at DESC LIMIT 1000")
    )
}

/// `$1` is now, `$2` the trades floor; `extra` binds from `$3`.
fn trades_bids_scope(extra: &str) -> String {
    let sql = TRADES_BIDS_SQL
        .replace(
            "LAND_CONTRACT",
            &format!(
                "(lower(t.network) = 'ethereum' AND \
              ((t.chain_id = 1 AND recv.contract_address IN ('{}', '{}')) OR \
               (t.chain_id = 11155111 AND recv.contract_address IN ('{}', '{}'))))",
                crate::land_authz::LAND_MAINNET.land_registry,
                crate::land_authz::LAND_MAINNET.estate_registry,
                crate::land_authz::LAND_SEPOLIA.land_registry,
                crate::land_authz::LAND_SEPOLIA.estate_registry,
            ),
        )
        .replace("COUNTED", &super::trades::counted_sql("t.", "$2"))
        .replace("SERVED_CHAIN", &super::catalog::served_chain_sql("t"))
        .replace("SHIFT", super::catalog::ITEM_ID_SHIFT)
        .replace(
            "TOKEN_NUM",
            &super::catalog::uint_or_null("recv_erc721.token_id"),
        )
        .replace(
            "ITEM_NUM",
            &super::catalog::uint_or_null("recv_item.item_id"),
        )
        .replace(
            "CHAIN_STATUS",
            &super::trade_events::chain_status_sql("t", "$1"),
        )
        .replace(
            "CHAIN_CURRENT",
            &format!(
                "{} AND NOT {}",
                super::trade_events::verdicts_current_sql("t", "$1"),
                super::trade_events::unverifiable_sql("t")
            ),
        );
    format!("SELECT * FROM ({sql}) b WHERE TRUE{extra}")
}

/// The mirror's bids of `f` at `now`, on `conn`.
pub(super) async fn trades_rows(
    conn: &mut PgConnection,
    state: &MarketplaceState,
    f: &BidFilters,
    now: DateTime<Utc>,
    polygon: Option<&SellerBounds>,
) -> Result<Vec<Bid>, Response> {
    if f.network_db_filter.as_ref().is_some_and(|n| n.is_empty()) {
        return Ok(Vec::new());
    }
    let mut extra = String::new();
    let mut binds: Vec<String> = Vec::new();
    let next = |binds: &mut Vec<String>, v: String| {
        binds.push(v);
        format!("${}", binds.len() + 2)
    };
    if let Some(v) = &f.bidder {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND lower(b.bidder) = {p}"));
    }
    if let Some(v) = &f.contract_address {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND b.contract_address = {p}"));
    }
    if let Some(v) = &f.token_id {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND lower(b.token_id) = {p}"));
    }
    if let Some(v) = &f.item_id {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND lower(b.item_id) = {p}"));
    }
    if let Some(v) = &f.status {
        let p = next(&mut binds, v.clone());
        extra.push_str(&format!(" AND b.status = {p}"));
    }
    if let Some(nets) = &f.network_db_filter {
        let placeholders: Vec<String> = nets.iter().map(|n| next(&mut binds, n.clone())).collect();
        extra.push_str(&format!(" AND b.network IN ({})", placeholders.join(", ")));
    }
    let cutoff = now.naive_utc();
    let query = |sql: String, binds: &[String]| {
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(cutoff)
            .bind(super::trades::mirrored_since(&state.settings, cutoff));
        for b in binds {
            q = q.bind(b.clone());
        }
        q
    };
    // Check ownership source coverage before seller and pagination.
    let affected = |ownership_filter: &str| {
        let scope = trades_bids_scope(&format!("{extra} AND {ownership_filter}"));
        query(format!("SELECT EXISTS ({scope}) AS affected"), &binds)
    };
    if owned_items::walks_here(state) {
        for net in ChainNet::ALL {
            let network = match net {
                ChainNet::Ethereum => Network::Ethereum,
                ChainNet::Matic => Network::Matic,
            };
            if f.network.is_some_and(|asked| asked != network) {
                continue;
            }
            let checked = polygon.filter(|_| net == ChainNet::Matic);
            if checked.is_some_and(|bounds| bounds.nft_rows_published) {
                continue;
            }
            if !affected(&format!(
                "lower(b.network) = '{}' AND b.uses_nft_owner",
                net.db_name()
            ))
            .fetch_one(&mut *conn)
            .await
            .map_err(read_error)?
            .get::<bool, _>("affected")
            {
                continue;
            }
            let chain = state.settings.catalogue().chain_id(net);
            if checked.is_some()
                || !chain_walk::cursor_with_rows_publication(&mut *conn, INDEXER_NFT, net, &["nft"])
                    .await
                    .map_err(read_error)?
                    .is_some_and(|(cursor, published)| {
                        published && chain_walk::fresh_fold_on(&cursor, chain, now).is_some()
                    })
            {
                return Err(nft_unavailable(
                    net,
                    &format!(
                        "the {} bids in this read need an up-to-date nft ownership publication on chain {chain}",
                        net.db_name()
                    ),
                ));
            }
        }
    }
    // The legacy graph leg does not supply sellers for the local trades leg.
    if f.network != Some(Network::Matic)
        && affected("b.uses_land_owner")
            .fetch_one(&mut *conn)
            .await
            .map_err(read_error)?
            .get::<bool, _>("affected")
    {
        let (chain, _) = super::catalog::served_chains();
        readers::require_land_on(&mut *conn, state, chain).await?;
    }
    if let Some(v) = &f.seller {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND lower(b.seller) = {p}"));
    }
    extra.push_str(" ORDER BY b.created_at DESC LIMIT 1000");
    let rows = query(trades_bids_scope(&extra), &binds)
        .fetch_all(conn)
        .await
        .map_err(read_error)?;
    Ok(rows
        .iter()
        .map(|r| {
            let (network, _) = network_and_chain(&r.get::<String, _>("network"));
            // The trade's own chain, which the leg keeps to the served one.
            let chain_id = wire_chain(r.get("chain_id"), network);
            let created_at: i64 = r.get("created_at");
            Bid {
                id: r.get("id"),
                bidder: r.get("bidder"),
                price: r.get("price"),
                created_at,
                updated_at: created_at,
                fingerprint: r.get("fingerprint"),
                status: r.get("status"),
                seller: r.get("seller"),
                network,
                chain_id,
                contract_address: r.get("contract_address"),
                expires_at: r.get("expires_at"),
                token_id: r.get("token_id"),
                item_id: r.get("item_id"),
                trade_id: Some(r.get("id")),
                trade_contract_address: r.get("trade_contract_address"),
                bid_address: None,
                blockchain_id: None,
                block_number: None,
            }
        })
        .collect())
}

// The collections subgraphs' `Bid` has no `fingerprint`; it reads back empty.
const BID_FIELDS: &str = "id bidAddress nftAddress tokenId bidder seller price status blockchainId blockNumber expiresAt createdAt updatedAt";

fn legacy_where(f: &BidFilters, now_ms: i64) -> Value {
    let mut w = json!({ "expiresAt_gt": now_ms.to_string() });
    if let Some(v) = &f.bidder {
        w["bidder"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.seller {
        w["seller"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.contract_address {
        w["nftAddress"] = json!(v.to_lowercase());
    }
    if let Some(v) = &f.token_id {
        w["tokenId"] = json!(v);
    }
    if let Some(v) = &f.status {
        w["status"] = json!(v);
    }
    w
}

fn legacy_order(sort_by: Option<BidSortBy>) -> (&'static str, &'static str) {
    match sort_by {
        Some(BidSortBy::RecentlyUpdated) => ("updatedAt", "desc"),
        Some(BidSortBy::MostExpensive) => ("price", "desc"),
        _ => ("createdAt", "desc"),
    }
}

/// Convert a legacy Graph row with the chain selected for that leg.
fn bid_from_graph(b: &Value, network: Network, chain_id: ChainId) -> Bid {
    let ms = |key: &str| {
        let raw = readers::int_of(b, key);
        if raw != 0 && raw < 100_000_000_000 {
            seconds_to_ms(raw)
        } else {
            raw
        }
    };
    Bid {
        id: readers::str_of(b, "id"),
        bidder: readers::str_of(b, "bidder"),
        price: readers::str_of(b, "price"),
        created_at: ms("createdAt"),
        updated_at: ms("updatedAt"),
        fingerprint: readers::str_of(b, "fingerprint"),
        status: readers::opt_str_of(b, "status").unwrap_or_else(|| "open".to_string()),
        seller: readers::str_of(b, "seller"),
        network,
        chain_id,
        contract_address: readers::str_of(b, "nftAddress"),
        expires_at: ms("expiresAt"),
        token_id: readers::opt_str_of(b, "tokenId"),
        item_id: None,
        trade_id: None,
        trade_contract_address: None,
        bid_address: readers::opt_str_of(b, "bidAddress"),
        blockchain_id: readers::opt_str_of(b, "blockchainId"),
        block_number: readers::opt_str_of(b, "blockNumber"),
    }
}

async fn legacy_leg(
    state: &MarketplaceState,
    f: &BidFilters,
    window: i64,
    now_ms: i64,
) -> Result<(Vec<Bid>, i64), Response> {
    let networks = match f.network {
        Some(n) => vec![n.graph()],
        None => vec![graph::Network::Ethereum, graph::Network::Matic],
    };
    graph_leg(state, f, window, networks, now_ms, false).await
}

/// Native legacy bids refer to NFTs, and the unsupported networks have
/// no bid rows on either legacy source.
fn legacy_bids_possible(f: &BidFilters) -> bool {
    f.item_id.is_none() && !f.network_db_filter.as_ref().is_some_and(|n| n.is_empty())
}

/// The subgraph reader's legacy bids of `networks` unexpired at `now_ms`
/// over the merged window and their capped total. `served` labels each row
/// with the chain the node serves on its network (a walking node's
/// catalogue), else with the network's configured chain.
async fn graph_leg(
    state: &MarketplaceState,
    f: &BidFilters,
    window: i64,
    networks: Vec<graph::Network>,
    now_ms: i64,
    served: bool,
) -> Result<(Vec<Bid>, i64), Response> {
    if !legacy_bids_possible(f) {
        return Ok((Vec::new(), 0));
    }
    let where_ = legacy_where(f, now_ms);
    let (order_by, direction) = legacy_order(f.sort_by);
    let mut rows = Vec::new();
    let mut total = 0;
    for network in networks {
        let Some(url) = network.url(&state.index) else {
            continue;
        };
        let page = GraphPage {
            entity: "bids",
            filter_type: "Bid_filter",
            fields: BID_FIELDS,
            where_: where_.clone(),
            order_by,
            direction,
            first: 0,
            skip: 0,
        };
        let got = readers::top_rows(state, network, url, Ttl::S30, &page, window)
            .await
            .map_err(|e| readers::upstream_failed("/v1/bids", &e))?;
        total += readers::count_to(
            state,
            network,
            url,
            "bids",
            "Bid_filter",
            where_.clone(),
            readers::count_cap(0, window),
        )
        .await
        .map_err(|e| readers::upstream_failed("/v1/bids", &e))?;
        let wire = Network::from(network);
        let chain_id = if served {
            let net = match wire {
                Network::Ethereum => ChainNet::Ethereum,
                Network::Matic => ChainNet::Matic,
            };
            wire_chain(state.settings.catalogue().chain_id(net), wire)
        } else {
            wire.chain_id()
        };
        rows.extend(got.iter().map(|b| bid_from_graph(b, wire, chain_id)));
    }
    Ok((rows, total))
}

/// Whether this node serves Polygon's legacy bids from the sales index: it
/// walks the nft index (whose transfers name a bid's seller) on its
/// catalogue, mainnet or the Sepolia/Amoy pair (`owned_items::walks_here`),
/// and the sales index (`MARKETPLACE_SALES_INDEX_ENABLED`).
pub fn polygon_book_here(state: &MarketplaceState) -> bool {
    owned_items::walks_here(state) && state.settings.market.indexes.sales_enabled
}

/// Whether the Polygon sales cursor has a fresh published fold on the
/// served chain. A forward walk can advance without changing that fold.
fn sales_published(c: &Cursor, chain_id: i64, now: DateTime<Utc>) -> bool {
    c.indexer == INDEXER_SALES && chain_walk::fresh_fold_on(c, chain_id, now).is_some()
}

/// Bid-book reads refused while the sales fold's writes await publication.
const UNPUBLISHED_FOLD_READS: dcl_metrics::CounterFamily<1> = dcl_metrics::CounterFamily::new(
    "catalyst_shop_bid_book_unpublished_fold_reads_total",
    "Bid-book reads refused because the sales index has not published the fold, by network.",
    Some(dcl_metrics::Unit::Count),
    &["network"],
);
const NFT_UNAVAILABLE_READS: dcl_metrics::CounterFamily<1> = dcl_metrics::CounterFamily::new(
    "catalyst_shop_bid_nft_unavailable_reads_total",
    "Bid reads refused with nft-index unavailable, by network.",
    Some(dcl_metrics::Unit::Count),
    &["network"],
);
pub const FAMILIES: &[dcl_metrics::Descriptor] = &[
    UNPUBLISHED_FOLD_READS.descriptor,
    NFT_UNAVAILABLE_READS.descriptor,
];

fn nft_unavailable(net: ChainNet, why: &str) -> Response {
    NFT_UNAVAILABLE_READS.handle([net.db_name()]).increment(1);
    owned_items::not_ready(why)
}

/// What a bid's seller is read against on a [`bids_snapshot`].
pub(super) struct SellerBounds {
    /// The last block of the nft walk's log: a seller can be read from the
    /// transfers of a bid at or below it.
    nft_last: i64,
    nft_rows_published: bool,
    /// The lowest point of an nft rewind the sales fold has not rechecked
    /// yet (`shop.bid_seller_recheck`): a seller the fold inferred for a bid
    /// created past it may name an owner the rewind took away.
    recheck_from: Option<i64>,
}

/// The bids' read-only snapshot and request instant, taken after the
/// snapshot is established,
/// so a fold the snapshot sees was published before it, and shared by the
/// checks, the reads and the subgraph's Ethereum filter. The trades mirror
/// and its verdicts must be ready on it; with `polygon`, the Polygon bid
/// book too, and what a seller is read against comes back. The sales and nft
/// cursors must carry the chain the node serves on Polygon. `Err` is the
/// unavailable envelope naming the index that is not ready, or a 500.
pub(super) async fn bids_snapshot(
    state: &MarketplaceState,
    polygon: bool,
) -> Result<
    (
        Transaction<'static, Postgres>,
        Option<SellerBounds>,
        DateTime<Utc>,
    ),
    Response,
> {
    let mut tx = state.pool.begin().await.map_err(read_error)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(read_error)?;
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(format!(
            "{}ms",
            super::nft_reads::READ_STATEMENT_TIMEOUT.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(read_error)?;
    // The statement above established the snapshot.
    let now = Utc::now();
    super::trades::ready_on(state, &mut tx, now).await?;
    if !polygon {
        return Ok((tx, None, now));
    }
    // The node's catalogue: the chain `walks_here` keeps the index settings on.
    let chain = state.settings.catalogue().chain_id(ChainNet::Matic);
    let sales = chain_walk::cursor_with_rows_publication(
        &mut tx,
        INDEXER_SALES,
        ChainNet::Matic,
        &["nft_bid"],
    )
    .await
    .map_err(read_error)?;
    let Some((_, rows_published)) = sales.filter(|(c, _)| sales_published(c, chain, now)) else {
        return Err(unavailable(
            Source::SalesIndex,
            "the matic sales index is not published (catching up, rewound after a reorg, stopped, or MARKETPLACE_SALES_INDEX_ENABLED off)",
        ));
    };
    if !rows_published {
        UNPUBLISHED_FOLD_READS
            .handle([ChainNet::Matic.db_name()])
            .increment(1);
        return Err(unavailable(
            Source::SalesIndex,
            "the matic bid book holds a fold the sales index has not published yet",
        ));
    }
    let nft =
        chain_walk::cursor_with_rows_publication(&mut tx, INDEXER_NFT, ChainNet::Matic, &["nft"])
            .await
            .map_err(read_error)?;
    let Some((nft, nft_rows_published)) =
        nft.filter(|(c, _)| chain_walk::fresh_fold_on(c, chain, now).is_some())
    else {
        return Err(nft_unavailable(
            ChainNet::Matic,
            "the matic nft index is not published (catching up, rewound after a reorg, stopped, or MARKETPLACE_NFT_INDEX_ENABLED off)",
        ));
    };
    super::catalogue::require_collections_on(&mut tx, state, ChainNet::Matic, now)
        .await
        .map_err(index_source_error)?;
    let recheck_from: Option<i64> = sqlx::query_scalar(
        "SELECT min(from_block) FROM shop.bid_seller_recheck WHERE network = 'matic'",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(read_error)?;
    Ok((
        tx,
        Some(SellerBounds {
            nft_last: nft.last_block,
            nft_rows_published,
            recheck_from,
        }),
        now,
    ))
}

/// Whether `s` is a decimal `uint256`, as the index stores token ids.
fn is_uint(s: &str) -> bool {
    !s.is_empty() && s.len() <= 78 && s.bytes().all(|b| b.is_ascii_digit())
}

/// The stored statuses a wire status names: the subgraph's `sold` is an
/// acceptance, its `cancelled` a cancellation or a bid another acceptance
/// dropped.
fn stored_statuses(status: &str) -> &'static [&'static str] {
    match status {
        "open" => &["open"],
        "sold" => &["accepted"],
        "cancelled" => &["cancelled", "invalidated"],
        _ => &[],
    }
}

/// `b` is a bid on the catalogue the nft index walks on Polygon's served
/// chain `chain`: a collection row of that chain and, on a testnet, verified
/// there (`NftIndex::load`'s address set). The bid contract takes any
/// ERC721, and the collections subgraph kept no bid on an asset it had not
/// catalogued; the nft walk never reads such a token's transfers.
///
/// `probe` keeps it one primary-key lookup per bid (`OFFSET 0` stops the
/// planner turning it into a join), for the unresolved-seller check: as a
/// join the planner takes the few bids that check reads for one row and
/// compares each with the whole catalogue. A page and a total join it.
fn catalogued(chain: i64, probe: bool) -> String {
    let membership = super::catalogue::collection_membership_sql(ChainNet::Matic, chain);
    let barrier = if probe { " OFFSET 0" } else { "" };
    format!(
        "EXISTS (SELECT 1 FROM shop.collections c WHERE {membership} AND c.contract_address = b.contract_address{barrier})"
    )
}

/// The filters of `f` but its seller over `shop.nft_bid b` at `now_ms`, from
/// `$4` (`$1` is the cutoff in seconds, `$2` and `$3` the [`SellerBounds`]); `None` when
/// no bid can match (a token id that is not a decimal). Each read adds the
/// catalogue ([`catalogued`]).
fn polygon_scope(f: &BidFilters, now_ms: i64, binds: &mut Vec<String>) -> Option<String> {
    binds.clear();
    binds.push(now_ms.div_euclid(1000).to_string());
    let mut sql = String::from("b.network = 'matic' AND b.expires_at > $1::bigint");
    let next = |binds: &mut Vec<String>, v: String| {
        binds.push(v);
        format!("${}", binds.len() + 2)
    };
    if let Some(v) = &f.bidder {
        let p = next(binds, v.to_lowercase());
        sql.push_str(&format!(" AND b.bidder = {p}"));
    }
    if let Some(v) = &f.contract_address {
        let p = next(binds, v.to_lowercase());
        sql.push_str(&format!(" AND b.contract_address = {p}"));
    }
    if let Some(v) = &f.token_id {
        if !is_uint(v) {
            return None;
        }
        let p = next(binds, v.clone());
        sql.push_str(&format!(" AND b.token_id = {p}::numeric"));
    }
    if let Some(v) = &f.status {
        let list: Vec<String> = stored_statuses(v)
            .iter()
            .map(|s| next(binds, s.to_string()))
            .collect();
        if list.is_empty() {
            return None;
        }
        sql.push_str(&format!(" AND b.status IN ({})", list.join(", ")));
    }
    Some(sql)
}

/// Whether the stored seller of a bid stands: the fold inferred it from a
/// transfer before any nft rewind it has not rechecked (`$3`), or an
/// acceptance named it.
const STORED_SELLER_STANDS: &str =
    "($3::bigint IS NULL OR b.created_block <= $3::bigint OR b.status = 'accepted')";

/// A bid's seller now: the one the fold stored while it stands
/// ([`STORED_SELLER_STANDS`]), or the token's latest transfer before the bid
/// in the nft walk's log when the log reaches the bid's block (`$2`) and
/// holds the contract's whole history: no backfill of the contract below
/// its walk is owed (`FOLD_BIDS_SQL`'s rule), since a transfer the backfill
/// has yet to add may be the latest. NULL when neither proves one.
fn seller_now() -> String {
    format!(
        "COALESCE(CASE WHEN {STORED_SELLER_STANDS} THEN b.seller END, \
         (SELECT n.to_address FROM shop.nft_event n \
          WHERE n.network = 'matic' AND n.contract_address = b.contract_address AND n.token_id = b.token_id AND n.kind = 'transfer' \
            AND (n.block_number, n.log_index) < (b.created_block, b.created_log_index) AND b.created_block <= $2::bigint \
            AND NOT EXISTS ( \
              SELECT 1 FROM shop.nft_contract k \
              WHERE k.network = 'matic' AND k.contract_address = b.contract_address AND k.backfill_from_block IS NOT NULL \
                AND COALESCE(k.backfilled_to_block, k.backfill_from_block - 1) < k.walked_from_block - 1) \
          ORDER BY n.block_number DESC, n.log_index DESC LIMIT 1))"
    )
}

/// The bid book's order for `sort_by`, ties by id as [`sort_bids`] (bytes).
fn polygon_order_by(sort_by: Option<BidSortBy>) -> &'static str {
    match sort_by {
        Some(BidSortBy::RecentlyUpdated) => "x.updated_at DESC, x.id COLLATE \"C\" ASC",
        Some(BidSortBy::MostExpensive) => "x.price DESC, x.id COLLATE \"C\" ASC",
        _ => "x.created_at DESC, x.id COLLATE \"C\" ASC",
    }
}

/// Polygon's legacy bids of `f` over the merged `window` from
/// `shop.nft_bid`, and their total capped as the reader's, on a
/// [`bids_snapshot`] at `now_ms`. Before reading them it checks every bid in
/// the request's scope (its filters but the seller's) for a seller, and
/// answers `None` when one has none: the nft walk has not reached its block
/// or the contract's history yet, so the bid could neither show its seller
/// nor be matched by one.
async fn polygon_rows(
    conn: &mut PgConnection,
    state: &MarketplaceState,
    f: &BidFilters,
    window: i64,
    bounds: &SellerBounds,
    now_ms: i64,
) -> Result<Option<(Vec<Bid>, i64)>, sqlx::Error> {
    let chain = state.settings.catalogue().chain_id(ChainNet::Matic);
    // The bid contract the sales walk reads on that chain.
    let bid_contract = super::sales_index::SalesIndex::for_chain(chain)
        .map_or(super::sales_index::BID_V2_MATIC, |i| i.bid_contract());
    let mut binds = Vec::new();
    let Some(scope) = polygon_scope(f, now_ms, &mut binds) else {
        return Ok(Some((Vec::new(), 0)));
    };
    let bind_all = |sql: String, binds: &[String]| {
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(binds[0].clone())
            .bind(bounds.nft_last.to_string())
            .bind(bounds.recheck_from.map(|b| b.to_string()));
        for b in &binds[1..] {
            q = q.bind(b.clone());
        }
        q
    };
    // Only a bid the fold left without a seller (`nft_bid_seller_unknown_idx`)
    // or whose stored seller a pending recheck sets aside can lack one now.
    let unknown = format!(
        "SELECT EXISTS (SELECT 1 FROM shop.nft_bid b WHERE {scope} AND {} \
           AND (b.seller IS NULL OR NOT {STORED_SELLER_STANDS}) AND {} IS NULL) AS unknown",
        catalogued(chain, true),
        seller_now()
    );
    let unknown: bool = bind_all(unknown, &binds)
        .fetch_one(&mut *conn)
        .await?
        .get("unknown");
    if unknown {
        return Ok(None);
    }
    let seller = match &f.seller {
        Some(v) => {
            binds.push(v.to_lowercase());
            format!(" WHERE x.seller = ${}", binds.len() + 2)
        }
        None => String::new(),
    };
    let book = format!(
        "SELECT b.contract_address || '-' || b.token_id::text || '-' || b.bidder AS id, b.bidder, b.price, \
                b.created_at, b.updated_at, b.status, {} AS seller, b.contract_address, b.expires_at, \
                b.token_id::text AS token_id, b.bid_id, b.updated_block \
         FROM shop.nft_bid b WHERE {scope} AND {}",
        seller_now(),
        catalogued(chain, false)
    );
    let rows = bind_all(
        format!(
            "SELECT x.id, x.bidder, x.price::text AS price, x.created_at, x.updated_at, x.status, x.seller, x.contract_address, \
                    x.expires_at, x.token_id, x.bid_id, x.updated_block \
             FROM ({book}) x{seller} ORDER BY {} LIMIT {}",
            polygon_order_by(f.sort_by),
            window.max(0)
        ),
        &binds,
    )
    .fetch_all(&mut *conn)
    .await?;
    let total: i64 = bind_all(
        format!(
            "SELECT count(*) AS n FROM (SELECT 1 FROM ({book}) x{seller} LIMIT {}) c",
            readers::count_cap(0, window)
        ),
        &binds,
    )
    .fetch_one(&mut *conn)
    .await?
    .get("n");
    let bids = rows
        .iter()
        .map(|r| {
            let status: String = r.get("status");
            Bid {
                id: r.get("id"),
                bidder: r.get("bidder"),
                price: r.get("price"),
                created_at: seconds_to_ms(r.get("created_at")),
                updated_at: seconds_to_ms(r.get("updated_at")),
                fingerprint: String::new(),
                status: match status.as_str() {
                    "accepted" => "sold",
                    "invalidated" => "cancelled",
                    s => s,
                }
                .to_string(),
                seller: r.get("seller"),
                network: Network::Matic,
                chain_id: wire_chain(chain, Network::Matic),
                contract_address: r.get("contract_address"),
                expires_at: seconds_to_ms(r.get("expires_at")),
                token_id: Some(r.get("token_id")),
                item_id: None,
                trade_id: None,
                trade_contract_address: None,
                bid_address: Some(bid_contract.to_string()),
                blockchain_id: Some(r.get("bid_id")),
                block_number: Some(r.get::<i64, _>("updated_block").to_string()),
            }
        })
        .collect();
    Ok(Some((bids, total)))
}

pub fn sort_bids(rows: &mut [Bid], sort_by: Option<BidSortBy>) {
    rows.sort_by(|a, b| {
        let id = a.id.cmp(&b.id);
        match sort_by {
            Some(BidSortBy::RecentlyUpdated) => b.updated_at.cmp(&a.updated_at).then(id),
            Some(BidSortBy::MostExpensive) => {
                let price = |bid: &Bid| {
                    alloy_primitives::U256::from_str_radix(&bid.price, 10).unwrap_or_default()
                };
                price(b).cmp(&price(a)).then(id)
            }
            _ => b.created_at.cmp(&a.created_at).then(id),
        }
    });
}

pub async fn list_bids(
    state: &MarketplaceState,
    f: &BidFilters,
) -> Result<(Vec<Bid>, i64), Response> {
    let limit = f.limit.clamp(0, readers::READER_MAX_FIRST);
    readers::check_window(limit, f.offset)?;
    let window = f.offset + limit;
    let (mut merged, legacy_total, trades) = if polygon_book_here(state) {
        let legacy = legacy_bids_possible(f);
        let ethereum = legacy && f.network != Some(Network::Matic);
        if ethereum {
            readers::require_networks(state, Some(graph::Network::Ethereum))?;
        }
        let polygon = legacy && f.network != Some(Network::Ethereum);
        let (mut tx, bounds, now) = bids_snapshot(state, polygon).await?;
        let now_ms = now.timestamp_millis();
        let read = async {
            let book = match &bounds {
                Some(bounds) => polygon_rows(&mut tx, state, f, window, bounds, now_ms)
                    .await
                    .map_err(read_error)?,
                None => Some((Vec::new(), 0)),
            };
            let trades = trades_rows(&mut tx, state, f, now, bounds.as_ref()).await?;
            Ok::<_, Response>((book, trades))
        }
        .await;
        tx.rollback().await.ok();
        let (book, trades) = read?;
        let (mut rows, mut total) = book.ok_or_else(|| {
            nft_unavailable(
                ChainNet::Matic,
                "the matic nft index has not reached the block or the history of a bid in this read: its seller is not known yet",
            )
        })?;
        if ethereum {
            let (more, more_total) = graph_leg(
                state,
                f,
                window,
                vec![graph::Network::Ethereum],
                now_ms,
                true,
            )
            .await?;
            rows.extend(more);
            total += more_total;
        }
        (rows, total, trades)
    } else {
        readers::require_networks(state, f.network.map(|n| n.graph()))?;
        // The trades readiness and rows on one short snapshot, released
        // before the subgraph is awaited, whose filter takes its instant.
        let (mut tx, _, now) = bids_snapshot(state, false).await?;
        let trades = trades_rows(&mut tx, state, f, now, None).await;
        tx.rollback().await.ok();
        let trades = trades?;
        let (rows, total) = legacy_leg(state, f, window, now.timestamp_millis()).await?;
        (rows, total, trades)
    };
    let total = legacy_total + trades.len() as i64;
    merged.extend(trades);
    sort_bids(&mut merged, f.sort_by);
    Ok((
        merged
            .into_iter()
            .skip(f.offset as usize)
            .take(limit as usize)
            .collect(),
        total,
    ))
}

pub async fn get_bids(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let pg = get_pagination_params(&pairs);
    let filters = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return ApiError::from(e).into_response(),
    };
    match list_bids(&state, &filters).await {
        Ok((data, total)) => {
            let page = if pg.limit > 0 {
                pg.offset / pg.limit
            } else {
                0
            };
            let pages = if !data.is_empty() && pg.limit > 0 {
                (total + pg.limit - 1) / pg.limit
            } else {
                0
            };
            Json(BidsEnvelope {
                ok: true,
                data: BidsPage {
                    results: data,
                    total,
                    page,
                    pages,
                    limit: pg.limit,
                },
            })
            .into_response()
        }
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_bid() -> Bid {
        Bid {
            id: "7f7b021a-9207-4868-9543-722fbc674787".to_string(),
            bidder: "0x57d1721f6223eb434e20f5b4a88e494008ea0542".to_string(),
            price: "200000000000000000000".to_string(),
            created_at: 1_782_326_697_937,
            updated_at: 1_782_326_697_937,
            fingerprint: "".to_string(),
            status: "open".to_string(),
            seller: "0x597d94248c5181232e83ee054b74f01548090f33".to_string(),
            network: Network::Matic,
            chain_id: ChainId::MaticMainnet,
            contract_address: "0xf1483f042614105cb943d3dd67157256cd003028".to_string(),
            expires_at: 1_784_865_600_000,
            token_id: Some(
                "1579684375028357800468770415255056484783426431008236668814664663050".to_string(),
            ),
            item_id: Some("15".to_string()),
            trade_id: Some("7f7b021a-9207-4868-9543-722fbc674787".to_string()),
            trade_contract_address: Some("0xa40b1d129b8906888720686f3a01921ddf37716f".to_string()),
            bid_address: None,
            blockchain_id: None,
            block_number: None,
        }
    }

    fn minimal_bid() -> Bid {
        Bid {
            id: "legacy-1".to_string(),
            bidder: "0xabc".to_string(),
            price: "1".to_string(),
            created_at: 1,
            updated_at: 2,
            fingerprint: "0xf".to_string(),
            status: "cancelled".to_string(),
            seller: "0xdef".to_string(),
            network: Network::Ethereum,
            chain_id: ChainId::EthereumMainnet,
            contract_address: "0xc0".to_string(),
            expires_at: 3,
            token_id: None,
            item_id: None,
            trade_id: None,
            trade_contract_address: None,
            bid_address: Some("0xb1d".to_string()),
            blockchain_id: Some("bid-0xc0-61".to_string()),
            block_number: Some("123456".to_string()),
        }
    }

    #[test]
    fn the_bid_book_trusts_a_published_recent_polygon_sales_fold_mid_pass_too() {
        let now = Utc::now();
        let published = Cursor {
            indexer: INDEXER_SALES.to_string(),
            network: "matic".to_string(),
            start_block: 1,
            last_block: 120,
            last_block_hash: None,
            last_block_time: None,
            fold_through: Some(100),
            folded_block: Some(100),
            folded_at: Some(now.naive_utc() - chrono::Duration::minutes(1)),
            full_history: true,
            reorg_rewinds: 0,
            updated_at: now.naive_utc(),
            chain_id: Some(137),
            owner_generation: 1,
            quarantined_at: None,
            quarantine_reason: None,
            rebuild_requested_at: None,
            diverged_at: None,
        };
        assert!(
            sales_published(&published, 137, now),
            "a pass that advanced last_block past the fold it has not folded yet"
        );
        let not = |c: Cursor| !sales_published(&c, 137, now);
        assert!(not(Cursor {
            folded_block: None,
            ..published.clone()
        }));
        assert!(not(Cursor {
            full_history: false,
            ..published.clone()
        }));
        assert!(not(Cursor {
            chain_id: Some(80002),
            ..published.clone()
        }));
        let amoy = Cursor {
            chain_id: Some(80002),
            ..published.clone()
        };
        assert!(
            sales_published(&amoy, 80002, now),
            "a node serving Amoy takes Amoy's fold"
        );
        assert!(not(Cursor {
            quarantined_at: Some(now.naive_utc()),
            ..published.clone()
        }));
        assert!(not(Cursor {
            folded_at: Some(now.naive_utc() - chrono::Duration::minutes(31)),
            ..published.clone()
        }));
        assert!(not(Cursor {
            indexer: INDEXER_NFT.to_string(),
            ..published.clone()
        }));
        assert_eq!(stored_statuses("cancelled"), &["cancelled", "invalidated"]);
        assert_eq!(stored_statuses("sold"), &["accepted"]);
        assert!(stored_statuses("transferred").is_empty());
    }

    #[test]
    fn wire_identity_bids_envelope() {
        let new = BidsEnvelope {
            ok: true,
            data: BidsPage {
                results: vec![full_bid(), minimal_bid()],
                total: 842,
                page: 0,
                pages: 9,
                limit: 100,
            },
        };
        let old = json!({
            "ok": true,
            "data": {
                "results": [
                    { "id": "7f7b021a-9207-4868-9543-722fbc674787", "bidder": "0x57d1721f6223eb434e20f5b4a88e494008ea0542", "price": "200000000000000000000", "createdAt": 1_782_326_697_937_i64, "updatedAt": 1_782_326_697_937_i64, "fingerprint": "", "status": "open", "seller": "0x597d94248c5181232e83ee054b74f01548090f33", "network": "MATIC", "chainId": 137, "contractAddress": "0xf1483f042614105cb943d3dd67157256cd003028", "expiresAt": 1_784_865_600_000_i64, "tokenId": "1579684375028357800468770415255056484783426431008236668814664663050", "itemId": "15", "tradeId": "7f7b021a-9207-4868-9543-722fbc674787", "tradeContractAddress": "0xa40b1d129b8906888720686f3a01921ddf37716f" },
                    { "id": "legacy-1", "bidder": "0xabc", "price": "1", "createdAt": 1, "updatedAt": 2, "fingerprint": "0xf", "status": "cancelled", "seller": "0xdef", "network": "ETHEREUM", "chainId": 1, "contractAddress": "0xc0", "expiresAt": 3, "bidAddress": "0xb1d", "blockchainId": "bid-0xc0-61", "blockNumber": "123456" },
                ],
                "total": 842, "page": 0, "pages": 9, "limit": 100,
            }
        });
        assert_eq!(serde_json::to_value(&new).unwrap(), old);
        let empty = BidsEnvelope {
            ok: true,
            data: BidsPage {
                results: vec![],
                total: 0,
                page: 0,
                pages: 0,
                limit: 100,
            },
        };
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            json!({ "ok": true, "data": { "results": [], "total": 0, "page": 0, "pages": 0, "limit": 100 } })
        );
    }

    #[test]
    fn the_mirror_leg_reads_bid_trades_and_the_legacy_where_binds_the_graph_fields() {
        let sql =
            trades_bids_scope(" AND lower(b.bidder) = $3 ORDER BY b.created_at DESC LIMIT 1000");
        assert!(
            sql.contains("t.type = 'bid'")
                && sql.contains("trade_cancellations")
                && sql.contains("shop.items i")
                && sql.contains("LIMIT 1000"),
            "{sql}"
        );
        let pairs: Vec<(String, String)> = [
            ("bidder", "0xB"),
            ("network", "MATIC"),
            ("sortBy", "most_expensive"),
            ("itemId", "3"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let f = parse_filters(&pairs).unwrap();
        assert_eq!(
            f.network_db_filter,
            Some(vec!["MATIC".to_string(), "POLYGON".to_string()])
        );
        let w = legacy_where(&f, 7);
        assert_eq!(w["bidder"], "0xb");
        assert_eq!(w["expiresAt_gt"], "7");
        assert_eq!(legacy_order(f.sort_by), ("price", "desc"));
        let mut rows = vec![minimal_bid(), full_bid()];
        sort_bids(&mut rows, f.sort_by);
        assert_eq!(rows[0].id, full_bid().id);
    }

    /// The bid book's scope is the catalogue the nft index walks on the
    /// served Polygon chain, verified there on a testnet, and its rows carry
    /// that chain.
    #[test]
    fn the_bid_scope_is_the_walked_catalogue_of_the_served_chain() {
        let mainnet = catalogued(137, false);
        assert!(
            mainnet.contains("c.chain_id = 137")
                && !mainnet.contains("testnet_verified")
                && !mainnet.contains("OFFSET 0"),
            "{mainnet}"
        );
        let amoy = catalogued(80_002, true);
        assert!(
            amoy.contains("c.chain_id = 80002 AND c.testnet_verified)")
                && amoy.contains("OFFSET 0"),
            "{amoy}"
        );
        assert_eq!(wire_chain(80_002, Network::Matic), ChainId::MaticAmoy);
        assert_eq!(wire_chain(137, Network::Matic), ChainId::MaticMainnet);
        assert_eq!(
            super::super::sales_index::SalesIndex::for_chain(80_002).map(|i| i.bid_contract()),
            Some("0x4b66eab79cc03a96fb6275cfcdf23c0db431606d"),
            "an Amoy bid names Amoy's bid contract"
        );
    }
}
