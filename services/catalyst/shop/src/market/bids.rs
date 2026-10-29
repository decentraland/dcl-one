//! `/v1/bids`: off-chain bids from the trades mirror (`type = 'bid'`, open
//! unless expired or cancelled here; `sold` or `cancelled` by the chain's
//! verdict once the network's trade-events index is published,
//! `trade_events::chain_status_sql`) merged with the legacy on-chain `bids`
//! of the collections subgraph reader (first ≤ 100, 30 s; a window past the
//! graph's 5,000 skip is read by keyset up to `readers::DEEP_WINDOW`). A
//! mirror bid's seller is the item's creator when the bid is on an item and
//! the nft's current owner (the nft index, or land_authz for LAND and
//! estates) when it is on an nft; empty while no index holds the token.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;

use super::graph;
use super::readers::{self, GraphPage, Ttl, seconds_to_ms};
use super::wire::{
    ApiError, ChainId, InvalidParameterError, Network, Params, get_pagination_params,
    get_parameter, network_and_chain,
};
use super::{MarketplaceState, SharedState};

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
        upper(t.network) AS network, sent_erc20.amount::text AS price, recv_erc721.token_id, \
        COALESCE(recv_item.item_id, CASE WHEN recv_erc721.token_id IS NOT NULL THEN div(recv_erc721.token_id::numeric, SHIFT::numeric)::text END) AS item_id, \
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
      AND i.item_id = COALESCE(recv_item.item_id::numeric, CASE WHEN recv_erc721.token_id IS NOT NULL THEN div(recv_erc721.token_id::numeric, SHIFT::numeric) END) \
 LEFT JOIN shop.nft n ON recv_erc721.token_id IS NOT NULL AND n.network = lower(t.network) AND n.contract_address = recv.contract_address \
      AND n.token_id = recv_erc721.token_id::numeric \
 LEFT JOIN land_authz.token_right tr ON recv_erc721.token_id IS NOT NULL AND tr.token_address = recv.contract_address AND tr.token_id = recv_erc721.token_id::numeric \
 WHERE t.type = 'bid' AND t.expires_at > $1 AND COUNTED";

/// The bids leg's `(id, status)` over every mirrored bid, for the index tests.
#[cfg(test)]
pub(crate) fn test_trades_bids_sql() -> String {
    format!("SELECT q.id, q.status FROM ({}) q", trades_bids_sql(""))
}

/// `$1` is now, `$2` the trades floor; `extra` binds from `$3`.
fn trades_bids_sql(extra: &str) -> String {
    let sql = TRADES_BIDS_SQL
        .replace("COUNTED", &super::trades::counted_sql("t.", "$2"))
        .replace("SHIFT", super::catalog::ITEM_ID_SHIFT)
        .replace(
            "CHAIN_STATUS",
            &super::trade_events::chain_status_sql("t", "$1"),
        );
    format!("SELECT * FROM ({sql}) b WHERE TRUE{extra} ORDER BY b.created_at DESC LIMIT 1000")
}

pub async fn trades_leg(state: &MarketplaceState, f: &BidFilters) -> Result<Vec<Bid>, ApiError> {
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
    if let Some(v) = &f.seller {
        let p = next(&mut binds, v.to_lowercase());
        extra.push_str(&format!(" AND lower(b.seller) = {p}"));
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
    let now = Utc::now().naive_utc();
    let mut q = sqlx::query(sqlx::AssertSqlSafe(trades_bids_sql(&extra)))
        .bind(now)
        .bind(super::trades::mirrored_since(&state.settings, now));
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(&state.pool).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let (network, chain_id) = network_and_chain(&r.get::<String, _>("network"));
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

pub async fn legacy_leg(
    state: &MarketplaceState,
    f: &BidFilters,
    window: i64,
) -> Result<(Vec<Bid>, i64), Response> {
    if f.item_id.is_some() || f.network_db_filter.as_ref().is_some_and(|n| n.is_empty()) {
        return Ok((Vec::new(), 0));
    }
    let networks = match f.network {
        Some(n) => vec![n.graph()],
        None => vec![graph::Network::Ethereum, graph::Network::Matic],
    };
    let where_ = legacy_where(f, readers::now_ms());
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

pub fn sort_bids(rows: &mut [Bid], sort_by: Option<BidSortBy>) {
    rows.sort_by(|a, b| {
        let id = a.id.cmp(&b.id);
        match sort_by {
            Some(BidSortBy::RecentlyUpdated) => b.updated_at.cmp(&a.updated_at).then(id),
            Some(BidSortBy::MostExpensive) => b
                .price
                .parse::<u128>()
                .unwrap_or(0)
                .cmp(&a.price.parse::<u128>().unwrap_or(0))
                .then(id),
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
    readers::require_networks(state, f.network.map(|n| n.graph()))?;
    super::trades::require_ready(state).await?;
    let (mut merged, legacy_total) = legacy_leg(state, f, f.offset + limit).await?;
    let trades = trades_leg(state, f)
        .await
        .map_err(IntoResponse::into_response)?;
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
        let sql = trades_bids_sql(" AND lower(b.bidder) = $3");
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
}
