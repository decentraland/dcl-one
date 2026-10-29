//! `/lambdas/users/{address}/{wearables,emotes,names}` and the grouped
//! `/lambdas/collections/*-by-owner/{owner}` lists over the subgraph readers.
//! The old queries grouped in SQL over the squid; here the owner's copies
//! come back from the subgraph newest first and are grouped, filtered, sorted
//! (UCA order) and paged in Rust exactly as the old handler did after its
//! query. Definitions come from the content entities peer sync ingested.
//! Without a reader the routes answer the unavailable envelope, as their
//! `/marketplace-api/v1/users` twins do: the explorer reads these lambdas,
//! and an empty page from an unconfigured node showed every avatar owning
//! nothing (the old `squid_pool: None` behaviour the stage-5 rule replaced).

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::content::queries::active_entity_docs_by_pointers;
use crate::shared::query_params::{
    MAX_PAGE_SIZE, NonPositivePolicy, OversizePolicy, Pagination, QueryParams,
    parse_pagination_with, parse_query_string, qs_get_string,
};

use super::definitions::{
    extract_emote_definition, extract_wearable_definition, locale_cmp, rarity_rank,
};
use super::graph::{self, OwnedCopy};
use super::readers;
use super::{MarketplaceState, SharedState, bad_request, service_unavailable};

fn pagination_object(
    params: &QueryParams,
    max_page_size: i64,
) -> Result<Pagination, Box<Response>> {
    parse_pagination_with(
        params,
        max_page_size,
        OversizePolicy::Reject,
        NonPositivePolicy::PassThrough,
    )
    .map_err(|msg| Box::new(bad_request(&msg)))
}

struct GroupedItem {
    urn: String,
    name: String,
    category: String,
    rarity: String,
    individual: Vec<IndividualData>,
    min_transferred_at: i64,
    max_transferred_at: i64,
}

struct IndividualData {
    token_id: String,
    transferred_at: i64,
    price: String,
}

/// One group per item urn in first-seen (newest copy) order; the representative
/// fields come from the most recent copy, as the old `array_agg(... ORDER BY
/// transferred_at DESC)[1]` did.
fn group_copies(copies: &[OwnedCopy]) -> Vec<GroupedItem> {
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut groups: Vec<GroupedItem> = Vec::new();
    for c in copies {
        let idx = match index.get(c.urn.as_str()) {
            Some(i) => *i,
            None => {
                groups.push(GroupedItem {
                    urn: c.urn.clone(),
                    name: c.name.clone(),
                    category: c.category.clone(),
                    rarity: c.rarity.clone(),
                    individual: Vec::new(),
                    min_transferred_at: c.transferred_at,
                    max_transferred_at: c.transferred_at,
                });
                index.insert(c.urn.as_str(), groups.len() - 1);
                groups.len() - 1
            }
        };
        let g = &mut groups[idx];
        g.min_transferred_at = g.min_transferred_at.min(c.transferred_at);
        g.max_transferred_at = g.max_transferred_at.max(c.transferred_at);
        g.individual.push(IndividualData {
            token_id: c.token_id.clone(),
            transferred_at: c.transferred_at,
            price: c.price.clone(),
        });
    }
    groups
}

fn filter_groups(
    items: &mut Vec<GroupedItem>,
    filter_category: Option<&str>,
    filter_rarity: Option<&str>,
    filter_name: Option<&str>,
) {
    if let Some(c) = filter_category {
        items.retain(|i| i.category == c);
    }
    if let Some(r) = filter_rarity {
        items.retain(|i| i.rarity == r);
    }
    if let Some(n) = filter_name {
        let needle = n.to_lowercase();
        items.retain(|i| i.name.to_lowercase().contains(&needle));
    }
}

