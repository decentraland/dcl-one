//! The lambdas catalogue: `/lambdas/collections` (and `/lambdas/nfts/collections`),
//! the global `/lambdas/collections/{wearables,emotes}` catalogue and
//! `/lambdas/outfits/{address}`. The old handlers read the squid's `item`
//! and `collection` tables; here the on-chain legs read the `items` and
//! `collections` mirrors (the collections subgraphs, `mirrors`), the base
//! wearables and every definition come from the content entities peer sync
//! ingested, and the outfits' extra-slot names from the marketplace
//! subgraph reader. Before the items mirror has a complete pass the on-chain
//! leg's URNs come from the collections subgraphs themselves, one `items`
//! document per network with the same filters (`subgraph_item_urns`); a
//! catalogue query with no collections subgraph configured answers the
//! unavailable envelope.
//!
//! `collectionType` on the wearables catalogue is the old node's deliberate
//! divergence from upstream (2026-08-26): with no `collectionType` in the
//! query the answer is upstream's, byte for byte.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value, json};
use sqlx::PgPool;

use crate::content::queries::active_entity_docs_by_pointers;
use crate::shared::cache::TtlMap;
use crate::shared::http_util::lambdas_internal_error;
use crate::shared::query_params::{parse_int_js, parse_query_string, qs_get_array, qs_get_string};

use super::base_wearables::{self, BASE_AVATARS_COLLECTION_ID, BaseWearable};
use super::definitions::{
    SORTED_RARITIES, default_sort_direction, extract_emote_definition, extract_wearable_definition,
    locale_cmp, validate_rarity, validate_sort,
};
use super::graph::{self, Network, THE_GRAPH_PAGE_SIZE, graph_query};
use super::mirrors::{self, Mirror};
use super::{MarketplaceState, SharedState, Source, bad_request, not_found, unavailable};

const COLLECTIONS_TTL: Duration = Duration::from_secs(300);
const CATALOG_TTL: Duration = Duration::from_secs(60);
const BASE_WEARABLES_TTL: Duration = Duration::from_secs(300);
/// The old 60 s memo was evicted by every deployment of the address's
/// outfits entity; v2 keeps the profile memo's 10 s instead of a cross-module
/// eviction, so a save shows up within ten seconds.
const OUTFITS_TTL: Duration = Duration::from_secs(10);
const COLLECTIONS_PER_NETWORK: i64 = 1000;

/// The catalogue's memo.
pub struct Caches {
    collections: TtlMap<(), Value>,
    catalog: TtlMap<String, Value>,
    /// The on-chain URN leg from the subgraphs, per query, while the mirror has no pass.
    catalog_urns: TtlMap<String, Arc<Vec<String>>>,
    base_wearables: TtlMap<(), Arc<Vec<BaseWearable>>>,
    outfits: TtlMap<String, Value>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            collections: TtlMap::bounded(COLLECTIONS_TTL, 1),
            catalog: TtlMap::bounded(CATALOG_TTL, 4096),
            catalog_urns: TtlMap::bounded(CATALOG_TTL, 4096),
            base_wearables: TtlMap::bounded(BASE_WEARABLES_TTL, 1),
            outfits: TtlMap::bounded(OUTFITS_TTL, 50_000),
        }
    }
}

/// The parsed base wearables, memoized; an empty answer is not kept.
pub(crate) async fn cached_base_wearables(state: &MarketplaceState) -> Arc<Vec<BaseWearable>> {
    state
        .lambdas
        .base_wearables
        .get_or_fetch((), || async {
            match base_wearables::fetch_base_wearables(&state.pool).await {
                Ok(base) if !base.is_empty() => Ok(Arc::new(base)),
                Ok(_) => Err(()),
                Err(e) => {
                    tracing::warn!(error = %e, "base wearables lookup failed");
                    Err(())
                }
            }
        })
        .await
        .unwrap_or_default()
}

