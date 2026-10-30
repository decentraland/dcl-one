//! `/v1/rankings/{entity}/{timeframe}` over the collections subgraph day
//! rollups (`itemsDayDatas`, `accountsDayDatas`) for `day`, `week` and
//! `month` (15 min; at most 5 pages of 1000 rows per network, so a busy
//! month is ranked on its most recent rollups) and `/v1/volume/{timeframe}`
//! over `analyticsDayDatas` (≤ 366 rows, 15 min). The `all` timeframe reads
//! the lifetime totals the old one read from the squid's copy of the same
//! subgraph entities: `items` (`sales`, `volume`) for wearables and emotes,
//! `accounts` for creators (`sales_gt: 0, collections_gt: 0`, ordered by
//! `primarySales`, `primarySalesEarned` or `earned` as before) and
//! collectors (`purchases_gt: 0`), in the old database order, at most 5
//! pages of 1000 per network (the old query had no bound), merged per
//! address across the networks; `/v1/volume/all` sums every
//! `analyticsDayDatas` row (id keyset, at most 10 pages per network).
//! `/v1/stats/{category}/{stat}` is below.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};

use super::graph;
use super::readers::{self, Ttl, cached};
use super::wire::{ApiError, InvalidParameterError, Params, bn_add, bn_cmp};
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

#[derive(Debug, Clone, Serialize)]
pub struct ItemRank {
    pub id: String,
    pub sales: i64,
    pub volume: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatorRank {
    pub id: String,
    pub sales: i64,
    pub earned: String,
    #[serde(rename = "uniqueCollectors")]
    pub unique_collectors: i64,
    pub collections: i64,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone)]
pub struct ItemsDayDataFragment {
    pub id: String,
    pub sales: i64,
    pub volume: String,
}

#[derive(Debug, Clone)]
pub struct CreatorsDayDataFragment {
    pub id: String,
    pub sales: i64,
    pub earned: String,
    pub unique_collections_sales: i64,
    pub unique_collectors_total: i64,
}

#[derive(Debug, Clone)]
pub struct CollectorsDayDataFragment {
    pub id: String,
    pub purchases: i64,
    pub spent: String,
    pub unique_and_mythic_items: i64,
    pub creators_supported_total: i64,
}

/// A day rollup id is `<day>-<entity id>`; the entity id is what ranks.
fn strip_day(id: &str, from: i64) -> String {
    if id.contains('-') && from != 0 {
        id[id.find('-').unwrap_or(0) + 1..].to_string()
    } else {
        id.to_string()
    }
}

pub fn get_unique_items_from_items_day_data(
    fragments: Vec<ItemsDayDataFragment>,
    from: i64,
) -> Vec<ItemRank> {
    let mut acc: HashMap<String, ItemRank> = HashMap::new();
    for f in fragments {
        let item_id = strip_day(&f.id, from);
        let entry = acc.entry(item_id.clone()).or_insert_with(|| ItemRank {
            id: item_id.clone(),
            sales: 0,
            volume: "0".to_string(),
        });
        entry.sales += f.sales;
        entry.volume = bn_add(&entry.volume, &f.volume);
    }
    acc.into_values().collect()
}

pub fn get_unique_creators_from_creators_day_data(
    fragments: Vec<CreatorsDayDataFragment>,
) -> Vec<CreatorRank> {
    let mut acc: HashMap<String, CreatorsDayDataFragment> = HashMap::new();
    for f in fragments {
        let address =
            f.id.find('-')
                .map(|i| f.id[i + 1..].to_string())
                .unwrap_or_else(|| f.id.clone());
        let entry = acc
            .entry(address.clone())
            .or_insert_with(|| CreatorsDayDataFragment {
                id: address.clone(),
                sales: 0,
                earned: "0".to_string(),
                unique_collections_sales: 0,
                unique_collectors_total: 0,
            });
        entry.sales += f.sales;
        entry.earned = bn_add(&entry.earned, &f.earned);
        entry.unique_collections_sales = entry
            .unique_collections_sales
            .max(f.unique_collections_sales);
        entry.unique_collectors_total =
            entry.unique_collectors_total.max(f.unique_collectors_total);
    }
    acc.into_values()
        .map(|f| CreatorRank {
            id: f.id,
            sales: f.sales,
            earned: f.earned,
            unique_collectors: f.unique_collectors_total,
            collections: f.unique_collections_sales,
        })
        .collect()
}

