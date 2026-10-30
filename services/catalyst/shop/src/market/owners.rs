//! `/v1/owners`: the holders of one item, from the collections subgraph
//! `nfts` reader by `contractAddress` and `itemBlockchainId`, ordered by
//! `issuedId` (first ≤ 100, 30 s; past the graph's 5,000 skip by keyset up
//! to `readers::DEEP_WINDOW`); the total is a capped count.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};

use super::graph;
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
            .skip(offset as usize)
            .take(first as usize)
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
    let first = p.get_number("first", None).map(|f| f as i64);
    let skip = p.get_number("skip", None).map(|f| f as i64);
    let (contract_address, item_id) = match (contract_address, item_id) {
        (Some(c), Some(i)) => (c, i),
        _ => {
            return ApiError::bad_request("itemId and contractAddress are necessary params.")
                .into_response();
        }
    };
    if let Err(r) = readers::require_both_collections(&state) {
        return r;
    }
    let filters = OwnersFilters {
        contract_address,
        item_id,
        first,
        skip,
        sort_by: Some(OwnersSortBy::IssuedId),
        order_direction,
    };
    match fetch_and_count(&state, &filters).await {
        Ok((data, total)) => Json(DataTotal { data, total }).into_response(),
        Err(r) => r,
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