fn catalog_params_from_query(qs: &str) -> CatalogParams {
    let p = parse_query_string(qs);
    CatalogParams {
        collection_id: qs_get_array(&p, "collectionId"),
        collection_type: qs_get_array(&p, "collectionType"),
        category: qs_get_array(&p, "category"),
        rarity: qs_get_string(&p, "rarity"),
        name: qs_get_string(&p, "name"),
        order_by: qs_get_string(&p, "orderBy"),
        direction: qs_get_string(&p, "direction"),
        wearable_id: qs_get_array(&p, "wearableId"),
        emote_id: qs_get_array(&p, "emoteId"),
        text_search: qs_get_string(&p, "textSearch"),
        last_id: qs_get_string(&p, "lastId"),
        limit: qs_get_string(&p, "limit"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionType {
    BaseWearable,
    OnChain,
    ThirdParty,
}

impl CollectionType {
    fn parse(raw: &str) -> Option<Self> {
        match raw.to_lowercase().as_str() {
            "base-wearable" => Some(Self::BaseWearable),
            "on-chain" => Some(Self::OnChain),
            "third-party" => Some(Self::ThirdParty),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::BaseWearable => "base-wearable",
            Self::OnChain => "on-chain",
            Self::ThirdParty => "third-party",
        }
    }
}

/// Preserves caller order and drops repeats. An unrecognised value is
/// refused rather than ignored: a silently-dropped typo would widen the
/// result set to everything.
fn parse_collection_types(raw: &[String]) -> Result<Vec<CollectionType>, Box<Response>> {
    let mut out = Vec::new();
    for value in raw {
        let t = CollectionType::parse(value).ok_or_else(|| {
            bad_request(&format!("Invalid collectionType '{value}'. Valid values are: 'base-wearable', 'on-chain', 'third-party'"))
        })?;
        if !out.contains(&t) {
            out.push(t);
        }
    }
    Ok(out)
}

/// One `key=value` pair as `URLSearchParams.toString()` writes it
/// (application/x-www-form-urlencoded: a space is `+`, `*-._` and
/// alphanumerics stay, everything else, `~` included, is percent-encoded).
fn qp(key: &str, value: &str) -> String {
    format!(
        "{key}={}",
        url::form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>()
    )
}

fn lower_nonempty(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::to_lowercase)
        .filter(|s| !s.is_empty())
}

/// All opt-in; the vocabulary and error wording of `/lambdas/explorer/{address}/wearables`.
#[derive(Debug, Default, Clone)]
struct ExtendedFilters {
    collection_types: Vec<CollectionType>,
    categories: Vec<String>,
    rarity: Option<String>,
    name: Option<String>,
    sort: Option<String>,
    direction: Option<String>,
}

impl ExtendedFilters {
    /// True once the caller has expressed an intent; `sort` alone does not count.
    fn narrows(&self) -> bool {
        !self.collection_types.is_empty()
            || !self.categories.is_empty()
            || self.rarity.is_some()
            || self.name.is_some()
    }

    fn allows(&self, t: CollectionType) -> bool {
        self.collection_types.is_empty() || self.collection_types.contains(&t)
    }

    fn query_parts(&self) -> Vec<String> {
        let mut parts: Vec<String> = self
            .collection_types
            .iter()
            .map(|t| format!("collectionType={}", t.as_str()))
            .collect();
        parts.extend(self.categories.iter().map(|c| qp("category", c)));
        parts.extend(self.rarity.iter().map(|r| qp("rarity", r)));
        parts.extend(self.name.iter().map(|n| qp("name", n)));
        if let Some(s) = &self.sort {
            parts.push(qp("orderBy", s));
            parts.extend(self.direction.iter().map(|d| format!("direction={d}")));
        }
        parts
    }

    fn write_json(&self, m: &mut Map<String, Value>) {
        if !self.collection_types.is_empty() {
            let v: Vec<&str> = self.collection_types.iter().map(|t| t.as_str()).collect();
            m.insert("collectionTypes".into(), json!(v));
        }
        if !self.categories.is_empty() {
            m.insert("categories".into(), json!(self.categories));
        }
        if let Some(r) = &self.rarity {
            m.insert("rarity".into(), json!(r));
        }
        if let Some(n) = &self.name {
            m.insert("name".into(), json!(n));
        }
        if let Some(s) = &self.sort {
            m.insert("orderBy".into(), json!(s));
            if let Some(d) = &self.direction {
                m.insert("direction".into(), json!(d));
            }
        }
    }
}

/// `orderBy` needs the query to resolve to a single tier: base wearables and
/// on-chain items are paged from different stores behind one urn-keyed
/// cursor, so a global order across both is not well defined.
fn parse_extended_filters(p: &CatalogParams) -> Result<ExtendedFilters, Box<Response>> {
    let collection_types = parse_collection_types(&p.collection_type)?;
    let categories: Vec<String> = p
        .category
        .iter()
        .map(|c| c.to_lowercase())
        .filter(|c| !c.is_empty())
        .collect();
    let rarity = lower_nonempty(&p.rarity);
    if let Some(r) = &rarity {
        validate_rarity(r).map_err(|e| Box::new(bad_request(&e)))?;
    }
    let sort = lower_nonempty(&p.order_by);
    let direction = match &sort {
        None => None,
        Some(s) => {
            let d = match p.direction.as_deref() {
                Some(d) if !d.is_empty() => d.to_uppercase(),
                _ => default_sort_direction(s).to_string(),
            };
            validate_sort(s, &d).map_err(|e| Box::new(bad_request(&e)))?;
            Some(d)
        }
    };
    if sort.is_some() && collection_types.len() != 1 {
        return Err(Box::new(bad_request(
            "Sorting the global catalog needs a single collectionType: base wearables and on-chain items are paged from different stores, so 'orderBy' is only well defined within one of them. Add exactly one 'collectionType' (for example 'collectionType=on-chain').",
        )));
    }
    Ok(ExtendedFilters {
        collection_types,
        categories,
        rarity,
        name: lower_nonempty(&p.name),
        sort,
        direction,
    })
}

const MAX_LIMIT: i64 = 500;
const BASE_EMOTES_COLLECTION_ID: &str = "urn:decentraland:off-chain:base-emotes";

/// The mirror spells L1 URNs `ethereum`; a cursor or filter in the squid's
/// `mainnet` spelling names the same item.
fn to_mirror_urn(urn: &str) -> String {
    urn.replace(":mainnet:", ":ethereum:")
}

#[derive(Debug, Default)]
pub struct CatalogParams {
    collection_id: Vec<String>,
    collection_type: Vec<String>,
    category: Vec<String>,
    rarity: Option<String>,
    name: Option<String>,
    order_by: Option<String>,
    direction: Option<String>,
    wearable_id: Vec<String>,
    emote_id: Vec<String>,
    text_search: Option<String>,
    last_id: Option<String>,
    limit: Option<String>,
}

#[derive(Debug)]
struct CatalogFilters {
    collection_ids: Option<Vec<String>>,
    item_ids: Option<Vec<String>>,
    text_search: Option<String>,
    /// Empty for an upstream-shaped query, so the echoed `filters` is upstream's.
    extended: ExtendedFilters,
}

impl CatalogFilters {
    fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(c) = &self.collection_ids {
            m.insert("collectionIds".into(), json!(c));
        }
        if let Some(i) = &self.item_ids {
            m.insert("itemIds".into(), json!(i));
        }
        if let Some(t) = &self.text_search {
            m.insert("textSearch".into(), json!(t));
        }
        self.extended.write_json(&mut m);
        Value::Object(m)
    }
}

struct CatalogQuery {
    filters: CatalogFilters,
    limit: i64,
    last_id: Option<String>,
}

/// `limit` in 1..=500, else 500, read with `parseInt` as lamb2's
/// `clampLimit` does (decentraland/lamb2:src/logic/items-catalog.ts:88-97).
fn clamp_limit(raw: &Option<String>) -> i64 {
    match raw.as_deref().and_then(parse_int_js) {
        Some(n) if (1..=MAX_LIMIT).contains(&n) => n,
        _ => MAX_LIMIT,
    }
}

fn non_empty(v: Vec<String>) -> Option<Vec<String>> {
    (!v.is_empty()).then_some(v)
}

fn parse_catalog_query(
    p: &CatalogParams,
    item_ids_in: &[String],
    id_param_name: &str,
    extended: ExtendedFilters,
) -> Result<CatalogQuery, Box<Response>> {
    let collection_ids: Vec<String> = p.collection_id.iter().map(|s| s.to_lowercase()).collect();
    let item_ids: Vec<String> = item_ids_in.iter().map(|s| s.to_lowercase()).collect();
    let text_search = lower_nonempty(&p.text_search);
    if collection_ids.is_empty()
        && item_ids.is_empty()
        && text_search.is_none()
        && !extended.narrows()
    {
        return Err(Box::new(bad_request(&format!(
            "You must use one of the filters: 'textSearch', 'collectionId' or '{id_param_name}'"
        ))));
    }
    if text_search.as_ref().is_some_and(|t| t.chars().count() < 3) {
        return Err(Box::new(bad_request(
            "The text search must be at least 3 characters long",
        )));
    }
    let items_label = if id_param_name == "wearableId" {
        "wearables"
    } else {
        "emotes"
    };
    if item_ids.len() as i64 > MAX_LIMIT {
        return Err(Box::new(bad_request(&format!(
            "You can't ask for more than {MAX_LIMIT} {items_label}"
        ))));
    }
    if collection_ids.len() as i64 > MAX_LIMIT {
        return Err(Box::new(bad_request(&format!(
            "You can't filter for more than {MAX_LIMIT} collection ids"
        ))));
    }
    Ok(CatalogQuery {
        filters: CatalogFilters {
            collection_ids: non_empty(collection_ids),
            item_ids: non_empty(item_ids),
            text_search,
            extended,
        },
        limit: clamp_limit(&p.limit),
        last_id: lower_nonempty(&p.last_id),
    })
}

fn build_next_query(
    filters: &CatalogFilters,
    limit: i64,
    next_last_id: &str,
    id_param_name: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.extend(
        filters
            .collection_ids
            .iter()
            .flatten()
            .map(|id| qp("collectionId", id)),
    );
    parts.extend(
        filters
            .item_ids
            .iter()
            .flatten()
            .map(|id| qp(id_param_name, id)),
    );
    parts.extend(filters.text_search.iter().map(|t| qp("textSearch", t)));
    parts.extend(filters.extended.query_parts());
    parts.push(format!("limit={limit}"));
    parts.push(qp("lastId", next_last_id));
    parts.join("&")
}

enum Bind {
    Text(String),
    TextArray(Vec<String>),
    Int(i64),
}

/// The on-chain leg over the `items` mirror, keyset-paged by lowercased urn
/// in byte order (the Rust merge's order). The mirror is a few tens of
/// thousands of rows: the filters scan it.
async fn fetch_item_urns(
    pool: &PgPool,
    item_type_prefix: &str,
    filters: &CatalogFilters,
    limit: i64,
    last_id: &Option<String>,
) -> Result<Vec<String>, sqlx::Error> {
    let wearable = item_type_prefix == "wearable";
    let item_type_clause = if wearable {
        "i.item_type IN ('wearable_v1', 'wearable_v2', 'smart_wearable_v1')"
    } else {
        "i.item_type = 'emote_v1'"
    };
    let mut sql = format!("SELECT i.urn FROM shop.items i WHERE {item_type_clause}");
    let mut binds: Vec<Bind> = Vec::new();
    let push = |binds: &mut Vec<Bind>, b: Bind| -> usize {
        binds.push(b);
        binds.len()
    };
    if let Some(cids) = &filters.collection_ids {
        let clauses: Vec<String> = cids
            .iter()
            .map(|c| {
                let i = push(
                    &mut binds,
                    Bind::Text(format!(
                        "{}:%",
                        escape_like(&to_mirror_urn(&c.to_lowercase()))
                    )),
                );
                format!("lower(i.urn) LIKE ${i} ESCAPE '\\'")
            })
            .collect();
        sql.push_str(&format!(" AND ({})", clauses.join(" OR ")));
    }
    if let Some(iids) = &filters.item_ids {
        let i = push(
            &mut binds,
            Bind::TextArray(
                iids.iter()
                    .map(|s| to_mirror_urn(&s.to_lowercase()))
                    .collect(),
            ),
        );
        sql.push_str(&format!(" AND lower(i.urn) = ANY(${i})"));
    }
    if let Some(t) = &filters.text_search {
        // lamb2's `searchText_contains` on the subgraph item
        // (decentraland/lamb2:src/logic/fetch-elements/fetch-items-by-filters.ts:116-119):
        // a case-sensitive substring of the mirrored `searchText`, the
        // lowercased name for a row the mirror has not refilled since the
        // column was added.
        let i = push(&mut binds, Bind::Text(format!("%{}%", escape_like(t))));
        sql.push_str(&format!(
            " AND COALESCE(i.search_text, lower(i.name)) LIKE ${i} ESCAPE '\\'"
        ));
    }
    let ext = &filters.extended;
    if !ext.categories.is_empty() {
        let i = push(&mut binds, Bind::TextArray(ext.categories.clone()));
        sql.push_str(&format!(" AND lower(i.category) = ANY(${i})"));
    }
    if let Some(r) = &ext.rarity {
        let i = push(&mut binds, Bind::Text(r.clone()));
        sql.push_str(&format!(" AND lower(i.rarity) = ${i}"));
    }
    if let Some(n) = &ext.name {
        let i = push(&mut binds, Bind::Text(format!("%{}%", escape_like(n))));
        sql.push_str(&format!(" AND i.name ILIKE ${i} ESCAPE '\\'"));
    }
    if let Some(cursor) = last_id {
        let i = push(
            &mut binds,
            Bind::Text(to_mirror_urn(&cursor.to_lowercase())),
        );
        sql.push_str(&format!(" AND lower(i.urn) COLLATE \"C\" > ${i}"));
    }
    let order = match (ext.sort.as_deref(), ext.direction.as_deref()) {
        (Some("rarity"), Some(dir)) => {
            let ranks: Vec<String> = SORTED_RARITIES
                .iter()
                .enumerate()
                .map(|(i, r)| format!("WHEN '{r}' THEN {i}"))
                .collect();
            format!(
                "CASE lower(i.rarity) {} ELSE -1 END {dir}, lower(i.urn) COLLATE \"C\" ASC",
                ranks.join(" ")
            )
        }
        (Some("date"), Some(dir)) => format!("i.created_at {dir}, lower(i.urn) COLLATE \"C\" ASC"),
        (Some("name"), Some(dir)) => format!("lower(i.name) {dir}, lower(i.urn) COLLATE \"C\" ASC"),
        _ => "lower(i.urn) COLLATE \"C\" ASC".to_string(),
    };
    let i = push(&mut binds, Bind::Int(limit + 1));
    sql.push_str(&format!(" ORDER BY {order} LIMIT ${i}"));
    let mut query = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql));
    for b in binds {
        query = match b {
            Bind::Text(s) => query.bind(s),
            Bind::TextArray(a) => query.bind(a),
            Bind::Int(n) => query.bind(n),
        };
    }
    query.fetch_all(pool).await
}

