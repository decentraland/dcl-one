//! `/v1/owners`: the holders of one item's copies by `contractAddress` and
//! `itemId` (the item's blockchain id), ordered by `issuedId` (descending
//! unless `orderDirection=asc`; first ≤ 100, `first + skip` ≤
//! `readers::DEEP_WINDOW`); the total is a count capped at
//! `readers::count_cap`.
//!
//! On a node that walks the nft index the copies come from `shop.nft`
//! ([`indexed_owners`]), on one read-only snapshot on which both networks'
//! nft folds are fresh and published and the catalogue names the item.
//! Otherwise they come from the collections subgraph `nfts` reader by
//! `contractAddress` and `itemBlockchainId` (30 s; past the graph's 5,000
//! skip by keyset), which needs both collections subgraphs.

use alloy_primitives::U256;
use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};

use super::chain_walk::{self, ChainNet};
use super::graph;
use super::nft_index::Catalogue;
use super::owned_items::{self, Rule};
use super::readers::{self, GraphPage, Ttl};
use super::wire::{ApiError, DataTotal, Params};
use super::{MarketplaceState, SharedState};

pub const OWNERS_QUERY_DEFAULT_LIMIT: i64 = 20;

#[derive(Debug, Clone, Copy)]
pub enum OwnersSortBy {
    IssuedId,
}

#[derive(Debug, Clone, Default)]
pub struct OwnersFilters {
    pub contract_address: String,
    pub item_id: String,
    pub first: Option<i64>,
    pub skip: Option<i64>,
    pub sort_by: Option<OwnersSortBy>,
    pub order_direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct Owner {
    #[serde(rename = "issuedId")]
    pub issued_id: String,
    #[serde(rename = "ownerId")]
    pub owner_id: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
}

#[derive(serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TopOwner {
    address: String,
    nfts: i64,
    items: i64,
    collections: i64,
    last_acquired_at: i64,
    spent_wei: String,
}

const OWNER_FIELDS: &str = "issuedId tokenId owner { address }";

pub async fn fetch_and_count(
    state: &MarketplaceState,
    f: &OwnersFilters,
) -> Result<(Vec<Owner>, i64), Response> {
    let (first, skip) = readers::deep_page(f.first, f.skip, OWNERS_QUERY_DEFAULT_LIMIT)?;
    let direction = if f.order_direction.as_deref() == Some("asc") {
        "asc"
    } else {
        "desc"
    };
    let where_ = json!({ "contractAddress": f.contract_address.to_lowercase(), "itemBlockchainId": f.item_id });
    let mut rows = Vec::new();
    let mut total = 0;
    for network in [graph::Network::Ethereum, graph::Network::Matic] {
        let Some(url) = network.url(&state.index) else {
            continue;
        };
        let page = GraphPage {
            entity: "nfts",
            filter_type: "NFT_filter",
            fields: OWNER_FIELDS,
            where_: where_.clone(),
            order_by: "issuedId",
            direction,
            first,
            skip,
        };
        let (got, offset) =
            readers::source_rows(state, network, url, Ttl::S30, &page, first, skip, 1)
                .await
                .map_err(|e| readers::upstream_failed("/v1/owners", &e))?;
        let got: Vec<Value> = got
            .into_iter()
            .skip(crate::market::casts::page(offset))
            .take(crate::market::casts::page(first))
            .collect();
        if got.is_empty() {
            continue;
        }
        total += readers::count_to(
            state,
            network,
            url,
            "nfts",
            "NFT_filter",
            where_.clone(),
            readers::count_cap(first, skip),
        )
        .await
        .map_err(|e| readers::upstream_failed("/v1/owners", &e))?;
        rows.extend(got.iter().map(|n| {
            Owner {
                issued_id: readers::str_of(n, "issuedId"),
                owner_id: readers::at(n, "owner/address")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                token_id: readers::str_of(n, "tokenId"),
            }
        }));
        break;
    }
    Ok((rows, total))
}

pub async fn get_owners(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let p = Params::new(&pairs);
    let contract_address = p.get_string("contractAddress", None);
    let item_id = p.get_string("itemId", None);
    let order_direction = p.get_string("orderDirection", Some("desc"));
    let first = p.get_integer("first");
    let skip = p.get_integer("skip");
    let (contract_address, item_id) = match (contract_address, item_id) {
        (Some(c), Some(i)) => (c, i),
        _ => {
            return ApiError::bad_request("itemId and contractAddress are necessary params.")
                .into_response();
        }
    };
    let filters = OwnersFilters {
        contract_address,
        item_id,
        first,
        skip,
        sort_by: Some(OwnersSortBy::IssuedId),
        order_direction,
    };
    let read = if owned_items::walks_here(&state) {
        indexed_owners(&state, &filters).await
    } else {
        if let Err(r) = readers::require_both_collections(&state) {
            return r;
        }
        fetch_and_count(&state, &filters).await
    };
    match read {
        Ok((data, total)) => Json(DataTotal { data, total }).into_response(),
        Err(r) => r,
    }
}

