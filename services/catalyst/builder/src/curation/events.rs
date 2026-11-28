//! A collection's review timeline (`builder.collection_events`): append-only,
//! newest last by `(created_at, id)`. Every insert stamps its row after the
//! collection's newest one, so the order is the order of the writes whatever
//! clock stamped them; the caller holds the collection's row lock.

use chrono::NaiveDateTime;
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::http::{ApiError, iso_timestamp};

/// Who wrote an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Creator,
    Curator,
    Validator,
    System,
}

impl Actor {
    fn as_str(self) -> &'static str {
        match self {
            Actor::Creator => "creator",
            Actor::Curator => "curator",
            Actor::Validator => "validator",
            Actor::System => "system",
        }
    }
}

/// The event types the validator's verdict writes once per validation and
/// reason (`collection_events_verdict_validation_idx`).
pub const VERDICT_TYPES: &str = "('review.automated_passed', 'review.automated_rejected', 'review.automated_error', 'review.human_required')";

/// The curators' decisions.
pub const DECISION_TYPES: &str = "('review.approved', 'review.rejected')";

fn naive_iso<S: Serializer>(t: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
    iso_timestamp(&t.and_utc(), s)
}

/// A `builder.collection_events` row as builder-server answers it.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EventOut {
    pub id: Uuid,
    pub collection_id: Uuid,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub actor_address: Option<String>,
    pub payload: Value,
    #[serde(serialize_with = "naive_iso")]
    pub created_at: NaiveDateTime,
}

impl EventOut {
    /// The event as a creator reads it: without the id a verdict is matched
    /// by and without the validator's own failures.
    pub fn redacted(mut self) -> Self {
        if let Value::Object(payload) = &mut self.payload {
            payload.remove("validationId");
            if let Some(Value::Array(items)) = payload.get_mut("items") {
                for item in items.iter_mut().filter_map(Value::as_object_mut) {
                    item.remove("error");
                }
            }
        }
        self
    }
}

const COLUMNS: &str = "id, collection_id, type, actor, actor_address, payload, created_at";

fn stamp(collection: &str) -> String {
    format!(
        "GREATEST(COALESCE($6, timezone('UTC', clock_timestamp())), \
         (SELECT max(created_at) + interval '1 microsecond' FROM builder.collection_events WHERE collection_id = {collection}))"
    )
}

/// Appends an event to `collection`'s timeline, stamped at `at` (the
/// database's clock when `None`) or just after its newest event.
pub async fn record(
    conn: &mut PgConnection,
    collection: Uuid,
    kind: &str,
    actor: Actor,
    address: Option<&str>,
    payload: Value,
    at: Option<NaiveDateTime>,
) -> Result<EventOut, ApiError> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "INSERT INTO builder.collection_events ({COLUMNS}) \
         SELECT $7, $1, $2, $3, $4, $5, {} RETURNING {COLUMNS}",
        stamp("$1")
    )))
    .bind(collection)
    .bind(kind)
    .bind(actor.as_str())
    .bind(address)
    .bind(payload)
    .bind(at)
    .bind(Uuid::new_v4())
    .fetch_one(conn)
    .await?)
}

/// [`record`] for a verdict's event, written once per validation and
/// reason; false when it was written before.
pub async fn record_once(
    conn: &mut PgConnection,
    collection: Uuid,
    kind: &str,
    actor: Actor,
    payload: &serde_json::value::RawValue,
    at: NaiveDateTime,
) -> Result<bool, ApiError> {
    Ok(sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO builder.collection_events ({COLUMNS}) \
         SELECT $7, $1, $2, $3, $4, $5, {} \
         ON CONFLICT (collection_id, type, (payload->>'validationId'), (COALESCE(payload->>'reason', ''))) \
           WHERE type IN {VERDICT_TYPES} DO NOTHING",
        stamp("$1")
    )))
    .bind(collection)
    .bind(kind)
    .bind(actor.as_str())
    .bind(None::<String>)
    .bind(sqlx::types::Json(payload))
    .bind(Some(at))
    .bind(Uuid::new_v4())
    .execute(conn)
    .await?
    .rows_affected()
        == 1)
}

/// One page of a collection's timeline, newest first, and its total.
pub async fn page(
    conn: &mut PgConnection,
    collection: Uuid,
    limit: i64,
    offset: i64,
) -> Result<(Vec<EventOut>, i64), ApiError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM builder.collection_events WHERE collection_id = $1",
    )
    .bind(collection)
    .fetch_one(&mut *conn)
    .await?;
    let rows = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM builder.collection_events WHERE collection_id = $1 \
         ORDER BY created_at DESC, id DESC LIMIT $2 OFFSET $3"
    )))
    .bind(collection)
    .bind(limit)
    .bind(offset)
    .fetch_all(conn)
    .await?;
    Ok((rows, total))
}

