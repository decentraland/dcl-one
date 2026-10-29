//! The explorer's backpack: `/lambdas/explorer/{address}/{wearables,emotes}`
//! and the same at `/explorer/{address}/...`. Three legs, merged, filtered,
//! sorted (UCA order, rarest or newest first) and paged in Rust exactly as
//! the old handler did: the base wearables (content entities), the owner's
//! on-chain copies (the collections subgraph readers, plus the active escrow
//! grants of `shop.usage_grants`, the item type from the `items`
//! mirror, the definitions from the content entities) and the third-party
//! wearables (`third_party`). The old on-chain leg read the squid's `nft`
//! table; without the subgraph readers this route answers the unavailable
//! envelope, as the other ownership lambdas do.

use std::collections::HashMap;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::Row;

use crate::content::queries::active_entity_docs_by_pointers;
use crate::shared::cache::TtlMap;
use crate::shared::query_params::{
    MAX_PAGE_SIZE as SHARED_MAX_PAGE_SIZE, NonPositivePolicy, OversizePolicy,
    parse_pagination_with, parse_query_string, qs_get_array, qs_get_string,
};

use super::definitions::{
    default_sort_direction, locale_cmp, rarity_rank, validate_rarity, validate_sort,
};
use super::graph;
use super::lambdas_catalog::cached_base_wearables;
use super::readers;
use super::third_party::fetch_all_third_party_wearables;
use super::{MarketplaceState, SharedState, bad_request, service_unavailable};

const EXPLORER_CACHE_TTL: Duration = Duration::from_secs(30);
const EXPLORER_CACHE_MAX_ENTRIES: usize = 50_000;

/// `(address, sort, direction, collection types, category, filters)`.
pub type ExplorerKey = (String, String, String, String, &'static str, String);

pub fn explorer_cache() -> TtlMap<ExplorerKey, Value> {
    TtlMap::bounded(EXPLORER_CACHE_TTL, EXPLORER_CACHE_MAX_ENTRIES)
}

const MAX_PAGE_SIZE: i64 = SHARED_MAX_PAGE_SIZE as i64;
const VALID_COLLECTION_TYPES: [&str; 3] = ["base-wearable", "on-chain", "third-party"];

struct ExplorerQuery {
    page_num: i64,
    page_size: i64,
    name: Option<String>,
    categories: Vec<String>,
    rarity: Option<String>,
    sort: String,
    direction: String,
    collection_types: Vec<String>,
    trimmed: bool,
}

fn parse_query(
    query: Option<&str>,
    valid_collection_types: &[&str],
) -> Result<ExplorerQuery, Box<Response>> {
    let params = parse_query_string(query.unwrap_or(""));
    let get_first = |key: &str| -> Option<String> { qs_get_string(&params, key) };
    let collection_types: Vec<String> = qs_get_array(&params, "collectionType");
    if collection_types
        .iter()
        .any(|t| !valid_collection_types.contains(&t.as_str()))
    {
        return Err(Box::new(bad_request(&format!(
            "Invalid collection type. Valid types are: {}.",
            valid_collection_types.join(", ")
        ))));
    }
    let pagination = parse_pagination_with(
        &params,
        MAX_PAGE_SIZE,
        OversizePolicy::Reject,
        NonPositivePolicy::PassThrough,
    )
    .map_err(|e| Box::new(bad_request(&e)))?;
    let name = get_first("name").map(|n| n.to_lowercase());
    let categories: Vec<String> = qs_get_array(&params, "category")
        .iter()
        .map(|v| v.to_lowercase())
        .collect();
    let rarity = get_first("rarity").map(|r| r.to_lowercase());
    if let Some(r) = &rarity {
        validate_rarity(r).map_err(|e| Box::new(bad_request(&e)))?;
    }
    let sort = get_first("orderBy")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "rarity".to_string());
    let direction = match get_first("direction") {
        Some(d) => d.to_uppercase(),
        None => default_sort_direction(&sort).to_string(),
    };
    validate_sort(&sort, &direction).map_err(|e| Box::new(bad_request(&e)))?;
    let trimmed = matches!(get_first("trimmed").as_deref(), Some("true") | Some("1"));
    Ok(ExplorerQuery {
        page_num: pagination.page_num,
        page_size: pagination.page_size,
        name,
        categories,
        rarity,
        sort,
        direction,
        collection_types,
        trimmed,
    })
}