/// Where the on-chain leg's URNs come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UrnSource {
    /// `shop.items` after a complete pass.
    Mirror,
    /// The configured collections subgraphs, asked per request.
    Subgraphs,
}

async fn urn_source(state: &MarketplaceState) -> Result<UrnSource, Response> {
    match mirrors::require_ready(state, Mirror::Items).await {
        Ok(()) => Ok(UrnSource::Mirror),
        Err(_) if graph::any_collections_reader(&state.index) => Ok(UrnSource::Subgraphs),
        Err(envelope) => Err(envelope),
    }
}

async fn item_urns(
    state: &MarketplaceState,
    source: UrnSource,
    item_type_prefix: &str,
    filters: &CatalogFilters,
    limit: i64,
    last_id: &Option<String>,
) -> Result<Vec<String>, Response> {
    match source {
        UrnSource::Mirror => {
            fetch_item_urns(&state.pool, item_type_prefix, filters, limit, last_id)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "lambdas catalogue query failed");
                    lambdas_internal_error()
                })
        }
        UrnSource::Subgraphs => {
            if filters.extended.sort.as_deref() == Some("rarity") {
                return Err(unavailable(
                    Source::ItemsMirror,
                    "ordering the catalogue by rarity needs the items mirror, which has no complete pass in this database; the collections subgraphs cannot rank rarities",
                ));
            }
            subgraph_item_urns(state, item_type_prefix == "wearable", filters, limit, last_id).await.map_err(|(network, e)| {
                tracing::warn!(network = network.db_name(), error = %e, "lambdas catalogue: collections subgraph reader failed");
                unavailable(Source::CollectionsSubgraph(network), "the collections subgraph reader did not answer")
            })
        }
    }
}

/// One `items` row of a collections subgraph, the columns the sort and the
/// in-memory `name` filter need.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SubgraphItem {
    urn: String,
    created_at: i64,
    name: String,
}

fn subgraph_lit(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn subgraph_list(v: &[String]) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())
}

/// The `where` document for one network: the clauses `fetch_item_urns`
/// applies, ANDed; `None` when the filters name only the other network's
/// collections or items (nothing to ask here). The `name` filter, an ILIKE
/// on the mirror, is `searchText_contains` here (a superset: the search
/// text starts with the lowercased name) and cut back in memory.
fn subgraph_where(
    network: Network,
    wearable: bool,
    filters: &CatalogFilters,
    last_id: &Option<String>,
) -> Option<String> {
    let mut clauses = vec![if wearable {
        "{ itemType_in: [wearable_v1, wearable_v2, smart_wearable_v1] }".to_string()
    } else {
        "{ itemType_in: [emote_v1] }".to_string()
    }];
    if let Some(cids) = &filters.collection_ids {
        let mine: Vec<String> = cids
            .iter()
            .map(|c| to_mirror_urn(&c.to_lowercase()))
            .filter(|c| Network::of_urn(c) == Some(network))
            .map(|c| format!("{{ urn_starts_with: {} }}", subgraph_lit(&format!("{c}:"))))
            .collect();
        if mine.is_empty() {
            return None;
        }
        clauses.push(format!("{{ or: [{}] }}", mine.join(", ")));
    }
    if let Some(iids) = &filters.item_ids {
        let mine: Vec<String> = iids
            .iter()
            .map(|s| to_mirror_urn(&s.to_lowercase()))
            .filter(|u| Network::of_urn(u) == Some(network))
            .collect();
        if mine.is_empty() {
            return None;
        }
        clauses.push(format!("{{ urn_in: {} }}", subgraph_list(&mine)));
    }
    if let Some(t) = &filters.text_search {
        clauses.push(format!("{{ searchText_contains: {} }}", subgraph_lit(t)));
    }
    let ext = &filters.extended;
    if !ext.categories.is_empty() {
        let field = if wearable {
            "searchWearableCategory_in"
        } else {
            "searchEmoteCategory_in"
        };
        clauses.push(format!("{{ {field}: {} }}", subgraph_list(&ext.categories)));
    }
    if let Some(r) = &ext.rarity {
        clauses.push(format!("{{ rarity: {} }}", subgraph_lit(r)));
    }
    if let Some(n) = &ext.name {
        clauses.push(format!("{{ searchText_contains: {} }}", subgraph_lit(n)));
    }
    if let Some(cursor) = last_id {
        clauses.push(format!(
            "{{ urn_gt: {} }}",
            subgraph_lit(&to_mirror_urn(&cursor.to_lowercase()))
        ));
    }
    Some(format!("{{ and: [{}] }}", clauses.join(", ")))
}