fn validate_and_sort(items: &mut [GroupedItem], params: &QueryParams) -> Result<(), String> {
    let sort = qs_get_string(params, "orderBy")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "rarity".to_string());
    let direction = qs_get_string(params, "direction")
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| {
            if sort == "name" {
                "ASC".to_string()
            } else {
                "DESC".to_string()
            }
        });

    let by_urn = |a: &GroupedItem, b: &GroupedItem| locale_cmp(&a.urn, &b.urn);

    match (sort.as_str(), direction.as_str()) {
        ("rarity", "DESC") => items.sort_by(|a, b| {
            rarity_rank(&b.rarity)
                .cmp(&rarity_rank(&a.rarity))
                .then_with(|| by_urn(a, b))
        }),
        ("rarity", "ASC") => items.sort_by(|a, b| {
            rarity_rank(&b.rarity)
                .cmp(&rarity_rank(&a.rarity))
                .then_with(|| by_urn(a, b))
                .reverse()
        }),
        ("name", "ASC") => {
            items.sort_by(|a, b| locale_cmp(&a.name, &b.name).then_with(|| by_urn(a, b)))
        }
        ("name", "DESC") => items.sort_by(|a, b| {
            locale_cmp(&a.name, &b.name)
                .then_with(|| by_urn(a, b))
                .reverse()
        }),
        ("date", "ASC") => items.sort_by(|a, b| {
            a.min_transferred_at
                .cmp(&b.min_transferred_at)
                .then_with(|| by_urn(b, a))
        }),
        ("date", "DESC") => items.sort_by(|a, b| {
            b.max_transferred_at
                .cmp(&a.max_transferred_at)
                .then_with(|| by_urn(a, b))
        }),
        _ => return Err(format!("Invalid sorting requested: {sort} {direction}")),
    }
    Ok(())
}

fn grouped_to_json(item: &GroupedItem) -> Value {
    let individual: Vec<Value> = item
        .individual
        .iter()
        .map(|d| {
            json!({
                "id": format!("{}:{}", item.urn, d.token_id),
                "tokenId": d.token_id,
                "transferredAt": d.transferred_at.to_string(),
                "price": d.price,
            })
        })
        .collect();
    json!({
        "urn": item.urn,
        "amount": item.individual.len(),
        "individualData": individual,
        "name": item.name,
        "category": item.category,
        "rarity": item.rarity,
    })
}

/// JavaScript `slice` semantics on the sorted list: a negative offset counts
/// from the end, an end past the length clamps.
fn js_page(len: i64, offset: i64, limit: i64) -> (usize, usize) {
    let js_index = |i: i64| -> i64 { if i < 0 { (len + i).max(0) } else { i.min(len) } };
    let start = js_index(offset);
    let end = js_index(offset.saturating_add(limit));
    if start < end {
        (start as usize, end as usize)
    } else {
        (0, 0)
    }
}

async fn entities_by_pointer(
    state: &MarketplaceState,
    pointers: &[String],
) -> HashMap<String, Value> {
    let mut by_pointer: HashMap<String, Value> = HashMap::new();
    if pointers.is_empty() {
        return by_pointer;
    }
    let docs = active_entity_docs_by_pointers(&state.pool, pointers)
        .await
        .unwrap_or_default();
    for ent in docs.iter().filter_map(|d| d.to_value()) {
        if let Some(ptrs) = ent.get("pointers").and_then(|p| p.as_array()) {
            for p in ptrs.iter().filter_map(Value::as_str) {
                by_pointer.insert(p.to_lowercase(), ent.clone());
            }
        }
    }
    by_pointer
}