struct OwnedItem {
    urn: String,
    individual_data: Vec<Value>,
    rarity: String,
    item_type: Option<String>,
    amount: i64,
    name: String,
    category: String,
    min_transferred_at: f64,
    max_transferred_at: f64,
    entity: Value,
    is_base: bool,
    is_third_party: bool,
    has_date: bool,
}

/// One owned copy: `(urn, token id, transferred at (seconds), rarity, price)`.
type CopyRow = (String, String, i64, Option<String>, Option<String>);

/// The owner's copies of `category` (the subgraph readers) and their active
/// escrow grants, newest first, grouped by URN in first-seen order.
async fn fetch_owned_items(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
) -> Result<Vec<OwnedItem>, String> {
    let copies = graph::owned_copies(&state.index, owner, category).await?;
    let mut rows: Vec<CopyRow> = copies
        .iter()
        .map(|c| {
            (
                c.urn.clone(),
                c.token_id.clone(),
                c.transferred_at,
                Some(c.rarity.clone()).filter(|r| !r.is_empty()),
                Some(c.price.clone()),
            )
        })
        .collect();
    let grants = sqlx::query(
        "SELECT urn, COALESCE(token_id, '') AS token_id, extract(epoch FROM granted_at)::bigint AS granted_at \
         FROM shop.usage_grants WHERE grantee_address = $1 AND status = 'active' AND category = $2 AND unlock_at > $3 \
         ORDER BY granted_at DESC LIMIT 1000",
    )
    .bind(owner.to_lowercase())
    .bind(category)
    .bind(Utc::now().naive_utc())
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("usage grants: {e}"))?;
    for g in &grants {
        rows.push((
            graph::normalize_urn(&g.get::<String, _>("urn")),
            g.get("token_id"),
            g.get("granted_at"),
            None,
            None,
        ));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.2));
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let mut grouped: HashMap<String, OwnedItem> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (urn, token_id, transferred_at, rarity, price) in rows {
        let transferred_num = transferred_at as f64;
        let individual = json!({
            "id": format!("{urn}:{token_id}"),
            "tokenId": token_id,
            "transferredAt": transferred_at.to_string(),
            "price": price.unwrap_or_default(),
        });
        match grouped.get_mut(&urn) {
            Some(existing) => {
                existing.individual_data.push(individual);
                existing.amount += 1;
                existing.min_transferred_at = existing.min_transferred_at.min(transferred_num);
                existing.max_transferred_at = existing.max_transferred_at.max(transferred_num);
                if existing.rarity.is_empty()
                    && let Some(r) = rarity
                {
                    existing.rarity = r;
                }
            }
            None => {
                order.push(urn.clone());
                grouped.insert(
                    urn.clone(),
                    OwnedItem {
                        urn: urn.clone(),
                        individual_data: vec![individual],
                        rarity: rarity.unwrap_or_default(),
                        item_type: None,
                        amount: 1,
                        name: String::new(),
                        category: String::new(),
                        min_transferred_at: transferred_num,
                        max_transferred_at: transferred_num,
                        entity: Value::Null,
                        is_base: false,
                        is_third_party: false,
                        has_date: true,
                    },
                );
            }
        }
    }

    let pointers: Vec<String> = order.iter().map(|u| u.to_lowercase()).collect();
    let (docs, types) = tokio::join!(
        active_entity_docs_by_pointers(&state.pool, &pointers),
        item_types(state, &order)
    );
    let docs = docs.map_err(|e| format!("entities: {e}"))?;
    let mut entity_by_pointer: HashMap<String, Value> = HashMap::new();
    for entity in docs.iter().filter_map(|d| d.to_value()) {
        if let Some(ptrs) = entity.get("pointers").and_then(|p| p.as_array()) {
            for ptr in ptrs.iter().filter_map(Value::as_str) {
                entity_by_pointer
                    .entry(ptr.to_lowercase())
                    .or_insert_with(|| entity.clone());
            }
        }
    }
    let mut items: Vec<OwnedItem> = Vec::new();
    for urn in order {
        let Some(mut item) = grouped.remove(&urn) else {
            continue;
        };
        let Some(entity) = entity_by_pointer.get(&urn.to_lowercase()) else {
            continue;
        };
        let (name, cat) = extract_name_category(entity, category);
        item.name = name;
        item.category = cat;
        item.entity = entity.clone();
        item.item_type = types.get(&urn).cloned();
        items.push(item);
    }
    Ok(items)
}