/// `(orderBy, orderDirection)`: the mirror's `date` and `name` sorts as the
/// subgraph spells them (`searchText` stands in for the name: it starts with
/// it, lowercased); byte order of the URN otherwise, the cursor's order.
fn subgraph_order(ext: &ExtendedFilters) -> (&'static str, &'static str) {
    let dir = if ext.direction.as_deref() == Some("DESC") {
        "desc"
    } else {
        "asc"
    };
    match ext.sort.as_deref() {
        Some("date") => ("createdAt", dir),
        Some("name") => ("searchText", dir),
        _ => ("urn", "asc"),
    }
}

fn parse_items(data: &Value) -> Vec<SubgraphItem> {
    data.get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let urn = item.get("urn").and_then(Value::as_str)?.to_string();
            let name = ["/metadata/wearable/name", "/metadata/emote/name"]
                .iter()
                .find_map(|p| item.pointer(p).and_then(Value::as_str))
                .unwrap_or("")
                .to_lowercase();
            let created_at = item
                .get("createdAt")
                .and_then(Value::as_str)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            Some(SubgraphItem {
                urn,
                created_at,
                name,
            })
        })
        .collect()
}

/// The networks' rows merged into one page in the requested order, the
/// `name` filter applied, cut to `limit`.
fn merge_items(mut items: Vec<SubgraphItem>, ext: &ExtendedFilters, limit: i64) -> Vec<String> {
    if let Some(n) = &ext.name {
        items.retain(|i| i.name.contains(n.as_str()));
    }
    let desc = ext.direction.as_deref() == Some("DESC");
    items.sort_by(|a, b| {
        let primary = match ext.sort.as_deref() {
            Some("date") => a.created_at.cmp(&b.created_at),
            Some("name") => a.name.cmp(&b.name),
            _ => std::cmp::Ordering::Equal,
        };
        let primary = if desc { primary.reverse() } else { primary };
        primary.then_with(|| a.urn.to_lowercase().cmp(&b.urn.to_lowercase()))
    });
    items.truncate(limit.max(0) as usize);
    items.into_iter().map(|i| i.urn).collect()
}

/// The on-chain leg over the collections subgraphs: `limit + 1` URNs (the
/// mirror's page plus one, so the caller sees whether there is more) in the
/// mirror's order; a `name` filter scans one full page per network instead,
/// since it is decided here. Memoized per query for [`CATALOG_TTL`].
async fn subgraph_item_urns(
    state: &MarketplaceState,
    wearable: bool,
    filters: &CatalogFilters,
    limit: i64,
    last_id: &Option<String>,
) -> Result<Vec<String>, (Network, String)> {
    let key = format!(
        "urns|{wearable}|{limit}|{}|{:?}",
        last_id.as_deref().unwrap_or(""),
        filters
    );
    state
        .lambdas
        .catalog_urns
        .get_or_fetch(key, || async {
            let ext = &filters.extended;
            let page = limit + 1;
            let first = if ext.name.is_some() { THE_GRAPH_PAGE_SIZE } else { page };
            let (order_by, direction) = subgraph_order(ext);
            let mut items = Vec::new();
            for network in state.settings.collection_networks() {
                let Some(url) = network.url(&state.index) else {
                    continue;
                };
                let Some(where_doc) = subgraph_where(network, wearable, filters, last_id) else {
                    continue;
                };
                let doc = format!(
                    "{{ items(first: {first}, where: {where_doc}, orderBy: {order_by}, orderDirection: {direction}) \
                     {{ urn createdAt metadata {{ wearable {{ name }} emote {{ name }} }} }} }}"
                );
                let data = graph_query(&state.index, url, &doc, json!({})).await.map_err(|e| (network, e))?;
                items.extend(parse_items(&data));
            }
            Ok(Arc::new(merge_items(items, ext, page)))
        })
        .await
        .map(|a| (*a).clone())
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn id_key(v: &Value) -> String {
    v["id"].as_str().unwrap_or("").to_lowercase()
}

fn paginate(definitions: Vec<Value>, limit: i64) -> (Vec<Value>, Option<String>) {
    paginate_merged(Vec::new(), definitions, limit)
}

/// `merged` is served first and never reordered; `on_chain` is sorted behind
/// it by `localeCompare` of the lowercased id, as lamb2's
/// `paginateCatalogResults` (decentraland/lamb2:src/logic/items-catalog.ts:45-61).
fn paginate_merged(
    mut merged: Vec<Value>,
    mut on_chain: Vec<Value>,
    limit: i64,
) -> (Vec<Value>, Option<String>) {
    on_chain.sort_by(|a, b| locale_cmp(&id_key(a), &id_key(b)));
    merged.extend(on_chain);
    let has_more = merged.len() as i64 > limit;
    if has_more {
        merged.truncate(limit as usize);
    }
    let next = if has_more {
        merged
            .last()
            .and_then(|d| d["id"].as_str())
            .map(String::from)
    } else {
        None
    };
    (merged, next)
}

fn filter_and_extract_base_wearables(
    base: &[BaseWearable],
    filters: &CatalogFilters,
    last_id: &Option<String>,
    max_results: usize,
    content_public_url: &str,
) -> Vec<Value> {
    let ext = &filters.extended;
    let mut matched: Vec<&BaseWearable> = base
        .iter()
        .filter(|w| {
            let lc_urn = w.urn.to_lowercase();
            let haystack = w.english_name.as_deref().unwrap_or(&w.name).to_lowercase();
            last_id.as_ref().is_none_or(|lid| lc_urn > *lid)
                && filters
                    .item_ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&lc_urn))
                && filters
                    .text_search
                    .as_ref()
                    .is_none_or(|t| haystack.contains(t.as_str()))
                && (ext.categories.is_empty()
                    || ext.categories.contains(&w.category.to_lowercase()))
                && ext.rarity.is_none()
                && ext
                    .name
                    .as_ref()
                    .is_none_or(|n| haystack.contains(n.as_str()))
        })
        .collect();
    // `localeCompare` of the lowercased urns (wearables-catalog-handler.ts:107).
    matched.sort_by(|a, b| locale_cmp(&a.urn.to_lowercase(), &b.urn.to_lowercase()));
    matched.truncate(max_results);
    matched
        .into_iter()
        .filter_map(|w| extract_wearable_definition(&w.entity, content_public_url))
        .collect()
}

async fn definitions_for(
    state: &MarketplaceState,
    pointers: Vec<String>,
    extract: fn(&Value, &str) -> Option<Value>,
) -> Vec<Value> {
    if pointers.is_empty() {
        return Vec::new();
    }
    let docs = active_entity_docs_by_pointers(&state.pool, &pointers)
        .await
        .unwrap_or_default();
    docs.iter()
        .filter_map(|d| d.to_value())
        .filter_map(|e| extract(&e, &state.settings.content_public_url))
        .collect()
}

