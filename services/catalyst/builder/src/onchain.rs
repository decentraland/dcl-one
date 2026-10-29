//! The builder's on-chain reads: an address's published collections and
//! items (`GET /v1/{address}/collections`, `/v1/{address}/items`) and the
//! curation console's committee and under-review listings. The old crate
//! read the marketplace squid over a side pool; v2 runs no squid, so these
//! are the L1/L2 collections subgraph readers (`marketplace::graph`, the
//! ONE guarded client with its retry ladder and body cap), shaped exactly as
//! the squid rows were for the sites' `BuilderCollectionSchema`,
//! `OrphanItemSchema` and `ReviewRow`. With no collections subgraph
//! configured the address routes answer 503 and the curation listings empty,
//! as the old node did with `BUILDER_MARKETPLACE_PG_CONNECTION_STRING` unset.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;
use serde_json::{Value, json};

use crate::chain_index::ChainIndex;
use crate::chain_index::graph::{THE_GRAPH_PAGE_SIZE, any_collections_reader, graph_query};

use super::http::ApiError;

/// Collection/item sync status served on curation and on-chain rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "builder/", rename_all = "snake_case")
)]
pub enum CollectionStatus {
    Synced,
    UnderReview,
    Unsynced,
}

/// The only collection type this reader serves (third-party collections are
/// not indexed by the collections subgraphs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "builder/", rename_all = "snake_case")
)]
pub enum CollectionType {
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "builder/", rename_all = "snake_case")
)]
pub enum CurationStatus {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "builder/", rename_all = "snake_case")
)]
pub enum ItemKind {
    SmartWearable,
    Emote,
    Wearable,
}

/// Row of `GET /v1/{address}/collections` (consumed by the sites creator-hub
/// collections list and metrics).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct BuilderCollectionOut {
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub chain_id: Option<i64>,
    pub contract_address: String,
    pub count: i32,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub created_at: Option<i64>,
    pub creator: Option<String>,
    pub id: String,
    pub is_approved: bool,
    pub is_published: bool,
    pub name: String,
    pub network: Option<String>,
    pub owner: Option<String>,
    pub pending: bool,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub reviewed_at: Option<i64>,
    pub status: CollectionStatus,
    pub third_party_id: Option<String>,
    pub thumbs: Vec<String>,
    #[serde(rename = "type")]
    pub kind: CollectionType,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub updated_at: Option<i64>,
    pub urn: Option<String>,
}

/// Member of the curation committee (`GET /v1/collections/curation`).
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CommitteeMemberOut {
    pub address: String,
    pub name: String,
}

/// Latest curation attached to a collection under review.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ReviewCurationOut {
    pub assignee: String,
    pub collection_id: String,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub created_at: Option<i64>,
    pub id: String,
    pub status: CurationStatus,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub updated_at: Option<i64>,
}

/// Row of the `collections` list in `GET /v1/collections/curation`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ReviewRowOut {
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub created_at: Option<i64>,
    pub curation: Option<ReviewCurationOut>,
    pub has_reviews: bool,
    pub id: String,
    pub is_approved: bool,
    pub is_programmatic: bool,
    pub item_count: i32,
    pub name: String,
    pub owner: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub reviewed_at: Option<i64>,
    pub status: CollectionStatus,
    #[serde(rename = "type")]
    pub kind: CollectionType,
}

/// Row of `GET /v1/{address}/items`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct OrphanItemOut {
    pub beneficiary: Option<String>,
    pub category: Option<String>,
    pub collection_id: Option<String>,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub created_at: Option<i64>,
    pub grad: Option<String>,
    pub id: String,
    pub image: Option<String>,
    pub is_published: bool,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub max_supply: Option<i64>,
    pub name: String,
    pub network: Option<String>,
    pub price: Option<String>,
    pub rarity: Option<String>,
    pub status: CollectionStatus,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub total_supply: Option<i64>,
    #[serde(rename = "type")]
    pub kind: ItemKind,
    #[serde(rename = "updatedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub updated_at: Option<i64>,
    pub urn: Option<String>,
}