/// The mirrored item type of each URN (`wearable_v2`, `smart_wearable_v1`, ...).
async fn item_types(state: &MarketplaceState, urns: &[String]) -> HashMap<String, String> {
    if urns.is_empty() {
        return HashMap::new();
    }
    sqlx::query("SELECT urn, item_type FROM shop.items WHERE urn = ANY($1)")
        .bind(urns)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
        .iter()
        .map(|r| (r.get("urn"), r.get("item_type")))
        .collect()
}

fn extract_name_category(entity: &Value, category: &str) -> (String, String) {
    let md = entity.get("metadata").cloned().unwrap_or(Value::Null);
    let mut name = md
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        name = md
            .get("i18n")
            .and_then(|i| i.as_array())
            .and_then(|arr| arr.first())
            .and_then(|e| e.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
    }
    let cat = if category == "emote" {
        md.get("emoteDataADR74")
            .or_else(|| md.get("emoteDataV0"))
            .and_then(|d| d.get("category"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        md.get("data")
            .and_then(|d| d.get("category"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    };
    (name, cat)
}

fn sort_items(items: &mut [OwnedItem], sort: &str, direction: &str) {
    let by_urn = |a: &OwnedItem, b: &OwnedItem| locale_cmp(&a.urn, &b.urn);
    match (sort, direction) {
        ("rarity", dir) => {
            items.sort_by(|a, b| {
                let ra = rarity_rank(&a.rarity);
                let rb = rarity_rank(&b.rarity);
                let has_a = !a.rarity.is_empty() && ra >= 0;
                let has_b = !b.rarity.is_empty() && rb >= 0;
                let cmp = match (has_a, has_b) {
                    (true, true) => rb.cmp(&ra).then_with(|| by_urn(a, b)),
                    (false, false) => by_urn(a, b),
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                };
                if dir == "ASC" { cmp.reverse() } else { cmp }
            });
        }
        ("name", dir) => {
            items.sort_by(|a, b| {
                let cmp = locale_cmp(&a.name, &b.name).then_with(|| by_urn(a, b));
                if dir == "DESC" { cmp.reverse() } else { cmp }
            });
        }
        ("date", dir) => {
            items.sort_by(|a, b| {
                use std::cmp::Ordering;
                if dir == "DESC" {
                    match (a.has_date, b.has_date) {
                        (true, true) => b
                            .max_transferred_at
                            .partial_cmp(&a.max_transferred_at)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| by_urn(a, b)),
                        (false, false) => by_urn(a, b),
                        (true, false) => Ordering::Less,
                        (false, true) => Ordering::Greater,
                    }
                } else {
                    match (a.has_date, b.has_date) {
                        (true, true) => a
                            .min_transferred_at
                            .partial_cmp(&b.min_transferred_at)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| by_urn(b, a)),
                        (false, false) => by_urn(a, b),
                        (true, false) => Ordering::Greater,
                        (false, true) => Ordering::Less,
                    }
                }
            });
        }
        _ => {}
    }
}

fn passes_filters(item: &OwnedItem, q: &ExplorerQuery) -> bool {
    if let Some(rarity) = &q.rarity
        && (item.rarity.is_empty() || &item.rarity != rarity)
    {
        return false;
    }
    if let Some(name) = &q.name
        && (item.name.is_empty() || !item.name.to_lowercase().contains(name))
    {
        return false;
    }
    if !q.categories.is_empty()
        && (item.category.is_empty() || !q.categories.contains(&item.category))
    {
        return false;
    }
    true
}

fn item_to_value(item: &OwnedItem, include_item_type: bool) -> Value {
    let mut obj = serde_json::Map::new();
    if item.is_base {
        obj.insert("type".to_string(), json!("base-wearable"));
        obj.insert("urn".to_string(), json!(item.urn));
        obj.insert("name".to_string(), json!(item.name));
        obj.insert("category".to_string(), json!(item.category));
        obj.insert(
            "individualData".to_string(),
            Value::Array(item.individual_data.clone()),
        );
        obj.insert("amount".to_string(), json!(item.amount));
        obj.insert("entity".to_string(), item.entity.clone());
        return Value::Object(obj);
    }
    if item.is_third_party {
        obj.insert("type".to_string(), json!("third-party"));
        obj.insert("urn".to_string(), json!(item.urn));
        obj.insert(
            "individualData".to_string(),
            Value::Array(item.individual_data.clone()),
        );
        obj.insert("amount".to_string(), json!(item.amount));
        obj.insert("name".to_string(), json!(item.name));
        obj.insert("category".to_string(), json!(item.category));
        obj.insert("entity".to_string(), item.entity.clone());
        return Value::Object(obj);
    }
    obj.insert("type".to_string(), json!("on-chain"));
    obj.insert("urn".to_string(), json!(item.urn));
    obj.insert("amount".to_string(), json!(item.amount));
    obj.insert(
        "individualData".to_string(),
        Value::Array(item.individual_data.clone()),
    );
    obj.insert("name".to_string(), json!(item.name));
    obj.insert("rarity".to_string(), json!(item.rarity));
    if include_item_type {
        obj.insert(
            "itemType".to_string(),
            json!(item.item_type.clone().unwrap_or_default()),
        );
    }
    obj.insert("category".to_string(), json!(item.category));
    obj.insert("entity".to_string(), item.entity.clone());
    Value::Object(obj)
}

fn item_to_trimmed_value(item: &OwnedItem) -> Value {
    let mut entity = match &item.entity {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    if !entity.contains_key("id") {
        entity.insert("id".to_string(), json!(item.urn));
    }
    let thumb_file = entity
        .get("metadata")
        .and_then(|m| m.get("thumbnail"))
        .and_then(|t| t.as_str())
        .unwrap_or("thumbnail.png")
        .to_string();
    let thumb_hash = entity
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|files| {
            files
                .iter()
                .find(|f| f.get("file").and_then(|x| x.as_str()) == Some(thumb_file.as_str()))
        })
        .and_then(|f| f.get("hash").cloned());
    if let Some(h) = thumb_hash {
        entity.insert("thumbnail".to_string(), h);
    }
    if let Some(Value::Object(md)) = entity.get_mut("metadata") {
        let name_missing = md
            .get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.is_empty())
            .unwrap_or(true);
        if name_missing {
            md.insert("name".to_string(), json!(item.name));
        }
    }
    entity.insert(
        "individualData".to_string(),
        Value::Array(item.individual_data.clone()),
    );
    let mut obj = serde_json::Map::new();
    obj.insert("entity".to_string(), Value::Object(entity));
    obj.insert("amount".to_string(), json!(item.amount));
    Value::Object(obj)
}

async fn fetch_base_items(state: &MarketplaceState) -> Vec<OwnedItem> {
    cached_base_wearables(state)
        .await
        .iter()
        .map(|bw| OwnedItem {
            individual_data: vec![json!({ "id": bw.urn })],
            urn: bw.urn.clone(),
            rarity: String::new(),
            item_type: None,
            amount: 1,
            name: bw.name.clone(),
            category: bw.category.clone(),
            min_transferred_at: 0.0,
            max_transferred_at: 0.0,
            entity: bw.entity.clone(),
            is_base: true,
            is_third_party: false,
            has_date: false,
        })
        .collect()
}

async fn fetch_third_party_items(state: &MarketplaceState, owner: &str) -> Vec<OwnedItem> {
    fetch_all_third_party_wearables(state, owner, None)
        .await
        .into_iter()
        .map(|e| OwnedItem {
            amount: e.individual_data.len() as i64,
            individual_data: e.individual_data,
            urn: e.urn,
            rarity: String::new(),
            item_type: None,
            name: e.name,
            category: e.category,
            min_transferred_at: 0.0,
            max_transferred_at: 0.0,
            entity: e.entity,
            is_base: false,
            is_third_party: true,
            has_date: false,
        })
        .collect()
}

async fn explorer_items(
    state: &MarketplaceState,
    addr: &str,
    query: Option<&str>,
    category: &'static str,
    valid_collection_types: &[&'static str],
) -> Response {
    if !super::is_valid_eth_address(&addr.to_lowercase()) {
        return bad_request("Address must be a valid Ethereum address");
    }
    let q = match parse_query(query, valid_collection_types) {
        Ok(q) => q,
        Err(resp) => return *resp,
    };
    let on_chain_allowed =
        q.collection_types.is_empty() || q.collection_types.iter().any(|t| t == "on-chain");
    if on_chain_allowed && let Err(envelope) = readers::require_both_collections(state) {
        return envelope;
    }
    let addr_lc = addr.to_lowercase();
    let mut ct_sorted = q.collection_types.clone();
    ct_sorted.sort();
    let mut cats_sorted = q.categories.clone();
    cats_sorted.sort();
    let filter_key = format!(
        "name={}|cats={}|rarity={}|trimmed={}",
        q.name.as_deref().unwrap_or(""),
        cats_sorted.join(","),
        q.rarity.as_deref().unwrap_or(""),
        q.trimmed,
    );
    let cache_key: ExplorerKey = (
        addr_lc.clone(),
        q.sort.clone(),
        q.direction.clone(),
        ct_sorted.join(","),
        category,
        filter_key,
    );
    let (page_num, page_size) = (q.page_num, q.page_size);
    let valid: Vec<&'static str> = valid_collection_types.to_vec();
    let result = state
        .explorer_cache
        .get_or_fetch(cache_key, || {
            compute_explorer_items(state, &addr_lc, q, category, &valid)
        })
        .await;
    match result {
        Ok(v) => {
            let total = v.get("totalAmount").cloned().unwrap_or_else(|| json!(0));
            let offset = page_num.saturating_sub(1).saturating_mul(page_size).max(0) as usize;
            let limit = page_size.max(0) as usize;
            let page: Vec<Value> = v
                .get("elements")
                .and_then(|e| e.as_array())
                .map(|all| all.iter().skip(offset).take(limit).cloned().collect())
                .unwrap_or_default();
            Json(json!({ "elements": page, "totalAmount": total, "pageNum": page_num, "pageSize": page_size })).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, address = %addr_lc, category, "explorer backpack reader failed");
            service_unavailable("ownership readers unavailable")
        }
    }
}

async fn compute_explorer_items(
    state: &MarketplaceState,
    addr: &str,
    q: ExplorerQuery,
    category: &'static str,
    valid_collection_types: &[&'static str],
) -> Result<Value, String> {
    let base_allowed = valid_collection_types.contains(&"base-wearable")
        && (q.collection_types.is_empty()
            || q.collection_types.iter().any(|t| t == "base-wearable"));
    let on_chain_allowed =
        q.collection_types.is_empty() || q.collection_types.iter().any(|t| t == "on-chain");
    let third_party_allowed = valid_collection_types.contains(&"third-party")
        && (q.collection_types.is_empty() || q.collection_types.iter().any(|t| t == "third-party"));
    let base_leg = async {
        if base_allowed {
            fetch_base_items(state).await
        } else {
            Vec::new()
        }
    };
    let owned_leg = async {
        if on_chain_allowed {
            fetch_owned_items(state, addr, category).await
        } else {
            Ok(Vec::new())
        }
    };
    let third_party_leg = async {
        if third_party_allowed {
            fetch_third_party_items(state, addr).await
        } else {
            Vec::new()
        }
    };
    let (mut items, owned, third_party) = tokio::join!(base_leg, owned_leg, third_party_leg);
    items.extend(owned?);
    items.extend(third_party);
    items.retain(|it| passes_filters(it, &q));
    sort_items(&mut items, &q.sort, &q.direction);
    let total = items.len() as i64;
    let include_item_type = category == "wearable";
    let elements: Vec<Value> = items
        .iter()
        .map(|it| {
            if q.trimmed {
                item_to_trimmed_value(it)
            } else {
                item_to_value(it, include_item_type)
            }
        })
        .collect();
    Ok(json!({ "elements": elements, "totalAmount": total }))
}

/// `GET /lambdas/explorer/{address}/wearables`, `/explorer/{address}/wearables`.
pub async fn explorer_wearables(
    State(state): State<SharedState>,
    Path(addr): Path<String>,
    req: Request,
) -> Response {
    let query = req.uri().query().map(|q| q.to_string());
    explorer_items(
        &state,
        &addr,
        query.as_deref(),
        "wearable",
        &VALID_COLLECTION_TYPES,
    )
    .await
}

/// `GET /lambdas/explorer/{address}/emotes`, `/explorer/{address}/emotes`.
pub async fn explorer_emotes(
    State(state): State<SharedState>,
    Path(addr): Path<String>,
    req: Request,
) -> Response {
    let query = req.uri().query().map(|q| q.to_string());
    explorer_items(&state, &addr, query.as_deref(), "emote", &["on-chain"]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(urn: &str, name: &str, rarity: &str, has_date: bool, max_at: f64) -> OwnedItem {
        OwnedItem {
            urn: urn.to_string(),
            individual_data: Vec::new(),
            rarity: rarity.to_string(),
            item_type: None,
            amount: 1,
            name: name.to_string(),
            category: String::new(),
            min_transferred_at: max_at,
            max_transferred_at: max_at,
            entity: Value::Null,
            is_base: !has_date,
            is_third_party: false,
            has_date,
        }
    }

    fn urns(items: &[OwnedItem]) -> Vec<&str> {
        items.iter().map(|i| i.urn.as_str()).collect()
    }

    #[test]
    fn locale_cmp_places_basefemale_between_balbo_and_basketball() {
        let mut v = vec!["basketball_shorts", "BaseFemale", "balbo_beard"];
        v.sort_by(|a, b| locale_cmp(a, b));
        assert_eq!(v, vec!["balbo_beard", "BaseFemale", "basketball_shorts"]);
        assert_eq!(locale_cmp("abc", "ABC"), std::cmp::Ordering::Less);
    }

    #[test]
    fn sort_rarity_rarest_optional_has_before_hasnot() {
        let mut items = vec![
            owned("urn:base_b", "B", "", false, 0.0),
            owned("urn:onchain_a", "A", "common", true, 0.0),
            owned("urn:onchain_c", "C", "legendary", true, 0.0),
            owned("urn:base_a", "A", "", false, 0.0),
        ];
        sort_items(&mut items, "rarity", "DESC");
        assert_eq!(
            urns(&items),
            vec!["urn:onchain_c", "urn:onchain_a", "urn:base_a", "urn:base_b"]
        );
    }

    #[test]
    fn sort_rarity_asc_reverses() {
        let mut items = vec![
            owned("urn:onchain_a", "A", "common", true, 0.0),
            owned("urn:base_a", "A", "", false, 0.0),
            owned("urn:onchain_c", "C", "legendary", true, 0.0),
        ];
        sort_items(&mut items, "rarity", "ASC");
        assert_eq!(
            urns(&items),
            vec!["urn:base_a", "urn:onchain_a", "urn:onchain_c"]
        );
    }

    #[test]
    fn sort_date_newest_optional() {
        let mut items = vec![
            owned("urn:base", "Base", "", false, 0.0),
            owned("urn:old", "Old", "common", true, 5.0),
            owned("urn:new", "New", "common", true, 9.0),
        ];
        sort_items(&mut items, "date", "DESC");
        assert_eq!(urns(&items), vec!["urn:new", "urn:old", "urn:base"]);
    }

    #[test]
    fn the_query_validates_types_rarity_and_sort() {
        assert!(parse_query(Some("collectionType=nope"), &VALID_COLLECTION_TYPES).is_err());
        assert!(
            parse_query(Some("collectionType=third-party"), &["on-chain"]).is_err(),
            "emotes take on-chain only"
        );
        assert!(parse_query(Some("rarity=ultra"), &VALID_COLLECTION_TYPES).is_err());
        assert!(parse_query(Some("orderBy=price"), &VALID_COLLECTION_TYPES).is_err());
        let q = parse_query(
            Some("orderBy=NAME&trimmed=1&category=Hat"),
            &VALID_COLLECTION_TYPES,
        )
        .unwrap_or_else(|_| panic!("parse"));
        assert_eq!(
            (q.sort.as_str(), q.direction.as_str(), q.trimmed),
            ("name", "ASC", true)
        );
        assert_eq!(q.categories, vec!["hat".to_string()]);
    }

    #[test]
    fn a_trimmed_element_carries_the_thumbnail_hash_and_the_copies() {
        let mut item = owned("urn:x", "Hat", "rare", true, 1.0);
        item.entity = json!({ "id": "bafyentity", "content": [{ "file": "thumbnail.png", "hash": "bafythumb" }], "metadata": { "name": "" } });
        item.individual_data = vec![json!({ "id": "urn:x:1", "tokenId": "1" })];
        let v = item_to_trimmed_value(&item);
        assert_eq!(v["entity"]["thumbnail"], "bafythumb");
        assert_eq!(v["entity"]["metadata"]["name"], "Hat");
        assert_eq!(v["entity"]["individualData"][0]["tokenId"], "1");
        assert_eq!(v["amount"], 1);
        let full = item_to_value(&item, true);
        assert_eq!(full["type"], "on-chain");
        assert_eq!(full["itemType"], "");
    }
}