/// The bits of a v2 token id below its item id: `item << 216 | issued`.
const V2_ISSUED_BITS: usize = 216;

/// The item a page of [`indexed_owners`] lists, as the catalogue names it,
/// in the scope `nft_reads::collection_source` reads (not the registrar; on
/// a testnet, verified on the catalogue's chain), Ethereum first as the
/// subgraph reader asks. An Ethereum (v1) item's copies carry its wearable
/// id, the suffix of its URN after its collection's (`i.urn = c.urn || ':'
/// || n.wearable_id`); a Polygon (v2) item's are the tokens its id encodes.
const ITEM_SQL: &str = "\
SELECT i.network, CASE WHEN i.network = 'ethereum' THEN substr(i.urn, length(c.urn) + 2) END AS wearable_id \
FROM shop.items i \
JOIN shop.collections c ON c.network = i.network AND c.contract_address = i.contract_address \
WHERE i.network IN ('ethereum', 'matic') AND i.contract_address = $1 AND i.item_id = $2::numeric \
  AND i.contract_address <> $3 \
  AND (i.network = 'matic' OR starts_with(i.urn, c.urn || ':')) \
  AND (NOT $4 OR (c.testnet_verified AND i.testnet_verified \
       AND c.chain_id = CASE i.network WHEN 'ethereum' THEN $5 ELSE $6 END \
       AND i.chain_id = CASE i.network WHEN 'ethereum' THEN $5 ELSE $6 END)) \
ORDER BY i.network = 'matic' \
LIMIT 1";

/// Which copies of the item a page reads.
enum Copies {
    /// A v2 item's held tokens: its token id range, the item id checked too.
    /// Within one item the token order is the issue order (the fold decodes
    /// `issued_id` from the token id), so a page walks the primary key's
    /// range instead of sorting every copy.
    Polygon { low: U256, high: U256, item: U256 },
    /// A v1 wearable's held tokens in its collection, by issued id.
    Ethereum { wearable: String },
}

type PgQuery<'q> = sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>;

impl Copies {
    /// The predicate over `shop.nft n` (the contract is `$3`, the leg's own
    /// binds follow it) and the page's order.
    fn filter(&self, descending: bool) -> (&'static str, String) {
        let dir = if descending { "DESC" } else { "ASC" };
        match self {
            Self::Polygon { .. } => (
                "n.network = 'matic' AND n.contract_address = $3 \
                 AND n.token_id BETWEEN $4::numeric AND $5::numeric AND n.item_id = $6::numeric \
                 AND n.owner IS NOT NULL",
                format!("n.token_id {dir}"),
            ),
            Self::Ethereum { .. } => (
                "n.network = 'ethereum' AND n.contract_address = $3 AND n.wearable_id = $4 \
                 AND n.owner IS NOT NULL",
                format!("n.issued_id {dir}, n.token_id {dir}"),
            ),
        }
    }

    /// The leg's own binds, after the contract.
    fn bind<'q>(&self, q: PgQuery<'q>) -> PgQuery<'q> {
        match self {
            Self::Polygon { low, high, item } => q
                .bind(low.to_string())
                .bind(high.to_string())
                .bind(item.to_string()),
            Self::Ethereum { wearable } => q.bind(wearable.clone()),
        }
    }
}

/// The copies of `item` on `contract` the catalogue names, or `None` when it
/// names no such item.
async fn copies_of(
    conn: &mut PgConnection,
    catalogue: Catalogue,
    contract: &str,
    item: U256,
) -> Result<Option<Copies>, sqlx::Error> {
    let row = sqlx::query(ITEM_SQL)
        .bind(contract)
        .bind(item.to_string())
        .bind(catalogue.registrar())
        .bind(catalogue == Catalogue::Testnet)
        .bind(catalogue.chain_id(ChainNet::Ethereum))
        .bind(catalogue.chain_id(ChainNet::Matic))
        .fetch_optional(conn)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let network: String = row.try_get("network")?;
    if network == ChainNet::Ethereum.db_name() {
        return Ok(Some(Copies::Ethereum {
            wearable: row.try_get("wearable_id")?,
        }));
    }
    // An id wider than the token id's top 40 bits encodes no token.
    if item.bit_len() > 256 - V2_ISSUED_BITS {
        return Ok(None);
    }
    let low = item << V2_ISSUED_BITS;
    let high = low | ((U256::from(1u8) << V2_ISSUED_BITS) - U256::from(1u8));
    Ok(Some(Copies::Polygon { low, high, item }))
}