/// `pagination` echoes the request's `lastId` (lowercased, left out when
/// absent), as decentraland/lamb2:src/controllers/handlers/wearables-catalog-handler.ts:68-72.
fn catalog_response(
    items_key: &str,
    items: Vec<Value>,
    filters: &CatalogFilters,
    limit: i64,
    last_id: Option<&str>,
    next_last_id: Option<String>,
    id_param_name: &str,
) -> Response {
    let mut pagination = Map::new();
    pagination.insert("limit".into(), json!(limit));
    if let Some(last_id) = last_id {
        pagination.insert("lastId".into(), json!(last_id));
    }
    if let Some(nl) = next_last_id {
        let next = format!("?{}", build_next_query(filters, limit, &nl, id_param_name));
        pagination.insert("next".into(), json!(next));
    }
    Json(json!({
        items_key: items,
        "filters": filters.to_json(),
        "pagination": Value::Object(pagination),
    }))
    .into_response()
}

async fn catalog_wearables_with_base(state: &MarketplaceState, query: CatalogQuery) -> Response {
    let filters = &query.filters;
    let limit = query.limit;
    let only_base_collection = matches!(&filters.collection_ids, Some(c) if c.len() == 1 && c[0] == BASE_AVATARS_COLLECTION_ID);
    let base_collection_allowed = filters
        .collection_ids
        .as_ref()
        .is_none_or(|c| c.iter().any(|id| id == BASE_AVATARS_COLLECTION_ID))
        && filters.extended.allows(CollectionType::BaseWearable);
    let cursor_in_base_range = query
        .last_id
        .as_ref()
        .is_none_or(|l| l.starts_with(BASE_AVATARS_COLLECTION_ID));
    let in_base_range = base_collection_allowed && cursor_in_base_range;
    let on_chain_cursor = if in_base_range {
        None
    } else {
        query.last_id.clone()
    };
    let want_on_chain = !only_base_collection && filters.extended.allows(CollectionType::OnChain);
    let source = if want_on_chain {
        match urn_source(state).await {
            Ok(s) => Some(s),
            Err(envelope) => return envelope,
        }
    } else {
        None
    };

    // The urn leg is fetched at limit+1 concurrently and cut to what the base leg leaves.
    let base_leg = async {
        if !in_base_range {
            return Vec::new();
        }
        let base = cached_base_wearables(state).await;
        filter_and_extract_base_wearables(
            &base,
            filters,
            &query.last_id,
            (limit + 1) as usize,
            &state.settings.content_public_url,
        )
    };
    let urns_leg = async {
        match source {
            Some(source) => {
                item_urns(
                    state,
                    source,
                    "wearable",
                    filters,
                    limit + 1,
                    &on_chain_cursor,
                )
                .await
            }
            None => Ok(Vec::new()),
        }
    };
    let (off_chain, urns) = tokio::join!(base_leg, urns_leg);
    let mut urns = match urns {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    let remaining = limit - off_chain.len() as i64;
    let mut on_chain_defs = Vec::new();
    if want_on_chain && remaining >= 0 {
        urns.truncate((remaining + 1) as usize);
        let pointers = urns.iter().map(|u| u.to_lowercase()).collect();
        on_chain_defs = definitions_for(state, pointers, extract_wearable_definition).await;
    }
    let (items, next_last_id) = paginate_merged(off_chain, on_chain_defs, limit);
    catalog_response(
        "wearables",
        items,
        filters,
        limit,
        query.last_id.as_deref(),
        next_last_id,
        "wearableId",
    )
}

async fn catalog(
    state: &MarketplaceState,
    query: CatalogQuery,
    item_type_prefix: &str,
    id_param_name: &str,
    items_key: &str,
    extract: fn(&Value, &str) -> Option<Value>,
) -> Response {
    let source = match urn_source(state).await {
        Ok(s) => s,
        Err(envelope) => return envelope,
    };
    let key = format!(
        "{item_type_prefix}|{}|{}|{:?}",
        query.limit,
        query.last_id.as_deref().unwrap_or(""),
        query.filters
    );
    let page = state
        .lambdas
        .catalog
        .get_or_fetch(key, || async {
            let urns = item_urns(
                state,
                source,
                item_type_prefix,
                &query.filters,
                query.limit,
                &query.last_id,
            )
            .await?;
            let pointers = urns.iter().map(|u| u.to_lowercase()).collect();
            let definitions = definitions_for(state, pointers, extract).await;
            let (items, next_last_id) = paginate(definitions, query.limit);
            Ok::<Value, Response>(json!({ "items": items, "next": next_last_id }))
        })
        .await;
    let page = match page {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let items = page["items"].as_array().cloned().unwrap_or_default();
    let next_last_id = page["next"].as_str().map(str::to_string);
    catalog_response(
        items_key,
        items,
        &query.filters,
        query.limit,
        query.last_id.as_deref(),
        next_last_id,
        id_param_name,
    )
}

/// With `extensions` off (`LAMBDAS_CATALOG_EXTENSIONS_ENABLED`, off under
/// the mirror posture) the extended filters are ignored, and alone do not
/// count as a filter, as in lamb2 (decentraland/lamb2:src/logic/items-catalog.ts:14-43).
fn wearables_query(qs: &str, extensions: bool) -> Result<CatalogQuery, Box<Response>> {
    let p = catalog_params_from_query(qs);
    let extended = if extensions {
        parse_extended_filters(&p)?
    } else {
        ExtendedFilters::default()
    };
    parse_catalog_query(&p, &p.wearable_id, "wearableId", extended)
}

/// `GET /lambdas/collections/wearables`.
pub async fn collections_wearables_catalog(
    State(state): State<SharedState>,
    request: Request,
) -> Response {
    let extensions = state.settings.lambdas_catalog_extensions;
    match wearables_query(request.uri().query().unwrap_or(""), extensions) {
        Ok(query) => catalog_wearables_with_base(&state, query).await,
        Err(resp) => *resp,
    }
}

/// `GET /lambdas/collections/emotes`.
pub async fn collections_emotes_catalog(
    State(state): State<SharedState>,
    request: Request,
) -> Response {
    let p = catalog_params_from_query(request.uri().query().unwrap_or(""));
    match parse_catalog_query(&p, &p.emote_id, "emoteId", ExtendedFilters::default()) {
        Ok(query) => {
            catalog(
                &state,
                query,
                "emote",
                "emoteId",
                "emotes",
                extract_emote_definition,
            )
            .await
        }
        Err(resp) => *resp,
    }
}

fn base_collections() -> Vec<Value> {
    vec![
        json!({ "id": BASE_AVATARS_COLLECTION_ID, "name": "Base Wearables" }),
        json!({ "id": BASE_EMOTES_COLLECTION_ID, "name": "Base Emotes" }),
    ]
}

/// The first 1,000 collections of each network in urn byte order, the old
/// squid query's page (`collections_from_squid`).
async fn mirrored_collections(pool: &PgPool) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query_as(
        "(SELECT urn, name FROM shop.collections WHERE network = 'ethereum' ORDER BY urn COLLATE \"C\" LIMIT $1) \
         UNION ALL \
         (SELECT urn, name FROM shop.collections WHERE network = 'matic' ORDER BY urn COLLATE \"C\" LIMIT $1)",
    )
    .bind(COLLECTIONS_PER_NETWORK)
    .fetch_all(pool)
    .await
}

/// `GET /lambdas/collections`, `/lambdas/nfts/collections`: the two base
/// collections and the mirrored ones; before the mirror's first complete pass
/// the unavailable envelope.
pub async fn nfts_collections(State(state): State<SharedState>) -> Response {
    if let Err(envelope) = mirrors::require_ready(&state, Mirror::Collections).await {
        return envelope;
    }
    let cached = state
        .lambdas
        .collections
        .get_or_fetch((), || async {
            let mut collections = base_collections();
            let items = mirrored_collections(&state.pool).await?;
            collections.extend(
                items
                    .into_iter()
                    .map(|(urn, name)| json!({ "id": urn, "name": name })),
            );
            Ok::<Value, sqlx::Error>(json!({ "collections": collections }))
        })
        .await;
    match cached {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lambdas collections query failed");
            lambdas_internal_error()
        }
    }
}

