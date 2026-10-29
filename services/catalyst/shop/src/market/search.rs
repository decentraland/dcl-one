// from: marketplace-server src/logic/catalog/creator-profiles.ts (the creator search), src/logic/catalog/collection-search.ts, src/ports/search-suggest/component.ts, src/controllers/handlers/creator-search-handler.ts, src/controllers/handlers/search-suggest-handler.ts
//! The shop's search dropdown: `/v3/catalog/creators/search` (creators whose
//! profile name or one of their NAMEs matches every term of the query) and
//! `/v3/catalog/suggest` (the items the grid would show for the query, the
//! matching collections and creators, with creator names resolved, in one
//! answer).
//!
//! Upstream matches over precomputed word tables with pg_trgm's
//! `word_similarity`; v2 defers pg_trgm until measured, so a name matches
//! when every normalized query
//! term is inside it, and ranks in the same tiers: a name that IS the query
//! (its words in any order, or run together as a NAME is), then one that
//! starts with it, then any other match. The creator directory (approved
//! creators with their item and collection counts, the active profile's name
//! and face, their NAMEs) is one statement cached for 15 minutes: creators
//! number in the thousands and a dropdown asks on every keystroke.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;

use super::catalog::escape_like;
use super::items::{self, CreditCatalogItem};
use super::mirrors::{Mirror, require_ready};
use super::readers::{Ttl, cached};
use super::wire::{ApiError, Params};
use super::{MarketplaceState, SharedState};

/// Words a query drops, but only when it has anything else to search for:
/// "atari x rtfkt" is a search for atari and rtfkt.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "of", "and", "x", "de", "la", "el", "los", "las", "y",
];
const MAX_TERMS: usize = 6;
pub const CREATOR_SEARCH_DEFAULT_LIMIT: i64 = 4;
pub const CREATOR_SEARCH_MAX_LIMIT: i64 = 10;
/// NAMEs kept per creator: enough to be found by, bounded for the whales.
const CREATOR_MAX_NAMES: usize = 10;
pub const COLLECTION_SUGGEST_DEFAULT_LIMIT: i64 = 4;
pub const COLLECTION_SUGGEST_MAX_LIMIT: i64 = 10;
/// The window the collections' sales tiebreak counts over (the sales reader's).
const COLLECTION_SALES_WINDOW_DAYS: i64 = 90;
pub const SUGGEST_ITEMS_DEFAULT_LIMIT: i64 = 5;
pub const SUGGEST_SECTION_MAX_LIMIT: i64 = 10;
const SUGGEST_CACHE: &str = "public,max-age=60,s-maxage=60";