pub fn get_unique_collectors_from_collectors_day_data(
    fragments: Vec<CollectorsDayDataFragment>,
) -> Vec<CollectorRank> {
    let mut acc: HashMap<String, CollectorsDayDataFragment> = HashMap::new();
    for f in fragments {
        let address =
            f.id.find('-')
                .map(|i| f.id[i + 1..].to_string())
                .unwrap_or_else(|| f.id.clone());
        let entry = acc
            .entry(address.clone())
            .or_insert_with(|| CollectorsDayDataFragment {
                id: address.clone(),
                purchases: 0,
                spent: "0".to_string(),
                unique_and_mythic_items: 0,
                creators_supported_total: 0,
            });
        entry.purchases += f.purchases;
        entry.spent = bn_add(&entry.spent, &f.spent);
        entry.unique_and_mythic_items =
            entry.unique_and_mythic_items.max(f.unique_and_mythic_items);
        entry.creators_supported_total = entry
            .creators_supported_total
            .max(f.creators_supported_total);
    }
    acc.into_values()
        .map(|f| CollectorRank {
            id: f.id,
            purchases: f.purchases,
            spent: f.spent,
            unique_and_mythic_items: f.unique_and_mythic_items,
            creators_supported: f.creators_supported_total,
        })
        .collect()
}

fn sort_items(ranks: &mut [ItemRank], sort_by: Option<RankingsSortBy>) {
    match sort_by {
        Some(RankingsSortBy::MostSales) => ranks.sort_by(|a, b| {
            b.sales
                .cmp(&a.sales)
                .then_with(|| bn_cmp(&b.volume, &a.volume))
                .then_with(|| a.id.cmp(&b.id))
        }),
        _ => ranks.sort_by(|a, b| {
            bn_cmp(&b.volume, &a.volume)
                .then_with(|| b.sales.cmp(&a.sales))
                .then_with(|| a.id.cmp(&b.id))
        }),
    }
}

fn sort_creators(ranks: &mut [CreatorRank], sort_by: Option<RankingsSortBy>) {
    match sort_by {
        Some(RankingsSortBy::MostSales) => {
            ranks.sort_by(|a, b| b.sales.cmp(&a.sales).then_with(|| a.id.cmp(&b.id)))
        }
        _ => ranks.sort_by(|a, b| bn_cmp(&b.earned, &a.earned).then_with(|| a.id.cmp(&b.id))),
    }
}