/// `GET /lambdas/outfits/{address}`, `/outfits/{address}`: the address's
/// outfits entity (through the read-through, as profiles), keeping only the
/// outfits it may wear (`sanitize_outfits`), with `namesForExtraSlots` (the
/// names it owns); an address without names keeps only slots 0-4. An entity
/// without outfits is answered as deployed (decentraland/lamb2's
/// `getOutfits`). A failed content read or third-party check is lamb2's
/// bare 500, a failed ownership or names read its 502 `FetcherError`.
pub async fn outfits(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let address = id.to_lowercase();
    let st = state.clone();
    let cached = state
        .lambdas
        .outfits
        .get_or_fetch(address.clone(), || async move {
            let pointer = format!("{address}:outfits");
            let entities = super::profiles::active_entities(&st, std::slice::from_ref(&pointer))
                .await
                .map_err(|()| super::lambdas_internal_error())?;
            let Some(mut entity) = entities.into_iter().next() else {
                return Ok::<Value, Response>(Value::Null);
            };
            let has_outfits = entity
                .pointer("/metadata/outfits")
                .and_then(Value::as_array)
                .is_some_and(|o| !o.is_empty());
            if !has_outfits {
                return Ok(entity);
            }
            // `owned_names` answers empty without a marketplace subgraph or
            // for an id that is not an address.
            let (sanitized, names) = tokio::join!(
                sanitize_outfits(&st, &address, &mut entity),
                super::graph::owned_names(&st.index, &address)
            );
            sanitized?;
            let owned_names: Vec<String> = names
                .map_err(|e| super::third_party::fetch_error(&address, &e))?
                .iter()
                .map(|n| n.name.clone())
                .collect();
            apply_names(&mut entity, &owned_names);
            Ok(entity)
        })
        .await;
    match cached {
        Ok(v) if !v.is_null() => Json(v).into_response(),
        Ok(_) => not_found("Outfits not found"),
        Err(resp) => resp,
    }
}

/// Drops every outfit with a wearable the address does not own, as
/// decentraland/lamb2's `getOutfits` does: base and off-chain items pass,
/// third-party items go through `third_party::sanitize_third_party_urns`,
/// and any other must be an owned item (and, given a token id, that token),
/// rewritten `<urn>:<tokenId or first owned tokenId>`. The kept wearables
/// are lowercased. The upstream walk stops at the first wearable not owned,
/// and its third-party check then decides alone for the items seen so far:
/// an outfit `[owned third-party, not owned]` is kept with the first only.
/// Without a collections subgraph nothing is checked, as profiles do. A
/// failed ownership read answers the old lambdas' 502 `FetcherError`; a
/// failed third-party check lamb2's bare 500, as its checker's error is not
/// one `errorHandler` maps.
async fn sanitize_outfits(
    state: &MarketplaceState,
    address: &str,
    entity: &mut Value,
) -> Result<(), Response> {
    let Some(outfits) = entity
        .pointer_mut("/metadata/outfits")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    if outfits.is_empty() {
        return Ok(());
    }
    if !graph::any_collections_reader(&state.index) {
        for outfit in outfits.iter_mut() {
            if let Some(wearables) = outfit
                .pointer_mut("/outfit/wearables")
                .and_then(Value::as_array_mut)
            {
                for w in wearables.iter_mut() {
                    if let Some(lower) = w.as_str().map(str::to_lowercase) {
                        *w = Value::String(lower);
                    }
                }
            }
        }
        return Ok(());
    }
    let copies = graph::owned_copies(&state.index, address, "wearable")
        .await
        .map_err(|e| super::third_party::fetch_error(address, &e))?;
    let mut tokens: HashMap<&str, Vec<&str>> = HashMap::new();
    for c in copies.iter() {
        tokens
            .entry(c.urn.as_str())
            .or_default()
            .push(c.token_id.as_str());
    }

    // Per outfit: the wearables kept, the third-party ones among them, and
    // whether every other one was owned.
    let walked: Vec<(Vec<String>, Vec<String>, bool)> = outfits
        .iter()
        .map(|outfit| {
            let mut wearables = Vec::new();
            let mut third_party = Vec::new();
            let mut all_owned = true;
            let worn = outfit
                .pointer("/outfit/wearables")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str);
            for wearable in worn {
                if wearable.contains("off-chain") || wearable.contains("base-avatars") {
                    wearables.push(wearable.to_string());
                    continue;
                }
                if super::third_party::is_third_party_urn(wearable) {
                    wearables.push(wearable.to_string());
                    third_party.push(wearable.to_string());
                    continue;
                }
                let (urn, token_id) = super::profiles::split_urn_and_token_id(wearable);
                let owned = tokens.get(urn).and_then(|held| match token_id {
                    Some(t) => held.contains(&t).then_some(t),
                    None => held.first().copied(),
                });
                match owned {
                    Some(t) => wearables.push(format!("{urn}:{t}")),
                    None => {
                        all_owned = false;
                        break;
                    }
                }
            }
            (wearables, third_party, all_owned)
        })
        .collect();
    let worn_third_party: Vec<String> = walked
        .iter()
        .flat_map(|(_, tp, _)| tp.iter().cloned())
        .collect();
    let owned_third_party: HashSet<String> = if worn_third_party.is_empty() {
        HashSet::new()
    } else {
        super::third_party::sanitize_third_party_urns(state, address, &worn_third_party)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, address, "outfits: third-party items could not be checked");
                super::lambdas_internal_error()
            })?
            .into_iter()
            .collect()
    };
    let kept: Vec<Value> = outfits
        .drain(..)
        .zip(walked)
        .filter_map(|(mut outfit, (wearables, third_party, all_owned))| {
            let all_owned = if third_party.is_empty() {
                all_owned
            } else {
                third_party.iter().all(|w| owned_third_party.contains(w))
            };
            if !all_owned {
                return None;
            }
            let lowered = wearables.iter().map(|w| Value::String(w.to_lowercase()));
            if let Some(slot) = outfit.pointer_mut("/outfit/wearables") {
                *slot = Value::Array(lowered.collect());
            }
            Some(outfit)
        })
        .collect();
    *outfits = kept;
    Ok(())
}