fn words(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The query as matched: one term per whitespace-separated word collapsed to
/// its letters and digits ("t-shirt" is tshirt), de-duplicated, stopwords
/// dropped when anything else remains, at most six.
pub fn query_terms(search: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in search.split_whitespace() {
        let term: String = words(word).concat();
        if !term.is_empty() && !terms.contains(&term) {
            terms.push(term);
        }
    }
    let kept: Vec<String> = terms
        .iter()
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .cloned()
        .collect();
    let mut out = if kept.is_empty() { terms } else { kept };
    out.truncate(MAX_TERMS);
    out
}

/// A string in the shapes a query is compared against.
#[derive(Debug, Clone, PartialEq)]
struct Forms {
    phrase: String,
    sorted: String,
    compact: String,
}

fn forms(value: &str) -> Forms {
    let w = words(value);
    let mut sorted = w.clone();
    sorted.sort();
    Forms {
        phrase: w.join(" "),
        sorted: sorted.join(" "),
        compact: w.concat(),
    }
}

/// How a name answers the query, higher first: 2 it IS the query, 1 it
/// starts with it, 0 every term is inside it; `None` when a term is missing.
fn name_tier(name: &Forms, query: &Forms, terms: &[String]) -> Option<u8> {
    if terms.is_empty() || name.compact.is_empty() {
        return None;
    }
    if !terms
        .iter()
        .all(|t| name.phrase.contains(t.as_str()) || name.compact.contains(t.as_str()))
    {
        return None;
    }
    if name.sorted == query.sorted || name.compact == query.compact {
        return Some(2);
    }
    if name.phrase.starts_with(&query.phrase) || name.compact.starts_with(&query.compact) {
        return Some(1);
    }
    Some(0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreatorEntry {
    pub address: String,
    pub name: Option<String>,
    pub face: Option<String>,
    pub names: Vec<String>,
    pub items: i64,
    pub collections: i64,
}

impl CreatorEntry {
    /// What to call the creator: the profile name, else the first NAME.
    pub fn display_name(&self) -> Option<String> {
        self.name.clone().or_else(|| self.names.first().cloned())
    }
}

/// A creator the dropdown can offer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct CreatorSearchHit {
    pub address: String,
    pub name: String,
    pub face: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub items: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub collections: i64,
}

const CREATOR_DIRECTORY_SQL: &str = "WITH creators AS ( \
       SELECT lower(creator) AS address, count(*)::int8 AS items, count(DISTINCT contract_address)::int8 AS collections \
       FROM shop.items WHERE search_is_collection_approved = true AND creator IS NOT NULL GROUP BY 1 \
     ) \
     SELECT c.address, c.items, c.collections, d.entity_id, \
            NULLIF(trim(d.entity_metadata->'avatars'->0->>'name'), '') AS name, \
            d.entity_metadata->'avatars'->0->'avatar'->'snapshots'->>'face256' AS face, \
            COALESCE((SELECT array_agg(n.ens_subdomain ORDER BY n.minted_block, n.ens_subdomain) FROM shop.nft n \
                      WHERE n.owner = c.address AND n.ens_subdomain IS NOT NULL), '{}') AS names \
     FROM creators c \
     LEFT JOIN content.active_pointers ap ON ap.pointer = c.address \
     LEFT JOIN content.deployments d ON d.entity_id = ap.entity_id";

/// The creator's face: the CDN's or this node's `/profile-images` for the
/// active profile; one it never uploaded only where something derives it.
fn face_url(state: &MarketplaceState, entity_id: &str, face: Option<&str>) -> Option<String> {
    let face = match face.filter(|f| !f.is_empty()) {
        Some(face) => face,
        None if state.derives_faces() => "",
        None => return None,
    };
    let mut metadata = json!({ "avatars": [{ "avatar": { "snapshots": { "face256": face } } }] });
    super::profiles::rewrite_snapshot_urls(
        entity_id,
        &mut metadata,
        state.settings.profile_cdn_base_url.as_deref(),
        &state.settings.content_public_url,
        &state.settings.profile_images_public_url,
    );
    metadata["avatars"][0]["avatar"]["snapshots"]["face256"]
        .as_str()
        .map(str::to_string)
}

pub async fn creator_directory(state: &MarketplaceState) -> Result<Vec<CreatorEntry>, ApiError> {
    let value = cached(
        state,
        Ttl::M15,
        "search:creator-directory".to_string(),
        || async {
            let rows = sqlx::query(CREATOR_DIRECTORY_SQL)
                .fetch_all(&state.pool)
                .await
                .map_err(|e| e.to_string())?;
            let entries: Vec<CreatorEntry> = rows
                .iter()
                .map(|r| {
                    let entity_id: Option<String> = r.get("entity_id");
                    let face: Option<String> = r.get("face");
                    let mut names: Vec<String> = r.get("names");
                    names.truncate(CREATOR_MAX_NAMES);
                    CreatorEntry {
                        address: r.get("address"),
                        name: r.get("name"),
                        face: entity_id
                            .as_deref()
                            .and_then(|id| face_url(state, id, face.as_deref())),
                        names,
                        items: r.get("items"),
                        collections: r.get("collections"),
                    }
                })
                .collect();
            state.warm_faces(
                rows.iter()
                    .filter_map(|r| r.get::<Option<String>, _>("entity_id")),
            );
            serde_json::to_value(entries).map_err(|e| e.to_string())
        },
    )
    .await
    .map_err(ApiError::Internal)?;
    serde_json::from_value(Arc::unwrap_or_clone(value))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// Creators whose profile name or a NAME matches every term, best first:
/// the tier, then how much they have published (a tie goes to the one with
/// a shop to browse), then name and address so pages are stable.
pub fn rank_creators(
    directory: &[CreatorEntry],
    search: &str,
    first: usize,
) -> Vec<CreatorSearchHit> {
    let terms = query_terms(search);
    let query = forms(&terms.join(" "));
    let mut hits: Vec<(u8, &CreatorEntry)> = directory
        .iter()
        .filter_map(|c| {
            let tier = c
                .name
                .iter()
                .chain(c.names.iter())
                .filter_map(|n| name_tier(&forms(n), &query, &terms))
                .max()?;
            Some((tier, c))
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| {
        tb.cmp(ta)
            .then(b.items.cmp(&a.items))
            .then(b.collections.cmp(&a.collections))
            .then(a.display_name().cmp(&b.display_name()))
            .then(a.address.cmp(&b.address))
    });
    hits.into_iter()
        .take(first)
        .map(|(_, c)| CreatorSearchHit {
            address: c.address.clone(),
            name: c.display_name().unwrap_or_else(|| c.address.clone()),
            face: c.face.clone(),
            items: c.items,
            collections: c.collections,
        })
        .collect()
}

fn clamp(value: Option<f64>, default: i64, max: i64) -> usize {
    value
        .filter(|v| v.is_finite())
        .map(|v| v as i64)
        .unwrap_or(default)
        .clamp(1, max) as usize
}

/// `GET /v3/catalog/creators/search?search=&first=`: nothing typed is nobody
/// suggested, not an error.
pub async fn get_creator_search(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if let Err(envelope) = require_ready(&state, Mirror::Items).await {
        return envelope;
    }
    let p = Params::new(&pairs);
    let search = p.get_string("search", None).unwrap_or_default();
    let first = clamp(
        p.get_number("first", None),
        CREATOR_SEARCH_DEFAULT_LIMIT,
        CREATOR_SEARCH_MAX_LIMIT,
    );
    if query_terms(&search).is_empty() {
        return (
            [(CACHE_CONTROL, SUGGEST_CACHE)],
            Json(json!({ "data": [] })),
        )
            .into_response();
    }
    match creator_directory(&state).await {
        Ok(directory) => (
            [(CACHE_CONTROL, SUGGEST_CACHE)],
            Json(json!({ "data": rank_creators(&directory, &search, first) })),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct SuggestedCollection {
    #[cfg_attr(feature = "ts", ts(rename = "contractAddress"))]
    pub contract_address: String,
    pub name: String,
    pub creator: String,
    #[cfg_attr(feature = "ts", ts(rename = "creatorName"))]
    pub creator_name: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub items: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sales: i64,
}

/// Approved collections with at least one approved item whose name holds
/// every term (the bounded ILIKE pre-filter), ranked in the name tiers,
/// then by approved items, then by sales in the window, then name and
/// address.
pub async fn search_collections(
    state: &MarketplaceState,
    search: &str,
    first: usize,
    directory: &[CreatorEntry],
) -> Result<Vec<SuggestedCollection>, ApiError> {
    let terms = query_terms(search);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let patterns: Vec<String> = terms
        .iter()
        .map(|t| format!("%{}%", escape_like(t)))
        .collect();
    let rows = sqlx::query(
        "SELECT c.contract_address, c.name, lower(c.creator) AS creator, count(*)::int8 AS items \
         FROM shop.collections c \
         JOIN shop.items i ON i.network = c.network AND i.contract_address = c.contract_address AND i.search_is_collection_approved = true \
         WHERE c.is_approved = true \
           AND (SELECT bool_and(regexp_replace(lower(c.name), '[^[:alnum:]]+', '', 'g') LIKE p OR lower(c.name) LIKE p) FROM unnest($1::text[]) AS p) \
         GROUP BY c.contract_address, c.name, c.creator",
    )
    .bind(&patterns)
    .fetch_all(&state.pool)
    .await?;
    let sales = collection_sales(state).await;
    let query = forms(&terms.join(" "));
    let mut hits: Vec<(u8, SuggestedCollection)> = rows
        .iter()
        .filter_map(|r| {
            let name: String = r.get("name");
            let tier = name_tier(&forms(&name), &query, &terms)?;
            let contract: String = r.get("contract_address");
            let creator: String = r.get("creator");
            Some((
                tier,
                SuggestedCollection {
                    sales: sales.get(&contract).copied().unwrap_or(0),
                    creator_name: directory
                        .iter()
                        .find(|c| c.address == creator)
                        .and_then(CreatorEntry::display_name),
                    contract_address: contract,
                    name,
                    creator,
                    items: r.get("items"),
                },
            ))
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| {
        tb.cmp(ta)
            .then(b.items.cmp(&a.items))
            .then(b.sales.cmp(&a.sales))
            .then(a.name.cmp(&b.name))
            .then(a.contract_address.cmp(&b.contract_address))
    });
    Ok(hits.into_iter().take(first).map(|(_, c)| c).collect())
}

/// Sales per collection in the window, from the sales reader; an
/// unreachable reader costs the tiebreak, not the answer.
async fn collection_sales(state: &MarketplaceState) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    if let Ok(sales) = items::recent_sales(state, COLLECTION_SALES_WINDOW_DAYS).await {
        for sale in &sales {
            if let Some(contract) = sale.get("searchContractAddress").and_then(Value::as_str) {
                *out.entry(contract.to_lowercase()).or_default() += 1;
            }
        }
    }
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedCatalogItem {
    #[serde(flatten)]
    pub item: CreditCatalogItem,
    pub creator_name: Option<String>,
}

/// `GET /v3/catalog/suggest?search=&items=&collections=&creators=`: the
/// items the grid shows for the query (its first rows and its total, so "See
/// all (N)" is that number), the matching collections and creators, run in
/// parallel over one creator directory.
pub async fn get_search_suggest(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let p = Params::new(&pairs);
    let search = p.get_string("search", None).unwrap_or_default();
    let items_first = clamp(
        p.get_number("items", None),
        SUGGEST_ITEMS_DEFAULT_LIMIT,
        SUGGEST_SECTION_MAX_LIMIT,
    );
    let collections_first = clamp(
        p.get_number("collections", None),
        COLLECTION_SUGGEST_DEFAULT_LIMIT,
        SUGGEST_SECTION_MAX_LIMIT,
    );
    let creators_first = clamp(
        p.get_number("creators", None),
        CREATOR_SEARCH_DEFAULT_LIMIT,
        SUGGEST_SECTION_MAX_LIMIT,
    );
    let empty = json!({ "items": { "data": [], "total": 0 }, "collections": { "data": [] }, "creators": { "data": [] } });
    if query_terms(&search).is_empty() {
        return ([(CACHE_CONTROL, SUGGEST_CACHE)], Json(empty)).into_response();
    }
    let directory = match creator_directory(&state).await {
        Ok(d) => d,
        Err(e) => return e.into_response(),
    };
    let item_pairs = vec![
        ("search".to_string(), search.clone()),
        ("first".to_string(), items_first.to_string()),
    ];
    let (items_page, collections) = tokio::join!(
        items::catalog_items_page(&state, &item_pairs),
        search_collections(&state, &search, collections_first, &directory)
    );
    let (item_rows, total) = match items_page {
        Ok(page) => page,
        Err(r) => return r,
    };
    let collections = match collections {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let names: HashMap<&str, Option<String>> = directory
        .iter()
        .map(|c| (c.address.as_str(), c.display_name()))
        .collect();
    let items: Vec<SuggestedCatalogItem> = item_rows
        .into_iter()
        .map(|item| {
            let creator_name = names
                .get(item.item.creator.to_lowercase().as_str())
                .cloned()
                .flatten();
            SuggestedCatalogItem { item, creator_name }
        })
        .collect();
    let body = json!({
        "items": { "data": items, "total": total },
        "collections": { "data": collections },
        "creators": { "data": rank_creators(&directory, &search, creators_first) },
    });
    ([(CACHE_CONTROL, SUGGEST_CACHE)], Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creator(address: &str, name: Option<&str>, names: &[&str], items: i64) -> CreatorEntry {
        CreatorEntry {
            address: address.into(),
            name: name.map(str::to_string),
            face: None,
            names: names.iter().map(|n| n.to_string()).collect(),
            items,
            collections: 1,
        }
    }

    #[test]
    fn the_query_is_normalized_term_by_term() {
        assert_eq!(query_terms("Atari x RTFKT"), vec!["atari", "rtfkt"]);
        assert_eq!(
            query_terms("x"),
            vec!["x"],
            "a stopword alone is still a query"
        );
        assert_eq!(
            query_terms("T-Shirt t-shirt  o'brien"),
            vec!["tshirt", "obrien"]
        );
        assert_eq!(query_terms("a b c d e f g h").len(), MAX_TERMS);
        assert!(query_terms(" -- ").is_empty());
    }

    #[test]
    fn a_name_is_ranked_exact_then_prefix_then_partial() {
        let terms = query_terms("hat");
        let q = forms(&terms.join(" "));
        assert_eq!(name_tier(&forms("HAT"), &q, &terms), Some(2));
        assert_eq!(name_tier(&forms("Hat Pirate"), &q, &terms), Some(1));
        assert_eq!(
            name_tier(&forms("Hatch"), &q, &terms),
            Some(1),
            "a dropdown answers while the name is being typed"
        );
        assert_eq!(name_tier(&forms("Pirate Hat"), &q, &terms), Some(0));
        assert_eq!(name_tier(&forms("Treasure"), &q, &terms), None);
        let terms = query_terms("pirate hat");
        let q = forms(&terms.join(" "));
        assert_eq!(
            name_tier(&forms("Hat, Pirate"), &q, &terms),
            Some(2),
            "its words in any order"
        );
        assert_eq!(
            name_tier(&forms("Pirate Captain"), &q, &terms),
            None,
            "every term must match"
        );
        let terms = query_terms("star ducks");
        assert_eq!(
            name_tier(&forms("STARDUCKS"), &forms(&terms.join(" ")), &terms),
            Some(2),
            "a NAME is the words run together"
        );
    }

    #[test]
    fn creators_match_by_profile_name_or_name_and_rank_by_tier_then_catalogue() {
        let directory = vec![
            creator("0x01", Some("Duck Maker"), &[], 50),
            creator("0x02", None, &["starducks"], 5),
            creator("0x03", Some("Ducks"), &["quack"], 1),
            creator("0x04", Some("Other"), &["duckling"], 80),
            creator("0x05", Some("Nobody"), &[], 99),
        ];
        let hits = rank_creators(&directory, "ducks", 10);
        let order: Vec<&str> = hits.iter().map(|h| h.address.as_str()).collect();
        assert_eq!(
            order,
            vec!["0x03", "0x02"],
            "exact over partial; 'Duck Maker' and 'duckling' lack the term: {hits:?}"
        );
        assert_eq!(hits[1].name, "starducks", "no profile name: the first NAME");
        let hits = rank_creators(&directory, "duck", 10);
        assert_eq!(
            hits.iter().map(|h| h.address.as_str()).collect::<Vec<_>>(),
            vec!["0x04", "0x01", "0x03", "0x02"],
            "{hits:?}"
        );
        assert_eq!(rank_creators(&directory, "duck", 2).len(), 2);
    }

    #[tokio::test]
    async fn an_empty_query_suggests_nothing_without_a_lookup() {
        use tower::ServiceExt;
        let state = crate::market::tests::support::state_without_db_settings(
            crate::market::Settings::for_tests(),
        );
        let app = crate::market::market_router(state);
        let req = axum::http::Request::builder()
            .uri("/v3/catalog/suggest?search=%20--%20")
            .body(axum::body::Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), axum::http::StatusCode::OK);
        assert_eq!(res.headers()[CACHE_CONTROL], SUGGEST_CACHE);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), 1 << 16)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            body,
            json!({ "items": { "data": [], "total": 0 }, "collections": { "data": [] }, "creators": { "data": [] } })
        );
    }

    #[tokio::test]
    async fn the_directory_reads_the_active_profile_and_the_catalogue_counts() {
        use crate::market::tests::support::state_with_db;
        let Some(state) = state_with_db(crate::market::Settings::for_tests()).await else {
            return;
        };
        let pool = state.pool.clone();
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let creator = format!("0x{tag}00000000");
        let contract = format!("0x{tag}eeeeeeee");
        let entity = format!("bafy{tag}");
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, \
                name, category, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 0, 137, 'wearable_v2', $3, $2, 'common', 0, 10, 10, 0, 'Quackers Hat', 'hat', true, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(&contract)
        .bind(&creator)
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}:0"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ($1, 'v3', 'profile', $2, $3::json, now(), ARRAY[$1], now(), '[]'::json)",
        )
        .bind(&creator)
        .bind(&entity)
        .bind(json!({ "avatars": [{ "name": "Quackmaster", "avatar": { "snapshots": { "face256": "bafyface" } } }] }).to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'profile')").bind(&creator).bind(&entity).execute(&pool).await.unwrap();
        let directory = creator_directory(&state).await.unwrap();
        let entry = directory
            .iter()
            .find(|c| c.address == creator)
            .expect("the creator is in the directory");
        assert_eq!(
            (entry.name.as_deref(), entry.items, entry.collections),
            (Some("Quackmaster"), 1, 1)
        );
        assert!(
            entry
                .face
                .as_deref()
                .is_some_and(|f| f.ends_with(&format!("profile-images/entities/{entity}/face.png"))),
            "{entry:?}"
        );
        let hits = rank_creators(&directory, "quackmas", 4);
        assert_eq!(
            hits.first().map(|h| h.address.as_str()),
            Some(creator.as_str())
        );
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
            .bind(&creator)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
            .bind(&entity)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.items WHERE contract_address = $1")
            .bind(&contract)
            .execute(&pool)
            .await
            .unwrap();
    }
}