fn sort_collectors(ranks: &mut [CollectorRank], sort_by: Option<RankingsSortBy>) {
    match sort_by {
        Some(RankingsSortBy::MostSales) => {
            ranks.sort_by(|a, b| b.purchases.cmp(&a.purchases).then_with(|| a.id.cmp(&b.id)))
        }
        _ => ranks.sort_by(|a, b| bn_cmp(&b.spent, &a.spent).then_with(|| a.id.cmp(&b.id))),
    }
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

/// The pages a day-data walk may take per network: 5000 rows.
pub const DAY_DATA_MAX_PAGES: usize = 5;

/// The day-rollup filter: `date >= from`, the old crate's `AND date >=
/// {from/1000}`. A rollup is dated at its day's midnight and `from` is a
/// midnight, so `date_gt` dropped the window's first day: `/rankings/*/day`
/// answered `[]` and `/volume/day` 0 while the source had sales.
pub(crate) fn day_data_where(mut where_: Value, from_seconds: i64) -> Value {
    where_["date_gte"] = json!(from_seconds);
    where_
}

pub(crate) const VOLUME_QUERY: &str = "query Volume($first: Int!, $from: Int!) { analyticsDayDatas(first: $first, orderBy: date, orderDirection: desc, where: { date_gte: $from }) { id date sales volume creatorsEarnings daoEarnings } }";

/// Every `entity` row with `date >= from` on every configured network, id
/// keyset, at most [`DAY_DATA_MAX_PAGES`] pages per network; 15 min.
async fn day_datas(
    state: &MarketplaceState,
    entity: &'static str,
    filter_type: &'static str,
    fields: &'static str,
    where_: Value,
    from_seconds: i64,
    route: &'static str,
) -> Result<Vec<Value>, Response> {
    let where_ = day_data_where(where_, from_seconds);
    let mut out = Vec::new();
    for (network, url) in readers::require_both_collections(state)? {
        let key = format!("daydata:{}:{entity}:{where_}", network.db_name());
        let document = format!(
            "query DayData($first: Int!, $where: {filter_type}) {{ {entity}(first: $first, orderBy: id, orderDirection: asc, where: $where) {{ {fields} }} }}"
        );
        let value = cached(state, Ttl::M15, key, || async {
            let mut rows = Vec::new();
            let mut last_id: Option<String> = None;
            for _ in 0..DAY_DATA_MAX_PAGES {
                let mut w = where_.clone();
                if let Some(id) = &last_id {
                    w["id_gt"] = Value::String(id.clone());
                }
                let data = readers::query(
                    state,
                    url,
                    &document,
                    json!({ "first": graph::THE_GRAPH_PAGE_SIZE, "where": w }),
                )
                .await?;
                let page = readers::rows(&data, entity);
                let n = page.len() as i64;
                last_id = page.last().map(|r| readers::str_of(r, "id"));
                rows.extend(page);
                if n < graph::THE_GRAPH_PAGE_SIZE {
                    break;
                }
            }
            Ok(Value::Array(rows))
        })
        .await
        .map_err(|e| readers::upstream_failed(route, &e))?;
        out.extend(value.as_array().cloned().unwrap_or_default());
    }
    Ok(out)
}

/// The rows an all-time ranking reads per network: 5 pages of 1000.
pub const LIFETIME_MAX_ROWS: i64 = 5 * graph::THE_GRAPH_PAGE_SIZE;

/// The newest-first lifetime rows of `entity` on every configured network,
/// in `order_by` descending, `want` rows at most per network (skip pages of
/// 1000 inside The Graph's 5000 skip window); 15 min.
#[allow(clippy::too_many_arguments)]
async fn lifetime_rows(
    state: &MarketplaceState,
    entity: &'static str,
    filter_type: &'static str,
    fields: &'static str,
    where_: Value,
    order_by: &'static str,
    want: i64,
    route: &'static str,
) -> Result<Vec<Vec<Value>>, Response> {
    let want = want.clamp(0, LIFETIME_MAX_ROWS);
    let document = format!(
        "query Lifetime($first: Int!, $skip: Int!, $where: {filter_type}) {{ {entity}(first: $first, skip: $skip, orderBy: {order_by}, orderDirection: desc, where: $where) {{ {fields} }} }}"
    );
    let mut out = Vec::new();
    for (network, url) in readers::require_both_collections(state)? {
        let key = format!(
            "lifetime:{}:{entity}:{order_by}:{want}:{where_}",
            network.db_name()
        );
        let value = cached(state, Ttl::M15, key, || async {
            let mut rows = Vec::new();
            while (rows.len() as i64) < want {
                let first = (want - rows.len() as i64).min(graph::THE_GRAPH_PAGE_SIZE);
                let data = readers::query(
                    state,
                    url,
                    &document,
                    json!({ "first": first, "skip": rows.len(), "where": where_ }),
                )
                .await?;
                let page = readers::rows(&data, entity);
                let n = page.len() as i64;
                rows.extend(page);
                if n < first {
                    break;
                }
            }
            Ok(Value::Array(rows))
        })
        .await
        .map_err(|e| readers::upstream_failed(route, &e))?;
        out.push(value.as_array().cloned().unwrap_or_default());
    }
    Ok(out)
}

/// The old all-time order: the database's `ORDER BY <key> DESC` per network
/// (no re-sort); across networks, the merged totals by the same key.
fn merge_in_order<T, K: Ord>(
    per_network: Vec<Vec<T>>,
    id: impl Fn(&T) -> String,
    merge: impl Fn(&mut T, T),
    key: impl Fn(&T) -> K,
) -> Vec<T> {
    let mut order: Vec<String> = Vec::new();
    let mut acc: HashMap<String, T> = HashMap::new();
    let networks = per_network.len();
    for rows in per_network {
        for row in rows {
            let k = id(&row);
            match acc.get_mut(&k) {
                Some(existing) => merge(existing, row),
                None => {
                    order.push(k.clone());
                    acc.insert(k, row);
                }
            }
        }
    }
    let mut merged: Vec<T> = order.into_iter().filter_map(|k| acc.remove(&k)).collect();
    if networks > 1 {
        merged.sort_by_key(|row| std::cmp::Reverse(key(row)));
    }
    merged
}

/// A big-number string as a sortable key: longer is larger, then lexical.
fn bn_key(s: &str) -> (usize, String) {
    let t = s.trim_start_matches('0');
    (t.len(), t.to_string())
}

async fn all_time_rankings(
    state: &MarketplaceState,
    entity: RankingEntity,
    f: &RankingsFilters,
) -> Result<RankingResponse, Response> {
    let want = f
        .first
        .unwrap_or(LIFETIME_MAX_ROWS)
        .clamp(0, LIFETIME_MAX_ROWS);
    let route = "/v1/rankings";
    let truncate = |n: usize| {
        f.first
            .map(|first| first.max(0) as usize)
            .unwrap_or(n)
            .min(n)
    };
    match entity {
        RankingEntity::Wearables | RankingEntity::Emotes => {
            let mut where_ = json!({ "sales_gt": 0 });
            match entity {
                RankingEntity::Wearables => where_["searchWearableCategory_not"] = Value::Null,
                _ => where_["searchEmoteCategory_not"] = Value::Null,
            }
            if let Some(r) = &f.rarity {
                where_["rarity"] = json!(r);
            }
            let order_by = if f.sort_by == Some(RankingsSortBy::MostSales) {
                "sales"
            } else {
                "volume"
            };
            let per_network = lifetime_rows(
                state,
                "items",
                "Item_filter",
                "id sales volume",
                where_,
                order_by,
                want,
                route,
            )
            .await?;
            let rows = per_network
                .into_iter()
                .map(|rows| {
                    rows.iter()
                        .map(|r| ItemRank {
                            id: readers::str_of(r, "id"),
                            sales: readers::int_of(r, "sales"),
                            volume: readers::str_of(r, "volume"),
                        })
                        .collect()
                })
                .collect();
            let mut ranks = merge_in_order(
                rows,
                |r: &ItemRank| r.id.clone(),
                |a, b| {
                    a.sales += b.sales;
                    a.volume = bn_add(&a.volume, &b.volume);
                },
                |r| {
                    if order_by == "sales" {
                        (0, r.sales, (0, String::new()))
                    } else {
                        (0, 0, bn_key(&r.volume))
                    }
                },
            );
            ranks.truncate(truncate(ranks.len()));
            Ok(RankingResponse::Items(ranks))
        }
        RankingEntity::Creators => {
            let order_by = match f.sort_by {
                Some(RankingsSortBy::MostSales) => "primarySales",
                Some(RankingsSortBy::MostVolume) => "primarySalesEarned",
                None => "earned",
            };
            let per_network = lifetime_rows(
                state,
                "accounts",
                "Account_filter",
                "id sales earned primarySales primarySalesEarned collections uniqueCollectorsTotal",
                json!({ "sales_gt": 0, "collections_gt": 0 }),
                order_by,
                want,
                route,
            )
            .await?;
            let rows = per_network
                .into_iter()
                .map(|rows| {
                    rows.iter()
                        .map(|r| {
                            (
                                CreatorRank {
                                    id: readers::str_of(r, "id"),
                                    sales: readers::int_of(r, "sales"),
                                    earned: readers::str_of(r, "earned"),
                                    unique_collectors: readers::int_of(r, "uniqueCollectorsTotal"),
                                    collections: readers::int_of(r, "collections"),
                                },
                                readers::int_of(r, "primarySales"),
                                readers::str_of(r, "primarySalesEarned"),
                            )
                        })
                        .collect()
                })
                .collect();
            let mut ranks = merge_in_order(
                rows,
                |(r, _, _): &(CreatorRank, i64, String)| r.id.clone(),
                |a, b| {
                    a.0.sales += b.0.sales;
                    a.0.earned = bn_add(&a.0.earned, &b.0.earned);
                    a.0.unique_collectors = a.0.unique_collectors.max(b.0.unique_collectors);
                    a.0.collections += b.0.collections;
                    a.1 += b.1;
                    a.2 = bn_add(&a.2, &b.2);
                },
                |(r, primary_sales, primary_earned)| match order_by {
                    "primarySales" => (*primary_sales, (0, String::new())),
                    "primarySalesEarned" => (0, bn_key(primary_earned)),
                    _ => (0, bn_key(&r.earned)),
                },
            );
            ranks.truncate(truncate(ranks.len()));
            Ok(RankingResponse::Creators(
                ranks.into_iter().map(|(r, _, _)| r).collect(),
            ))
        }
        RankingEntity::Collectors => {
            let order_by = if f.sort_by == Some(RankingsSortBy::MostSales) {
                "purchases"
            } else {
                "spent"
            };
            let per_network = lifetime_rows(
                state,
                "accounts",
                "Account_filter",
                "id purchases spent uniqueAndMythicItemsTotal creatorsSupportedTotal",
                json!({ "purchases_gt": 0 }),
                order_by,
                want,
                route,
            )
            .await?;
            let rows = per_network
                .into_iter()
                .map(|rows| {
                    rows.iter()
                        .map(|r| CollectorRank {
                            id: readers::str_of(r, "id"),
                            purchases: readers::int_of(r, "purchases"),
                            spent: readers::str_of(r, "spent"),
                            unique_and_mythic_items: readers::int_of(
                                r,
                                "uniqueAndMythicItemsTotal",
                            ),
                            creators_supported: readers::int_of(r, "creatorsSupportedTotal"),
                        })
                        .collect()
                })
                .collect();
            let mut ranks = merge_in_order(
                rows,
                |r: &CollectorRank| r.id.clone(),
                |a, b| {
                    a.purchases += b.purchases;
                    a.spent = bn_add(&a.spent, &b.spent);
                    a.unique_and_mythic_items += b.unique_and_mythic_items;
                    a.creators_supported = a.creators_supported.max(b.creators_supported);
                },
                |r| {
                    if order_by == "purchases" {
                        (r.purchases, (0, String::new()))
                    } else {
                        (0, bn_key(&r.spent))
                    }
                },
            );
            ranks.truncate(truncate(ranks.len()));
            Ok(RankingResponse::Collectors(ranks))
        }
    }
}

pub async fn fetch_rankings(
    state: &MarketplaceState,
    entity: RankingEntity,
    f: &RankingsFilters,
) -> Result<RankingResponse, Response> {
    if f.from == 0 {
        return all_time_rankings(state, entity, f).await;
    }
    let from_seconds = f.from / 1000;
    match entity {
        RankingEntity::Wearables | RankingEntity::Emotes => {
            let mut where_ = json!({});
            match entity {
                RankingEntity::Wearables => where_["searchWearableCategory_not"] = Value::Null,
                _ => where_["searchEmoteCategory_not"] = Value::Null,
            }
            if let Some(r) = &f.rarity {
                where_["searchRarity"] = json!(r);
            }
            let rows = day_datas(
                state,
                "itemsDayDatas",
                "ItemsDayData_filter",
                "id sales volume",
                where_,
                from_seconds,
                "/v1/rankings",
            )
            .await?;
            let fragments = rows
                .iter()
                .map(|r| ItemsDayDataFragment {
                    id: readers::str_of(r, "id"),
                    sales: readers::int_of(r, "sales"),
                    volume: readers::str_of(r, "volume"),
                })
                .collect();
            let mut ranks = get_unique_items_from_items_day_data(fragments, f.from);
            sort_items(&mut ranks, f.sort_by);
            if let Some(first) = f.first {
                ranks.truncate(first.max(0) as usize);
            }
            Ok(RankingResponse::Items(ranks))
        }
        RankingEntity::Creators => {
            let rows = day_datas(
                state,
                "accountsDayDatas",
                "AccountsDayData_filter",
                "id sales earned uniqueCollectionsSales uniqueCollectorsTotal",
                json!({ "sales_gt": 0 }),
                from_seconds,
                "/v1/rankings",
            )
            .await?;
            let fragments = rows
                .iter()
                .map(|r| CreatorsDayDataFragment {
                    id: readers::str_of(r, "id"),
                    sales: readers::int_of(r, "sales"),
                    earned: readers::str_of(r, "earned"),
                    unique_collections_sales: readers::int_of(r, "uniqueCollectionsSales"),
                    unique_collectors_total: readers::int_of(r, "uniqueCollectorsTotal"),
                })
                .collect();
            let mut ranks = get_unique_creators_from_creators_day_data(fragments);
            sort_creators(&mut ranks, f.sort_by);
            if let Some(first) = f.first {
                ranks.truncate(first.max(0) as usize);
            }
            Ok(RankingResponse::Creators(ranks))
        }
        RankingEntity::Collectors => {
            let rows = day_datas(
                state,
                "accountsDayDatas",
                "AccountsDayData_filter",
                "id purchases spent uniqueAndMythicItems creatorsSupportedTotal",
                json!({ "purchases_gt": 0 }),
                from_seconds,
                "/v1/rankings",
            )
            .await?;
            let fragments = rows
                .iter()
                .map(|r| CollectorsDayDataFragment {
                    id: readers::str_of(r, "id"),
                    purchases: readers::int_of(r, "purchases"),
                    spent: readers::str_of(r, "spent"),
                    unique_and_mythic_items: readers::int_of(r, "uniqueAndMythicItems"),
                    creators_supported_total: readers::int_of(r, "creatorsSupportedTotal"),
                })
                .collect();
            let mut ranks = get_unique_collectors_from_collectors_day_data(fragments);
            sort_collectors(&mut ranks, f.sort_by);
            if let Some(first) = f.first {
                ranks.truncate(first.max(0) as usize);
            }
            Ok(RankingResponse::Collectors(ranks))
        }
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

pub const VOLUME_MAX_DAYS: i64 = 366;

pub fn accumulate(days: &[Value]) -> VolumeData {
    let mut out = VolumeData {
        sales: 0,
        volume: "0".to_string(),
        creators_earnings: "0".to_string(),
        dao_earnings: "0".to_string(),
    };
    for d in days {
        out.sales += readers::int_of(d, "sales");
        out.volume = bn_add(&out.volume, &readers::str_of(d, "volume"));
        out.creators_earnings = bn_add(
            &out.creators_earnings,
            &readers::str_of(d, "creatorsEarnings"),
        );
        out.dao_earnings = bn_add(&out.dao_earnings, &readers::str_of(d, "daoEarnings"));
    }
    out
}

/// The pages `/v1/volume/all` walks per network: 10,000 days.
pub const VOLUME_ALL_MAX_PAGES: usize = 10;

pub(crate) const VOLUME_ALL_QUERY: &str = "query VolumeAll($first: Int!, $where: AnalyticsDayData_filter) { analyticsDayDatas(first: $first, orderBy: id, orderDirection: asc, where: $where) { id date sales volume creatorsEarnings daoEarnings } }";

/// Every analytics day row of every configured network (the old `SUM` over
/// the squid's `analytics_day_data`), id keyset; 15 min.
async fn all_days(state: &MarketplaceState) -> Result<Vec<Value>, Response> {
    let mut days = Vec::new();
    for (network, url) in readers::require_both_collections(state)? {
        let key = format!("volume:{}:all", network.db_name());
        let value = cached(state, Ttl::M15, key, || async {
            let mut rows = Vec::new();
            let mut last_id: Option<String> = None;
            for _ in 0..VOLUME_ALL_MAX_PAGES {
                let where_ = match &last_id {
                    Some(id) => json!({ "id_gt": id }),
                    None => json!({}),
                };
                let data = readers::query(
                    state,
                    url,
                    VOLUME_ALL_QUERY,
                    json!({ "first": graph::THE_GRAPH_PAGE_SIZE, "where": where_ }),
                )
                .await?;
                let page = readers::rows(&data, "analyticsDayDatas");
                let n = page.len() as i64;
                last_id = page.last().map(|r| readers::str_of(r, "id"));
                rows.extend(page);
                if n < graph::THE_GRAPH_PAGE_SIZE {
                    break;
                }
            }
            Ok(Value::Array(rows))
        })
        .await
        .map_err(|e| readers::upstream_failed("/v1/volume", &e))?;
        days.extend(value.as_array().cloned().unwrap_or_default());
    }
    Ok(days)
}

pub async fn fetch_volume(
    state: &MarketplaceState,
    tf: AnalyticsTimeframe,
) -> Result<VolumeData, Response> {
    let from = get_timestamp_from_timeframe(tf);
    if from == 0 {
        return Ok(accumulate(&all_days(state).await?));
    }
    let mut days = Vec::new();
    for (network, url) in readers::require_both_collections(state)? {
        let key = format!("volume:{}:{}", network.db_name(), from / 1000);
        let value = cached(state, Ttl::M15, key, || async {
            readers::query(
                state,
                url,
                VOLUME_QUERY,
                json!({ "first": VOLUME_MAX_DAYS, "from": from / 1000 }),
            )
            .await
        })
        .await
        .map_err(|e| readers::upstream_failed("/v1/volume", &e))?;
        days.extend(readers::rows(&value, "analyticsDayDatas"));
    }
    Ok(accumulate(&days))
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
        let f = super::nfts::NftFilters {
            is_on_sale: p.get_boolean("isOnSale").then_some(true),
            adjacent_to_road: p.get_boolean("adjacentToRoad"),
            min_distance_to_plaza: p.get_number("minDistanceToPlaza", None),
            max_distance_to_plaza: p.get_number("maxDistanceToPlaza", None),
            max_estate_size: p.get_number("maxEstateSize", None),
            min_estate_size: p.get_number("minEstateSize", None),
            min_price: p
                .get_string("minPrice", None)
                .filter(|s| !s.trim().is_empty()),
            max_price: p
                .get_string("maxPrice", None)
                .filter(|s| !s.trim().is_empty()),
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
    fn day_rollups_collapse_to_one_row_per_entity() {
        let items = get_unique_items_from_items_day_data(
            vec![
                ItemsDayDataFragment {
                    id: "19000-0xa-1".into(),
                    sales: 2,
                    volume: "10".into(),
                },
                ItemsDayDataFragment {
                    id: "19001-0xa-1".into(),
                    sales: 1,
                    volume: "5".into(),
                },
                ItemsDayDataFragment {
                    id: "19001-0xb-2".into(),
                    sales: 9,
                    volume: "1".into(),
                },
            ],
            1,
        );
        let mut ranks = items;
        sort_items(&mut ranks, None);
        assert_eq!(
            (
                ranks[0].id.as_str(),
                ranks[0].sales,
                ranks[0].volume.as_str()
            ),
            ("0xa-1", 3, "15")
        );
        sort_items(&mut ranks, Some(RankingsSortBy::MostSales));
        assert_eq!(ranks[0].id, "0xb-2");
        let mut creators = get_unique_creators_from_creators_day_data(vec![
            CreatorsDayDataFragment {
                id: "19000-0xc".into(),
                sales: 1,
                earned: "7".into(),
                unique_collections_sales: 2,
                unique_collectors_total: 4,
            },
            CreatorsDayDataFragment {
                id: "19001-0xc".into(),
                sales: 2,
                earned: "3".into(),
                unique_collections_sales: 1,
                unique_collectors_total: 5,
            },
        ]);
        sort_creators(&mut creators, None);
        assert_eq!(
            serde_json::to_value(&creators[0]).unwrap(),
            json!({ "id": "0xc", "sales": 3, "earned": "10", "uniqueCollectors": 5, "collections": 2 })
        );
        let mut collectors =
            get_unique_collectors_from_collectors_day_data(vec![CollectorsDayDataFragment {
                id: "19000-0xd".into(),
                purchases: 1,
                spent: "7".into(),
                unique_and_mythic_items: 2,
                creators_supported_total: 4,
            }]);
        sort_collectors(&mut collectors, Some(RankingsSortBy::MostSales));
        assert_eq!(
            serde_json::to_value(&collectors[0]).unwrap(),
            json!({ "id": "0xd", "purchases": 1, "spent": "7", "uniqueAndMythicItems": 2, "creatorsSupported": 4 })
        );
    }

    #[test]
    fn volume_sums_the_day_rows_as_big_numbers() {
        let days = vec![
            json!({ "sales": 2, "volume": "99999999999999999999", "creatorsEarnings": "1", "daoEarnings": "2" }),
            json!({ "sales": 3, "volume": "1", "creatorsEarnings": "1", "daoEarnings": "0" }),
        ];
        assert_eq!(
            accumulate(&days),
            VolumeData {
                sales: 5,
                volume: "100000000000000000000".into(),
                creators_earnings: "2".into(),
                dao_earnings: "2".into()
            }
        );
        assert_eq!(get_timestamp_from_timeframe(AnalyticsTimeframe::All), 0);
        assert!(get_timestamp_from_timeframe(AnalyticsTimeframe::Week) % 86_400_000 == 0);
    }

    /// One network keeps the source's order; two merge per id and re-rank by
    /// the same key, as big numbers.
    #[test]
    fn lifetime_rows_merge_per_address_across_networks() {
        let one = merge_in_order(
            vec![vec![
                ItemRank {
                    id: "b".into(),
                    sales: 1,
                    volume: "5".into(),
                },
                ItemRank {
                    id: "a".into(),
                    sales: 9,
                    volume: "5".into(),
                },
            ]],
            |r: &ItemRank| r.id.clone(),
            |_, _| {},
            |r| bn_key(&r.volume),
        );
        assert_eq!(
            one.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        let merged = merge_in_order(
            vec![
                vec![
                    ItemRank {
                        id: "a".into(),
                        sales: 1,
                        volume: "900".into(),
                    },
                    ItemRank {
                        id: "b".into(),
                        sales: 1,
                        volume: "800".into(),
                    },
                ],
                vec![ItemRank {
                    id: "b".into(),
                    sales: 2,
                    volume: "99999999999999999999".into(),
                }],
            ],
            |r: &ItemRank| r.id.clone(),
            |a, b| {
                a.sales += b.sales;
                a.volume = bn_add(&a.volume, &b.volume);
            },
            |r| bn_key(&r.volume),
        );
        assert_eq!(
            (
                merged[0].id.as_str(),
                merged[0].sales,
                merged[0].volume.as_str()
            ),
            ("b", 3, "100000000000000000799")
        );
        assert_eq!(merged[1].id, "a");
        assert!(bn_key("0100") > bn_key("99") && bn_key("0") == bn_key(""));
    }

    /// A rollup dated exactly at the window start (midnight of now - N days,
    /// the day timeframe's only rollup before today's) is inside the window.
    #[test]
    fn the_window_start_day_is_included() {
        let from = get_timestamp_from_timeframe(AnalyticsTimeframe::Day) / 1000;
        let w = day_data_where(json!({ "searchRarity": "epic" }), from);
        assert_eq!(w["date_gte"], json!(from));
        assert!(w.get("date_gt").is_none());
        assert_eq!(w["searchRarity"], "epic");
        assert!(VOLUME_QUERY.contains("date_gte: $from") && !VOLUME_QUERY.contains("date_gt:"));
        let rollup_date = from;
        assert!(
            rollup_date >= from,
            "the >= filter keeps a rollup dated at the start; > dropped it"
        );
    }
}