/// lamb2's metadata: `{ outfits, namesForExtraSlots }` only, the slots 0-4
/// first and the extra slots (> 4) after them only for a name owner; an
/// outfit without a numeric slot is in neither group.
fn apply_names(entity: &mut Value, owned_names: &[String]) {
    let Some(metadata) = entity.get_mut("metadata").filter(|m| m.is_object()) else {
        return;
    };
    let outfits = metadata
        .get_mut("outfits")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();
    let slot = |o: &Value| o.get("slot").and_then(Value::as_f64);
    let (normal, extra): (Vec<Value>, Vec<Value>) = outfits
        .into_iter()
        .filter(|o| slot(o).is_some())
        .partition(|o| slot(o).is_some_and(|s| s <= 4.0));
    let mut kept = normal;
    if !owned_names.is_empty() {
        kept.extend(extra);
    }
    *metadata = json!({ "outfits": kept, "namesForExtraSlots": owned_names });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters(
        collection_ids: Option<Vec<&str>>,
        item_ids: Option<Vec<&str>>,
        text: Option<&str>,
        ext: ExtendedFilters,
    ) -> CatalogFilters {
        CatalogFilters {
            collection_ids: collection_ids.map(|v| v.into_iter().map(String::from).collect()),
            item_ids: item_ids.map(|v| v.into_iter().map(String::from).collect()),
            text_search: text.map(String::from),
            extended: ext,
        }
    }

    #[test]
    fn subgraph_where_carries_the_mirror_clauses_per_network() {
        let f = filters(None, None, Some("abc"), ExtendedFilters::default());
        assert_eq!(
            subgraph_where(Network::Matic, true, &f, &None).as_deref(),
            Some(
                r#"{ and: [{ itemType_in: [wearable_v1, wearable_v2, smart_wearable_v1] }, { searchText_contains: "abc" }] }"#
            )
        );
        assert_eq!(
            subgraph_where(
                Network::Matic,
                false,
                &f,
                &Some("urn:decentraland:matic:collections-v2:0xab:1".into())
            )
            .as_deref(),
            Some(
                r#"{ and: [{ itemType_in: [emote_v1] }, { searchText_contains: "abc" }, { urn_gt: "urn:decentraland:matic:collections-v2:0xab:1" }] }"#
            )
        );
        let f = filters(
            Some(vec![
                "urn:decentraland:mainnet:collections-v1:halloween_2019",
                "urn:decentraland:matic:collections-v2:0xAB",
            ]),
            None,
            None,
            ExtendedFilters::default(),
        );
        assert_eq!(
            subgraph_where(Network::Ethereum, true, &f, &None).as_deref(),
            Some(
                r#"{ and: [{ itemType_in: [wearable_v1, wearable_v2, smart_wearable_v1] }, { or: [{ urn_starts_with: "urn:decentraland:ethereum:collections-v1:halloween_2019:" }] }] }"#
            )
        );
        assert!(
            subgraph_where(Network::Matic, true, &f, &None)
                .unwrap()
                .contains(r#"{ urn_starts_with: "urn:decentraland:matic:collections-v2:0xab:" }"#)
        );
        let f = filters(
            Some(vec!["urn:decentraland:off-chain:base-avatars"]),
            None,
            None,
            ExtendedFilters::default(),
        );
        assert_eq!(
            subgraph_where(Network::Matic, true, &f, &None),
            None,
            "an off-chain collection has nothing on a subgraph"
        );
        let f = filters(
            None,
            Some(vec!["urn:decentraland:matic:collections-v2:0xab:1"]),
            None,
            ExtendedFilters::default(),
        );
        assert_eq!(subgraph_where(Network::Ethereum, true, &f, &None), None);
        assert!(
            subgraph_where(Network::Matic, true, &f, &None)
                .unwrap()
                .contains(r#"{ urn_in: ["urn:decentraland:matic:collections-v2:0xab:1"] }"#)
        );
        let ext = ExtendedFilters {
            categories: vec!["hat".into()],
            rarity: Some("epic".into()),
            name: Some("sn\"ail".into()),
            ..Default::default()
        };
        let doc =
            subgraph_where(Network::Matic, true, &filters(None, None, None, ext), &None).unwrap();
        assert!(doc.contains(r#"{ searchWearableCategory_in: ["hat"] }, { rarity: "epic" }, { searchText_contains: "sn\"ail" }"#), "{doc}");
    }

    #[test]
    fn subgraph_order_and_merge_follow_the_mirror_sorts() {
        assert_eq!(subgraph_order(&ExtendedFilters::default()), ("urn", "asc"));
        let date_desc = ExtendedFilters {
            sort: Some("date".into()),
            direction: Some("DESC".into()),
            ..Default::default()
        };
        assert_eq!(subgraph_order(&date_desc), ("createdAt", "desc"));
        assert_eq!(
            subgraph_order(&ExtendedFilters {
                sort: Some("name".into()),
                direction: Some("ASC".into()),
                ..Default::default()
            }),
            ("searchText", "asc")
        );
        let items = parse_items(&json!({ "items": [
            { "urn": "urn:b", "createdAt": "20", "metadata": { "wearable": { "name": "Snail Helmet" }, "emote": null } },
            { "urn": "urn:a", "createdAt": "30", "metadata": { "wearable": null, "emote": { "name": "Dance" } } },
            { "urn": "urn:c", "createdAt": "10", "metadata": { "wearable": { "name": "Snail Boots" } } },
            { "createdAt": "1" }
        ] }));
        assert_eq!(items.len(), 3);
        assert_eq!(
            merge_items(items.clone(), &ExtendedFilters::default(), 2),
            ["urn:a", "urn:b"]
        );
        assert_eq!(
            merge_items(items.clone(), &date_desc, 3),
            ["urn:a", "urn:b", "urn:c"]
        );
        let snail = ExtendedFilters {
            name: Some("snail".into()),
            ..Default::default()
        };
        assert_eq!(merge_items(items, &snail, 10), ["urn:b", "urn:c"]);
    }

    #[test]
    fn cursor_mainnet_to_mirror_spelling() {
        for (input, want) in [
            (
                "urn:decentraland:mainnet:collections-v1:0xabc:0",
                "urn:decentraland:ethereum:collections-v1:0xabc:0",
            ),
            (
                "urn:decentraland:matic:collections-v2:0xabc:0",
                "urn:decentraland:matic:collections-v2:0xabc:0",
            ),
            (
                "urn:decentraland:off-chain:base-avatars:eyes_00",
                "urn:decentraland:off-chain:base-avatars:eyes_00",
            ),
        ] {
            assert_eq!(to_mirror_urn(input), want);
        }
    }

    #[test]
    fn clamp_limit_defaults_and_caps() {
        assert_eq!(clamp_limit(&None), MAX_LIMIT);
        for raw in ["0", "9999", "abc"] {
            assert_eq!(clamp_limit(&Some(raw.into())), MAX_LIMIT);
        }
        assert_eq!(clamp_limit(&Some("10".into())), 10);
    }

    fn parse_plain(p: &CatalogParams) -> Result<CatalogQuery, Box<Response>> {
        parse_catalog_query(p, &[], "wearableId", ExtendedFilters::default())
    }

    #[test]
    fn parse_requires_a_filter() {
        assert!(parse_plain(&CatalogParams::default()).is_err());
    }

    #[test]
    fn an_upstream_shaped_query_is_untouched_by_the_extension() {
        let q = wearables_query("collectionId=urn:c1", true).unwrap_or_else(|_| panic!("parse"));
        assert_eq!(q.filters.to_json(), json!({ "collectionIds": ["urn:c1"] }));
        assert_eq!(
            build_next_query(&q.filters, 50, "urn:c1:5", "wearableId"),
            "collectionId=urn%3Ac1&limit=50&lastId=urn%3Ac1%3A5"
        );
    }

    #[test]
    fn without_the_extensions_the_query_is_lamb2s() {
        for qs in [
            "collectionType=on-chain",
            "category=eyewear",
            "rarity=ultra",
            "orderBy=rarity",
        ] {
            assert!(
                wearables_query(qs, false).is_err(),
                "{qs} alone is no filter and is not validated"
            );
        }
        let q = wearables_query(
            "textSearch=hat&collectionType=nope&rarity=ultra&orderBy=x",
            false,
        )
        .unwrap_or_else(|_| panic!("parse"));
        assert_eq!(q.filters.to_json(), json!({ "textSearch": "hat" }));
    }

    #[test]
    fn the_next_query_is_encoded_as_url_search_params() {
        let q = wearables_query("textSearch=a b*~", true).unwrap_or_else(|_| panic!("parse"));
        assert_eq!(
            build_next_query(&q.filters, 5, "urn:x", "wearableId"),
            "textSearch=a+b*%7E&limit=5&lastId=urn%3Ax"
        );
    }

    #[test]
    fn limit_is_read_with_parse_int() {
        assert_eq!(clamp_limit(&Some("10abc".into())), 10);
        assert_eq!(clamp_limit(&Some("abc".into())), MAX_LIMIT);
        assert_eq!(clamp_limit(&Some("0".into())), MAX_LIMIT);
        assert_eq!(clamp_limit(&Some("501".into())), MAX_LIMIT);
        assert_eq!(clamp_limit(&None), MAX_LIMIT);
    }

    #[test]
    fn on_chain_definitions_sort_by_locale_compare() {
        // Byte order puts `_` (0x5f) after `-` and digits; UCA puts both
        // punctuation marks before digits and letters.
        let defs = vec![json!({ "id": "urn:a:b1" }), json!({ "id": "urn:a:b_1" })];
        let (items, _) = paginate(defs, 10);
        let ids: Vec<&str> = items.iter().filter_map(|d| d["id"].as_str()).collect();
        assert_eq!(ids, vec!["urn:a:b_1", "urn:a:b1"]);
    }

    #[test]
    fn collection_type_alone_satisfies_the_filter_requirement() {
        let q =
            wearables_query("collectionType=on-chain", true).unwrap_or_else(|_| panic!("parse"));
        assert_eq!(
            q.filters.extended.collection_types,
            vec![CollectionType::OnChain]
        );
        assert!(!q.filters.extended.allows(CollectionType::BaseWearable));
        assert!(q.filters.extended.allows(CollectionType::OnChain));
    }

    #[test]
    fn each_extended_filter_alone_satisfies_the_filter_requirement() {
        for qs in ["category=eyewear", "rarity=mythic", "name=aviator"] {
            assert!(wearables_query(qs, true).is_ok(), "{qs} should be a filter");
        }
    }

    #[test]
    fn ordering_alone_is_not_a_filter_and_needs_one_tier() {
        assert!(wearables_query("orderBy=rarity", true).is_err());
        assert!(wearables_query("orderBy=rarity&collectionType=on-chain", true).is_ok());
        assert!(wearables_query("category=hat&orderBy=rarity", true).is_err());
        assert!(
            wearables_query(
                "orderBy=rarity&collectionType=on-chain&collectionType=base-wearable",
                true
            )
            .is_err()
        );
    }

    #[test]
    fn an_unknown_collection_type_or_rarity_is_refused() {
        assert!(wearables_query("collectionType=on-chian", true).is_err());
        assert!(wearables_query("rarity=ultra", true).is_err());
        assert_eq!(
            validate_rarity("ultra").unwrap_err(),
            "Invalid rarity requested: 'ultra'."
        );
    }

    #[test]
    fn direction_defaults_per_sort_field_and_validates() {
        let q = wearables_query("collectionType=on-chain&orderBy=name", true)
            .unwrap_or_else(|_| panic!("parse"));
        assert_eq!(q.filters.extended.direction.as_deref(), Some("ASC"));
        let q = wearables_query("collectionType=on-chain&orderBy=rarity", true)
            .unwrap_or_else(|_| panic!("parse"));
        assert_eq!(q.filters.extended.direction.as_deref(), Some("DESC"));
        assert!(
            wearables_query(
                "collectionType=on-chain&orderBy=rarity&direction=sideways",
                true
            )
            .is_err()
        );
    }

    #[test]
    fn extended_filters_survive_pagination_and_echo_only_when_used() {
        let q = wearables_query(
            "collectionType=on-chain&category=hat&rarity=mythic&orderBy=date",
            true,
        )
        .unwrap_or_else(|_| panic!("parse"));
        let next = build_next_query(&q.filters, 50, "urn:c1:5", "wearableId");
        for expected in [
            "collectionType=on-chain",
            "category=hat",
            "rarity=mythic",
            "orderBy=date",
            "direction=DESC",
        ] {
            assert!(next.contains(expected), "next dropped {expected}: {next}");
        }
        let q = wearables_query("collectionType=base-wearable&category=eyewear", true)
            .unwrap_or_else(|_| panic!("parse"));
        let echoed = q.filters.to_json();
        assert_eq!(echoed["collectionTypes"], json!(["base-wearable"]));
        assert_eq!(echoed["categories"], json!(["eyewear"]));
        assert!(echoed.get("rarity").is_none() && echoed.get("orderBy").is_none());
    }

    #[test]
    fn a_rarity_filter_excludes_the_ungraded_base_tier() {
        let q = wearables_query("rarity=mythic", true).unwrap_or_else(|_| panic!("parse"));
        let base = BaseWearable {
            urn: "urn:decentraland:off-chain:base-avatars:aviatorstyle".into(),
            name: "aviatorstyle".into(),
            english_name: None,
            category: "eyewear".into(),
            entity: json!({}),
        };
        assert!(
            filter_and_extract_base_wearables(&[base], &q.filters, &None, 10, "http://x")
                .is_empty()
        );
    }

    #[test]
    fn parse_rejects_short_text_search_and_lowercases_ids() {
        let p = CatalogParams {
            text_search: Some("ab".into()),
            ..Default::default()
        };
        assert!(parse_plain(&p).is_err());
        let p = CatalogParams {
            collection_id: vec!["URN:Decentraland".into()],
            ..Default::default()
        };
        assert_eq!(
            parse_plain(&p)
                .unwrap_or_else(|_| panic!("parse"))
                .filters
                .collection_ids
                .unwrap(),
            vec!["urn:decentraland".to_string()]
        );
    }

    #[test]
    fn paginate_sorts_slices_and_reports_next() {
        let (items, next) = paginate(
            vec![
                json!({ "id": "urn:c" }),
                json!({ "id": "urn:a" }),
                json!({ "id": "urn:b" }),
            ],
            2,
        );
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], "urn:a");
        assert_eq!(items[1]["id"], "urn:b");
        assert_eq!(next.as_deref(), Some("urn:b"));
        let (items, next) = paginate(vec![json!({ "id": "urn:a" })], 2);
        assert_eq!(items.len(), 1);
        assert!(next.is_none());
    }

    #[test]
    fn build_next_query_orders_params() {
        let f = CatalogFilters {
            collection_ids: Some(vec!["urn:c1".into()]),
            item_ids: None,
            text_search: Some("hat".into()),
            extended: ExtendedFilters::default(),
        };
        assert_eq!(
            build_next_query(&f, 50, "urn:c1:5", "wearableId"),
            "collectionId=urn%3Ac1&textSearch=hat&limit=50&lastId=urn%3Ac1%3A5"
        );
    }

    /// lamb2's `getOutfits`: slots 0-4 first, the extra ones after them for a
    /// name owner only, an outfit without a slot dropped, other keys dropped.
    #[test]
    fn outfits_without_names_keep_the_first_five_slots() {
        let mut e = json!({ "metadata": { "outfits": [{ "slot": 0 }, { "slot": 4 }, { "slot": 5 }, { "slot": 9 }, {}], "extra": 1 } });
        apply_names(&mut e, &[]);
        assert_eq!(
            e["metadata"],
            json!({ "outfits": [{ "slot": 0 }, { "slot": 4 }], "namesForExtraSlots": [] })
        );
        let mut e = json!({ "metadata": { "outfits": [{ "slot": 7 }, { "slot": 0 }, {}] } });
        apply_names(&mut e, &["alice".to_string()]);
        assert_eq!(
            e["metadata"],
            json!({ "outfits": [{ "slot": 0 }, { "slot": 7 }], "namesForExtraSlots": ["alice"] })
        );
    }

    /// lamb2's third-party checker error is not one `errorHandler` maps: a
    /// failed check (here the mapping read) is the bare 500, not the 502.
    #[tokio::test]
    async fn a_failed_third_party_check_is_lamb2s_bare_500() {
        let state = super::super::tests::support::state_with_empty_readers(None).await;
        let mut entity = json!({ "metadata": { "outfits": [{ "slot": 0, "outfit": { "wearables": [
            "urn:decentraland:matic:collections-thirdparty:tp:c1:hat:mainnet:0x0000000000000000000000000000000000000001:1"
        ] } }] } });
        let resp = sanitize_outfits(
            &state,
            "0x0000000000000000000000000000000000000001",
            &mut entity,
        )
        .await
        .unwrap_err();
        assert_eq!(resp.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({ "error": "Internal Server Error" })
        );
    }
}
