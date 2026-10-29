//! `/v1/users/{address}/{wearables,emotes,names}` and their `grouped`,
//! `urn-token` and `names-only` forms over the stage-4 graph readers
//! (`graph::owned_copies`, `graph::owned_names`; 30 s) with the economy
//! usage-grant overlay: an active grant whose `unlock_at` lies ahead is one
//! more element, `status: "leased"` with its `unlockAt`, never mistaken for
//! an owned copy of the same URN. Pages are 1-based as the old
//! `PaginatedResponse::new_1based` made them; `totalItems` counts distinct
//! URNs on the flat wearable and emote lists.

use std::collections::{BTreeMap, HashMap};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::Serialize;
use sqlx::Row;

use super::graph::{self, OwnedCopy};
use super::readers;
use super::wire::{Params, fix_urn};
use super::{MarketplaceState, SharedState};

pub const FIRST_DEFAULT: i64 = 100;
pub const SKIP_DEFAULT: i64 = 0;

#[derive(Debug, Clone, Default)]
pub struct UserAssetsFilters {
    pub first: i64,
    pub skip: i64,
    pub category: Option<String>,
    pub rarity: Option<String>,
    pub name: Option<String>,
    pub order_by: Option<String>,
    pub direction: Option<String>,
    pub item_type: Option<Vec<String>>,
}

