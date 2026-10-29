//! Cursor and pass markers for the upstream mirrors (`explore.mirror_state`).
//!
//! A mirror pass pages through an upstream catalogue. Each page persists the
//! next offset, so a process killed mid-pass resumes at that page with the
//! same `pass_started_at` (the sweep cutoff and the unlisted check derive from
//! it). Finishing a pass clears the cursor and, only when every page fetched
//! and every row upserted, advances `last_complete_pass_at`: that timestamp
//! and its lag are what `/api/status` reports as the catalogue's freshness.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

/// Where a pass starts: fresh at offset 0, or resumed from a persisted cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassStart {
    pub started_at: DateTime<Utc>,
    pub offset: i64,
    pub rows: i64,
    pub resumed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PassEnd {
    pub complete: bool,
    pub rows: i64,
    pub swept: i64,
    pub error: Option<String>,
}

/// One mirror's state as `/api/status` reports it.
#[derive(Debug, Clone, Serialize)]
pub struct MirrorStatus {
    pub mirror: String,
    pub source_url: String,
    /// A pass is running (or was interrupted) and will resume from this offset.
    pub in_flight_offset: Option<i64>,
    pub pass_started_at: Option<DateTime<Utc>>,
    pub last_pass_at: Option<DateTime<Utc>>,
    pub last_pass_complete: Option<bool>,
    pub last_pass_rows: Option<i64>,
    pub last_pass_swept: Option<i64>,
    pub last_complete_pass_at: Option<DateTime<Utc>>,
    pub last_complete_pass_rows: Option<i64>,
    /// Seconds since the last complete pass; `None` when there was none yet.
    pub lag_secs: Option<i64>,
    pub last_error: Option<String>,
}

fn utc(naive: Option<NaiveDateTime>) -> Option<DateTime<Utc>> {
    naive.map(|n| n.and_utc())
}

/// Reads the persisted cursor; resumes when one is in flight for the same
/// source, otherwise records a fresh pass at offset 0.
pub async fn begin_pass(pool: &PgPool, mirror: &str, source_url: &str) -> sqlx::Result<PassStart> {
    let row: Option<(Option<NaiveDateTime>, Option<i64>, i64, String)> = sqlx::query_as(
        "SELECT pass_started_at, next_offset, pass_rows, source_url \
         FROM explore.mirror_state WHERE mirror = $1",
    )
    .bind(mirror)
    .fetch_optional(pool)
    .await?;
    if let Some((Some(started_at), Some(offset), rows, source)) = row
        && source == source_url
    {
        return Ok(PassStart {
            started_at: started_at.and_utc(),
            offset,
            rows,
            resumed: true,
        });
    }
    let started_at = Utc::now();
    sqlx::query(
        "INSERT INTO explore.mirror_state (mirror, source_url, pass_started_at, next_offset, pass_rows, updated_at) \
         VALUES ($1, $2, $3, 0, 0, timezone('UTC', now())) \
         ON CONFLICT (mirror) DO UPDATE SET \
           source_url = EXCLUDED.source_url, pass_started_at = EXCLUDED.pass_started_at, \
           next_offset = 0, pass_rows = 0, updated_at = timezone('UTC', now())",
    )
    .bind(mirror)
    .bind(source_url)
    .bind(started_at.naive_utc())
    .execute(pool)
    .await?;
    Ok(PassStart {
        started_at,
        offset: 0,
        rows: 0,
        resumed: false,
    })
}

/// Persists the cursor after a page landed.
pub async fn advance(pool: &PgPool, mirror: &str, next_offset: i64, rows: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE explore.mirror_state SET next_offset = $2, pass_rows = $3, updated_at = timezone('UTC', now()) \
         WHERE mirror = $1",
    )
    .bind(mirror)
    .bind(next_offset)
    .bind(rows)
    .execute(pool)
    .await?;
    Ok(())
}

/// Clears the cursor and records the outcome; a complete pass moves
/// `last_complete_pass_at`.
pub async fn finish_pass(pool: &PgPool, mirror: &str, end: &PassEnd) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE explore.mirror_state SET \
           next_offset = NULL, pass_rows = 0, \
           last_pass_at = timezone('UTC', now()), last_pass_complete = $2, \
           last_pass_rows = $3, last_pass_swept = $4, last_error = $5, \
           last_complete_pass_at = CASE WHEN $2 THEN timezone('UTC', now()) ELSE last_complete_pass_at END, \
           last_complete_pass_rows = CASE WHEN $2 THEN $3 ELSE last_complete_pass_rows END, \
           updated_at = timezone('UTC', now()) \
         WHERE mirror = $1",
    )
    .bind(mirror)
    .bind(end.complete)
    .bind(end.rows)
    .bind(end.swept)
    .bind(&end.error)
    .execute(pool)
    .await?;
    Ok(())
}

/// When `mirror` last completed a pass; `None` before the first.
pub async fn last_complete_pass(
    pool: &PgPool,
    mirror: &str,
) -> sqlx::Result<Option<NaiveDateTime>> {
    Ok(sqlx::query_scalar(
        "SELECT last_complete_pass_at FROM explore.mirror_state WHERE mirror = $1",
    )
    .bind(mirror)
    .fetch_optional(pool)
    .await?
    .flatten())
}

/// Records a failure without clearing the cursor, so the next run resumes.
pub async fn record_error(pool: &PgPool, mirror: &str, error: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE explore.mirror_state SET last_error = $2, updated_at = timezone('UTC', now()) WHERE mirror = $1",
    )
    .bind(mirror)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

type StatusRow = (
    String,
    String,
    Option<NaiveDateTime>,
    Option<i64>,
    Option<NaiveDateTime>,
    Option<bool>,
    Option<i64>,
    Option<i64>,
    Option<NaiveDateTime>,
    Option<i64>,
    Option<String>,
);

pub async fn status(pool: &PgPool) -> sqlx::Result<Vec<MirrorStatus>> {
    let rows: Vec<StatusRow> = sqlx::query_as(
        "SELECT mirror, source_url, pass_started_at, next_offset, last_pass_at, last_pass_complete, \
                last_pass_rows, last_pass_swept, last_complete_pass_at, last_complete_pass_rows, last_error \
         FROM explore.mirror_state ORDER BY mirror",
    )
    .fetch_all(pool)
    .await?;
    let now = Utc::now();
    Ok(rows
        .into_iter()
        .map(
            |(
                mirror,
                source_url,
                pass_started_at,
                next_offset,
                last_pass_at,
                last_pass_complete,
                last_pass_rows,
                last_pass_swept,
                last_complete_pass_at,
                last_complete_pass_rows,
                last_error,
            )| {
                let last_complete_pass_at = utc(last_complete_pass_at);
                MirrorStatus {
                    mirror,
                    source_url,
                    in_flight_offset: next_offset,
                    pass_started_at: utc(pass_started_at),
                    last_pass_at: utc(last_pass_at),
                    last_pass_complete,
                    last_pass_rows,
                    last_pass_swept,
                    last_complete_pass_at,
                    last_complete_pass_rows,
                    lag_secs: last_complete_pass_at.map(|t| (now - t).num_seconds()),
                    last_error,
                }
            },
        )
        .collect())
}
