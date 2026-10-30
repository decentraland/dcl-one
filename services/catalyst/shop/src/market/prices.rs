//! `/v1/prices`: the price histogram of what is on sale. The item leg runs
//! the v2 catalog query over the items mirror with `isOnSale`, tallies each
//! item's `COALESCE(min listing price, store price)` and memoizes the
//! histogram 5 min per filter set. The nft leg (LAND, estates, names and
//! `assetType=nft`) tallies the order price of every matching nft on sale
//! from the nft index (`nft_reads::nft_prices`, upstream's rule; the old port
//! answered `{}` for LAND and names), the envelope until it is published.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

use super::catalog::{CatalogFilters, MAX_NUMERIC_NUMBER, build_catalog_query};
use super::mirrors::require_priced;
use super::nfts::NftFilters;
use super::readers::{Ttl, cached};
use super::wire::{
    ApiError, Histogram, InvalidParameterError, Network, NftCategory, NumericKey, Params,
    consolidate,
};
use super::{MarketplaceState, SharedState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Nft,
    Item,
}

#[derive(Debug, Clone, Default)]
pub struct PriceFilters {
    pub category: Option<String>,
    pub asset_type: Option<AssetType>,
    pub is_wearable_head: bool,
    pub is_wearable_accessory: bool,
    pub is_wearable_smart: bool,
    pub wearable_category: Option<String>,
    pub wearable_genders: Vec<String>,
    pub emote_category: Option<String>,
    pub emote_genders: Vec<String>,
    pub emote_play_mode: Vec<String>,
    pub contract_addresses: Vec<String>,
    pub item_rarities: Vec<String>,
    pub network: Option<Network>,
    pub adjacent_to_road: bool,
    pub min_distance_to_plaza: Option<f64>,
    pub max_distance_to_plaza: Option<f64>,
    pub max_estate_size: Option<f64>,
    pub min_estate_size: Option<f64>,
    pub emote_has_sound: bool,
    pub emote_has_geometry: bool,
    pub emote_outcome_type: Option<String>,
}

pub type PricesResponse = Histogram;

#[derive(Debug, Serialize)]
pub struct PricesEnvelope {
    pub data: PricesResponse,
}

fn is_fetching_land(f: &PriceFilters) -> bool {
    f.adjacent_to_road
        || f.min_distance_to_plaza.is_some()
        || f.max_distance_to_plaza.is_some()
        || f.min_estate_size.is_some()
        || f.max_estate_size.is_some()
        || matches!(
            f.category.as_deref(),
            Some("land") | Some("parcel") | Some("estate")
        )
}

fn to_catalog_filters(f: &PriceFilters) -> CatalogFilters {
    CatalogFilters {
        is_wearable_head: f.is_wearable_head,
        is_wearable_accessory: f.is_wearable_accessory,
        is_wearable_smart: f.is_wearable_smart,
        wearable_category: f.wearable_category.clone(),
        wearable_genders: f.wearable_genders.clone(),
        emote_category: f.emote_category.clone(),
        emote_genders: f.emote_genders.clone(),
        emote_play_mode: f.emote_play_mode.clone(),
        contract_addresses: f.contract_addresses.clone(),
        rarities: f.item_rarities.clone(),
        network: f.network,
        emote_has_geometry: f.emote_has_geometry,
        emote_has_sound: f.emote_has_sound,
        emote_outcome_type: f.emote_outcome_type.clone(),
        category: f.category.as_deref().and_then(NftCategory::parse),
        is_on_sale: Some(true),
        first: None,
        skip: None,
        ..Default::default()
    }
}

/// The histogram SQL: every on-sale item's effective minimum price.
pub fn build_prices_query(
    f: &PriceFilters,
    now: chrono::NaiveDateTime,
    since: Option<chrono::NaiveDateTime>,
) -> (String, sqlx::postgres::PgArguments) {
    let (inner, args) = build_catalog_query(&to_catalog_filters(f), true, now, since, false);
    let sql = format!(
        "SELECT COALESCE(NULLIF(catalog.min_price_num, {MAX_NUMERIC_NUMBER}::numeric), CASE WHEN catalog.search_is_store_minter THEN catalog.price::numeric END)::text AS price FROM ({inner}) AS catalog"
    );
    (sql, args)
}

