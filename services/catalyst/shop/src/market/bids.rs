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
//! A node that walks the nft and sales indexes serves the legacy bids of
//! both networks (ERC721BidV2 on Polygon, ERC721Bid on Ethereum) from
//! `shop.nft_bid` ([`book_rows`]) and the mirror's bids on one read-only
//! snapshot and one instant ([`bids_snapshot`]). On it the trades mirror is
//! ready, each network's sales fold published and recent (on Ethereum
//! mainnet with its bid history complete), its nft index fresh, on Ethereum
//! the land-authz index ready, and every bid in the request's scope has a
//! seller: the one the fold stored, or the token's latest transfer before
//! the bid in the log that holds it (the land-authz log for LAND and
//! estates, the nft walk's for any other token) when that log reaches the
//! bid's block and holds the token's whole history. The scope is the
//! catalogue the nft index walks (and on Ethereum the LAND, Estate and names
//! registries), as the subgraphs only kept bids on assets they catalogued.
//! A bid neither proves fails the read closed rather than being served
//! without its seller or left out of a `seller` filter. Its statuses are the
//! subgraphs' (`open`, `sold`, `cancelled`); a bid the contract dropped when
//! another bid on its token was accepted reads `cancelled`, since it can no
//! longer be accepted.
//! On a node that does not walk the indexes, the legacy bids come from the
//! collections subgraph reader (first ≤ 100, 30 s; a window past the graph's
//! 5,000 skip is read by keyset up to `readers::DEEP_WINDOW`), filtered at
//! the same instant as the mirror's.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, Row, Transaction};