pub fn parse_user_assets_params(pairs: &[(String, String)]) -> UserAssetsFilters {
    const MAX_LIMIT: i64 = 1000;
    let p = Params::new(pairs);
    let limit = p.get_number("limit", None).map(|n| n as i64);
    let offset = p.get_number("offset", None).map(|n| n as i64);
    let first = p.get_number("first", None).map(|n| n as i64);
    let skip = p.get_number("skip", None).map(|n| n as i64);
    let requested_limit = limit.or(first).unwrap_or(FIRST_DEFAULT);
    let requested_skip = offset.or(skip).unwrap_or(SKIP_DEFAULT).max(0);
    let item_type_list = p.get_list("itemType", &[]);
    UserAssetsFilters {
        first: requested_limit.clamp(0, MAX_LIMIT),
        skip: requested_skip,
        category: p.get_string("category", None),
        rarity: p.get_string("rarity", None),
        name: p.get_string("name", None),
        order_by: p.get_string("orderBy", None),
        direction: p.get_string("direction", None),
        item_type: if item_type_list.is_empty() {
            None
        } else {
            Some(item_type_list)
        },
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct AssetsHttpResponse<T> {
    pub ok: bool,
    pub data: PaginatedAssetsBody<T>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct PaginatedAssetsBody<T> {
    pub elements: Vec<T>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub page: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub pages: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub limit: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
    #[serde(rename = "totalItems", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number", optional))]
    pub total_items: Option<i64>,
}

/// The old `PaginatedResponse::new_1based` over `limit`/`skip`.
pub fn create_paginated_response<T>(
    elements: Vec<T>,
    total: i64,
    first: i64,
    skip: i64,
    total_items: Option<i64>,
) -> AssetsHttpResponse<T> {
    let limit = if first == 0 { 1 } else { first };
    let page = skip / limit + 1;
    let pages = (total + limit - 1) / limit;
    AssetsHttpResponse {
        ok: true,
        data: PaginatedAssetsBody {
            elements,
            page,
            pages,
            limit,
            total,
            total_items,
        },
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct ProfileWearable {
    pub urn: String,
    pub id: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub category: String,
    #[serde(rename = "transferredAt")]
    pub transferred_at: Option<String>,
    pub name: String,
    pub rarity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub status: Option<String>,
    #[serde(rename = "unlockAt", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number", optional))]
    pub unlock_at: Option<i64>,
}

pub type ProfileEmote = ProfileWearable;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct ProfileName {
    pub name: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub price: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct UrnToken {
    pub urn: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct NameOnly {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct IndividualData {
    pub id: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "transferredAt")]
    pub transferred_at: String,
    pub price: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct GroupedWearable {
    pub urn: String,
    pub amount: String,
    #[serde(rename = "individualData")]
    pub individual_data: Vec<IndividualData>,
    pub name: String,
    pub rarity: String,
    #[serde(rename = "minTransferredAt")]
    pub min_transferred_at: String,
    #[serde(rename = "maxTransferredAt")]
    pub max_transferred_at: String,
    pub category: String,
    #[serde(rename = "itemType", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub item_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub status: Option<String>,
    #[serde(rename = "unlockAt", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number", optional))]
    pub unlock_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UsageGrant {
    pub urn: String,
    pub token_id: Option<String>,
    pub category: String,
    pub unlock_at: i64,
}

/// The owner's active, still-locked grants, newest first.
pub async fn active_grants(state: &MarketplaceState, owner: &str) -> Vec<UsageGrant> {
    let rows = sqlx::query("SELECT urn, token_id, category, unlock_at FROM shop.usage_grants WHERE grantee_address = $1 AND status = 'active' AND unlock_at > $2 ORDER BY granted_at DESC LIMIT 1000")
        .bind(owner.to_lowercase())
        .bind(Utc::now().naive_utc())
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .map(|r| UsageGrant {
            urn: fix_urn(&r.get::<String, _>("urn")),
            token_id: r.get("token_id"),
            category: r.get("category"),
            unlock_at: r
                .get::<chrono::NaiveDateTime, _>("unlock_at")
                .and_utc()
                .timestamp_millis(),
        })
        .collect()
}

/// A grant as an owned-copy row: name and rarity from the items mirror when
/// it has the URN.
async fn grants_as_copies(
    state: &MarketplaceState,
    grants: &[UsageGrant],
    category: &str,
) -> Vec<(OwnedCopy, i64)> {
    let wanted: Vec<&UsageGrant> = grants.iter().filter(|g| g.category == category).collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let urns: Vec<String> = wanted.iter().map(|g| g.urn.clone()).collect();
    let rows = sqlx::query("SELECT urn, name, rarity, category, price::text AS price FROM shop.items WHERE urn = ANY($1)").bind(&urns).fetch_all(&state.pool).await.unwrap_or_default();
    let meta: HashMap<String, (String, String, String, String)> = rows
        .iter()
        .map(|r| {
            (
                r.get("urn"),
                (
                    r.get("name"),
                    r.get("rarity"),
                    r.get("category"),
                    r.get("price"),
                ),
            )
        })
        .collect();
    wanted
        .into_iter()
        .map(|g| {
            let (name, rarity, item_category, price) =
                meta.get(&g.urn).cloned().unwrap_or_default();
            (
                OwnedCopy {
                    urn: g.urn.clone(),
                    token_id: g.token_id.clone().unwrap_or_else(|| "0".to_string()),
                    transferred_at: 0,
                    price,
                    rarity,
                    name,
                    category: item_category,
                },
                g.unlock_at,
            )
        })
        .collect()
}

fn profile_row(c: &OwnedCopy, leased: Option<i64>) -> ProfileWearable {
    ProfileWearable {
        urn: c.urn.clone(),
        id: format!("{}:{}", c.urn, c.token_id),
        token_id: c.token_id.clone(),
        category: c.category.clone(),
        transferred_at: Some(c.transferred_at.to_string()),
        name: c.name.clone(),
        rarity: c.rarity.clone(),
        price: Some(c.price.clone()),
        status: leased.map(|_| "leased".to_string()),
        unlock_at: leased,
    }
}

async fn owned_or_unavailable(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
) -> Result<Vec<OwnedCopy>, Response> {
    readers::require_both_collections(state)?;
    graph::owned_copies(&state.index, owner, category)
        .await
        .map(|c| (*c).clone())
        .map_err(|e| readers::upstream_failed("/v1/users", &e))
}

/// The flat list: owned copies newest first, then the leased grants.
pub async fn profile_assets(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
    first: i64,
    skip: i64,
) -> Result<(Vec<ProfileWearable>, i64, i64), Response> {
    let owned = owned_or_unavailable(state, owner, category).await?;
    let grants = active_grants(state, owner).await;
    let leased = grants_as_copies(state, &grants, category).await;
    let mut rows: Vec<ProfileWearable> = owned.iter().map(|c| profile_row(c, None)).collect();
    rows.extend(
        leased
            .iter()
            .map(|(c, unlock)| profile_row(c, Some(*unlock))),
    );
    let total = rows.len() as i64;
    let total_items = rows
        .iter()
        .map(|r| r.urn.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len() as i64;
    Ok((
        rows.into_iter()
            .skip(skip.max(0) as usize)
            .take(first.max(0) as usize)
            .collect(),
        total,
        total_items,
    ))
}

fn item_type_of(category: &str, urn: &str, is_smart: bool) -> &'static str {
    if category == "emote" {
        "emote_v1"
    } else if is_smart {
        "smart_wearable_v1"
    } else if urn.contains(":collections-v1:") || urn.contains(":off-chain:base-avatars:") {
        "wearable_v1"
    } else {
        "wearable_v2"
    }
}

/// One row per URN over owned copies and grants, with the old filters
/// (`category`, `rarity`, `name`, `itemType`) and sorts.
pub async fn grouped_assets(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
    f: &UserAssetsFilters,
) -> Result<(Vec<GroupedWearable>, i64), Response> {
    let owned = owned_or_unavailable(state, owner, category).await?;
    let grants = active_grants(state, owner).await;
    let leased = grants_as_copies(state, &grants, category).await;
    let mut groups: BTreeMap<String, GroupedWearable> = BTreeMap::new();
    let smart: HashMap<String, bool> = if category == "wearable" && !owned.is_empty() {
        let urns: Vec<String> = owned.iter().map(|c| c.urn.clone()).collect();
        sqlx::query("SELECT urn, is_smart FROM shop.items WHERE urn = ANY($1)")
            .bind(&urns)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default()
            .iter()
            .map(|r| (r.get("urn"), r.get("is_smart")))
            .collect()
    } else {
        HashMap::new()
    };
    let all: Vec<(&OwnedCopy, Option<i64>)> = owned
        .iter()
        .map(|c| (c, None))
        .chain(leased.iter().map(|(c, u)| (c, Some(*u))))
        .collect();
    for (c, lease) in all {
        let g = groups
            .entry(c.urn.clone())
            .or_insert_with(|| GroupedWearable {
                urn: c.urn.clone(),
                amount: "0".to_string(),
                individual_data: Vec::new(),
                name: c.name.clone(),
                rarity: c.rarity.clone(),
                min_transferred_at: c.transferred_at.to_string(),
                max_transferred_at: c.transferred_at.to_string(),
                category: c.category.clone(),
                item_type: (category == "wearable").then(|| {
                    item_type_of(
                        category,
                        &c.urn,
                        smart.get(&c.urn).copied().unwrap_or(false),
                    )
                    .to_string()
                }),
                status: None,
                unlock_at: None,
            });
        g.individual_data.push(IndividualData {
            id: format!("{}:{}", c.urn, c.token_id),
            token_id: c.token_id.clone(),
            transferred_at: c.transferred_at.to_string(),
            price: c.price.clone(),
        });
        g.amount = g.individual_data.len().to_string();
        let min = g
            .min_transferred_at
            .parse::<i64>()
            .unwrap_or(0)
            .min(c.transferred_at);
        let max = g
            .max_transferred_at
            .parse::<i64>()
            .unwrap_or(0)
            .max(c.transferred_at);
        g.min_transferred_at = min.to_string();
        g.max_transferred_at = max.to_string();
        if let Some(unlock) = lease
            && g.status.is_none()
        {
            g.status = Some("leased".to_string());
            g.unlock_at = Some(unlock);
        }
    }
    let mut rows: Vec<GroupedWearable> = groups.into_values().collect();
    if let Some(cat) = &f.category {
        rows.retain(|g| g.category.eq_ignore_ascii_case(cat));
    }
    if let Some(rarity) = &f.rarity {
        rows.retain(|g| g.rarity.eq_ignore_ascii_case(rarity));
    }
    if let Some(name) = f
        .name
        .as_deref()
        .map(str::to_lowercase)
        .filter(|n| !n.is_empty())
    {
        rows.retain(|g| g.name.to_lowercase().contains(&name));
    }
    if let Some(types) = &f.item_type {
        rows.retain(|g| {
            g.item_type
                .as_ref()
                .is_some_and(|t| types.iter().any(|w| w == t))
        });
    }
    let desc = f
        .direction
        .as_deref()
        .is_none_or(|d| !d.eq_ignore_ascii_case("asc"));
    rows.sort_by(|a, b| {
        let ord = match f.order_by.as_deref() {
            Some("name") => a.name.cmp(&b.name),
            Some("rarity") => a.rarity.cmp(&b.rarity),
            _ => a
                .max_transferred_at
                .parse::<i64>()
                .unwrap_or(0)
                .cmp(&b.max_transferred_at.parse::<i64>().unwrap_or(0)),
        };
        let ord = if desc { ord.reverse() } else { ord };
        ord.then_with(|| a.urn.cmp(&b.urn))
    });
    let total = rows.len() as i64;
    Ok((
        rows.into_iter()
            .skip(f.skip.max(0) as usize)
            .take(f.first.max(0) as usize)
            .collect(),
        total,
    ))
}

pub async fn urn_tokens(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
    first: i64,
    skip: i64,
) -> Result<(Vec<UrnToken>, i64), Response> {
    let owned = owned_or_unavailable(state, owner, category).await?;
    let total = owned.len() as i64;
    Ok((
        owned
            .into_iter()
            .skip(skip.max(0) as usize)
            .take(first.max(0) as usize)
            .map(|c| UrnToken {
                urn: c.urn,
                token_id: c.token_id,
            })
            .collect(),
        total,
    ))
}

async fn names_of(
    state: &MarketplaceState,
    owner: &str,
) -> Result<Vec<graph::OwnedName>, Response> {
    readers::require_marketplace(state)?;
    graph::owned_names(&state.index, owner)
        .await
        .map(|n| (*n).clone())
        .map_err(|e| readers::upstream_failed("/v1/users/names", &e))
}

fn respond<T: Serialize>(
    elements: Vec<T>,
    total: i64,
    f: &UserAssetsFilters,
    total_items: Option<i64>,
) -> Response {
    Json(create_paginated_response(
        elements,
        total,
        f.first,
        f.skip,
        total_items,
    ))
    .into_response()
}

pub async fn get_user_wearables(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match profile_assets(&state, &address.to_lowercase(), "wearable", f.first, f.skip).await {
        Ok((data, total, total_items)) => respond(data, total, &f, Some(total_items)),
        Err(r) => r,
    }
}

pub async fn get_user_emotes(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match profile_assets(&state, &address.to_lowercase(), "emote", f.first, f.skip).await {
        Ok((data, total, total_items)) => respond(data, total, &f, Some(total_items)),
        Err(r) => r,
    }
}

pub async fn get_user_grouped_wearables(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match grouped_assets(&state, &address.to_lowercase(), "wearable", &f).await {
        Ok((data, total)) => respond(data, total, &f, None),
        Err(r) => r,
    }
}

pub async fn get_user_grouped_emotes(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match grouped_assets(&state, &address.to_lowercase(), "emote", &f).await {
        Ok((data, total)) => respond(data, total, &f, None),
        Err(r) => r,
    }
}

pub async fn get_user_wearables_urn_token(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match urn_tokens(&state, &address.to_lowercase(), "wearable", f.first, f.skip).await {
        Ok((data, total)) => respond(data, total, &f, None),
        Err(r) => r,
    }
}

pub async fn get_user_emotes_urn_token(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match urn_tokens(&state, &address.to_lowercase(), "emote", f.first, f.skip).await {
        Ok((data, total)) => respond(data, total, &f, None),
        Err(r) => r,
    }
}

pub async fn get_user_names(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match names_of(&state, &address.to_lowercase()).await {
        Ok(names) => {
            let total = names.len() as i64;
            let data: Vec<ProfileName> = names
                .into_iter()
                .skip(f.skip.max(0) as usize)
                .take(f.first.max(0) as usize)
                .map(|n| ProfileName {
                    name: n.name,
                    contract_address: n.contract_address,
                    token_id: n.token_id,
                    price: n.price,
                })
                .collect();
            respond(data, total, &f, None)
        }
        Err(r) => r,
    }
}

pub async fn get_user_names_only(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let f = parse_user_assets_params(&pairs);
    match names_of(&state, &address.to_lowercase()).await {
        Ok(names) => {
            let total = names.len() as i64;
            let data: Vec<NameOnly> = names
                .into_iter()
                .skip(f.skip.max(0) as usize)
                .take(f.first.max(0) as usize)
                .map(|n| NameOnly { name: n.name })
                .collect();
            respond(data, total, &f, None)
        }
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paginated_body_keeps_its_wire_shape() {
        let page = create_paginated_response(vec!["a", "b"], 10, 5, 5, Some(12));
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            json!({ "ok": true, "data": { "elements": ["a", "b"], "page": 2, "pages": 2, "limit": 5, "total": 10, "totalItems": 12 } })
        );
        let page = create_paginated_response(Vec::<&str>::new(), 7, 0, 3, None);
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            json!({ "ok": true, "data": { "elements": [], "page": 4, "pages": 7, "limit": 1, "total": 7 } })
        );
        let body = serde_json::to_value(create_paginated_response(
            Vec::<&str>::new(),
            0,
            100,
            0,
            None,
        ))
        .unwrap();
        assert_eq!(
            (
                body["data"]["page"].as_i64(),
                body["data"]["pages"].as_i64()
            ),
            (Some(1), Some(0))
        );
    }

    #[test]
    fn a_leased_row_carries_its_status_and_an_owned_one_does_not() {
        let copy = OwnedCopy {
            urn: "urn:decentraland:ethereum:collections-v2:0xabc:0".into(),
            token_id: "3".into(),
            transferred_at: 300,
            price: "10".into(),
            rarity: "common".into(),
            name: "Item".into(),
            category: "eyewear".into(),
        };
        let owned = serde_json::to_value(profile_row(&copy, None)).unwrap();
        assert_eq!(
            owned,
            json!({ "urn": copy.urn, "id": format!("{}:3", copy.urn), "tokenId": "3", "category": "eyewear", "transferredAt": "300", "name": "Item", "rarity": "common", "price": "10" })
        );
        let leased = serde_json::to_value(profile_row(&copy, Some(1_700_000_000_000))).unwrap();
        assert_eq!(
            (leased["status"].as_str(), leased["unlockAt"].as_i64()),
            (Some("leased"), Some(1_700_000_000_000))
        );
        assert_eq!(
            item_type_of(
                "wearable",
                "urn:decentraland:off-chain:base-avatars:hat",
                false
            ),
            "wearable_v1"
        );
        assert_eq!(
            item_type_of("wearable", &copy.urn, true),
            "smart_wearable_v1"
        );
        assert_eq!(item_type_of("emote", &copy.urn, false), "emote_v1");
        let f = parse_user_assets_params(&[
            ("limit".to_string(), "5000".to_string()),
            ("offset".to_string(), "-2".to_string()),
            ("itemType".to_string(), "wearable_v2".to_string()),
        ]);
        assert_eq!(
            (f.first, f.skip, f.item_type.as_deref()),
            (1000, 0, Some(&["wearable_v2".to_string()][..]))
        );
    }
}