/// The nft filters of the histogram's nft leg (`nft_reads::nft_prices`):
/// the price filters as the old `fromPriceFiltersToNFTFilters` spelled them.
pub fn to_nft_filters(f: &PriceFilters) -> NftFilters {
    let category = f.category.as_deref().and_then(NftCategory::parse);
    NftFilters {
        category,
        is_land: matches!(f.category.as_deref(), Some("land")),
        is_wearable_head: f.is_wearable_head,
        is_wearable_accessory: f.is_wearable_accessory,
        is_wearable_smart: f.is_wearable_smart,
        wearable_category: f.wearable_category.clone(),
        wearable_genders: f.wearable_genders.clone(),
        emote_category: f.emote_category.clone(),
        emote_genders: f.emote_genders.clone(),
        emote_play_mode: f.emote_play_mode.clone(),
        contract_addresses: f.contract_addresses.clone(),
        item_rarities: f.item_rarities.clone(),
        network: f.network,
        adjacent_to_road: f.adjacent_to_road,
        min_distance_to_plaza: f.min_distance_to_plaza,
        max_distance_to_plaza: f.max_distance_to_plaza,
        min_estate_size: f.min_estate_size,
        max_estate_size: f.max_estate_size,
        emote_has_sound: f.emote_has_sound,
        emote_has_geometry: f.emote_has_geometry,
        emote_outcome_type: f.emote_outcome_type.clone(),
        include_social_emotes: Some(true),
        ..Default::default()
    }
}

pub async fn prices(
    state: &MarketplaceState,
    f: &PriceFilters,
) -> Result<PricesResponse, Response> {
    // Upstream (marketplace-server ae872a4 `getPricesQuery`) prices LAND,
    // estates and names from the nfts query with `isOnSale`; `assetType=nft`
    // asks the same of secondary wearables and emotes. Both read the nft
    // index (the envelope until it is published).
    if is_fetching_land(f)
        || matches!(f.category.as_deref(), Some("ens"))
        || f.asset_type == Some(AssetType::Nft)
    {
        let nf = to_nft_filters(f);
        // Checked ahead of the memo, so a held answer never outlives its sources' budgets.
        super::nft_reads::require_prices_sources(state, &nf).await?;
        let key = format!("prices-nft:{f:?}");
        let value = cached(state, Ttl::M5, key, || async {
            let h = super::nft_reads::nft_prices(state, &nf)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Value::Array(
                h.into_iter()
                    .map(|(k, v)| serde_json::json!([k.0, v]))
                    .collect(),
            ))
        })
        .await
        .map_err(|e| ApiError::internal(e).into_response())?;
        return Ok(histogram_from(&value));
    }
    // The item prices read the trade legs too (`build_catalog_query`).
    require_priced(state).await?;
    super::trades::require_ready(state).await?;
    let key = format!("prices:{f:?}");
    let value = cached(state, Ttl::M5, key, || async {
        let now = Utc::now().naive_utc();
        let since = super::trades::mirrored_since(&state.settings, now);
        let (sql, args) = build_prices_query(f, now, since);
        let rows = sqlx::query_with(sqlx::AssertSqlSafe(sql), args)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
        let prices: Vec<String> = rows
            .iter()
            .filter_map(|r| r.try_get::<Option<String>, _>("price").ok().flatten())
            .collect();
        let tally = consolidate(prices);
        Ok(Value::Array(
            tally
                .into_iter()
                .map(|(k, v)| serde_json::json!([k.0, v]))
                .collect(),
        ))
    })
    .await
    .map_err(|e| ApiError::internal(e).into_response())?;
    Ok(histogram_from(&value))
}