async fn owned_items_response(
    state: &MarketplaceState,
    address: &str,
    item_category: &str,
    query_string: &str,
) -> Response {
    // Before any reader call, as the old lambdas did: an arbitrary path
    // string is otherwise forwarded to the subgraphs as `$owner` (two
    // queries per distinct string) and cached under it.
    if !super::is_valid_eth_address(address) {
        return bad_request("Address must be a valid Ethereum address");
    }
    let address = &address.to_lowercase();
    let params = parse_query_string(query_string);

    let include_definitions = params.contains_key("includeDefinitions");
    let include_entities = params.contains_key("includeEntities");
    if include_definitions && include_entities {
        return bad_request("Cannot use includeEntities and includeDefinitions together");
    }

    let pagination = match pagination_object(&params, MAX_PAGE_SIZE as i64) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    if let Err(msg) = validate_and_sort(&mut [], &params) {
        return bad_request(&msg);
    }

    if let Err(envelope) = readers::require_both_collections(state) {
        return envelope;
    }

    let copies = match graph::owned_copies(&state.index, address, item_category).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, address, item_category, "owned items reader failed");
            return service_unavailable("ownership readers unavailable");
        }
    };
    let mut items = group_copies(&copies);
    filter_groups(
        &mut items,
        qs_get_string(&params, "category").as_deref(),
        qs_get_string(&params, "rarity").as_deref(),
        qs_get_string(&params, "name").as_deref(),
    );

    if let Err(msg) = validate_and_sort(&mut items, &params) {
        return bad_request(&msg);
    }

    let total_amount = items.len();
    let (start, end) = js_page(total_amount as i64, pagination.offset, pagination.limit);
    let page: Vec<&GroupedItem> = items[start..end].iter().collect();

    let by_pointer = if include_definitions || include_entities {
        let pointers: Vec<String> = page.iter().map(|i| i.urn.to_lowercase()).collect();
        entities_by_pointer(state, &pointers).await
    } else {
        HashMap::new()
    };

    let content_public_url = &state.settings.content_public_url;
    let elements: Vec<Value> = page
        .iter()
        .map(|item| {
            let mut obj = grouped_to_json(item);
            let ent = by_pointer
                .get(&item.urn.to_lowercase())
                .cloned()
                .unwrap_or(Value::Null);
            if include_definitions {
                obj["definition"] = if ent.is_null() {
                    Value::Null
                } else if item_category == "emote" {
                    extract_emote_definition(&ent, content_public_url).unwrap_or(Value::Null)
                } else {
                    extract_wearable_definition(&ent, content_public_url).unwrap_or(Value::Null)
                };
            }
            if include_entities {
                obj["entity"] = ent;
            }
            obj
        })
        .collect();

    Json(json!({
        "elements": elements,
        "totalAmount": total_amount,
        "pageNum": pagination.page_num,
        "pageSize": pagination.page_size,
    }))
    .into_response()
}

pub async fn user_wearables(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    request: Request,
) -> Response {
    let query_string = request.uri().query().unwrap_or("").to_string();
    owned_items_response(&state, &address, "wearable", &query_string).await
}

pub async fn user_emotes(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    request: Request,
) -> Response {
    let query_string = request.uri().query().unwrap_or("").to_string();
    owned_items_response(&state, &address, "emote", &query_string).await
}

/// Names page with the land routes' pagination: an oversized page clamps to
/// 1000 and a non-positive page or size clamps to 1.
pub async fn user_names(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    request: Request,
) -> Response {
    if !super::is_valid_eth_address(&address) {
        return bad_request("Address must be a valid Ethereum address");
    }
    let address = address.to_lowercase();
    let params = parse_query_string(request.uri().query().unwrap_or(""));
    let p = parse_pagination_with(
        &params,
        MAX_PAGE_SIZE as i64,
        OversizePolicy::Clamp,
        NonPositivePolicy::ClampToOne,
    )
    .unwrap_or(Pagination {
        page_size: 1,
        page_num: 1,
        offset: 0,
        limit: 1,
    });
    if let Err(envelope) = readers::require_marketplace(&state) {
        return envelope;
    }
    let names = match graph::owned_names(&state.index, &address).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(error = %e, address, "names reader failed");
            return service_unavailable("ownership readers unavailable");
        }
    };
    let total = names.len();
    let (start, end) = js_page(total as i64, p.offset, p.limit);
    Json(json!({
        "elements": names[start..end],
        "totalAmount": total,
        "pageNum": p.page_num,
        "pageSize": p.page_size,
    }))
    .into_response()
}

fn sort_owned_by_rarity_then_urn(rows: &mut [(String, i64, String)]) {
    rows.sort_by(|a, b| {
        rarity_rank(&b.2)
            .cmp(&rarity_rank(&a.2))
            .then_with(|| locale_cmp(&a.0, &b.0))
    });
}

fn has_include_definitions(req: &Request) -> bool {
    req.uri()
        .query()
        .unwrap_or("")
        .split('&')
        .any(|p| p == "includeDefinitions" || p.starts_with("includeDefinitions="))
}

