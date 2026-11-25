// from: shop@94584a5c:app/src/lib/sales.ts (fetchSellerSales, fetchSalesSummary: the seller's earnings from /v1/sales), app/src/lib/payouts.ts (what upstream matched a sale's proceeds against, under proceeds-to-treasury: not carried); marketplace-server@ae872a4a:src/controllers/routes.ts:197 (GET /v1/sales, the reader these rows come from)
//! Seller proceeds from published MANA sales. The payment split records the
//! seller's net, protocol fees and creator royalties in integer base units.
//! The summary attributes royalties on resales of the creator's items and
//! reads both the payouts and item metadata in the sales snapshot.

use crate::shared::routing::get;
use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

use super::market::sales::parse_summary_filters;
use super::market::wire::{ApiError, ChainId, Network, Params};
use super::market::{MarketplaceState, SharedState};

const ROUTE_LIST: &str = "/v1/proceeds";
const ROUTE_SUMMARY: &str = "/v1/proceeds/summary";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProceedsRow {
    pub id: String,
    /// Milliseconds.
    pub timestamp: i64,
    pub network: Network,
    pub chain_id: ChainId,
    #[serde(rename = "type")]
    pub sale_type: String,
    pub contract_address: String,
    pub item_id: Option<String>,
    pub token_id: Option<String>,
    pub buyer: String,
    pub seller: String,
    pub gross_wei: String,
    pub fees_wei: String,
    pub royalties_wei: String,
    pub net_wei: String,
    pub tx_hash: String,
    pub currency: &'static str,
    pub settlement: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ProceedsFilters {
    pub seller: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub network: Option<Network>,
    pub first: Option<i64>,
    pub skip: Option<i64>,
}

pub fn parse_filters(pairs: &[(String, String)]) -> Result<ProceedsFilters, ApiError> {
    let summary = parse_summary_filters(pairs)?;
    let p = Params::new(pairs);
    let network = match p.get_string("network", None) {
        Some(raw) => Some(
            Network::parse(&raw)
                .ok_or_else(|| ApiError::bad_request("network must be ETHEREUM or MATIC"))?,
        ),
        None => None,
    };
    Ok(ProceedsFilters {
        seller: summary.seller,
        from: summary.from,
        to: summary.to,
        network,
        first: p.get_number("first", None).map(|v| v as i64),
        skip: p.get_number("skip", None).map(|v| v as i64),
    })
}

/// `GET /v1/proceeds`: newest first, merged across the networks.
pub async fn list_proceeds(
    state: &MarketplaceState,
    f: &ProceedsFilters,
) -> Result<(Vec<ProceedsRow>, i64), Response> {
    crate::market::sale_reads::proceeds_page(state, f).await
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProceedsSummary {
    pub seller: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub currency: &'static str,
    pub settlement: &'static str,
    /// Upstream's proceeds-to-treasury routing is not carried.
    pub treasury_routing: bool,
    pub sales: i64,
    pub mints: i64,
    pub resales: i64,
    pub gross_wei: String,
    pub fees_wei: String,
    pub royalties_wei: String,
    pub net_wei: String,
    /// Royalties the seller earned as a creator on other wallets' resales of their items.
    pub royalty_resales: i64,
    pub royalties_earned_wei: String,
    pub first_sale_at: Option<i64>,
    pub last_sale_at: Option<i64>,
}

pub async fn proceeds_summary(
    state: &MarketplaceState,
    f: &ProceedsFilters,
) -> Result<ProceedsSummary, Response> {
    crate::market::sale_reads::proceeds_summary(state, f).await
}

/// `GET /v1/proceeds?seller=&first=&skip=&from=&to=&network=`.
pub async fn get_proceeds(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };
    match list_proceeds(&state, &f).await {
        Ok((data, total)) => (
            [(CACHE_CONTROL, "no-store")],
            Json(json!({ "data": data, "total": total })),
        )
            .into_response(),
        Err(r) => r,
    }
}

/// `GET /v1/proceeds/summary?seller=&from=&to=`.
pub async fn get_proceeds_summary(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };
    match proceeds_summary(&state, &f).await {
        Ok(summary) => (
            [(CACHE_CONTROL, "no-store")],
            Json(json!({ "ok": true, "data": summary })),
        )
            .into_response(),
        Err(r) => r,
    }
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route(ROUTE_SUMMARY, get(get_proceeds_summary))
        .route(ROUTE_LIST, get(get_proceeds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_need_a_seller_and_a_known_network() {
        let pairs = |q: &[(&str, &str)]| {
            q.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        };
        assert!(parse_filters(&pairs(&[])).is_err());
        let f = parse_filters(&pairs(&[
            ("seller", "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            ("network", "MATIC"),
            ("from", "5"),
            ("first", "10"),
        ]))
        .unwrap();
        assert_eq!(
            (f.network, f.from, f.first, f.seller.as_str()),
            (
                Some(Network::Matic),
                Some(5),
                Some(10),
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            )
        );
        assert!(
            parse_filters(&pairs(&[
                ("seller", "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
                ("network", "MARS")
            ]))
            .is_err()
        );
    }
}
