//! `/v1/accounts` over the collections subgraph `accounts` reader: by
//! address (60 s) or ranked by one of the sort keys (first ≤ 100, 5 min; a
//! window past the graph's 5,000 skip read by keyset up to
//! `readers::DEEP_WINDOW`). An account's id is its address on both networks;
//! the two networks' rows are merged in memory and the total is a capped
//! count.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};

use super::graph;
use super::readers::{self, GraphPage, Ttl};
use super::wire::{DataTotal, Network, Params};
use super::{MarketplaceState, SharedState};

#[derive(Debug, Clone, Copy)]
pub enum AccountSortBy {
    MostSales,
    MostPurchases,
    MostRoyalties,
    MostCollections,
    MostEarned,
    MostSpent,
}

impl AccountSortBy {
    fn order_by(self) -> &'static str {
        match self {
            AccountSortBy::MostSales => "sales",
            AccountSortBy::MostPurchases => "purchases",
            AccountSortBy::MostRoyalties => "royalties",
            AccountSortBy::MostCollections => "collections",
            AccountSortBy::MostEarned => "earned",
            AccountSortBy::MostSpent => "spent",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AccountFilters {
    pub first: Option<i64>,
    pub skip: Option<i64>,
    pub sort_by: Option<AccountSortBy>,
    pub id: Option<String>,
    pub address: Vec<String>,
    pub network: Option<Network>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct Account {
    pub id: String,
    pub address: String,
    pub sales: i32,
    pub purchases: i32,
    pub spent: String,
    pub earned: String,
    pub royalties: String,
    pub collections: i32,
}

pub fn parse_filters(pairs: &[(String, String)]) -> AccountFilters {
    let p = Params::new(pairs);
    let sort_by = p
        .get_value(
            "sortBy",
            &[
                "most_sales",
                "most_purchases",
                "most_royalties",
                "most_collections",
                "most_earned",
                "most_spent",
            ],
            None,
        )
        .and_then(|s| match s.as_str() {
            "most_sales" => Some(AccountSortBy::MostSales),
            "most_purchases" => Some(AccountSortBy::MostPurchases),
            "most_royalties" => Some(AccountSortBy::MostRoyalties),
            "most_collections" => Some(AccountSortBy::MostCollections),
            "most_earned" => Some(AccountSortBy::MostEarned),
            "most_spent" => Some(AccountSortBy::MostSpent),
            _ => None,
        });
    AccountFilters {
        first: p.get_number("first", None).map(|f| f as i64),
        skip: p.get_number("skip", None).map(|f| f as i64),
        sort_by,
        id: p.get_string("id", None),
        address: p.get_list("address", &[]),
        network: p
            .get_value("network", &["ETHEREUM", "MATIC"], None)
            .and_then(|s| Network::parse(&s)),
    }
}

const ACCOUNT_FIELDS: &str = "id address sales purchases spent earned royalties collections";

pub fn account_from_graph(a: &Value) -> Account {
    let address = readers::str_of(a, "address");
    Account {
        id: address.clone(),
        address,
        sales: readers::int_of(a, "sales") as i32,
        purchases: readers::int_of(a, "purchases") as i32,
        spent: readers::str_of(a, "spent"),
        earned: readers::str_of(a, "earned"),
        royalties: readers::str_of(a, "royalties"),
        collections: readers::int_of(a, "collections") as i32,
    }
}

fn sort_key(a: &Account, sort_by: Option<AccountSortBy>) -> u128 {
    let num = |s: &str| s.parse::<u128>().unwrap_or(0);
    match sort_by {
        Some(AccountSortBy::MostSales) => a.sales as u128,
        Some(AccountSortBy::MostPurchases) => a.purchases as u128,
        Some(AccountSortBy::MostRoyalties) => num(&a.royalties),
        Some(AccountSortBy::MostCollections) => a.collections as u128,
        Some(AccountSortBy::MostSpent) => num(&a.spent),
        _ => num(&a.earned),
    }
}

pub async fn list_accounts(
    state: &MarketplaceState,
    f: &AccountFilters,
) -> Result<(Vec<Account>, i64), Response> {
    let mut addresses: Vec<String> = f.address.iter().map(|a| a.to_lowercase()).collect();
    if let Some(id) = &f.id {
        addresses.push(id.split('-').next().unwrap_or(id).to_lowercase());
    }
    let by_address = !addresses.is_empty();
    let (first, skip) = readers::deep_page(f.first, f.skip, 100)?;
    let networks: Vec<(graph::Network, &str)> = match f.network {
        Some(n) => vec![(n.graph(), readers::require_collections(state, n.graph())?)],
        None => readers::require_both_collections(state)?,
    };
    let where_ = if by_address {
        json!({ "address_in": addresses })
    } else {
        json!({})
    };
    let order_by = f.sort_by.unwrap_or(AccountSortBy::MostEarned).order_by();
    let ttl = if by_address { Ttl::S60 } else { Ttl::M5 };
    let mut rows: Vec<Account> = Vec::new();
    let mut total = 0;
    let mut offset = 0;
    let count = networks.len();
    for (network, url) in networks {
        let page = GraphPage {
            entity: "accounts",
            filter_type: "Account_filter",
            fields: ACCOUNT_FIELDS,
            where_: where_.clone(),
            order_by,
            direction: "desc",
            first,
            skip,
        };
        let (got, skip_after) =
            readers::source_rows(state, network, url, ttl, &page, first, skip, count)
                .await
                .map_err(|e| readers::upstream_failed("/v1/accounts", &e))?;
        offset = skip_after;
        total += if by_address {
            got.len() as i64
        } else {
            readers::count_to(
                state,
                network,
                url,
                "accounts",
                "Account_filter",
                where_.clone(),
                readers::count_cap(first, skip),
            )
            .await
            .map_err(|e| readers::upstream_failed("/v1/accounts", &e))?
        };
        rows.extend(got.iter().map(account_from_graph));
    }
    rows.sort_by(|a, b| {
        sort_key(b, f.sort_by)
            .cmp(&sort_key(a, f.sort_by))
            .then(a.address.cmp(&b.address))
    });
    Ok((
        rows.into_iter()
            .skip(offset as usize)
            .take(first as usize)
            .collect(),
        total,
    ))
}

pub async fn get_accounts(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    match list_accounts(&state, &parse_filters(&pairs)).await {
        Ok((data, total)) => Json(DataTotal { data, total }).into_response(),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graph_account_keeps_the_wire_shape_and_ranks_by_the_sort_key() {
        let a = account_from_graph(
            &json!({ "id": "0xa", "address": "0xa", "sales": 1, "purchases": 0, "spent": "0", "earned": "30", "royalties": "0", "collections": 1 }),
        );
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            json!({ "id": "0xa", "address": "0xa", "sales": 1, "purchases": 0, "spent": "0", "earned": "30", "royalties": "0", "collections": 1 })
        );
        let b = Account {
            address: "0xb".into(),
            id: "0xb".into(),
            earned: "9".into(),
            sales: 5,
            ..a.clone()
        };
        assert!(sort_key(&a, None) > sort_key(&b, None), "earned by default");
        assert!(
            sort_key(&b, Some(AccountSortBy::MostSales))
                > sort_key(&a, Some(AccountSortBy::MostSales))
        );
        let f = parse_filters(&[
            ("sortBy".to_string(), "most_spent".to_string()),
            ("address".to_string(), "0xA".to_string()),
        ]);
        assert!(matches!(f.sort_by, Some(AccountSortBy::MostSpent)) && f.address == vec!["0xA"]);
    }
}
