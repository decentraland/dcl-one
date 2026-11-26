//! Published per-network marketplace account totals and inventory.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::sale_reads;
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
    pub(super) fn order_by(self) -> &'static str {
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

pub async fn list_accounts(
    state: &MarketplaceState,
    f: &AccountFilters,
) -> Result<(Vec<Account>, i64), Response> {
    sale_reads::accounts(state, f).await
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
    fn sort_and_address_filters_are_preserved() {
        let f = parse_filters(&[
            ("sortBy".to_string(), "most_spent".to_string()),
            ("address".to_string(), "0xA".to_string()),
        ]);
        assert!(matches!(f.sort_by, Some(AccountSortBy::MostSpent)) && f.address == vec!["0xA"]);
    }
}