const MAX_ROWS: usize = 1000;
const MAX_REVIEW_ROWS: usize = 1000;
/// Curation pages walked for the committee before the answer is cut off.
const MAX_CURATION_PAGES: usize = 10;
/// Collections per `collection_in` chunk of the latest-curation lookup.
const CURATION_CHUNK: usize = 100;
pub const MARKETPLACE_DOWN: &str = "marketplace data unavailable";

/// The two collections subgraphs, with the squid's `Network` spelling and
/// chain id the old rows carried.
#[derive(Clone, Copy)]
struct Reader {
    url_of: fn(&ChainIndex) -> Option<&str>,
    network: &'static str,
    chain_id: i64,
}

const READERS: [Reader; 2] = [
    Reader {
        url_of: |s| s.settings.eth_collections_subgraph_url.as_deref(),
        network: "ETHEREUM",
        chain_id: 1,
    },
    Reader {
        url_of: |s| s.settings.matic_collections_subgraph_url.as_deref(),
        network: "POLYGON",
        chain_id: 137,
    },
];

pub fn configured(market: Option<&ChainIndex>) -> bool {
    market.is_some_and(any_collections_reader)
}

fn upstream(e: String) -> ApiError {
    tracing::warn!(error = %e, "collections subgraph read failed");
    ApiError::http(502, MARKETPLACE_DOWN)
}

#[derive(Debug)]
struct DbCollection {
    id: String,
    owner: Option<String>,
    creator: Option<String>,
    name: Option<String>,
    urn: Option<String>,
    items_count: Option<i32>,
    is_completed: Option<bool>,
    is_approved: Option<bool>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    reviewed_at: Option<i64>,
    network: Option<String>,
    chain_id: Option<i64>,
}

#[derive(Debug)]
struct DbItem {
    id: String,
    item_type: Option<String>,
    rarity: Option<String>,
    price: Option<String>,
    beneficiary: Option<String>,
    total_supply: Option<i64>,
    max_supply: Option<i64>,
    urn: Option<String>,
    image: Option<String>,
    collection_id: Option<String>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    name: Option<String>,
    category: Option<String>,
    network: Option<String>,
}

#[derive(Debug)]
struct DbReviewRow {
    id: String,
    name: Option<String>,
    creator: Option<String>,
    items_count: Option<i32>,
    is_completed: Option<bool>,
    is_approved: Option<bool>,
    created_at: Option<i64>,
    reviewed_at: Option<i64>,
    cur_id: Option<String>,
    cur_is_approved: Option<bool>,
    cur_curator: Option<String>,
    cur_timestamp: Option<i64>,
}

fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn opt_int(v: &Value, key: &str) -> Option<i64> {
    match v.get(key)? {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

fn opt_bool(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

fn parse_collection(v: &Value, reader: &Reader) -> Option<DbCollection> {
    Some(DbCollection {
        id: v.get("id")?.as_str()?.to_string(),
        owner: opt_str(v, "owner"),
        creator: opt_str(v, "creator"),
        name: opt_str(v, "name"),
        urn: opt_str(v, "urn"),
        items_count: opt_int(v, "itemsCount").and_then(|n| i32::try_from(n).ok()),
        is_completed: opt_bool(v, "isCompleted"),
        is_approved: opt_bool(v, "isApproved"),
        created_at: opt_int(v, "createdAt"),
        updated_at: opt_int(v, "updatedAt"),
        reviewed_at: opt_int(v, "reviewedAt"),
        network: Some(reader.network.to_string()),
        chain_id: Some(reader.chain_id),
    })
}

fn parse_item(v: &Value, reader: &Reader) -> Option<DbItem> {
    let meta = v.get("metadata").filter(|m| !m.is_null());
    let leg = |key: &str| meta.and_then(|m| m.get(key)).filter(|m| !m.is_null());
    let named = leg("wearable").or_else(|| leg("emote"));
    Some(DbItem {
        id: v.get("id")?.as_str()?.to_string(),
        item_type: opt_str(v, "itemType"),
        rarity: opt_str(v, "rarity"),
        price: v.get("price").map(|p| match p {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }),
        beneficiary: opt_str(v, "beneficiary"),
        total_supply: opt_int(v, "totalSupply"),
        max_supply: opt_int(v, "maxSupply"),
        urn: opt_str(v, "urn"),
        image: opt_str(v, "image"),
        collection_id: v
            .get("collection")
            .and_then(|c| c.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        created_at: opt_int(v, "createdAt"),
        updated_at: opt_int(v, "updatedAt"),
        name: named.and_then(|m| opt_str(m, "name")),
        category: named.and_then(|m| opt_str(m, "category")),
        network: Some(reader.network.to_string()),
    })
}

const COLLECTION_FIELDS: &str =
    "id owner creator name urn itemsCount isCompleted isApproved createdAt updatedAt reviewedAt";

/// The squid's `creator = $1 OR owner = $1 OR $1 = ANY(managers) OR $1 =
/// ANY(minters)` as four aliased lists, merged by id.
fn collections_query() -> String {
    format!(
        "query builderCollections($address: String!, $first: Int!) {{\
           asCreator: collections(first: $first, where: {{ creator: $address }}) {{ {COLLECTION_FIELDS} }}\
           asOwner: collections(first: $first, where: {{ owner: $address }}) {{ {COLLECTION_FIELDS} }}\
           asManager: collections(first: $first, where: {{ managers_contains: [$address] }}) {{ {COLLECTION_FIELDS} }}\
           asMinter: collections(first: $first, where: {{ minters_contains: [$address] }}) {{ {COLLECTION_FIELDS} }}\
         }}"
    )
}

const ITEMS_QUERY: &str = "query builderItems($address: String!, $first: Int!) {\
  items(first: $first, where: { creator: $address }, orderBy: createdAt, orderDirection: desc) {\
    id itemType rarity price beneficiary totalSupply maxSupply urn image collection { id } createdAt updatedAt\
    metadata { wearable { name category } emote { name category } }\
  }\
}";

const CURATORS_QUERY: &str = "query curators($idFrom: ID!, $first: Int!) {\
  curations(first: $first, where: { id_gt: $idFrom }, orderBy: id, orderDirection: asc) { id curator { id } }\
}";

const UNDER_REVIEW_QUERY: &str = "query underReview($first: Int!) {\
  collections(first: $first, where: { isCompleted: true, isApproved: false }, orderBy: createdAt, orderDirection: desc) {\
    id name creator itemsCount isCompleted isApproved createdAt reviewedAt\
  }\
}";

const LATEST_CURATIONS_QUERY: &str = "query latestCurations($ids: [String!]!, $first: Int!) {\
  curations(first: $first, where: { collection_in: $ids }, orderBy: timestamp, orderDirection: desc) {\
    id isApproved timestamp curator { id } collection { id }\
  }\
}";

fn list<'a>(data: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    data.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The address's published collections across the configured networks,
/// newest first, at most [`MAX_ROWS`].
pub async fn collections_for_address(
    market: &ChainIndex,
    address: &str,
) -> Result<Vec<BuilderCollectionOut>, ApiError> {
    let addr = address.to_ascii_lowercase();
    let mut seen = HashSet::new();
    let mut rows: Vec<DbCollection> = Vec::new();
    for reader in &READERS {
        let Some(url) = (reader.url_of)(market) else {
            continue;
        };
        let data = graph_query(
            market,
            url,
            &collections_query(),
            json!({ "address": addr, "first": THE_GRAPH_PAGE_SIZE }),
        )
        .await
        .map_err(upstream)?;
        for key in ["asCreator", "asOwner", "asManager", "asMinter"] {
            for v in list(&data, key) {
                if let Some(c) = parse_collection(v, reader)
                    && seen.insert(c.id.clone())
                {
                    rows.push(c);
                }
            }
        }
    }
    rows.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(MAX_ROWS);
    Ok(rows.into_iter().map(collection_out).collect())
}

/// The address's on-chain items, newest first; `only_orphans` asks for
/// collection-less items, which never exist on-chain, so it answers empty.
pub async fn items_for_address(
    market: &ChainIndex,
    address: &str,
    only_orphans: bool,
) -> Result<Vec<OrphanItemOut>, ApiError> {
    if only_orphans {
        return Ok(Vec::new());
    }
    let addr = address.to_ascii_lowercase();
    let mut rows: Vec<DbItem> = Vec::new();
    for reader in &READERS {
        let Some(url) = (reader.url_of)(market) else {
            continue;
        };
        let data = graph_query(
            market,
            url,
            ITEMS_QUERY,
            json!({ "address": addr, "first": THE_GRAPH_PAGE_SIZE }),
        )
        .await
        .map_err(upstream)?;
        rows.extend(list(&data, "items").filter_map(|v| parse_item(v, reader)));
    }
    rows.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(MAX_ROWS);
    Ok(rows.into_iter().map(item_out).collect())
}

/// Every wallet that ever curated, as the squid's `DISTINCT curator_id`
/// listed them: the curations walked in id order, at most
/// [`MAX_CURATION_PAGES`] pages per network.
pub async fn committee_members(market: &ChainIndex) -> Result<Vec<CommitteeMemberOut>, ApiError> {
    let mut addresses: Vec<String> = Vec::new();
    for reader in &READERS {
        let Some(url) = (reader.url_of)(market) else {
            continue;
        };
        let mut id_from = String::new();
        for _ in 0..MAX_CURATION_PAGES {
            let data = graph_query(
                market,
                url,
                CURATORS_QUERY,
                json!({ "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
            )
            .await
            .map_err(upstream)?;
            let page: Vec<&Value> = list(&data, "curations").collect();
            for c in &page {
                if let Some(addr) = c
                    .get("curator")
                    .and_then(|c| c.get("id"))
                    .and_then(Value::as_str)
                {
                    addresses.push(addr.to_string());
                }
            }
            if page.len() < THE_GRAPH_PAGE_SIZE as usize {
                break;
            }
            match page
                .last()
                .and_then(|c| c.get("id"))
                .and_then(Value::as_str)
            {
                Some(id) => id_from = id.to_string(),
                None => break,
            }
        }
    }
    Ok(shape_committee(addresses))
}

fn shape_committee(addresses: Vec<String>) -> Vec<CommitteeMemberOut> {
    let mut distinct: Vec<String> = addresses
        .into_iter()
        .filter_map(|addr| {
            let a = addr
                .split('-')
                .next()
                .unwrap_or(&addr)
                .trim()
                .to_ascii_lowercase();
            (!a.is_empty() && a.starts_with("0x")).then_some(a)
        })
        .collect();
    distinct.sort();
    distinct.dedup();
    distinct
        .into_iter()
        .map(|address| CommitteeMemberOut {
            name: short_address(&address),
            address,
        })
        .collect()
}

/// The completed, unapproved collections with their latest curation.
pub async fn collections_under_review(market: &ChainIndex) -> Result<Vec<ReviewRowOut>, ApiError> {
    let mut rows: Vec<DbReviewRow> = Vec::new();
    for reader in &READERS {
        let Some(url) = (reader.url_of)(market) else {
            continue;
        };
        let data = graph_query(
            market,
            url,
            UNDER_REVIEW_QUERY,
            json!({ "first": THE_GRAPH_PAGE_SIZE }),
        )
        .await
        .map_err(upstream)?;
        let collections: Vec<DbCollection> = list(&data, "collections")
            .filter_map(|v| parse_collection(v, reader))
            .collect();
        let ids: Vec<&str> = collections.iter().map(|c| c.id.as_str()).collect();
        let mut latest: BTreeMap<String, Value> = BTreeMap::new();
        for chunk in ids.chunks(CURATION_CHUNK) {
            let data = graph_query(
                market,
                url,
                LATEST_CURATIONS_QUERY,
                json!({ "ids": chunk, "first": THE_GRAPH_PAGE_SIZE }),
            )
            .await
            .map_err(upstream)?;
            for cur in list(&data, "curations") {
                let Some(collection) = cur
                    .get("collection")
                    .and_then(|c| c.get("id"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                latest
                    .entry(collection.to_string())
                    .or_insert_with(|| cur.clone());
            }
        }
        for c in collections {
            let cur = latest.get(&c.id);
            rows.push(DbReviewRow {
                cur_id: cur.and_then(|cur| opt_str(cur, "id")),
                cur_is_approved: cur.and_then(|cur| opt_bool(cur, "isApproved")),
                cur_curator: cur
                    .and_then(|cur| cur.get("curator"))
                    .and_then(|c| opt_str(c, "id")),
                cur_timestamp: cur.and_then(|cur| opt_int(cur, "timestamp")),
                id: c.id,
                name: c.name,
                creator: c.creator,
                items_count: c.items_count,
                is_completed: c.is_completed,
                is_approved: c.is_approved,
                created_at: c.created_at,
                reviewed_at: c.reviewed_at,
            });
        }
    }
    rows.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    rows.truncate(MAX_REVIEW_ROWS);
    Ok(rows.into_iter().map(review_row_out).collect())
}

fn to_ms(secs: Option<i64>) -> Option<i64> {
    match secs {
        Some(s) if s > 0 => Some(s.saturating_mul(1000)),
        _ => None,
    }
}

fn status_of(is_published: bool, is_approved: bool) -> CollectionStatus {
    if is_approved {
        CollectionStatus::Synced
    } else if is_published {
        CollectionStatus::UnderReview
    } else {
        CollectionStatus::Unsynced
    }
}

fn collection_out(c: DbCollection) -> BuilderCollectionOut {
    let is_published = c.is_completed.unwrap_or(false);
    let is_approved = c.is_approved.unwrap_or(false);
    BuilderCollectionOut {
        chain_id: c.chain_id,
        contract_address: c.id.clone(),
        count: c.items_count.unwrap_or(0),
        created_at: to_ms(c.created_at),
        creator: c.creator,
        id: c.id,
        is_approved,
        is_published,
        name: c.name.unwrap_or_default(),
        network: c.network,
        owner: c.owner,
        pending: false,
        reviewed_at: to_ms(c.reviewed_at),
        status: status_of(is_published, is_approved),
        third_party_id: None,
        thumbs: Vec::new(),
        kind: CollectionType::Standard,
        updated_at: to_ms(c.updated_at),
        urn: c.urn,
    }
}

fn short_address(addr: &str) -> String {
    if addr.len() >= 10 {
        format!("{}\u{2026}{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

fn review_row_out(r: DbReviewRow) -> ReviewRowOut {
    let collection_id = r.id.clone();
    let is_completed = r.is_completed.unwrap_or(false);
    let is_approved = r.is_approved.unwrap_or(false);

    let curator = r
        .cur_curator
        .as_deref()
        .map(|c| c.split('-').next().unwrap_or(c).trim().to_ascii_lowercase());

    let curation = match (r.cur_id.as_ref(), curator) {
        (Some(cid), Some(addr)) => Some(ReviewCurationOut {
            assignee: addr,
            collection_id,
            created_at: to_ms(r.cur_timestamp),
            id: cid.clone(),
            status: if r.cur_is_approved.unwrap_or(false) {
                CurationStatus::Approved
            } else {
                CurationStatus::Rejected
            },
            updated_at: to_ms(r.cur_timestamp),
        }),
        _ => None,
    };

    ReviewRowOut {
        created_at: to_ms(r.created_at),
        curation,
        has_reviews: r.cur_id.is_some(),
        id: r.id,
        is_approved,
        is_programmatic: false,
        item_count: r.items_count.unwrap_or(0),
        name: r.name.unwrap_or_default(),
        owner: r.creator,
        reviewed_at: to_ms(r.reviewed_at),
        status: status_of(is_completed, is_approved),
        kind: CollectionType::Standard,
    }
}

fn map_item_type(t: &str) -> ItemKind {
    if t.starts_with("smart") {
        ItemKind::SmartWearable
    } else if t.starts_with("emote") {
        ItemKind::Emote
    } else {
        ItemKind::Wearable
    }
}

fn item_out(i: DbItem) -> OrphanItemOut {
    let item_type = i.item_type.as_deref().unwrap_or("wearable_v2");
    let mapped = map_item_type(item_type);

    let name = i.name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        i.urn
            .as_deref()
            .and_then(|u| u.rsplit(':').next())
            .map(|s| s.to_string())
            .unwrap_or_else(|| i.id.clone())
    });
    OrphanItemOut {
        beneficiary: i.beneficiary,
        category: i.category,
        collection_id: i.collection_id,
        created_at: to_ms(i.created_at),
        grad: None,
        id: i.id,
        image: i.image,
        is_published: true,
        max_supply: i.max_supply,
        name,
        network: i.network,
        price: i.price,
        rarity: i.rarity,
        status: CollectionStatus::Synced,
        total_supply: i.total_supply,
        kind: mapped,
        updated_at: to_ms(i.updated_at),
        urn: i.urn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_collection() -> DbCollection {
        DbCollection {
            id: "0xc0ffee".into(),
            owner: Some("0xowner".into()),
            creator: Some("0xcreator".into()),
            name: Some("Hats".into()),
            urn: Some("urn:decentraland:matic:collections-v2:0xc0ffee".into()),
            items_count: Some(3),
            is_completed: Some(true),
            is_approved: Some(false),
            created_at: Some(1_700_000_000),
            updated_at: Some(1_700_000_100),
            reviewed_at: None,
            network: Some("matic".into()),
            chain_id: Some(137),
        }
    }

    fn db_item() -> DbItem {
        DbItem {
            id: "item-1".into(),
            item_type: Some("smart_wearable_v1".into()),
            rarity: Some("epic".into()),
            price: Some("1000000000000000000".into()),
            beneficiary: Some("0xbenef".into()),
            total_supply: Some(7),
            max_supply: Some(100),
            urn: Some("urn:decentraland:matic:collections-v2:0xc0ffee:0".into()),
            image: Some("https://img".into()),
            collection_id: Some("0xc0ffee".into()),
            created_at: Some(1_700_000_000),
            updated_at: None,
            name: Some("Cap".into()),
            category: Some("hat".into()),
            network: Some("matic".into()),
        }
    }

    fn db_review_row(with_curation: bool) -> DbReviewRow {
        DbReviewRow {
            id: "0xc0ffee".into(),
            name: Some("Hats".into()),
            creator: Some("0xcreator".into()),
            items_count: Some(3),
            is_completed: Some(true),
            is_approved: Some(false),
            created_at: Some(1_700_000_000),
            reviewed_at: None,
            cur_id: with_curation.then(|| "cur-1".into()),
            cur_is_approved: with_curation.then_some(false),
            cur_curator: with_curation.then(|| "0xCurator-0".into()),
            cur_timestamp: with_curation.then_some(1_700_000_200),
        }
    }

    fn old_to_ms(secs: Option<i64>) -> Value {
        match secs {
            Some(s) if s > 0 => json!(s.saturating_mul(1000)),
            _ => Value::Null,
        }
    }

    /// Compared as parsed JSON: object key order is not part of the contract and flips
    /// with serde_json's preserve_order feature.
    #[test]
    fn collection_wire_bytes_match_the_old_json_macro() {
        let c = db_collection();
        let old = json!({
            "id": c.id,
            "name": c.name.clone().unwrap_or_default(),
            "type": "standard",
            "is_published": true,
            "is_approved": false,
            "reviewed_at": old_to_ms(c.reviewed_at),
            "created_at": old_to_ms(c.created_at),
            "updated_at": old_to_ms(c.updated_at),
            "contract_address": c.id,
            "third_party_id": Value::Null,
            "urn": c.urn,
            "status": "under_review",
            "pending": false,
            "count": c.items_count.unwrap_or(0),
            "thumbs": Value::Array(vec![]),
            "owner": c.owner,
            "creator": c.creator,
            "network": c.network,
            "chain_id": c.chain_id,
        });
        assert_eq!(
            serde_json::to_value(collection_out(db_collection())).unwrap(),
            old
        );
    }

    #[test]
    fn item_wire_bytes_match_the_old_json_macro() {
        let i = db_item();
        let old = json!({
            "id": i.id,
            "name": i.name,
            "type": "smart_wearable",
            "status": "synced",
            "createdAt": old_to_ms(i.created_at),
            "updatedAt": old_to_ms(i.updated_at),
            "grad": Value::Null,
            "rarity": i.rarity,
            "category": i.category,
            "price": i.price,
            "beneficiary": i.beneficiary,
            "total_supply": i.total_supply,
            "max_supply": i.max_supply,
            "urn": i.urn,
            "image": i.image,
            "collection_id": i.collection_id,
            "is_published": true,
            "network": i.network,
        });
        assert_eq!(serde_json::to_value(item_out(db_item())).unwrap(), old);
    }

    #[test]
    fn review_row_wire_bytes_match_the_old_json_macro() {
        let r = db_review_row(true);
        let old = json!({
            "id": r.id,
            "name": r.name.clone().unwrap_or_default(),
            "type": "standard",
            "is_programmatic": false,
            "status": "under_review",
            "is_approved": false,
            "has_reviews": true,
            "item_count": r.items_count.unwrap_or(0),
            "owner": r.creator,
            "created_at": old_to_ms(r.created_at),
            "reviewed_at": old_to_ms(r.reviewed_at),
            "curation": {
                "id": "cur-1",
                "collection_id": r.id,
                "assignee": "0xcurator",
                "status": "rejected",
                "created_at": old_to_ms(r.cur_timestamp),
                "updated_at": old_to_ms(r.cur_timestamp),
            },
        });
        assert_eq!(
            serde_json::to_value(review_row_out(db_review_row(true))).unwrap(),
            old
        );

        let no_curation = serde_json::to_value(review_row_out(db_review_row(false))).unwrap();
        assert_eq!(no_curation["curation"], Value::Null);
        assert_eq!(no_curation["has_reviews"], json!(false));
    }

    #[test]
    fn committee_member_wire_bytes_match_the_old_json_macro() {
        let member = CommitteeMemberOut {
            address: "0xabcdef0123456789abcdef0123456789abcdef01".into(),
            name: short_address("0xabcdef0123456789abcdef0123456789abcdef01"),
        };
        let old = json!({
            "address": "0xabcdef0123456789abcdef0123456789abcdef01",
            "name": "0xabcd\u{2026}ef01",
        });
        assert_eq!(serde_json::to_value(&member).unwrap(), old);
        let shaped = shape_committee(vec![
            "0xB-1".into(),
            "0xa".into(),
            "0xb-2".into(),
            "junk".into(),
            " ".into(),
        ]);
        assert_eq!(
            shaped
                .iter()
                .map(|m| m.address.as_str())
                .collect::<Vec<_>>(),
            vec!["0xa", "0xb"],
            "distinct, lowercased, sorted"
        );
    }

    #[test]
    fn subgraph_rows_parse_into_the_squid_shapes() {
        let reader = &READERS[1];
        let c = parse_collection(
            &json!({ "id": "0xc0ffee", "owner": "0xo", "creator": "0xc", "name": "Hats", "urn": "urn:x", "itemsCount": 3,
                     "isCompleted": true, "isApproved": false, "createdAt": "1700000000", "updatedAt": 1700000100, "reviewedAt": "0" }),
            reader,
        )
        .unwrap();
        assert_eq!(
            (
                c.items_count,
                c.is_completed,
                c.created_at,
                c.updated_at,
                c.reviewed_at
            ),
            (
                Some(3),
                Some(true),
                Some(1_700_000_000),
                Some(1_700_000_100),
                Some(0)
            )
        );
        assert_eq!(
            (c.network.as_deref(), c.chain_id),
            (Some("POLYGON"), Some(137))
        );
        let out = collection_out(c);
        assert_eq!(
            out.reviewed_at, None,
            "a zero reviewedAt renders null as the squid's did"
        );
        let i = parse_item(
            &json!({ "id": "0xc0ffee-0", "itemType": "emote_v1", "rarity": "rare", "price": "10", "beneficiary": "0xb", "totalSupply": "1",
                     "maxSupply": "100", "urn": "urn:y", "image": null, "collection": { "id": "0xc0ffee" }, "createdAt": "5", "updatedAt": "6",
                     "metadata": { "wearable": null, "emote": { "name": "Dance", "category": "dance" } } }),
            reader,
        )
        .unwrap();
        assert_eq!(
            (
                i.name.as_deref(),
                i.category.as_deref(),
                i.collection_id.as_deref()
            ),
            (Some("Dance"), Some("dance"), Some("0xc0ffee"))
        );
        assert_eq!(item_out(i).kind, ItemKind::Emote);
        assert!(parse_collection(&json!({ "name": "no id" }), reader).is_none());
    }

    #[tokio::test]
    async fn without_a_collections_reader_the_address_reads_are_unavailable() {
        let market = crate::tests::support::market_without_db(
            crate::chain_index::IndexSettings::for_tests(),
        );
        assert!(!configured(Some(&market)));
        assert!(!configured(None));
        assert!(
            collections_for_address(&market, "0x797066a17f83425c1b4c7a8cca52d19095520a52")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(committee_members(&market).await.unwrap().is_empty());
        assert!(collections_under_review(&market).await.unwrap().is_empty());
    }
}