/// `/v1/owners` from the nft index: on one read-only snapshot, both
/// networks' nft folds fresh ([`Rule::Fresh`]) and published (no fold write
/// a publication has not acknowledged), and the catalogue able to name the
/// item; then the item's held copies, paged and counted on it.
async fn indexed_owners(
    state: &MarketplaceState,
    f: &OwnersFilters,
) -> Result<(Vec<Owner>, i64), Response> {
    let (first, skip) = readers::deep_page(f.first, f.skip, OWNERS_QUERY_DEFAULT_LIMIT)?;
    let Ok(item) = U256::from_str_radix(&f.item_id, 10) else {
        return Err(ApiError::bad_request("itemId must be a decimal item id").into_response());
    };
    let descending = f.order_direction.as_deref() != Some("asc");
    let catalogue = state.settings.catalogue();
    let mut tx = owned_items::snapshot(&state.pool, catalogue, &ChainNet::ALL, Rule::Fresh, true)
        .await
        .map_err(readers::index_source_error)?;
    super::nft_reads::set_read_timeout(&mut tx)
        .await
        .map_err(readers::index_read_error)?;
    for net in ChainNet::ALL {
        let published =
            chain_walk::rows_published_on(&mut tx, chain_walk::INDEXER_NFT, net, &["nft"], None)
                .await
                .map_err(readers::index_read_error)?;
        if !published {
            return Err(owned_items::not_ready(&format!(
                "the {} nft index holds a fold it has not published yet",
                net.db_name()
            )));
        }
    }
    let contract = f.contract_address.to_lowercase();
    let read = async {
        let Some(copies) = copies_of(&mut tx, catalogue, &contract, item).await? else {
            return Ok((Vec::new(), 0));
        };
        let (filter, order) = copies.filter(descending);
        let page_sql = format!(
            "SELECT n.issued_id::text AS issued_id, n.owner, n.token_id::text AS token_id \
             FROM shop.nft n WHERE {filter} ORDER BY {order} LIMIT $1 OFFSET $2"
        );
        let rows = copies
            .bind(
                sqlx::query(sqlx::AssertSqlSafe(page_sql))
                    .bind(first)
                    .bind(skip)
                    .bind(&contract),
            )
            .fetch_all(&mut *tx)
            .await?;
        // The capped total, as the subgraph reader counts.
        let count_sql = format!(
            "SELECT count(*) FROM (SELECT 1 FROM shop.nft n WHERE {filter} LIMIT $1 OFFSET $2) t"
        );
        let total: i64 = copies
            .bind(
                sqlx::query(sqlx::AssertSqlSafe(count_sql))
                    .bind(readers::count_cap(first, skip))
                    .bind(0_i64)
                    .bind(&contract),
            )
            .fetch_one(&mut *tx)
            .await?
            .try_get(0)?;
        let owners = rows
            .iter()
            .map(|r| {
                Ok(Owner {
                    issued_id: r
                        .try_get::<Option<String>, _>("issued_id")?
                        .unwrap_or_default(),
                    owner_id: r.try_get("owner")?,
                    token_id: r.try_get("token_id")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        Ok::<_, sqlx::Error>((owners, total))
    };
    let answer = read.await.map_err(readers::index_read_error)?;
    tx.rollback().await.ok();
    Ok(answer)
}

pub async fn get_top_owners(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let p = Params::new(&pairs);
    let Some(creator) = p
        .get_string("creator", None)
        .filter(|s| super::is_valid_eth_address(s))
    else {
        return ApiError::bad_request("A valid creator address is required").into_response();
    };
    let sort = p
        .get_string("sortBy", None)
        .unwrap_or_else(|| "nfts".into());
    if !matches!(
        sort.as_str(),
        "nfts" | "items" | "collections" | "recent" | "spent"
    ) {
        return ApiError::bad_request(
            "sortBy must be one of nfts, items, collections, recent, spent",
        )
        .into_response();
    }
    let ascending = match p.get_string("orderDirection", None).as_deref() {
        None | Some("desc") => false,
        Some("asc") => true,
        _ => return ApiError::bad_request("orderDirection must be asc or desc").into_response(),
    };
    let first = super::casts::saturating_i64(
        p.get_number("first", None)
            .filter(|n| !n.is_nan())
            .unwrap_or(20.0),
    )
    .clamp(1, 100);
    let skip = super::casts::saturating_i64(p.get_number("skip", None).unwrap_or_default()).max(0);
    if let Err(response) = readers::check_window(first, skip) {
        return response;
    }
    match super::sale_reads::top_owners(
        &state,
        &creator.to_lowercase(),
        &sort,
        ascending,
        first,
        skip,
    )
    .await
    {
        Ok(page) => Json(page).into_response(),
        Err(response) => response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_owner_row_keeps_its_wire_names() {
        let o = Owner {
            issued_id: "3".into(),
            owner_id: "0xa".into(),
            token_id: "103".into(),
        };
        assert_eq!(
            serde_json::to_value(&o).unwrap(),
            json!({ "issuedId": "3", "ownerId": "0xa", "tokenId": "103" })
        );
    }
}
