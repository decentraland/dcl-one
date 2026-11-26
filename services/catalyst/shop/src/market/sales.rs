//! Published MANA sales and seller summaries. Each request reads the required
//! networks, its results and its total in one marketplace snapshot.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::wire::{ApiError, ChainId, DataOnly, DataTotal, InvalidParameterError, Network, Params};
use super::{MarketplaceState, SharedState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaleSortBy {
    MostExpensive,
    RecentlySold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaleType {
    Order,
    Bid,
    Mint,
}

impl SaleType {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            SaleType::Order => "order",
            SaleType::Bid => "bid",
            SaleType::Mint => "mint",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SaleFilters {
    pub first: Option<i64>,
    pub skip: Option<i64>,
    pub sort_by: Option<SaleSortBy>,
    pub sale_type: Option<SaleType>,
    pub categories: Vec<String>,
    pub seller: Option<String>,
    pub buyer: Option<String>,
    pub contract_address: Option<String>,
    pub token_id: Option<String>,
    pub item_id: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub min_price: Option<String>,
    pub max_price: Option<String>,
    pub network: Option<Network>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct Sale {
    pub id: String,
    #[serde(rename = "itemId")]
    pub item_id: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    pub buyer: String,
    #[serde(rename = "chainId")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub chain_id: ChainId,
    pub network: Network,
    pub price: String,
    pub seller: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub timestamp: i64,
    #[serde(rename = "tokenId")]
    pub token_id: Option<String>,
    #[serde(rename = "txHash")]
    pub tx_hash: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub sale_type: String,
}

pub fn parse_filters(pairs: &[(String, String)]) -> Result<SaleFilters, InvalidParameterError> {
    let p = Params::new(pairs);
    let sort_by = p
        .get_value("sortBy", &["most_expensive", "recently_sold"], None)
        .map(|s| {
            if s == "most_expensive" {
                SaleSortBy::MostExpensive
            } else {
                SaleSortBy::RecentlySold
            }
        });
    let sale_type = p
        .get_value("type", &["order", "bid", "mint"], None)
        .map(|s| match s.as_str() {
            "bid" => SaleType::Bid,
            "mint" => SaleType::Mint,
            _ => SaleType::Order,
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
    Ok(SaleFilters {
        first: p.get_number("first", None).map(|v| v as i64),
        skip: p.get_number("skip", None).map(|v| v as i64),
        sort_by,
        sale_type,
        categories: p.get_list(
            "category",
            &["parcel", "estate", "wearable", "ens", "emote"],
        ),
        seller: p.get_address("seller", true, None),
        buyer: p.get_address("buyer", true, None),
        contract_address: p.get_address("contractAddress", true, None),
        token_id: p.get_string("tokenId", None),
        item_id: p.get_string("itemId", None),
        from: p.get_number("from", None).map(|v| v as i64),
        to: p.get_number("to", None).map(|v| v as i64),
        min_price: p.get_string("minPrice", None),
        max_price: p.get_string("maxPrice", None),
        network,
    })
}

pub async fn list_sales(
    state: &MarketplaceState,
    f: &SaleFilters,
) -> Result<(Vec<Sale>, i64), Response> {
    super::sale_reads::list(state, f).await
}

pub async fn get_sales(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let filters = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return ApiError::from(e).into_response(),
    };
    match list_sales(&state, &filters).await {
        Ok((data, total)) => Json(DataTotal { data, total }).into_response(),
        Err(r) => r,
    }
}

#[derive(Debug, Clone, Default)]
pub struct SalesSummaryFilters {
    pub seller: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct SalesSummaryCollection {
    pub contract_address: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sold: i64,
    pub earned_wei: String,
    pub earned_usd: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub unpriced_sales: i64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct SalesSummaryItem {
    pub contract_address: String,
    pub item_id: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sold_lifetime: i64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct SalesSummaryRoyalties {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub resales: i64,
    pub volume_wei: String,
    pub royalties_wei: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct SalesSummary {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub mints: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub resales: i64,
    pub earned_wei: String,
    pub earned_usd: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub unpriced_sales: i64,
    pub by_collection: Vec<SalesSummaryCollection>,
    pub by_item: Vec<SalesSummaryItem>,
    pub royalties: SalesSummaryRoyalties,
}

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
pub fn parse_summary_filters(pairs: &[(String, String)]) -> Result<SalesSummaryFilters, ApiError> {
    let p = Params::new(pairs);
    let seller = p
        .get_address("seller", true, None)
        .ok_or_else(|| ApiError::bad_request("A valid seller address is required"))?;
    let mut bounds: [Option<i64>; 2] = [None, None];
    for (slot, key) in ["from", "to"].iter().enumerate() {
        if let Some(raw) = p.get_string(key, None) {
            let parsed = raw
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= 0 && *v <= MAX_SAFE_INTEGER)
                .filter(|_| raw.chars().all(|c| c.is_ascii_digit()) && !raw.is_empty());
            bounds[slot] = Some(parsed.ok_or_else(|| {
                ApiError::bad_request(format!("{key} must be an epoch timestamp in milliseconds"))
            })?);
        }
    }
    if let (Some(from), Some(to)) = (bounds[0], bounds[1])
        && from > to
    {
        return Err(ApiError::bad_request(
            "from must be less than or equal to to",
        ));
    }
    Ok(SalesSummaryFilters {
        seller,
        from: bounds[0],
        to: bounds[1],
    })
}

pub async fn sales_summary(
    state: &MarketplaceState,
    f: &SalesSummaryFilters,
) -> Result<SalesSummary, Response> {
    let _slot = super::sale_reads::seller_slot(state).await?;
    let (mut snapshot, nets) = super::sale_reads::snapshot(state, None).await?;
    Ok(super::sale_reads::seller_stats(&mut snapshot, nets, f)
        .await?
        .summary)
}

pub async fn get_sales_summary(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let filters = match parse_summary_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };
    match sales_summary(&state, &filters).await {
        Ok(data) => Json(DataOnly { data }).into_response(),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(raw: &[(&str, &str)]) -> Vec<(String, String)> {
        raw.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn message(pairs: &[(String, String)]) -> String {
        match parse_summary_filters(pairs) {
            Err(e) => e.message(),
            Ok(_) => panic!("expected a rejection"),
        }
    }

    #[test]
    fn a_summary_needs_a_seller_that_is_really_an_address() {
        assert_eq!(message(&pairs(&[])), "A valid seller address is required");
        assert_eq!(
            message(&pairs(&[("seller", "vitalik.eth")])),
            "A valid seller address is required"
        );
        let filters = parse_summary_filters(&pairs(&[(
            "seller",
            "0xABCDEF0123456789012345678901234567890123",
        )]))
        .unwrap();
        assert_eq!(filters.seller, "0xabcdef0123456789012345678901234567890123");
        assert_eq!((filters.from, filters.to), (None, None));
    }

    #[test]
    fn a_bound_that_is_not_an_epoch_millisecond_names_itself() {
        for (key, value) in [
            ("from", "yesterday"),
            ("to", "-1"),
            ("from", "1.5"),
            ("to", "9007199254740992"),
            ("from", ""),
        ] {
            assert_eq!(
                message(&pairs(&[
                    ("seller", "0x1111111111111111111111111111111111111111"),
                    (key, value)
                ])),
                format!("{key} must be an epoch timestamp in milliseconds"),
                "{key}={value}"
            );
        }
        assert_eq!(
            message(&pairs(&[
                ("seller", "0x1111111111111111111111111111111111111111"),
                ("from", "200"),
                ("to", "100")
            ])),
            "from must be less than or equal to to"
        );
        let equal = parse_summary_filters(&pairs(&[
            ("seller", "0x1111111111111111111111111111111111111111"),
            ("from", "100"),
            ("to", "100"),
        ]))
        .unwrap();
        assert_eq!((equal.from, equal.to), (Some(100), Some(100)));
    }

    #[test]
    fn sales_filters_keep_milliseconds_and_normalize_the_seller() {
        let filters = parse_filters(&pairs(&[
            ("type", "mint"),
            ("from", "1700000000000"),
            ("category", "wearable"),
            ("seller", "0xABCDEF0123456789012345678901234567890123"),
        ]))
        .unwrap();
        assert_eq!(filters.sale_type, Some(SaleType::Mint));
        assert_eq!(filters.from, Some(1_700_000_000_000));
        assert_eq!(filters.categories, ["wearable"]);
        assert_eq!(
            filters.seller.as_deref(),
            Some("0xabcdef0123456789012345678901234567890123")
        );
    }
}