fn histogram_from(value: &Value) -> PricesResponse {
    let mut out: PricesResponse = BTreeMap::new();
    for pair in value.as_array().into_iter().flatten() {
        if let (Some(price), Some(count)) = (
            pair.get(0).and_then(Value::as_str),
            pair.get(1).and_then(Value::as_i64),
        ) {
            out.insert(NumericKey(price.to_string()), count);
        }
    }
    out
}

pub fn parse_filters(pairs: &[(String, String)]) -> Result<PriceFilters, InvalidParameterError> {
    let p = Params::new(pairs);
    let asset_type = p
        .get_string("assetType", None)
        .and_then(|s| match s.as_str() {
            "nft" => Some(AssetType::Nft),
            "item" => Some(AssetType::Item),
            _ => None,
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
    Ok(PriceFilters {
        category: p.get_string("category", None),
        asset_type,
        is_wearable_head: p.get_boolean("isWearableHead"),
        is_wearable_accessory: p.get_boolean("isWearableAccessory"),
        is_wearable_smart: p.get_boolean("isWearableSmart"),
        wearable_category: p.get_string("wearableCategory", None),
        wearable_genders: p.get_list("wearableGender", &[]),
        emote_category: p.get_string("emoteCategory", None),
        emote_genders: p.get_list("emoteGender", &[]),
        emote_play_mode: p.get_list("emotePlayMode", &[]),
        contract_addresses: p.get_address_list("contractAddress", true),
        item_rarities: p.get_list("itemRarity", &[]),
        network,
        adjacent_to_road: p.get_boolean("adjacentToRoad"),
        min_distance_to_plaza: p.get_number("minDistanceToPlaza", None),
        max_distance_to_plaza: p.get_number("maxDistanceToPlaza", None),
        max_estate_size: p.get_number("maxEstateSize", None),
        min_estate_size: p.get_number("minEstateSize", None),
        emote_has_sound: p.get_boolean("emoteHasSound"),
        emote_has_geometry: p.get_boolean("emoteHasGeometry"),
        emote_outcome_type: p.get_string("emoteOutcomeType", None),
    })
}

pub async fn get_prices(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let filters = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return ApiError::from(e).into_response(),
    };
    match prices(&state, &filters).await {
        Ok(data) => Json(PricesEnvelope { data }).into_response(),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consolidate_prices_tallies_and_serializes_in_numeric_order() {
        let out = consolidate(vec![
            "999".to_string(),
            "1000".to_string(),
            "999".to_string(),
            "0".to_string(),
            "100".to_string(),
        ]);
        assert_eq!(out.get(&NumericKey("999".into())), Some(&2));
        assert_eq!(out.get(&NumericKey("1000".into())), Some(&1));
        let keys: Vec<&str> = out.keys().map(|k| k.0.as_str()).collect();
        assert_eq!(keys, vec!["0", "100", "999", "1000"]);
        let s = serde_json::to_string(&PricesEnvelope { data: out }).unwrap();
        assert!(
            s.starts_with("{\"data\":{\"0\":1")
                && s.find("\"999\"").unwrap() < s.find("\"1000\"").unwrap(),
            "{s}"
        );
        assert!(
            NumericKey("999999999999999999000000000000000000".into())
                < NumericKey("1000000000000000000000000000000000000000".into())
        );
    }

    #[test]
    fn the_histogram_reads_the_on_sale_catalog_without_a_page() {
        let f = parse_filters(&[
            ("category".to_string(), "wearable".to_string()),
            ("itemRarity".to_string(), "epic".to_string()),
        ])
        .unwrap();
        let (sql, _) = build_prices_query(
            &f,
            chrono::DateTime::from_timestamp(1_700_000_000, 0)
                .unwrap()
                .naive_utc(),
            None,
        );
        assert!(
            sql.contains("AS price FROM (")
                && sql.contains("unified_trades")
                && !sql.contains("LIMIT"),
            "{sql}"
        );
        assert!(is_fetching_land(
            &parse_filters(&[("category".to_string(), "estate".to_string())]).unwrap()
        ));
        assert!(!is_fetching_land(&f));
    }
}
