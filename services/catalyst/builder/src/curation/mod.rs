//! The curation rows: a collection's reviews (`builder.collection_curations`),
//! an item's submissions for review (`builder.item_curations`), a
//! collection's review events and a third-party publication's slot cheques.
//! A row's state is the row itself: an item's curation status is its newest
//! item curation's, read where it is needed, never copied onto the item.
//!
//! Every insert of a curation goes through [`record_item_curation`] or
//! [`record_collection_curation`]: each locks the collection until the
//! caller's transaction ends and stamps the row after the newest one, so the
//! newest row by `created_at` is always the last written and no two rows of
//! one item or collection share an instant.

use sqlx::PgConnection;
use uuid::Uuid;

use crate::access::CurationGrant;
use crate::http::ApiError;

/// A boolean over an item aliased `i`: the item was submitted for curation.
/// A third-party item is published once this holds.
pub const ITEM_CURATED: &str =
    "EXISTS (SELECT 1 FROM builder.item_curations curated WHERE curated.item_id = i.id)";

/// A boolean over a collection aliased `c`: an item of the collection was
/// submitted for curation.
pub const COLLECTION_ITEMS_CURATED: &str = "EXISTS (SELECT 1 FROM builder.items curated_item \
     JOIN builder.item_curations curated ON curated.item_id = curated_item.id \
     WHERE curated_item.collection_id = c.id)";

/// `column`, an expression over `k`, of the newest curation of the item
/// `item`, NULL when the item has none. The newest curation is the last
/// created one; every read of an item's curation state goes through this.
pub fn newest_item_curation(item: &str, column: &str) -> String {
    format!(
        "(SELECT {column} FROM builder.item_curations k WHERE k.item_id = {item} \
         ORDER BY k.created_at DESC LIMIT 1)"
    )
}

/// `column`, an expression over `k`, of the newest curation of the first
/// curated item, by item id, of the collection `collection`: the curation
/// builder-server reads for a third-party collection and for an item of it
/// read by itself.
pub fn first_curated_item_curation(collection: &str, column: &str) -> String {
    format!(
        "(SELECT {} FROM builder.items ki WHERE ki.collection_id = {collection} \
         AND EXISTS (SELECT 1 FROM builder.item_curations kc WHERE kc.item_id = ki.id) \
         ORDER BY ki.id LIMIT 1)",
        newest_item_curation("ki.id", column)
    )
}

/// Whether `hash` is an item curation's content hash: the keccak-256 digest
/// of the item, 64 lowercase hex digits.
pub fn content_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `created_at` for a new row of `table` whose `key` is `$2`: now, or just
/// after the newest row of that key when that one is later, so the newest
/// row is always the last written and two rows never share an instant.
fn stamp_after(table: &str, key: &str) -> String {
    format!(
        "GREATEST(timezone('UTC', clock_timestamp()), \
         (SELECT max(created_at) + interval '1 microsecond' FROM builder.{table} WHERE {key} = $2))"
    )
}

/// Locks the collection row `$1` until the caller's transaction ends.
const LOCK_COLLECTION: &str = "SELECT 1 FROM builder.collections WHERE id = $1 FOR UPDATE";

/// Locks the collection row of the item `$1` until the caller's transaction
/// ends.
const LOCK_ITEM_COLLECTION: &str = "SELECT 1 FROM builder.collections c \
     JOIN builder.items i ON i.collection_id = c.id WHERE i.id = $1 FOR UPDATE OF c";

/// Submits `item` for curation as its newest curation, inside the caller's
/// transaction, with the item's collection locked until it ends; the
/// curation's id.
pub async fn record_item_curation(
    conn: &mut PgConnection,
    item: Uuid,
    status: &str,
    content_hash: &str,
    is_mapping_complete: bool,
) -> Result<Uuid, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query(LOCK_ITEM_COLLECTION)
        .bind(item)
        .execute(&mut *conn)
        .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at) \
         SELECT $1, $2, $3, $4, $5, at, at FROM (SELECT {} AS at) stamp",
        stamp_after("item_curations", "item_id")
    )))
    .bind(id)
    .bind(item)
    .bind(status)
    .bind(content_hash)
    .bind(is_mapping_complete)
    .execute(conn)
    .await?;
    Ok(id)
}

/// Records a new curation of the collection `grant` names as its newest,
/// inside the caller's transaction, with the collection locked until it
/// ends; the curation's id.
pub async fn record_collection_curation(
    conn: &mut PgConnection,
    grant: &impl CurationGrant,
    status: &str,
    assignee: Option<&str>,
) -> Result<Uuid, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query(LOCK_COLLECTION)
        .bind(grant.curated())
        .execute(&mut *conn)
        .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO builder.collection_curations (id, collection_id, status, assignee, created_at, updated_at) \
         SELECT $3, $2, $4, $5, at, at FROM (SELECT {} AS at) stamp",
        stamp_after("collection_curations", "collection_id")
    )))
    .bind(grant.curated())
    .bind(grant.curated())
    .bind(id)
    .bind(status)
    .bind(assignee)
    .execute(&mut *conn)
    .await?;
    Ok(id)
}