use super::chain_walk::{self, ChainNet, Cursor, INDEXER_NFT, INDEXER_SALES};
use super::graph;
use super::owned_items::{self, Rule};
use super::readers::{self, GraphPage, Ttl, seconds_to_ms};
use super::wire::{
    ApiError, ChainId, InvalidParameterError, Network, Params, get_pagination_params,
    get_parameter, network_and_chain,
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
        CASE WHEN recv_erc721.token_id IS NOT NULL THEN COALESCE(n.owner, tr.owner, '') ELSE COALESCE(i.creator, '') END AS seller, \
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
 LEFT JOIN shop.nft n ON recv_erc721.token_id IS NOT NULL AND n.network = lower(t.network) AND n.contract_address = recv.contract_address \
      AND n.token_id = TOKEN_NUM \
 LEFT JOIN land_authz.token_right tr ON recv_erc721.token_id IS NOT NULL AND tr.token_address = recv.contract_address AND tr.token_id = TOKEN_NUM \
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
    if owned_items::walks_here(state) {
        for net in ChainNet::ALL {
            let network = match net {
                ChainNet::Ethereum => Network::Ethereum,
                ChainNet::Matic => Network::Matic,
            };
            if f.network.is_some_and(|asked| asked != network)
                || chain_walk::rows_published_on(&mut *conn, INDEXER_NFT, net, &["nft"])
                    .await
                    .map_err(read_error)?
            {
                continue;
            }
            // Test the full scope before seller and pagination. An unpublished
            // owner can otherwise make a bid disappear from the seller's count.
            let scope = trades_bids_scope(&format!(
                "{extra} AND lower(b.network) = '{}' AND b.token_id IS NOT NULL",
                net.db_name()
            ));
            let affected = query(format!("SELECT EXISTS ({scope}) AS affected"), &binds)
                .fetch_one(&mut *conn)
                .await
                .map_err(read_error)?
                .get("affected");
            if affected {
                return Err(owned_items::not_ready(&format!(
                    "the {} nft index has not published the ownership used by the bids in this read",
                    net.db_name()
                )));
            }
        }
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

pub fn bid_from_graph(b: &Value, network: Network) -> Bid {
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
        chain_id: network.chain_id(),
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
    graph_leg(state, f, window, networks, now_ms).await
}

/// Native legacy bids refer to NFTs, and the unsupported networks have
/// no bid rows on either legacy source.
fn legacy_bids_possible(f: &BidFilters) -> bool {
    f.item_id.is_none() && !f.network_db_filter.as_ref().is_some_and(|n| n.is_empty())
}

/// The subgraph reader's legacy bids of `networks` unexpired at `now_ms`
/// over the merged window and their capped total (a node that does not walk
/// the indexes).
async fn graph_leg(
    state: &MarketplaceState,
    f: &BidFilters,
    window: i64,
    networks: Vec<graph::Network>,
    now_ms: i64,
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
        rows.extend(
            got.iter()
                .map(|b| bid_from_graph(b, Network::from(network))),
        );
    }
    Ok((rows, total))
}

/// Whether this node serves its legacy bids from the sales index: it walks
/// the nft index (whose transfers name a bid's seller) on its catalogue,
/// mainnet or the Sepolia/Amoy pair (`owned_items::walks_here`), and the
/// sales index (`MARKETPLACE_SALES_INDEX_ENABLED`).
pub fn book_here(state: &MarketplaceState) -> bool {
    owned_items::walks_here(state) && state.settings.market.indexes.sales_enabled
}

/// Whether a network's sales cursor has a fresh published fold on the
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
pub const FAMILIES: &[dcl_metrics::Descriptor] = &[UNPUBLISHED_FOLD_READS.descriptor];

/// What a network's bid sellers are read against on a [`bids_snapshot`].
pub(super) struct SellerBounds {
    net: ChainNet,
    /// The last block of the nft walk's log: a seller can be read from the
    /// transfers of a bid at or below it.
    nft_last: i64,
    /// The lowest point of an nft rewind the sales fold has not rechecked
    /// yet (`shop.bid_seller_recheck`): a seller the fold inferred for a bid
    /// created past it may name an owner the rewind took away.
    recheck_from: Option<i64>,
    /// On Ethereum, how far the land-authz log is proven on the sales
    /// walk's branch, at most the land index's published block: a LAND or
    /// estate seller can be read from the transfers of a bid at or below it
    /// ([`LAND_ON_SALES_BRANCH_SQL`]); -1 when nothing is proven.
    land_through: i64,
}

/// The highest block at which the land-authz index and the Ethereum sales
/// walk hold the same block hash, for the land index of `$1`: the land
/// cursor's block or its checkpoint, against the sales cursor's block or one
/// of its checkpoints (`shop.chain_checkpoint`, the newest 64 window ends).
/// The two walks are independent and each trails the head by its
/// confirmations only, so after a fork each can publish the same height on
/// another branch. A common hash proves their logs equal up to that block;
/// a LAND transfer read for a bid at or below it is on the bid's branch.
/// NULL when no height agrees: a land walk ahead of every block the sales
/// walk has hashed, or the two on different branches.
const LAND_ON_SALES_BRANCH_SQL: &str = "SELECT max(l.block) FROM land_authz.index_cursor c \
     CROSS JOIN LATERAL (VALUES (c.last_block, c.last_block_hash), (c.checkpoint_block, c.checkpoint_hash)) l(block, hash) \
     WHERE c.network = $1 AND l.hash IS NOT NULL AND EXISTS ( \
       SELECT 1 FROM shop.chain_cursor s \
       WHERE s.indexer = 'sales' AND s.network = 'ethereum' AND s.last_block = l.block AND lower(s.last_block_hash) = lower(l.hash) \
       UNION ALL \
       SELECT 1 FROM shop.chain_checkpoint k \
       WHERE k.indexer = 'sales' AND k.network = 'ethereum' AND k.block_number = l.block AND k.block_hash = lower(l.hash))";

/// The networks whose legacy bids a read of `f` takes from the book: none
/// for an item (the contracts bid on tokens) or a network the filter cannot
/// name.
fn book_nets(f: &BidFilters) -> Vec<ChainNet> {
    if !legacy_bids_possible(f) {
        return Vec::new();
    }
    match f.network {
        Some(Network::Ethereum) => vec![ChainNet::Ethereum],
        Some(Network::Matic) => vec![ChainNet::Matic],
        None => vec![ChainNet::Ethereum, ChainNet::Matic],
    }
}

/// The bids' read-only snapshot and request instant, taken after the
/// snapshot is established, so a fold the snapshot sees was published
/// before it, and shared by the checks and the reads. The trades mirror and
/// its verdicts must be ready on it, and for each of `nets` the bid book:
/// the sales cursor and the nft index published on the chain the node
/// serves there, with the collection membership that supplies the catalogue
/// (`catalogue::require_collections_on`); on Ethereum mainnet the bid
/// history repair complete (`ethereum_bid_history::complete_on`); and on
/// Ethereum the land-authz index ready, whose snapshot the read then runs
/// on, since LAND and estate sellers come from its log, as far as that log
/// is proven on the sales walk's branch ([`LAND_ON_SALES_BRANCH_SQL`]).
/// What each network's sellers are read against comes back. `Err` is the
/// unavailable envelope naming the index that is not ready, or a 500.
pub(super) async fn bids_snapshot(
    state: &MarketplaceState,
    nets: &[ChainNet],
) -> Result<
    (
        Transaction<'static, Postgres>,
        Vec<SellerBounds>,
        DateTime<Utc>,
    ),
    Response,
> {
    let db = |e: sqlx::Error| ApiError::internal(e.to_string()).into_response();
    let (mut tx, land) = if nets.contains(&ChainNet::Ethereum) {
        let snapshot = match state.land_network() {
            Some(net) => super::nft_reads::land_snapshot(&state.pool, net).await,
            None => None,
        }
        .ok_or_else(super::nft_reads::land_unavailable)?;
        let folded = snapshot.cursor.folded_block.unwrap_or(-1);
        (snapshot.tx, Some((snapshot.cursor.network, folded)))
    } else {
        let mut tx = state.pool.begin().await.map_err(db)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        (tx, None)
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
    super::trades::ready_on(state, &mut tx, now).await?;
    let catalogue = state.settings.catalogue();
    let mut bounds = Vec::new();
    for &net in nets {
        let chain = catalogue.chain_id(net);
        let sales = chain_walk::cursor(&mut *tx, INDEXER_SALES, net)
            .await
            .map_err(db)?;
        if !sales.is_some_and(|c| sales_published(&c, chain, now)) {
            return Err(unavailable(
                Source::SalesIndex,
                &format!(
                    "the {} sales index is not published (catching up, rewound after a reorg, stopped, or MARKETPLACE_SALES_INDEX_ENABLED off)",
                    net.db_name()
                ),
            ));
        }
        // Every write to the network's book, by any binary, waits for the
        // sales index's next publication (`chain_walk::rows_published_on`).
        if !chain_walk::rows_published_on(&mut tx, INDEXER_SALES, net, &["nft_bid"])
            .await
            .map_err(read_error)?
        {
            UNPUBLISHED_FOLD_READS.handle([net.db_name()]).increment(1);
            return Err(unavailable(
                Source::SalesIndex,
                &format!(
                    "the {} bid book holds a fold the sales index has not published yet",
                    net.db_name()
                ),
            ));
        }
        if net == ChainNet::Ethereum
            && chain == ChainNet::Ethereum.chain_id()
            && !super::ethereum_bid_history::complete_on(&mut tx)
                .await
                .map_err(db)?
        {
            return Err(unavailable(
                Source::SalesIndex,
                "the ethereum sales index is still reading the bids it walked past before it kept their whole lifecycle",
            ));
        }
        owned_items::check(&mut tx, catalogue, &[net], Rule::Fresh, false, now)
            .await
            .map_err(|e| e.into_response(|m| ApiError::internal(m.to_string()).into_response()))?;
        super::catalogue::require_collections_on(&mut tx, state, net, now).await?;
        let nft = chain_walk::cursor(&mut *tx, INDEXER_NFT, net)
            .await
            .map_err(db)?
            .ok_or_else(|| owned_items::not_ready("the nft index has no cursor"))?;
        let recheck_from: Option<i64> = sqlx::query_scalar(
            "SELECT min(from_block) FROM shop.bid_seller_recheck WHERE network = $1",
        )
        .bind(net.db_name())
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        let land_through = match (&land, net) {
            (Some((network, folded)), ChainNet::Ethereum) => {
                let shared: Option<i64> = sqlx::query_scalar(LAND_ON_SALES_BRANCH_SQL)
                    .bind(network)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db)?;
                shared.map_or(-1, |block| block.min(*folded))
            }
            _ => -1,
        };
        bounds.push(SellerBounds {
            net,
            nft_last: nft.last_block,
            recheck_from,
            land_through,
        });
    }
    Ok((tx, bounds, now))
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

/// What a network's bid book serves on the chain the node serves there
/// (`Settings::catalogue`): its bid contract and, on Ethereum, the chain's
/// LAND, Estate and names registrar (lowercase, empty where it has none).
struct Book {
    net: ChainNet,
    network: Network,
    chain: i64,
    bid_contract: &'static str,
    land: String,
    estate: String,
    names: String,
}

fn book_of(state: &MarketplaceState, net: ChainNet) -> Book {
    let chain = state.settings.catalogue().chain_id(net);
    let deployed = super::sales_index::SalesIndex::for_chain(chain).map(|i| i.bid_contract());
    match net {
        ChainNet::Matic => Book {
            net,
            network: Network::Matic,
            chain,
            bid_contract: deployed.unwrap_or(super::sales_index::BID_V2_MATIC),
            land: String::new(),
            estate: String::new(),
            names: String::new(),
        },
        ChainNet::Ethereum => {
            let deployments = super::nft_index::deployments::Deployments::for_chain(net, chain);
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
            Book {
                net,
                network: Network::Ethereum,
                chain,
                // Mainnet's ERC721Bid is the Ethereum sales walk's; a testnet
                // one is its sales index's deployment.
                bid_contract: if chain == ChainNet::Ethereum.chain_id() {
                    super::ethereum_sales::ERC721_BID
                } else {
                    deployed.unwrap_or(super::ethereum_sales::ERC721_BID)
                },
                land,
                estate,
                names: state.settings.catalogue().registrar().to_lowercase(),
            }
        }
    }
}

impl Book {
    /// `b` is a bid on the catalogue the nft index walks on the served
    /// chain: a collection row of that chain and, on a testnet, verified
    /// there (`NftIndex::load`'s address set), or on Ethereum one of the
    /// chain's LAND, Estate and names registrar. The bid contracts take any
    /// ERC721, and the subgraphs kept no bid on an asset they had not
    /// catalogued; the walks never read such a token's transfers.
    ///
    /// `probe` keeps the collection test one primary-key lookup per bid
    /// (`OFFSET 0` stops the planner turning it into a join), for the
    /// unresolved-seller check: as a join the planner takes the few bids
    /// that check reads for one row and compares each with the whole
    /// catalogue. A page and a total join it.
    fn catalogued(&self, probe: bool) -> String {
        let collection = format!(
            "EXISTS (SELECT 1 FROM shop.collections c WHERE c.network = '{}' AND c.contract_address = b.contract_address \
               AND c.chain_id = {}{}{})",
            self.net.db_name(),
            self.chain,
            if self.chain == self.net.chain_id() {
                ""
            } else {
                " AND c.testnet_verified"
            },
            if probe { " OFFSET 0" } else { "" }
        );
        match self.net {
            ChainNet::Matic => collection,
            ChainNet::Ethereum => format!(
                "({collection} OR b.contract_address IN ('{}', '{}', '{}'))",
                self.land, self.estate, self.names
            ),
        }
    }

    /// A bid's seller now: the one the fold stored while it stands
    /// ([`STORED_SELLER_STANDS`]), else the token's latest transfer before
    /// the bid. For LAND and estates that is the land-authz log, when the
    /// land index's published block reaches the bid's block and that log is
    /// proven on the sales walk's branch through it
    /// ([`SellerBounds::land_through`]; the nft walk does not read their
    /// transfers); for any other token the nft walk's
    /// log, when it reaches the bid's block (`$2`) and holds the contract's
    /// whole history: no backfill of the contract below its walk is owed
    /// (`FOLD_BIDS_SQL`'s rule), since a transfer the backfill has yet to add
    /// may be the latest. NULL when neither proves one.
    fn seller_now(&self, bounds: &SellerBounds) -> String {
        let nft = format!(
            "(SELECT n.to_address FROM shop.nft_event n \
              WHERE n.network = '{net}' AND n.contract_address = b.contract_address AND n.token_id = b.token_id AND n.kind = 'transfer' \
                AND (n.block_number, n.log_index) < (b.created_block, b.created_log_index) AND b.created_block <= $2::bigint \
                AND NOT EXISTS ( \
                  SELECT 1 FROM shop.nft_contract k \
                  WHERE k.network = '{net}' AND k.contract_address = b.contract_address AND k.backfill_from_block IS NOT NULL \
                    AND COALESCE(k.backfilled_to_block, k.backfill_from_block - 1) < k.walked_from_block - 1) \
              ORDER BY n.block_number DESC, n.log_index DESC LIMIT 1)",
            net = self.net.db_name()
        );
        let inferred = if self.land.is_empty() {
            nft
        } else {
            format!(
                "CASE WHEN b.contract_address IN ('{land}', '{estate}') THEN \
                   (SELECT a.account FROM land_authz.authz_event a \
                    WHERE a.token_address = b.contract_address AND a.token_id = b.token_id AND a.kind = 'transfer' \
                      AND (a.block_number, a.log_index) < (b.created_block, b.created_log_index) AND b.created_block <= {folded} \
                    ORDER BY a.block_number DESC, a.log_index DESC LIMIT 1) \
                 ELSE {nft} END",
                land = self.land,
                estate = self.estate,
                folded = bounds.land_through,
            )
        };
        format!("COALESCE(CASE WHEN {STORED_SELLER_STANDS} THEN b.seller END, {inferred})")
    }

    /// The index whose log a bid lacking a seller waits on.
    fn waits_on(&self) -> String {
        if self.land.is_empty() {
            "'nft'".to_string()
        } else {
            format!(
                "CASE WHEN b.contract_address IN ('{}', '{}') THEN 'land' ELSE 'nft' END",
                self.land, self.estate
            )
        }
    }
}

/// The filters of `f` but its seller over `shop.nft_bid b` of `net` at
/// `now_ms`, from `$4` (`$1` is the cutoff in whole seconds, `$2` and `$3`
/// the [`SellerBounds`]); `None` when no bid can match (a token id that is not a
/// decimal). Each read adds the catalogue ([`Book::catalogued`]).
fn scope(net: ChainNet, f: &BidFilters, now_ms: i64, binds: &mut Vec<String>) -> Option<String> {
    binds.clear();
    binds.push(now_ms.div_euclid(1000).to_string());
    let mut sql = format!(
        "b.network = '{}' AND b.expires_at > $1::bigint",
        net.db_name()
    );
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

/// The bid book's order for `sort_by`, ties by id as [`sort_bids`] (bytes).
fn book_order_by(sort_by: Option<BidSortBy>) -> &'static str {
    match sort_by {
        Some(BidSortBy::RecentlyUpdated) => "x.updated_at DESC, x.id COLLATE \"C\" ASC",
        Some(BidSortBy::MostExpensive) => "x.price DESC, x.id COLLATE \"C\" ASC",
        _ => "x.created_at DESC, x.id COLLATE \"C\" ASC",
    }
}

/// A bid in a read's scope whose seller is not known yet: the index whose
/// log it waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unresolved {
    Nft(ChainNet),
    Land,
}

impl Unresolved {
    fn envelope(self) -> Response {
        match self {
            Self::Nft(net) => owned_items::not_ready(&format!(
                "the {} nft index has not reached the block or the history of a bid in this read: its seller is not known yet",
                net.db_name()
            )),
            Self::Land => unavailable(
                Source::LandAuthzIndex,
                "the land-authz index has not reached the block of a LAND or estate bid in this read: its seller is not known yet",
            ),
        }
    }
}

/// One network's legacy bids of `f` over the merged `window` from
/// `shop.nft_bid`, and their total capped as the subgraph reader capped
/// each network's, on a [`bids_snapshot`] at `now_ms`. Before reading them
/// it checks every bid in the request's scope (its filters but the seller's)
/// for a seller, and answers which index one waits on when one has none: the
/// bid could neither show its seller nor be matched by one.
async fn book_rows(
    conn: &mut PgConnection,
    book: &Book,
    bounds: &SellerBounds,
    f: &BidFilters,
    window: i64,
    now_ms: i64,
) -> Result<Result<(Vec<Bid>, i64), Unresolved>, sqlx::Error> {
    let mut binds = Vec::new();
    let Some(scope) = scope(book.net, f, now_ms, &mut binds) else {
        return Ok(Ok((Vec::new(), 0)));
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
    let seller_now = book.seller_now(bounds);
    // Only a bid the fold left without a seller (`nft_bid_seller_unknown_idx`)
    // or whose stored seller a pending recheck sets aside can lack one now.
    let unknown = format!(
        "SELECT (SELECT {} FROM shop.nft_bid b WHERE {scope} AND {} \
           AND (b.seller IS NULL OR NOT {STORED_SELLER_STANDS}) AND {seller_now} IS NULL LIMIT 1) AS waits_on",
        book.waits_on(),
        book.catalogued(true),
    );
    let waits_on: Option<String> = bind_all(unknown, &binds)
        .fetch_one(&mut *conn)
        .await?
        .get("waits_on");
    match waits_on.as_deref() {
        Some("land") => return Ok(Err(Unresolved::Land)),
        Some(_) => return Ok(Err(Unresolved::Nft(book.net))),
        None => {}
    }
    let seller = match &f.seller {
        Some(v) => {
            binds.push(v.to_lowercase());
            format!(" WHERE x.seller = ${}", binds.len() + 2)
        }
        None => String::new(),
    };
    // Polygon's collections subgraph had no fingerprint; Ethereum's
    // marketplace subgraph spelled `Bytes` as 0x-hex.
    let fingerprint = match book.net {
        ChainNet::Matic => "''",
        ChainNet::Ethereum => "COALESCE(b.fingerprint, '0x')",
    };
    let rows_of = format!(
        "SELECT b.contract_address || '-' || b.token_id::text || '-' || b.bidder AS id, b.bidder, b.price, \
                b.created_at, b.updated_at, b.status, {seller_now} AS seller, b.contract_address, b.expires_at, \
                b.token_id::text AS token_id, b.bid_id, b.updated_block, {fingerprint} AS fingerprint \
         FROM shop.nft_bid b WHERE {scope} AND {}",
        book.catalogued(false)
    );
    let rows = bind_all(
        format!(
            "SELECT x.id, x.bidder, x.price::text AS price, x.created_at, x.updated_at, x.status, x.seller, x.contract_address, \
                    x.expires_at, x.token_id, x.bid_id, x.updated_block, x.fingerprint \
             FROM ({rows_of}) x{seller} ORDER BY {} LIMIT {}",
            book_order_by(f.sort_by),
            window.max(0)
        ),
        &binds,
    )
    .fetch_all(&mut *conn)
    .await?;
    let total: i64 = bind_all(
        format!(
            "SELECT count(*) AS n FROM (SELECT 1 FROM ({rows_of}) x{seller} LIMIT {}) c",
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
                fingerprint: r.get("fingerprint"),
                status: match status.as_str() {
                    "accepted" => "sold",
                    "invalidated" => "cancelled",
                    s => s,
                }
                .to_string(),
                seller: r.get("seller"),
                network: book.network,
                chain_id: wire_chain(book.chain, book.network),
                contract_address: r.get("contract_address"),
                expires_at: seconds_to_ms(r.get("expires_at")),
                token_id: Some(r.get("token_id")),
                item_id: None,
                trade_id: None,
                trade_contract_address: None,
                bid_address: Some(book.bid_contract.to_string()),
                blockchain_id: Some(r.get("bid_id")),
                block_number: Some(r.get::<i64, _>("updated_block").to_string()),
            }
        })
        .collect();
    Ok(Ok((bids, total)))
}

/// A statement of the bid book failed: one Postgres cancelled at the
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

/// `chain` as the wire's `ChainId`. The index settings admit only the
/// supported chains, so the network's default never stands in for a served one.
fn wire_chain(chain: i64, network: Network) -> ChainId {
    u64::try_from(chain)
        .ok()
        .and_then(|c| ChainId::try_from(c).ok())
        .unwrap_or_else(|| network.chain_id())
}

pub async fn list_bids(
    state: &MarketplaceState,
    f: &BidFilters,
) -> Result<(Vec<Bid>, i64), Response> {
    let limit = f.limit.clamp(0, readers::READER_MAX_FIRST);
    readers::check_window(limit, f.offset)?;
    let window = f.offset + limit;
    let (mut merged, legacy_total, trades) = if book_here(state) {
        let nets = book_nets(f);
        let (mut tx, bounds, now) = bids_snapshot(state, &nets).await?;
        let now_ms = now.timestamp_millis();
        let read = async {
            let mut legs = Vec::new();
            for b in &bounds {
                legs.push(
                    book_rows(&mut tx, &book_of(state, b.net), b, f, window, now_ms)
                        .await
                        .map_err(read_error)?,
                );
            }
            let trades = trades_rows(&mut tx, state, f, now).await?;
            Ok::<_, Response>((legs, trades))
        }
        .await;
        tx.rollback().await.ok();
        let (legs, trades) = read?;
        let mut rows = Vec::new();
        let mut total = 0;
        for leg in legs {
            let (more, more_total) = leg.map_err(Unresolved::envelope)?;
            rows.extend(more);
            total += more_total;
        }
        (rows, total, trades)
    } else {
        readers::require_networks(state, f.network.map(|n| n.graph()))?;
        // The trades readiness and rows on one short snapshot, released
        // before the subgraph is awaited, whose filter takes its instant.
        let (mut tx, _, now) = bids_snapshot(state, &[]).await?;
        let trades = trades_rows(&mut tx, state, f, now).await;
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
    /// served chain, verified there on a testnet, and on Ethereum that
    /// chain's LAND, Estate and names registrar; its rows carry the chain
    /// and its bid contract.
    #[tokio::test]
    async fn the_bid_scope_is_the_walked_catalogue_of_the_served_chain() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres@127.0.0.1:1/unused")
            .unwrap();
        let state = |testnet: bool| {
            let mut settings = super::super::Settings::for_tests();
            if testnet {
                settings.market.indexes.chain_id_ethereum = ChainNet::Ethereum.testnet_chain_id();
                settings.market.indexes.chain_id_matic = ChainNet::Matic.testnet_chain_id();
            }
            MarketplaceState::new(pool.clone(), crate::http::Outbound::for_tests(), settings)
        };
        let mainnet = book_of(&state(false), ChainNet::Matic).catalogued(false);
        assert!(
            mainnet.contains("c.chain_id = 137") && !mainnet.contains("testnet_verified"),
            "{mainnet}"
        );
        let amoy = book_of(&state(true), ChainNet::Matic);
        assert!(
            amoy.catalogued(true)
                .contains("c.chain_id = 80002 AND c.testnet_verified OFFSET 0)"),
            "{}",
            amoy.catalogued(true)
        );
        assert_eq!(
            amoy.bid_contract,
            "0x4b66eab79cc03a96fb6275cfcdf23c0db431606d"
        );
        let ethereum = book_of(&state(false), ChainNet::Ethereum);
        let land = crate::land_authz::LAND_REGISTRY_MAINNET.to_lowercase();
        assert!(
            ethereum.catalogued(false).contains("c.chain_id = 1)")
                && ethereum.catalogued(false).contains(&land),
            "{}",
            ethereum.catalogued(false)
        );
        assert_eq!(
            ethereum.bid_contract,
            super::super::ethereum_sales::ERC721_BID
        );
        let sepolia = book_of(&state(true), ChainNet::Ethereum);
        assert!(
            sepolia
                .catalogued(false)
                .contains("c.chain_id = 11155111 AND c.testnet_verified)")
                && !sepolia.catalogued(false).contains(&land),
            "{}",
            sepolia.catalogued(false)
        );
        assert_eq!(
            sepolia.bid_contract,
            "0x2c2835b95852fd975e087b3b25297322728792e2"
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