/// Each item's content hash as the newest of the last [`JUDGED_RUNS`]
/// verdicts that judged it saw it.
pub async fn judged_hashes(
    conn: &mut PgConnection,
    collection: Uuid,
) -> Result<std::collections::HashMap<String, String>, ApiError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT item->>'itemId', item->>'contentHash' FROM ( \
           SELECT e.payload, e.created_at, e.id FROM builder.collection_events e \
           WHERE e.collection_id = $1 AND jsonb_typeof(e.payload->'items') = 'array' \
             AND (e.type IN ('review.automated_passed', 'review.automated_rejected') \
               OR (e.type = 'review.human_required' AND e.payload->>'reason' = 'unsupported_items')) \
           ORDER BY e.created_at DESC, e.id DESC LIMIT $2) runs \
         CROSS JOIN LATERAL jsonb_array_elements(runs.payload->'items') WITH ORDINALITY AS judged(item, n) \
         WHERE jsonb_typeof(item->'itemId') = 'string' AND jsonb_typeof(item->'contentHash') = 'string' \
         ORDER BY runs.created_at DESC, runs.id DESC, n",
    )
    .bind(collection)
    .bind(JUDGED_RUNS)
    .fetch_all(conn)
    .await?;
    let mut judged = std::collections::HashMap::new();
    for (item, hash) in rows {
        judged.entry(item).or_insert(hash);
    }
    Ok(judged)
}

/// How many verdicts back an item's judged hash is looked for.
pub const JUDGED_RUNS: i64 = 50;

/// The newest start of `collection`: its validation id, item ids and
/// stamp.
pub async fn latest_start(
    conn: &mut PgConnection,
    collection: Uuid,
) -> Result<Option<(String, Vec<String>, NaiveDateTime)>, ApiError> {
    let row: Option<(Value, NaiveDateTime)> = sqlx::query_as(
        "SELECT payload, created_at FROM builder.collection_events \
         WHERE collection_id = $1 AND type = 'review.automated_started' ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(collection)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(payload, at)| {
        let field = |key: &str| payload.get(key).cloned().unwrap_or(Value::Null);
        let items = field("itemIds")
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        (
            field("validationId")
                .as_str()
                .unwrap_or_default()
                .to_string(),
            items,
            at,
        )
    }))
}

/// The newest verdict event of the validation `validation`: its type and
/// payload.
pub async fn verdict_of(
    conn: &mut PgConnection,
    collection: Uuid,
    validation: &str,
) -> Result<Option<(String, Map<String, Value>)>, ApiError> {
    let row: Option<(String, Value)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT type, payload FROM builder.collection_events WHERE collection_id = $1 \
         AND type IN {VERDICT_TYPES} AND payload->>'validationId' = $2 ORDER BY created_at DESC, id DESC LIMIT 1"
    )))
    .bind(collection)
    .bind(validation)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(kind, payload)| {
        (
            kind,
            match payload {
                Value::Object(map) => map,
                _ => Map::new(),
            },
        )
    }))
}

/// Whether a curator decided `collection` after the instant `since`.
pub async fn decided_since(
    conn: &mut PgConnection,
    collection: Uuid,
    since: NaiveDateTime,
) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS (SELECT 1 FROM builder.collection_events WHERE collection_id = $1 \
         AND type IN {DECISION_TYPES} AND created_at > $2)"
    )))
    .bind(collection)
    .bind(since)
    .fetch_one(conn)
    .await?)
}

/// When the validation `validation` started on the timeline, else `fallback`.
pub async fn started_at(
    conn: &mut PgConnection,
    collection: Uuid,
    validation: Uuid,
    fallback: NaiveDateTime,
) -> Result<NaiveDateTime, ApiError> {
    let at: Option<NaiveDateTime> = sqlx::query_scalar(
        "SELECT max(created_at) FROM builder.collection_events WHERE collection_id = $1 \
         AND type = 'review.automated_started' AND payload->>'validationId' = $2",
    )
    .bind(collection)
    .bind(validation.to_string())
    .fetch_one(conn)
    .await?;
    Ok(at.unwrap_or(fallback))
}

/// How many events of `kind` were written in `[from, to)`: on `collection`,
/// and by `address` on any collection.
pub async fn written(
    conn: &mut PgConnection,
    kind: &str,
    collection: Uuid,
    address: &str,
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> Result<(i64, i64), ApiError> {
    Ok(sqlx::query_as(
        "SELECT \
           (SELECT count(*) FROM builder.collection_events WHERE collection_id = $2 AND type = $1 \
              AND created_at >= $4 AND created_at < $5), \
           (SELECT count(*) FROM builder.collection_events WHERE actor_address = $3 AND type = $1 \
              AND created_at >= $4 AND created_at < $5 \
              AND type IN ('review.comment', 'review.appeal_requested'))",
    )
    .bind(kind)
    .bind(collection)
    .bind(address)
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await?)
}