fn collection_id_param(req: &Request) -> Option<String> {
    let params = parse_query_string(req.uri().query().unwrap_or(""));
    qs_get_string(&params, "collectionId").filter(|s| !s.is_empty())
}

pub(crate) fn is_third_party_name_urn(urn: &str) -> bool {
    let p: Vec<&str> = urn.split(':').collect();
    p.len() == 5
        && p[0] == "urn"
        && p[1] == "decentraland"
        && p[3] == "collections-thirdparty"
        && !p[4].is_empty()
}

fn third_party_collection_name(collection_id: &str) -> Option<String> {
    let cleaned: String = collection_id
        .split(':')
        .take(5)
        .collect::<Vec<_>>()
        .join(":");
    if !is_third_party_name_urn(&cleaned) {
        return None;
    }
    cleaned.split(':').nth(4).map(|s| s.to_string())
}

async fn items_by_owner(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
    include_definitions: bool,
    collection_id: Option<&str>,
    extract: fn(&Value, &str) -> Option<Value>,
) -> Response {
    if let Some(collection_id) = collection_id {
        // The provider's mapped wearables the owner holds on chain (`third_party`).
        let Some(name) = third_party_collection_name(collection_id) else {
            return bad_request("'collectionId' must be a valid third-party collection URN");
        };
        let content_public_url = &state.settings.content_public_url;
        let body: Vec<Value> = super::third_party::owned_third_party_urns(state, owner, &name)
            .await
            .into_iter()
            .map(|(urn, amount, entity)| {
                let mut obj = json!({ "urn": urn, "amount": amount });
                if include_definitions && let Some(def) = extract(&entity, content_public_url) {
                    obj["definition"] = def;
                }
                obj
            })
            .collect();
        return Json(json!(body)).into_response();
    }
    if let Err(envelope) = readers::require_both_collections(state) {
        return envelope;
    }
    let key = (
        owner.to_lowercase(),
        category.to_string(),
        include_definitions,
    );
    let body = state
        .by_owner_cache
        .get_or_fetch(key, || async {
            owned_items_body(state, owner, category, include_definitions, extract).await
        })
        .await;
    match body {
        Ok(body) => Json(body).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, owner, category, "items by owner reader failed");
            service_unavailable("ownership readers unavailable")
        }
    }
}

async fn owned_items_body(
    state: &MarketplaceState,
    owner: &str,
    category: &str,
    include_definitions: bool,
    extract: fn(&Value, &str) -> Option<Value>,
) -> Result<Value, String> {
    let copies = graph::owned_copies(&state.index, owner, category).await?;
    let mut rows: Vec<(String, i64, String)> = group_copies(&copies)
        .into_iter()
        .map(|g| {
            let rarity = g
                .individual
                .iter()
                .map(|_| g.rarity.as_str())
                .max()
                .unwrap_or("")
                .to_string();
            (g.urn, g.individual.len() as i64, rarity)
        })
        .collect();
    sort_owned_by_rarity_then_urn(&mut rows);

    if !include_definitions {
        return Ok(json!(
            rows.into_iter()
                .map(|(urn, amount, _)| json!({ "urn": urn, "amount": amount }))
                .collect::<Vec<_>>()
        ));
    }

    let pointers: Vec<String> = rows.iter().map(|(urn, ..)| urn.to_lowercase()).collect();
    let by_pointer = entities_by_pointer(state, &pointers).await;
    let content_public_url = &state.settings.content_public_url;
    let body: Vec<Value> = rows
        .into_iter()
        .map(|(urn, amount, _)| {
            let mut obj = json!({ "urn": urn, "amount": amount });
            if let Some(def) = by_pointer
                .get(&urn.to_lowercase())
                .and_then(|e| extract(e, content_public_url))
            {
                obj["definition"] = def;
            }
            obj
        })
        .collect();
    Ok(json!(body))
}

pub async fn wearables_by_owner(
    State(state): State<SharedState>,
    Path(owner): Path<String>,
    req: Request,
) -> Response {
    let include = has_include_definitions(&req);
    let collection_id = collection_id_param(&req);
    items_by_owner(
        &state,
        &owner,
        "wearable",
        include,
        collection_id.as_deref(),
        extract_wearable_definition,
    )
    .await
}

