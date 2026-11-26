//! The curation console's under-review listing, read from the L1/L2
//! collections subgraphs (`marketplace::graph`, the ONE guarded client with
//! its retry ladder and body cap) in the squid's `ReviewRow` shape, and the
//! committee member row (its committee is [`crate::committee`]'s). With no
//! collections subgraph configured the listing is empty.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use crate::chain_index::ChainIndex;
use crate::chain_index::graph::{THE_GRAPH_PAGE_SIZE, any_collections_reader, graph_query};

use chrono::{DateTime, Utc};

use super::http::{ApiError, iso_timestamp_opt};

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
    #[serde(serialize_with = "iso_timestamp_opt")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub created_at: Option<DateTime<Utc>>,
    pub id: String,
    pub status: CurationStatus,
    #[serde(serialize_with = "iso_timestamp_opt")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Row of the `collections` list in `GET /v1/collections/curation`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ReviewRowOut {
    #[serde(serialize_with = "iso_timestamp_opt")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub created_at: Option<DateTime<Utc>>,
    pub curation: Option<ReviewCurationOut>,
    pub has_reviews: bool,
    pub id: String,
    pub is_approved: bool,
    pub is_programmatic: bool,
    pub item_count: i32,
    pub name: String,
    pub owner: Option<String>,
    #[serde(serialize_with = "iso_timestamp_opt")]
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub reviewed_at: Option<DateTime<Utc>>,
    pub status: CollectionStatus,
    #[serde(rename = "type")]
    pub kind: CollectionType,
}

const MAX_REVIEW_ROWS: usize = 1000;
/// Collections per `collection_in` chunk of the latest-curation lookup.
const CURATION_CHUNK: usize = 100;
pub const MARKETPLACE_DOWN: &str = "marketplace data unavailable";

/// The two collections subgraphs, with the squid's `Network` spelling the
/// old rows carried.
#[derive(Clone, Copy)]
struct Reader {
    url_of: fn(&ChainIndex) -> Option<&str>,
}

const READERS: [Reader; 2] = [
    Reader {
        url_of: |s| s.settings.eth_collections_subgraph_url.as_deref(),
    },
    Reader {
        url_of: |s| s.settings.matic_collections_subgraph_url.as_deref(),
    },
];

pub fn configured(market: Option<&ChainIndex>) -> bool {
    market.is_some_and(any_collections_reader)
}

fn upstream(e: String) -> ApiError {
    tracing::warn!(error = %e, "collections subgraph read failed");
    ApiError::bad_gateway(MARKETPLACE_DOWN)
}

#[derive(Debug)]
struct DbCollection {
    id: String,
    creator: Option<String>,
    name: Option<String>,
    items_count: Option<i32>,
    is_completed: Option<bool>,
    is_approved: Option<bool>,
    created_at: Option<i64>,
    reviewed_at: Option<i64>,
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

fn parse_collection(v: &Value) -> Option<DbCollection> {
    Some(DbCollection {
        id: v.get("id")?.as_str()?.to_string(),
        creator: opt_str(v, "creator"),
        name: opt_str(v, "name"),
        items_count: opt_int(v, "itemsCount").and_then(|n| i32::try_from(n).ok()),
        is_completed: opt_bool(v, "isCompleted"),
        is_approved: opt_bool(v, "isApproved"),
        created_at: opt_int(v, "createdAt"),
        reviewed_at: opt_int(v, "reviewedAt"),
    })
}

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

impl From<&crate::committee::Member> for CommitteeMemberOut {
    fn from(member: &crate::committee::Member) -> Self {
        Self {
            address: member.address.clone(),
            name: short_address(&member.address),
        }
    }
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
            .filter_map(parse_collection)
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

fn to_time(secs: Option<i64>) -> Option<DateTime<Utc>> {
    secs.filter(|s| *s > 0)
        .and_then(|s| DateTime::from_timestamp(s, 0))
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
            created_at: to_time(r.cur_timestamp),
            id: cid.clone(),
            status: if r.cur_is_approved.unwrap_or(false) {
                CurationStatus::Approved
            } else {
                CurationStatus::Rejected
            },
            updated_at: to_time(r.cur_timestamp),
        }),
        _ => None,
    };

    ReviewRowOut {
        created_at: to_time(r.created_at),
        curation,
        has_reviews: r.cur_id.is_some(),
        id: r.id,
        is_approved,
        is_programmatic: false,
        item_count: r.items_count.unwrap_or(0),
        name: r.name.unwrap_or_default(),
        owner: r.creator,
        reviewed_at: to_time(r.reviewed_at),
        status: status_of(is_completed, is_approved),
        kind: CollectionType::Standard,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn iso_of(secs: Option<i64>) -> Value {
        match secs {
            Some(1_700_000_000) => json!("2023-11-14T22:13:20.000Z"),
            Some(1_700_000_100) => json!("2023-11-14T22:15:00.000Z"),
            Some(1_700_000_200) => json!("2023-11-14T22:16:40.000Z"),
            None => Value::Null,
            Some(other) => panic!("no fixture date for {other}"),
        }
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
            "created_at": iso_of(r.created_at),
            "reviewed_at": iso_of(r.reviewed_at),
            "curation": {
                "id": "cur-1",
                "collection_id": r.id,
                "assignee": "0xcurator",
                "status": "rejected",
                "created_at": iso_of(r.cur_timestamp),
                "updated_at": iso_of(r.cur_timestamp),
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
        let from_reader = CommitteeMemberOut::from(&crate::committee::Member {
            id: "0xabcdef0123456789abcdef0123456789abcdef01".into(),
            address: "0xabcdef0123456789abcdef0123456789abcdef01".into(),
        });
        assert_eq!(serde_json::to_value(&from_reader).unwrap(), old);
    }

    #[test]
    fn subgraph_rows_parse_into_the_squid_shapes() {
        let c = parse_collection(&json!({ "id": "0xc0ffee", "owner": "0xo", "creator": "0xc", "name": "Hats", "urn": "urn:x", "itemsCount": 3,
                     "isCompleted": true, "isApproved": false, "createdAt": "1700000000", "updatedAt": 1700000100, "reviewedAt": "0" }))
        .unwrap();
        assert_eq!(
            (c.items_count, c.is_completed, c.created_at, c.reviewed_at),
            (Some(3), Some(true), Some(1_700_000_000), Some(0))
        );
        assert!(parse_collection(&json!({ "name": "no id" })).is_none());
    }

    #[tokio::test]
    async fn without_a_collections_reader_the_address_reads_are_unavailable() {
        let market = crate::tests::support::market_without_db(
            crate::chain_index::IndexSettings::for_tests(),
        );
        assert!(!configured(Some(&market)));
        assert!(!configured(None));
        assert!(collections_under_review(&market).await.unwrap().is_empty());
    }
}
