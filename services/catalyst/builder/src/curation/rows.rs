//! The curation rows as builder-server answers them: a collection's or an
//! item's curation row with every column, and the shorter body of a row a
//! review request just created. Lists answer at most [`MAX_ROWS`] rows.

use chrono::NaiveDateTime;
use serde::{Serialize, Serializer};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::http::{ApiError, iso_timestamp};

/// The most rows a curation list answers: above the committee's listing
/// (3,502 collections) and the largest collection's item curations (12,665
/// items) in the reference data.
pub const MAX_ROWS: i64 = 20_000;

fn naive_iso<S: Serializer>(t: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
    iso_timestamp(&t.and_utc(), s)
}

/// A `builder.collection_curations` row.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CollectionCurationOut {
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub id: Uuid,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub collection_id: Uuid,
    pub status: String,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub updated_at: NaiveDateTime,
    pub assignee: Option<String>,
    pub reviewed_by: Option<String>,
    pub rejection_reasons: Option<Vec<String>>,
    pub rejection_message: Option<String>,
}

/// A `builder.item_curations` row.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ItemCurationOut {
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub id: Uuid,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub item_id: Uuid,
    pub status: String,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub updated_at: NaiveDateTime,
    pub content_hash: String,
    pub is_mapping_complete: Option<bool>,
}

/// `POST /v1/collections/:id/curation`'s body: the request it opened.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct CollectionCurationOpened {
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub id: Uuid,
    pub status: String,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub updated_at: NaiveDateTime,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub collection_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub assignee: Option<String>,
}

/// `POST /v1/items/:id/curation`'s body: the submission it opened.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "builder/"))]
pub struct ItemCurationOpened {
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub id: Uuid,
    pub status: String,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub created_at: NaiveDateTime,
    #[serde(serialize_with = "naive_iso")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub updated_at: NaiveDateTime,
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub item_id: Uuid,
    pub content_hash: String,
    pub is_mapping_complete: bool,
}

const COLLECTION_COLUMNS: &str = "k.id, k.collection_id, k.status, k.created_at, k.updated_at, \
     k.assignee, k.reviewed_by, k.rejection_reasons, k.rejection_message";
const ITEM_COLUMNS: &str =
    "k.id, k.item_id, k.status, k.created_at, k.updated_at, k.content_hash, k.is_mapping_complete";

/// The newest curation of the collection `id`.
pub async fn newest_of_collection(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<CollectionCurationOut>, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLLECTION_COLUMNS} FROM builder.collection_curations k \
         WHERE k.collection_id = $1 ORDER BY k.created_at DESC LIMIT 1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

/// The collection curation `id`.
pub async fn collection_curation(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<CollectionCurationOut, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLLECTION_COLUMNS} FROM builder.collection_curations k WHERE k.id = $1"
    )))
    .bind(id)
    .fetch_one(conn)
    .await?)
}

/// The newest curation of the item `id`.
pub async fn newest_of_item(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<ItemCurationOut>, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ITEM_COLUMNS} FROM builder.item_curations k \
         WHERE k.item_id = $1 ORDER BY k.created_at DESC LIMIT 1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

/// The newest curation of every collection, by collection id.
pub async fn newest_of_every_collection(
    conn: &mut PgConnection,
) -> Result<Vec<CollectionCurationOut>, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT ON (k.collection_id) {COLLECTION_COLUMNS} FROM builder.collection_curations k \
         ORDER BY k.collection_id, k.created_at DESC LIMIT $1"
    )))
    .bind(MAX_ROWS)
    .fetch_all(conn)
    .await?)
}

/// The newest curation of each collection `address` creates, manages or
/// mints on the chain copy, and of each collection of the providers
/// `providers`, by collection id.
pub async fn newest_of_wallet(
    conn: &mut PgConnection,
    address: &str,
    providers: &[String],
) -> Result<Vec<CollectionCurationOut>, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT ON (k.collection_id) {COLLECTION_COLUMNS} FROM builder.collection_curations k \
         WHERE k.collection_id IN ( \
           SELECT b.id FROM shop.collections s JOIN builder.collections b ON b.contract_address = s.contract_address \
           WHERE s.network = 'matic' AND (lower(s.creator) = $1 OR EXISTS (SELECT 1 FROM unnest(s.managers || s.minters) r \
             WHERE lower(r) = $1)) \
           UNION SELECT t.id FROM builder.collections t WHERE t.third_party_id = ANY($2)) \
         ORDER BY k.collection_id, k.created_at DESC LIMIT $3"
    )))
    .bind(address)
    .bind(providers)
    .bind(MAX_ROWS)
    .fetch_all(conn)
    .await?)
}

/// The newest curation of each item of the collection `id`, by item id;
/// only of `items` when given.
pub async fn newest_of_collection_items(
    conn: &mut PgConnection,
    id: Uuid,
    items: Option<&[Uuid]>,
) -> Result<Vec<ItemCurationOut>, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT ON (i.id) {ITEM_COLUMNS} FROM builder.items i \
         JOIN builder.item_curations k ON k.item_id = i.id \
         WHERE i.collection_id = $1 AND ($2::uuid[] IS NULL OR i.id = ANY($2)) \
         ORDER BY i.id, k.created_at DESC LIMIT $3"
    )))
    .bind(id)
    .bind(items)
    .bind(MAX_ROWS)
    .fetch_all(conn)
    .await?)
}