pub async fn emotes_by_owner(
    State(state): State<SharedState>,
    Path(owner): Path<String>,
    req: Request,
) -> Response {
    let include = has_include_definitions(&req);
    let collection_id = collection_id_param(&req);
    items_by_owner(
        &state,
        &owner,
        "emote",
        include,
        collection_id.as_deref(),
        extract_emote_definition,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAT: &str = "urn:decentraland:matic:collections-v2:0xc0ffee:1";
    const CAPE: &str = "urn:decentraland:matic:collections-v2:0xc0ffee:2";
    const OLD_HAT: &str = "urn:decentraland:ethereum:collections-v1:oldies:hat";

    fn copy(
        urn: &str,
        token: &str,
        at: i64,
        price: &str,
        rarity: &str,
        name: &str,
        category: &str,
    ) -> OwnedCopy {
        OwnedCopy {
            urn: urn.into(),
            token_id: token.into(),
            transferred_at: at,
            price: price.into(),
            rarity: rarity.into(),
            name: name.into(),
            category: category.into(),
        }
    }

    /// The same inventory the old SQL grouping test seeded, already newest first.
    fn inventory() -> Vec<OwnedCopy> {
        vec![
            copy(HAT, "3", 300, "10", "epic", "Hat", "hat"),
            copy(CAPE, "9", 250, "5", "rare", "Cape", "upper_body"),
            copy(HAT, "5", 200, "10", "epic", "Hat", "hat"),
            copy(HAT, "7", 100, "10", "epic", "Hat", "hat"),
            copy(OLD_HAT, "1", 50, "0", "", "", ""),
        ]
    }

    fn urns(items: &[GroupedItem]) -> Vec<&str> {
        items.iter().map(|i| i.urn.as_str()).collect()
    }

    #[test]
    fn owned_groups_come_back_grouped_and_filtered() {
        let mut items = group_copies(&inventory());
        items.sort_by(|a, b| a.urn.cmp(&b.urn));
        assert_eq!(urns(&items), vec![OLD_HAT, HAT, CAPE]);
        let hat = &items[1];
        assert_eq!(
            (
                hat.name.as_str(),
                hat.category.as_str(),
                hat.rarity.as_str()
            ),
            ("Hat", "hat", "epic")
        );
        assert_eq!((hat.min_transferred_at, hat.max_transferred_at), (100, 300));
        let copies: Vec<(&str, i64, &str)> = hat
            .individual
            .iter()
            .map(|d| (d.token_id.as_str(), d.transferred_at, d.price.as_str()))
            .collect();
        assert_eq!(
            copies,
            vec![("3", 300, "10"), ("5", 200, "10"), ("7", 100, "10")],
            "copies newest first"
        );
        let old = &items[0];
        assert_eq!(
            (
                old.name.as_str(),
                old.category.as_str(),
                old.rarity.as_str()
            ),
            ("", "", "")
        );
        assert_eq!(old.individual[0].price, "0", "no item row means price 0");

        let mut by_category = group_copies(&inventory());
        filter_groups(&mut by_category, Some("hat"), None, None);
        assert_eq!(urns(&by_category), vec![HAT]);
        let mut by_rarity = group_copies(&inventory());
        filter_groups(&mut by_rarity, None, Some("rare"), None);
        assert_eq!(urns(&by_rarity), vec![CAPE]);
        let mut by_name = group_copies(&inventory());
        filter_groups(&mut by_name, None, None, Some("cAp"));
        assert_eq!(urns(&by_name), vec![CAPE]);
        let mut none = group_copies(&inventory());
        filter_groups(&mut none, Some("hat"), Some("rare"), None);
        assert!(none.is_empty());
        let mut empty_category = group_copies(&inventory());
        filter_groups(&mut empty_category, Some(""), None, None);
        assert_eq!(
            urns(&empty_category),
            vec![OLD_HAT],
            "an empty filter keeps unnamed items"
        );
    }

    fn item(urn: &str, name: &str, rarity: &str, min_at: i64, max_at: i64) -> GroupedItem {
        GroupedItem {
            urn: urn.to_string(),
            name: name.to_string(),
            category: "hat".to_string(),
            rarity: rarity.to_string(),
            individual: Vec::new(),
            min_transferred_at: min_at,
            max_transferred_at: max_at,
        }
    }

    #[test]
    fn sort_rarest_then_urn() {
        let mut items = vec![
            item("urn:b", "B", "common", 0, 0),
            item("urn:a", "A", "legendary", 0, 0),
            item("urn:c", "C", "legendary", 0, 0),
        ];
        validate_and_sort(
            &mut items,
            &parse_query_string("orderBy=rarity&direction=DESC"),
        )
        .unwrap();
        assert_eq!(urns(&items), vec!["urn:a", "urn:c", "urn:b"]);
    }

    #[test]
    fn sort_least_rare_is_full_reverse() {
        let mk = || {
            vec![
                item("urn:b", "B", "common", 0, 0),
                item("urn:a", "A", "legendary", 0, 0),
                item("urn:c", "C", "legendary", 0, 0),
            ]
        };
        let (mut rarest, mut least) = (mk(), mk());
        validate_and_sort(
            &mut rarest,
            &parse_query_string("orderBy=rarity&direction=DESC"),
        )
        .unwrap();
        validate_and_sort(
            &mut least,
            &parse_query_string("orderBy=rarity&direction=ASC"),
        )
        .unwrap();
        let rev: Vec<&str> = urns(&rarest).into_iter().rev().collect();
        assert_eq!(urns(&least), rev);
        assert_eq!(urns(&least), vec!["urn:b", "urn:c", "urn:a"]);
    }

    #[test]
    fn sort_name_az_and_za_and_default_direction() {
        let mut az = vec![
            item("urn:2", "Zebra", "common", 0, 0),
            item("urn:1", "Apple", "common", 0, 0),
        ];
        validate_and_sort(&mut az, &parse_query_string("orderBy=name&direction=ASC")).unwrap();
        assert_eq!(urns(&az), vec!["urn:1", "urn:2"]);
        let mut za = vec![
            item("urn:1", "Apple", "common", 0, 0),
            item("urn:2", "Zebra", "common", 0, 0),
        ];
        validate_and_sort(&mut za, &parse_query_string("orderBy=name&direction=DESC")).unwrap();
        assert_eq!(urns(&za), vec!["urn:2", "urn:1"]);
        let mut items = vec![
            item("urn:2", "Zebra", "common", 0, 0),
            item("urn:1", "Apple", "common", 0, 0),
        ];
        validate_and_sort(&mut items, &parse_query_string("orderBy=name")).unwrap();
        assert_eq!(urns(&items), vec!["urn:1", "urn:2"]);
    }

    #[test]
    fn sort_date_oldest_has_reversed_urn_tiebreak_and_newest_orders_by_max() {
        let mut oldest = vec![
            item("urn:a", "A", "common", 10, 10),
            item("urn:b", "B", "common", 10, 10),
        ];
        validate_and_sort(
            &mut oldest,
            &parse_query_string("orderBy=date&direction=ASC"),
        )
        .unwrap();
        assert_eq!(urns(&oldest), vec!["urn:b", "urn:a"]);
        let mut newest = vec![
            item("urn:b", "B", "common", 10, 10),
            item("urn:a", "A", "common", 10, 10),
        ];
        validate_and_sort(
            &mut newest,
            &parse_query_string("orderBy=date&direction=DESC"),
        )
        .unwrap();
        assert_eq!(urns(&newest), vec!["urn:a", "urn:b"]);
        let mut items = vec![
            item("urn:a", "A", "common", 5, 5),
            item("urn:b", "B", "common", 9, 9),
        ];
        validate_and_sort(
            &mut items,
            &parse_query_string("orderBy=date&direction=DESC"),
        )
        .unwrap();
        assert_eq!(urns(&items), vec!["urn:b", "urn:a"]);
    }

    #[test]
    fn invalid_sort_combination_errors_with_reference_message() {
        let mut items = vec![item("urn:a", "A", "common", 0, 0)];
        let err = validate_and_sort(
            &mut items,
            &parse_query_string("orderBy=bogus&direction=ASC"),
        )
        .unwrap_err();
        assert_eq!(err, "Invalid sorting requested: bogus ASC");
        let err2 = validate_and_sort(
            &mut items,
            &parse_query_string("orderBy=rarity&direction=sideways"),
        )
        .unwrap_err();
        assert_eq!(err2, "Invalid sorting requested: rarity SIDEWAYS");
    }

    #[test]
    fn default_orderby_is_rarity_desc() {
        let mut items = vec![
            item("urn:b", "B", "common", 0, 0),
            item("urn:a", "A", "legendary", 0, 0),
        ];
        validate_and_sort(&mut items, &parse_query_string("")).unwrap();
        assert_eq!(urns(&items), vec!["urn:a", "urn:b"]);
    }

    #[test]
    fn js_page_slices_like_javascript() {
        assert_eq!(js_page(10, 0, 3), (0, 3));
        assert_eq!(js_page(10, 8, 5), (8, 10));
        assert_eq!(js_page(10, 12, 5), (0, 0));
        assert_eq!(js_page(10, -3, 2), (7, 9));
        assert_eq!(js_page(0, 0, 100), (0, 0));
    }

    #[test]
    fn grouped_json_shape() {
        let g = group_copies(&inventory());
        let v = grouped_to_json(&g[0]);
        assert_eq!(v["urn"], HAT);
        assert_eq!(v["amount"], 3);
        assert_eq!(
            v["individualData"][0],
            json!({ "id": format!("{HAT}:3"), "tokenId": "3", "transferredAt": "300", "price": "10" })
        );
        assert_eq!(v["rarity"], "epic");
    }

    #[test]
    fn by_owner_rows_sort_rarity_desc_then_urn_asc() {
        let row = |urn: &str, rarity: &str| (urn.to_string(), 1_i64, rarity.to_string());
        let mut rows = vec![
            row("urn:z:common", "common"),
            row("urn:b:mythic", "mythic"),
            row("urn:a:unknown", ""),
            row("urn:a:mythic", "mythic"),
            row("urn:a:unique", "unique"),
            row("urn:a:common", "common"),
        ];
        sort_owned_by_rarity_then_urn(&mut rows);
        let urns: Vec<&str> = rows.iter().map(|(urn, ..)| urn.as_str()).collect();
        assert_eq!(
            urns,
            vec![
                "urn:a:unique",
                "urn:a:mythic",
                "urn:b:mythic",
                "urn:a:common",
                "urn:z:common",
                "urn:a:unknown"
            ],
            "rarity rank DESC (unknown last), then URN ASC"
        );
    }

    #[test]
    fn third_party_collection_name_parsing() {
        assert_eq!(
            third_party_collection_name(
                "urn:decentraland:matic:collections-thirdparty:cryptomotors"
            ),
            Some("cryptomotors".to_string())
        );
        assert_eq!(
            third_party_collection_name(
                "urn:decentraland:matic:collections-thirdparty:cryptomotors:car:1"
            ),
            Some("cryptomotors".to_string())
        );
        assert_eq!(
            third_party_collection_name("urn:decentraland:matic:collections-v2:0xabc:0"),
            None
        );
        assert_eq!(
            third_party_collection_name("urn:decentraland:off-chain:base-avatars:eyebrows_00"),
            None
        );
        assert_eq!(third_party_collection_name("not-a-urn"), None);
        assert_eq!(
            third_party_collection_name("urn:decentraland:matic:collections-thirdparty:"),
            None
        );
    }

    #[test]
    fn collection_id_param_absent_and_empty_are_none() {
        let req = |q: &str| {
            axum::http::Request::builder()
                .uri(format!("http://x/collections/wearables-by-owner/0xabc?{q}"))
                .body(axum::body::Body::empty())
                .unwrap()
        };
        assert_eq!(collection_id_param(&req("includeDefinitions")), None);
        assert!(has_include_definitions(&req("includeDefinitions")));
        assert!(!has_include_definitions(&req("collectionId=x")));
        assert_eq!(collection_id_param(&req("collectionId=")), None);
        assert_eq!(
            collection_id_param(&req(
                "collectionId=urn:decentraland:matic:collections-thirdparty:cryptomotors"
            )),
            Some("urn:decentraland:matic:collections-thirdparty:cryptomotors".to_string())
        );
    }
}
