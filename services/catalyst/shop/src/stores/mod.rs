// from: shop@94584a5c:app/src/lib/store.ts (fetchStore, buildStoreMetadata, saveStore: the STORE entity and its owner-only deploy), app/src/lib/storeStats.ts (the store dashboard's per-collection and per-item figures), app/src/lib/sales.ts (fetchSalesSummary); marketplace@f3956826:webapp/src/modules/store/{sagas,utils}.ts (FETCH_STORE, UPDATE_STORE)
//! Stores: a creator's storefront settings (`GET /v1/stores/{address}`) and
//! the owner's store dashboard figures (`GET /v1/stores/{address}/stats`).
//!
//! The store itself is a content entity of type `store` at the pointer
//! `urn:decentraland:off-chain:marketplace-stores:<owner>` (the classic
//! Marketplace's account banner reads the same one), so ownership is the
//! content server's rule, not a Shop table: the deploy validator admits a
//! STORE entity only when its signer is the pointer's owner
//! (`content::validator::checker::validate_store_access`), which is the
//! owner-only management SHOP.md asks for; the Shop reads the active entity
//! from the same `content.active_pointers`/`deployments` rows the
//! `/content/entities/active` route serves and shapes it as the upstream
//! `CreatorStore` (cover URL on this node, description, the four links),
//! `exists: false` with the empty store when the owner never published one
//! (upstream's `fetchStore` answered the empty store on 404 and on error
//! alike; here an error is an error). Saving stays a signed content deploy
//! from the client; no Shop write exists to bypass the validator.
//!
//! The stats are the exact aggregate the upstream dashboard computed on the
//! client from a capped page of sales rows (`storeStats.ts`, which says so):
//! per collection and per item, sold and earned over the window, lifetime
//! mints per item, the seller's royalties as a creator, from the bounded
//! sales reader (`market::sales::sales_summary`, over the collections
//! subgraphs) joined with the items mirror for names, supply and prices.
//! Every amount is MANA wei, whatever the listing was priced in: the trade
//! settled in MANA and that is what the seller received (a resale's
//! proceeds are MANA; SHOP.md). A reader that is not configured, or a
//! seller past the reader's bound, is the unavailable envelope.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

use super::market::sales::{self, SalesSummary, SalesSummaryFilters};
use super::market::wire::{ApiError, Params};
use super::market::{MarketplaceState, SharedState, is_valid_eth_address};

pub const STORE_URN_PREFIX: &str = "urn:decentraland:off-chain:marketplace-stores:";
pub const LINK_NAMES: [&str; 4] = ["website", "twitter", "discord", "facebook"];

