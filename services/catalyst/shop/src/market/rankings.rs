//! Marketplace rankings and volume read published shop sales. Item rankings
//! use the items mirror for rarity and type. Lifetime creator rankings use
//! the original creators of collections. Timeframes start at midnight UTC.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use serde::Serialize;

use super::wire::{ApiError, InvalidParameterError, Params};
use super::{MarketplaceState, SharedState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingEntity {
    Wearables,
    Emotes,
    Creators,
    Collectors,
}

impl RankingEntity {
    pub fn parse_str(s: &str) -> Option<Self> {
        Some(match s {
            "wearables" => RankingEntity::Wearables,
            "emotes" => RankingEntity::Emotes,
            "creators" => RankingEntity::Creators,
            "collectors" => RankingEntity::Collectors,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingsSortBy {
    MostVolume,
    MostSales,
}

#[derive(Debug, Clone, Default)]
pub struct RankingsFilters {
    pub from: i64,
    pub first: Option<i64>,
    pub rarity: Option<String>,
    pub category: Option<String>,
    pub sort_by: Option<RankingsSortBy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticsTimeframe {
    Day,
    Week,
    Month,
    All,
}

impl AnalyticsTimeframe {
    pub fn parse_str(s: &str) -> Self {
        match s {
            "day" => AnalyticsTimeframe::Day,
            "week" => AnalyticsTimeframe::Week,
            "month" => AnalyticsTimeframe::Month,
            _ => AnalyticsTimeframe::All,
        }
    }
}

/// Midnight UTC at the start of the window as unix milliseconds; 0 for all time.
pub fn get_timestamp_from_timeframe(tf: AnalyticsTimeframe) -> i64 {
    let days = match tf {
        AnalyticsTimeframe::Day => 1,
        AnalyticsTimeframe::Week => 7,
        AnalyticsTimeframe::Month => 30,
        AnalyticsTimeframe::All => return 0,
    };
    (Utc::now() - Duration::days(days))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ItemRank {
    pub id: String,
    pub sales: i64,
    pub volume: String,
}

#[derive(Debug, Clone, serde::Deserialize, Serialize)]
pub struct CreatorRank {
    pub id: String,
    pub sales: i64,
    pub earned: String,
    #[serde(rename = "uniqueCollectors")]
    pub unique_collectors: i64,
    pub collections: i64,
}

#[derive(Debug, Clone, serde::Deserialize, Serialize)]
pub struct CollectorRank {
    pub id: String,
    pub purchases: i64,
    pub spent: String,
    #[serde(rename = "uniqueAndMythicItems")]
    pub unique_and_mythic_items: i64,
    #[serde(rename = "creatorsSupported")]
    pub creators_supported: i64,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RankingResponse {
    Items(Vec<ItemRank>),
    Creators(Vec<CreatorRank>),
    Collectors(Vec<CollectorRank>),
}

#[derive(Debug, Serialize)]
pub struct RankingsEnvelope {
    pub data: RankingResponse,
}

pub fn parse_filters(
    pairs: &[(String, String)],
    from: i64,
) -> Result<RankingsFilters, InvalidParameterError> {
    let p = Params::new(pairs);
    let sort_by = p
        .get_value("sortBy", &["most_volume", "most_sales"], None)
        .map(|s| {
            if s == "most_sales" {
                RankingsSortBy::MostSales
            } else {
                RankingsSortBy::MostVolume
            }
        });
    Ok(RankingsFilters {
        from,
        first: p.get_number("first", None).map(|v| v as i64),
        rarity: p.get_string("rarity", None),
        category: p.get_string("category", None),
        sort_by,
    })
}

/// The existing all-time response limit.
pub const LIFETIME_MAX_ROWS: i64 = 5000;

pub async fn fetch_rankings(
    state: &MarketplaceState,
    entity: RankingEntity,
    f: &RankingsFilters,
) -> Result<RankingResponse, Response> {
    match entity {
        RankingEntity::Wearables | RankingEntity::Emotes => {
            super::sale_reads::item_rankings(state, entity, f).await
        }
        RankingEntity::Collectors => super::sale_reads::collector_rankings(state, f).await,
        RankingEntity::Creators => super::sale_reads::creator_rankings(state, f).await,
    }
}

pub async fn get_rankings(
    State(state): State<SharedState>,
    Path((entity, timeframe)): Path<(String, String)>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let Some(entity) = RankingEntity::parse_str(&entity) else {
        return ApiError::bad_request(format!("Entity not supported: {}. Supported entities are: wearables, emotes, creators, collectors", entity)).into_response();
    };
    let from = get_timestamp_from_timeframe(AnalyticsTimeframe::parse_str(&timeframe));
    let filters = match parse_filters(&pairs, from) {
        Ok(f) => f,
        Err(e) => return ApiError::from(e).into_response(),
    };
    match fetch_rankings(&state, entity, &filters).await {
        Ok(data) => Json(RankingsEnvelope { data }).into_response(),
        Err(r) => r,
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct VolumeData {
    pub sales: i64,
    pub volume: String,
    #[serde(rename = "creatorsEarnings")]
    pub creators_earnings: String,
    #[serde(rename = "daoEarnings")]
    pub dao_earnings: String,
}

#[derive(Debug, Serialize)]
pub struct VolumeEnvelope {
    pub data: VolumeData,
}

pub async fn fetch_volume(
    state: &MarketplaceState,
    tf: AnalyticsTimeframe,
) -> Result<VolumeData, Response> {
    super::sale_reads::volume(state, get_timestamp_from_timeframe(tf)).await
}

pub async fn get_volume(
    State(state): State<SharedState>,
    Path(timeframe): Path<String>,
) -> Response {
    match fetch_volume(&state, AnalyticsTimeframe::parse_str(&timeframe)).await {
        Ok(data) => Json(VolumeEnvelope { data }).into_response(),
        Err(r) => r,
    }
}

/// `GET /v1/stats/{category}/{stat}` (catalyrst-market handlers/stats.rs):
/// `estate/size` tallies the sizes of the newest 100 matching estates from
/// the LAND and nft indexes (`nft_reads::estate_sizes`, the envelope until
/// both are published); any other pair is the old empty histogram. The
/// response carries the old hour of public caching and a weak ETag over the
/// data.
pub async fn get_stats(
    State(state): State<SharedState>,
    Path((category, stat)): Path<(String, String)>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let data = if category == "estate" && stat == "size" {
        let p = super::wire::Params::new(&pairs);
        let (min_price, max_price) = match (
            super::items::parse_price_param(&p, "minPrice"),
            super::items::parse_price_param(&p, "maxPrice"),
        ) {
            (Ok(min), Ok(max)) => (min, max),
            (Err(e), _) | (_, Err(e)) => return e.into_response(),
        };
        let f = super::nfts::NftFilters {
            is_on_sale: p.get_boolean("isOnSale").then_some(true),
            adjacent_to_road: p.get_boolean("adjacentToRoad"),
            min_distance_to_plaza: p.get_number("minDistanceToPlaza", None),
            max_distance_to_plaza: p.get_number("maxDistanceToPlaza", None),
            max_estate_size: p.get_number("maxEstateSize", None),
            min_estate_size: p.get_number("minEstateSize", None),
            min_price,
            max_price,
            ..Default::default()
        };
        match super::nft_reads::estate_sizes(&state, &f).await {
            Ok(d) => d,
            Err(r) => return r,
        }
    } else {
        super::wire::Histogram::new()
    };
    let body = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in body.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (
        [
            (
                axum::http::header::CACHE_CONTROL,
                "public,max-age=3600,s-maxage=3600".to_string(),
            ),
            (
                axum::http::header::CONTENT_TYPE,
                "application/json".to_string(),
            ),
            (
                axum::http::header::ETAG,
                format!("W/\"{}-{hash:x}\"", body.len()),
            ),
        ],
        format!("{{\"data\":{body}}}"),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_windows_start_at_utc_midnight() {
        assert_eq!(get_timestamp_from_timeframe(AnalyticsTimeframe::All), 0);
        assert!(get_timestamp_from_timeframe(AnalyticsTimeframe::Week) % 86_400_000 == 0);
    }
}