pub fn store_urn(owner: &str) -> String {
    format!("{STORE_URN_PREFIX}{}", owner.to_lowercase())
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct StoreLinks {
    pub website: String,
    pub twitter: String,
    pub discord: String,
    pub facebook: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub owner: String,
    pub urn: String,
    /// Whether an active STORE entity exists for the owner.
    pub exists: bool,
    pub entity_id: Option<String>,
    /// The entity's timestamp, milliseconds.
    pub updated_at: Option<i64>,
    /// The cover's URL on this node (`<content>/contents/<hash>`), empty without one.
    pub cover: String,
    pub cover_hash: String,
    pub description: String,
    pub links: StoreLinks,
}

impl Store {
    pub fn empty(owner: &str) -> Self {
        Self {
            owner: owner.to_lowercase(),
            urn: store_urn(owner),
            exists: false,
            entity_id: None,
            updated_at: None,
            cover: String::new(),
            cover_hash: String::new(),
            description: String::new(),
            links: StoreLinks::default(),
        }
    }
}

/// Upstream `linkUrl`: a link is kept only under `https://`.
fn link_url(links: Option<&Value>, name: &str) -> String {
    links
        .and_then(Value::as_array)
        .and_then(|l| {
            l.iter()
                .find(|x| x.get("name").and_then(Value::as_str) == Some(name))
        })
        .and_then(|x| x.get("url").and_then(Value::as_str))
        .filter(|u| u.starts_with("https://"))
        .unwrap_or("")
        .to_string()
}

/// The upstream `fetchStore` mapping over an active entity document.
pub fn store_from_entity(owner: &str, entity: &Value, content_public_url: &str) -> Store {
    let mut store = Store::empty(owner);
    let Some(metadata) = entity.get("metadata").filter(|m| m.is_object()) else {
        return store;
    };
    store.exists = true;
    store.entity_id = entity.get("id").and_then(Value::as_str).map(str::to_string);
    store.updated_at = entity.get("timestamp").and_then(Value::as_i64);
    let cover_file = metadata
        .get("images")
        .and_then(Value::as_array)
        .and_then(|imgs| {
            imgs.iter()
                .find(|i| i.get("name").and_then(Value::as_str) == Some("cover"))
        })
        .and_then(|i| i.get("file").and_then(Value::as_str));
    let cover_hash = cover_file.and_then(|file| {
        entity
            .get("content")
            .and_then(Value::as_array)
            .and_then(|c| {
                c.iter()
                    .find(|x| x.get("file").and_then(Value::as_str) == Some(file))
            })
            .and_then(|x| x.get("hash").and_then(Value::as_str))
    });
    if let Some(hash) = cover_hash {
        store.cover_hash = hash.to_string();
        store.cover = format!(
            "{}/contents/{hash}",
            content_public_url.trim_end_matches('/')
        );
    }
    store.description = metadata
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let links = metadata.get("links");
    store.links = StoreLinks {
        website: link_url(links, "website"),
        twitter: link_url(links, "twitter"),
        discord: link_url(links, "discord"),
        facebook: link_url(links, "facebook"),
    };
    store
}

pub async fn read_store(state: &MarketplaceState, owner: &str) -> Result<Store, ApiError> {
    let docs =
        crate::content::queries::active_entity_docs_by_pointers(&state.pool, &[store_urn(owner)])
            .await?;
    let Some(doc) = docs.into_iter().next() else {
        return Ok(Store::empty(owner));
    };
    let entity: Value = serde_json::from_slice(&doc.json)
        .map_err(|e| ApiError::internal(format!("store entity: {e}")))?;
    if entity.get("type").and_then(Value::as_str) != Some("store") {
        return Ok(Store::empty(owner));
    }
    Ok(store_from_entity(
        owner,
        &entity,
        &state.settings.content_public_url,
    ))
}

fn ok<T: Serialize>(data: T) -> Response {
    Json(serde_json::json!({ "ok": true, "data": data })).into_response()
}

/// `GET /v1/stores/{address}`.
pub async fn get_store(State(state): State<SharedState>, Path(address): Path<String>) -> Response {
    let owner = address.to_lowercase();
    if !is_valid_eth_address(&owner) {
        return ApiError::bad_request("the store owner must be an Ethereum address")
            .into_response();
    }
    match read_store(&state, &owner).await {
        Ok(store) => ([(CACHE_CONTROL, "public, max-age=60")], ok(store)).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoreItemStats {
    pub contract_address: String,
    pub item_id: String,
    pub name: String,
    pub thumbnail: Option<String>,
    pub rarity: String,
    pub category: String,
    /// Copies still mintable and copies minted, from the item's own supply.
    pub available: String,
    pub minted: String,
    pub max_supply: String,
    /// The CollectionStore price, MANA wei (`0` = not listed there).
    pub price_wei: String,
    /// First sales over the window, and what they brought in, MANA wei.
    pub sold: i64,
    pub earned_wei: String,
    /// First sales over the item's whole life.
    pub sold_lifetime: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoreCollectionStats {
    pub contract_address: String,
    pub name: String,
    pub is_approved: bool,
    pub sold: i64,
    pub earned_wei: String,
    pub items: Vec<StoreItemStats>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoreStats {
    pub owner: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub currency: &'static str,
    pub summary: SalesSummary,
    pub collections: Vec<StoreCollectionStats>,
}

/// The window's first sales per item, `(contract, item) -> (sold, earned wei)`.
pub fn per_item_window(
    rows: &[Value],
    from: Option<i64>,
    to: Option<i64>,
) -> std::collections::BTreeMap<(String, String), (i64, u128)> {
    let mut out: std::collections::BTreeMap<(String, String), (i64, u128)> = Default::default();
    for s in rows {
        if super::market::readers::str_of(s, "type") != "mint" {
            continue;
        }
        let ts = super::market::readers::int_of(s, "timestamp") * 1000;
        if from.is_some_and(|f| ts < f) || to.is_some_and(|t| ts > t) {
            continue;
        }
        let Some(item) = super::market::readers::opt_str_of(s, "searchItemId") else {
            continue;
        };
        let e = out
            .entry((
                super::market::readers::str_of(s, "searchContractAddress"),
                item,
            ))
            .or_default();
        e.0 += 1;
        e.1 = e.1.saturating_add(
            super::market::readers::str_of(s, "price")
                .parse::<u128>()
                .unwrap_or(0),
        );
    }
    out
}

pub async fn store_stats(
    state: &MarketplaceState,
    filters: &SalesSummaryFilters,
) -> Result<StoreStats, Response> {
    let seller = filters.seller.to_lowercase();
    let rows = sales::seller_sales_rows(state, &seller, "/v1/stores/{address}/stats").await?;
    let summary = sales::sales_summary(state, filters).await?;
    let window = per_item_window(&rows, filters.from, filters.to);
    let items = sqlx::query(
        "SELECT i.contract_address, i.item_id::text AS item_id, i.name, i.image, i.rarity, i.category, i.item_type, i.available::text AS available, \
                i.total_supply::text AS total_supply, i.max_supply::text AS max_supply, i.price::text AS price, i.created_at, c.name AS collection_name, c.is_approved \
         FROM shop.items i JOIN shop.collections c ON c.contract_address = i.contract_address AND c.network = i.network \
         WHERE i.creator = $1 ORDER BY i.contract_address, i.item_id",
    )
    .bind(&seller)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::from(e).into_response())?;
    let lifetime: std::collections::BTreeMap<(String, String), i64> = summary
        .by_item
        .iter()
        .map(|i| {
            (
                (i.contract_address.clone(), i.item_id.clone()),
                i.sold_lifetime,
            )
        })
        .collect();
    let mut collections: Vec<StoreCollectionStats> = Vec::new();
    for r in &items {
        let contract: String = r.get("contract_address");
        let item_id: String = r.get("item_id");
        let (sold, earned) = window
            .get(&(contract.clone(), item_id.clone()))
            .copied()
            .unwrap_or((0, 0));
        let item = StoreItemStats {
            contract_address: contract.clone(),
            item_id: item_id.clone(),
            name: r.get("name"),
            thumbnail: r.get("image"),
            rarity: r.get("rarity"),
            category: if r.get::<String, _>("item_type").starts_with("emote") {
                "emote".into()
            } else {
                "wearable".into()
            },
            available: r.get("available"),
            minted: r.get("total_supply"),
            max_supply: r.get("max_supply"),
            price_wei: r.get("price"),
            sold,
            earned_wei: earned.to_string(),
            sold_lifetime: lifetime
                .get(&(contract.clone(), item_id))
                .copied()
                .unwrap_or(0),
            created_at: r.get::<i64, _>("created_at") * 1000,
        };
        match collections.last_mut() {
            Some(c) if c.contract_address == contract => c.items.push(item),
            _ => collections.push(StoreCollectionStats {
                contract_address: contract,
                name: r.get("collection_name"),
                is_approved: r.get("is_approved"),
                sold: 0,
                earned_wei: "0".into(),
                items: vec![item],
            }),
        }
    }
    for c in &mut collections {
        let sold = c.items.iter().map(|i| i.sold).sum::<i64>();
        let earned = c.items.iter().fold(0u128, |acc, i| {
            acc.saturating_add(i.earned_wei.parse::<u128>().unwrap_or(0))
        });
        c.sold = sold;
        c.earned_wei = earned.to_string();
    }
    Ok(StoreStats {
        owner: seller,
        from: filters.from,
        to: filters.to,
        currency: "MANA",
        summary,
        collections,
    })
}

/// `GET /v1/stores/{address}/stats?from=&to=` (epoch milliseconds).
pub async fn get_store_stats(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let owner = address.to_lowercase();
    if !is_valid_eth_address(&owner) {
        return ApiError::bad_request("the store owner must be an Ethereum address")
            .into_response();
    }
    let mut with_seller: Vec<(String, String)> =
        pairs.into_iter().filter(|(k, _)| k != "seller").collect();
    with_seller.push(("seller".to_string(), owner));
    let filters = match sales::parse_summary_filters(&with_seller) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };
    let _ = Params::new(&with_seller);
    match store_stats(&state, &filters).await {
        Ok(stats) => ([(CACHE_CONTROL, "private, max-age=60")], ok(stats)).into_response(),
        Err(r) => r,
    }
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/v1/stores/{address}", get(get_store))
        .route("/v1/stores/{address}/stats", get(get_store_stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const OWNER: &str = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    fn entity() -> Value {
        json!({
            "id": "bafkreistore", "type": "store", "timestamp": 1_700_000_000_000i64, "pointers": [store_urn(OWNER)],
            "content": [{ "file": "cover/cover.jpeg", "hash": "bafkreicover" }],
            "metadata": {
                "id": store_urn(OWNER), "owner": OWNER.to_lowercase(), "description": "  Hats for everyone  ",
                "images": [{ "name": "cover", "file": "cover/cover.jpeg" }],
                "links": [{ "name": "website", "url": "https://hats.example" }, { "name": "twitter", "url": "http://insecure.example" }, { "name": "discord", "url": "https://discord.gg/hats" }],
                "version": 1
            }
        })
    }

    #[test]
    fn the_store_entity_maps_as_upstream_fetch_store_did() {
        let s = store_from_entity(OWNER, &entity(), "https://peer.example/content/");
        assert!(s.exists);
        assert_eq!(
            (s.owner.as_str(), s.entity_id.as_deref(), s.updated_at),
            (
                OWNER.to_lowercase().as_str(),
                Some("bafkreistore"),
                Some(1_700_000_000_000)
            )
        );
        assert_eq!(
            (s.cover.as_str(), s.cover_hash.as_str()),
            (
                "https://peer.example/content/contents/bafkreicover",
                "bafkreicover"
            )
        );
        assert_eq!(s.description, "Hats for everyone");
        assert_eq!(
            s.links,
            StoreLinks {
                website: "https://hats.example".into(),
                twitter: String::new(),
                discord: "https://discord.gg/hats".into(),
                facebook: String::new()
            },
            "an http link is dropped, a missing one is empty"
        );
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["coverHash"], "bafkreicover");
        assert_eq!(v["links"]["website"], "https://hats.example");
    }

    #[test]
    fn no_metadata_or_no_entity_is_the_empty_store() {
        let s = store_from_entity(
            OWNER,
            &json!({ "id": "x", "type": "store" }),
            "https://peer.example/content",
        );
        assert_eq!(s, Store::empty(OWNER));
        assert!(!s.exists && s.cover.is_empty() && s.urn == store_urn(OWNER));
        let mut without_cover = entity();
        without_cover["content"] = json!([]);
        let s = store_from_entity(OWNER, &without_cover, "https://peer.example/content");
        assert!(
            s.exists && s.cover.is_empty() && s.cover_hash.is_empty(),
            "a cover file the entity does not carry is no cover"
        );
    }

    #[test]
    fn the_window_counts_first_sales_per_item_only() {
        let rows = vec![
            json!({ "type": "mint", "timestamp": "1000", "price": "5", "searchContractAddress": "0xc", "searchItemId": "1" }),
            json!({ "type": "mint", "timestamp": "2000", "price": "7", "searchContractAddress": "0xc", "searchItemId": "1" }),
            json!({ "type": "order", "timestamp": "2000", "price": "9", "searchContractAddress": "0xc", "searchItemId": "1" }),
            json!({ "type": "mint", "timestamp": "3000", "price": "11", "searchContractAddress": "0xc", "searchItemId": "2" }),
        ];
        let w = per_item_window(&rows, Some(1_500_000), Some(2_500_000));
        assert_eq!(
            w.get(&("0xc".into(), "1".into())),
            Some(&(1, 7)),
            "one mint inside the window; the resale is not the creator's first sale"
        );
        assert!(!w.contains_key(&("0xc".into(), "2".into())));
        let all = per_item_window(&rows, None, None);
        assert_eq!(all.get(&("0xc".into(), "1".into())), Some(&(2, 12)));
    }
}
