//! The events catalogue mirror: pages through the upstream public catalogue
//! (`/api/events?list=all`, which serves approved, non-rejected, non-deleted
//! events, past and future) into `explore.event`, with the cursor and pass
//! markers of [`crate::mirror_state`] so a restart resumes and a
//! complete pass is provable.
//!
//! The mirror writes and deletes `origin = 'upstream'` rows only: an event
//! created on this node (`origin = 'local'`) is never overwritten or swept,
//! and a mirrored event's own fields are never edited here (the write routes
//! answer 409), so every pass writes the upstream's state. An operator's
//! moderation of it (`explore.event_moderation`) is local policy the upsert
//! re-applies to every row it writes. Its RSVPs and its moderation on this
//! node are local rows keyed by its id, deleted with it. Rows the pass did
//! not list are asked for one by one before the sweep: the public list drops
//! an organiser-deleted, rejected or unapproved event at once while the
//! detail endpoint keeps serving it with its flags, so refreshing from the
//! detail hides it from our lists within the pass; a detail 404 hides it the
//! same way, and only the grace sweep (two intervals, behind its fuse)
//! deletes a row.
//!
//! An event a binary from before `origin` created (it may still run against
//! a migrated database) is reclassified 'local' ([`reclaim_local`]) before
//! anything here could overwrite, hide or sweep it.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::MirrorSource;
use crate::http::read_body_capped;
use crate::mirror_state::{self, PassEnd};

pub const EVENTS_MIRROR: &str = "events";

/// Upstream clamps `limit` to 500 (events getEventList.ts).
const PAGE: i64 = 500;
/// A pass ends after this many consecutive failed pages so an unreachable
/// upstream cannot spin the offset forever.
const MAX_CONSECUTIVE_PAGE_FAILURES: u32 = 3;
/// Hard offset ceiling; the live catalogue is ~7.5k events, so hitting this
/// means the upstream is misbehaving, not that the catalogue grew 30x.
const MAX_OFFSET: i64 = 250_000;
const MAX_PAGE_BYTES: usize = 32 * 1024 * 1024;
const CONFIRM_LIMIT: i64 = 100;

/// The sweep refuses to delete anything when its candidate set exceeds
/// max(SWEEP_FUSE_MIN_CANDIDATES, 20% of the mirror-owned rows): a
/// well-formed but truncated upstream response still reports complete, and
/// must not mass-delete the catalogue.
const SWEEP_FUSE_MIN_CANDIDATES: i64 = 50;

/// The conflict action skips this node's own events (an id collision), the
/// sweep's exclusion rule; an operator's moderation of a mirrored event
/// (`event_moderation`) wins over the upstream's flags on every row written,
/// and a decided flag brings its actor and reason (NULL ones included: an
/// operator's `approved: false` clears the upstream's `approved_by`).
/// Upstream timestamps are RFC 3339 with a zone; they land as UTC
/// `timestamp without time zone`.
const UPSERT: &str = r#"
    INSERT INTO explore.event
        (id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms,
         recurrent, highlighted, trending, approved, attending, community_id,
         user_creator, coordinates_x, coordinates_y, description, raw, fetched_at, origin)
    SELECT u.id, u.name,
           u.start_at::timestamptz AT TIME ZONE 'UTC', u.finish_at::timestamptz AT TIME ZONE 'UTC',
           u.next_start_at::timestamptz AT TIME ZONE 'UTC', u.next_finish_at::timestamptz AT TIME ZONE 'UTC',
           u.duration_ms, u.recurrent, COALESCE(m.highlighted, u.highlighted),
           COALESCE(m.trending, u.trending), COALESCE(m.approved, u.approved), u.attending,
           u.community_id, u.user_creator, u.coordinates_x, u.coordinates_y, u.description,
           u.raw || jsonb_strip_nulls(jsonb_build_object(
               'approved', m.approved, 'rejected', m.rejected,
               'highlighted', m.highlighted, 'trending', m.trending,
               'deleted_by_admin', CASE WHEN m.hidden THEN true END,
               'deleted_at', CASE WHEN m.hidden
                   THEN to_char(m.set_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') END))
             || CASE WHEN m.approved IS NOT NULL
                    THEN jsonb_build_object('approved_by', m.approved_by) ELSE '{}' END
             || CASE WHEN m.rejected IS NOT NULL
                    THEN jsonb_build_object('rejected_by', m.rejected_by,
                                            'rejection_reason', m.rejection_reason) ELSE '{}' END
             || CASE WHEN m.hidden
                    THEN jsonb_build_object('deleted_by', m.deleted_by,
                                            'deleted_reason', m.deleted_reason) ELSE '{}' END,
           timezone('UTC', now()), 'upstream'
    FROM unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::int8[], $8::boolean[], $9::boolean[], $10::boolean[], $11::boolean[],
                $12::boolean[], $13::text[], $14::text[], $15::int4[], $16::int4[],
                $17::text[], $18::jsonb[])
         AS u(id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms,
              recurrent, highlighted, trending, approved, attending, community_id,
              user_creator, coordinates_x, coordinates_y, description, raw)
    LEFT JOIN explore.event_moderation m ON m.event_id = u.id
    ON CONFLICT (id) DO UPDATE SET
        name           = EXCLUDED.name,
        start_at       = EXCLUDED.start_at,
        finish_at      = EXCLUDED.finish_at,
        next_start_at  = EXCLUDED.next_start_at,
        next_finish_at = EXCLUDED.next_finish_at,
        duration_ms    = EXCLUDED.duration_ms,
        recurrent      = EXCLUDED.recurrent,
        highlighted    = EXCLUDED.highlighted,
        trending       = EXCLUDED.trending,
        approved       = EXCLUDED.approved,
        attending      = EXCLUDED.attending,
        community_id   = EXCLUDED.community_id,
        user_creator   = EXCLUDED.user_creator,
        coordinates_x  = EXCLUDED.coordinates_x,
        coordinates_y  = EXCLUDED.coordinates_y,
        description    = EXCLUDED.description,
        raw            = EXCLUDED.raw,
        fetched_at     = timezone('UTC', now())
    WHERE event.origin = 'upstream'
"#;

/// An event created by a binary from before `origin` (the default makes it
/// 'upstream'): its `events_local` row was written in the creating
/// transaction (the `create` payload, or a later write that kept that
/// transaction's `signed_at`). schemas/migrations/0007 runs the same rule once.
const RECLAIM_LOCAL: &str = r#"
    UPDATE explore.event e SET origin = 'local'
    FROM explore.events_local l
    WHERE l.id = e.id AND e.origin = 'upstream'
      AND (l.signed_payload ? 'create' OR l.signed_at = e.fetched_at)
"#;

/// Reclassifies what [`RECLAIM_LOCAL`] names (the module doc); idempotent,
/// run before every upsert and sweep and before a write route reads a row's
/// origin.
pub async fn reclaim_local(pool: &PgPool) -> sqlx::Result<()> {
    let n = sqlx::query(RECLAIM_LOCAL)
        .execute(pool)
        .await?
        .rows_affected();
    if n > 0 {
        tracing::info!(
            rows = n,
            "event mirror: reclassified local events an older binary created"
        );
    }
    Ok(())
}

/// The local RSVPs and moderation go with the event in the same statement,
/// and so do the ones an earlier sweep left behind on an event already gone;
/// the count is the events deleted.
const SWEEP: &str = r#"
    WITH gone AS (
        DELETE FROM explore.event e
        WHERE e.fetched_at < $1 AND e.origin = 'upstream'
        RETURNING e.id
    ), rsvps AS (
        DELETE FROM explore.event_attendance_local a
        WHERE a.event_id IN (SELECT id FROM gone)
           OR NOT EXISTS (SELECT 1 FROM explore.event e WHERE e.id = a.event_id)
    ), moderation AS (
        DELETE FROM explore.event_moderation m
        WHERE m.event_id IN (SELECT id FROM gone)
           OR NOT EXISTS (SELECT 1 FROM explore.event e WHERE e.id = m.event_id)
    )
    SELECT count(*) FROM gone
"#;

const SWEEP_CANDIDATES: &str = r#"
    SELECT count(*) FROM explore.event e
    WHERE e.fetched_at < $1 AND e.origin = 'upstream'
"#;

const MIRRORED_COUNT: &str = r#"
    SELECT count(*) FROM explore.event e WHERE e.origin = 'upstream'
"#;

/// Rows a detail 404 already hid ([`MARK_GONE`] keeps their old
/// `fetched_at`) sort last, so they cannot take every confirm slot from a
/// newly unlisted row; they are asked again only when slots are left.
const UNLISTED_CANDIDATES: &str = r#"
    SELECT e.id FROM explore.event e
    WHERE e.fetched_at < $1 AND e.origin = 'upstream'
    ORDER BY COALESCE(e.raw->>'deleted_by_user' = 'true', false), e.fetched_at
    LIMIT $2
"#;

/// A row whose upstream detail answers 404: hidden as the upstream detail
/// shows a deleted event (`deleted_by_user`, `deleted_at`), keeping the
/// first `deleted_at`, until the grace sweep deletes it with its RSVPs or a
/// later listing writes it back.
const MARK_GONE: &str = r#"
    UPDATE explore.event e
    SET raw = e.raw || jsonb_build_object(
        'deleted_by_user', true,
        'deleted_at', COALESCE(NULLIF(e.raw->'deleted_at', 'null'::jsonb), to_jsonb($2::text)))
    WHERE e.id = $1 AND e.origin = 'upstream'
"#;

#[derive(Debug, Default)]
pub struct PassOutcome {
    pub upserted: usize,
    pub skipped_rows: usize,
    pub failed_rows: usize,
    pub failed_pages: usize,
    pub swept: u64,
    /// Unlisted rows the upstream detail still served and that were refreshed
    /// from it (a deleted or rejected event picks up its flags here).
    pub unlisted_refreshed: usize,
    /// Unlisted rows the upstream detail answered 404 for, hidden at once and
    /// left to the grace sweep.
    pub unlisted_gone: u64,
    /// Unlisted rows the detail could not settle; the grace sweep decides.
    pub unlisted_unconfirmed: usize,
    /// The offset this pass resumed from, when a persisted cursor was found.
    pub resumed_from: Option<i64>,
    /// Shutdown interrupted the pass; the cursor stays for the next run.
    pub cancelled: bool,
    /// True only when every page fetched and every row upserted cleanly; the
    /// delete sweep is gated on it so a flaky cycle can never wipe rows it
    /// merely failed to see.
    pub complete: bool,
}

/// One full mirror pass. Fail-open per page and per row: an error logs,
/// counts, and the pass moves on.
pub async fn run_pass(
    client: &crate::http::Outbound,
    pool: &PgPool,
    source: &MirrorSource,
    interval: Duration,
    cancel: &CancellationToken,
) -> Result<PassOutcome> {
    let base = source.base();
    let start = mirror_state::begin_pass(pool, EVENTS_MIRROR, base).await?;
    let mut out = PassOutcome {
        complete: true,
        resumed_from: start.resumed.then_some(start.offset),
        upserted: start.rows as usize,
        ..Default::default()
    };
    let mut offset = start.offset;
    let mut consecutive_failures = 0u32;

    loop {
        if cancel.is_cancelled() {
            out.cancelled = true;
            out.complete = false;
            return Ok(out);
        }
        if offset >= MAX_OFFSET {
            tracing::warn!(offset, "event mirror: offset ceiling reached; ending pass");
            out.complete = false;
            break;
        }
        let url = format!("{base}/api/events?list=all&limit={PAGE}&offset={offset}");
        let fetched = tokio::select! {
            _ = cancel.cancelled() => {
                out.cancelled = true;
                out.complete = false;
                return Ok(out);
            }
            fetched = fetch_page(client, &url) => fetched,
        };
        match fetched {
            Ok(events) => {
                consecutive_failures = 0;
                if events.is_empty() {
                    break;
                }
                let count = events.len() as i64;
                upsert_page(pool, &events, &mut out).await;
                offset += PAGE;
                mirror_state::advance(pool, EVENTS_MIRROR, offset, out.upserted as i64).await?;
                if count < PAGE {
                    break;
                }
            }
            Err(e) => {
                out.failed_pages += 1;
                out.complete = false;
                consecutive_failures += 1;
                tracing::warn!(error = %e, url = %url, "event mirror: page fetch failed; skipped");
                let _ = mirror_state::record_error(pool, EVENTS_MIRROR, &format!("{e:#}")).await;
                if consecutive_failures >= MAX_CONSECUTIVE_PAGE_FAILURES {
                    tracing::warn!(
                        consecutive_failures,
                        "event mirror: too many consecutive page failures; ending pass"
                    );
                    break;
                }
                offset += PAGE;
                mirror_state::advance(pool, EVENTS_MIRROR, offset, out.upserted as i64).await?;
            }
        }
    }

    if out.complete && out.upserted > 0 {
        confirm_unlisted(pool, client, base, start.started_at, cancel, &mut out).await;
        let grace = interval
            .checked_mul(2)
            .and_then(|d| chrono::Duration::from_std(d).ok())
            .unwrap_or_else(|| chrono::Duration::hours(2));
        let cutoff = start.started_at - grace;
        match sweep_removed(pool, cutoff).await {
            Ok(SweepOutcome::Swept(n)) => out.swept = n,
            Ok(SweepOutcome::Refused { .. }) => {}
            Err(e) => tracing::warn!(error = %e, "event mirror: sweep failed"),
        }
    }
    mirror_state::finish_pass(
        pool,
        EVENTS_MIRROR,
        &PassEnd {
            complete: out.complete,
            rows: out.upserted as i64,
            swept: out.swept as i64,
            error: (!out.complete).then(|| {
                format!(
                    "{} failed pages, {} failed rows",
                    out.failed_pages, out.failed_rows
                )
            }),
        },
    )
    .await?;
    Ok(out)
}

#[derive(Debug, PartialEq)]
pub enum DetailOutcome {
    Present(Value),
    Gone,
    Unknown,
}

/// What one upstream `GET /api/events/{id}` answer settles: a 200 envelope with
/// the event is authoritative, a 404 is authoritative, everything else is not.
pub fn detail_outcome(status: u16, body: Option<&Value>) -> DetailOutcome {
    match (status, body) {
        (404, _) => DetailOutcome::Gone,
        (200, Some(body)) if body.get("ok").and_then(Value::as_bool) == Some(true) => {
            match body.get("data") {
                Some(data) if data.is_object() => DetailOutcome::Present(data.clone()),
                _ => DetailOutcome::Unknown,
            }
        }
        _ => DetailOutcome::Unknown,
    }
}

async fn confirm_unlisted(
    pool: &PgPool,
    client: &crate::http::Outbound,
    base: &str,
    pass_start: DateTime<Utc>,
    cancel: &CancellationToken,
    out: &mut PassOutcome,
) {
    if let Err(e) = reclaim_local(pool).await {
        tracing::warn!(error = %e, "event mirror: reclassifying local events failed; unlisted rows left to the next pass");
        return;
    }
    let ids: Vec<String> = match sqlx::query_scalar(UNLISTED_CANDIDATES)
        .bind(pass_start.naive_utc())
        .bind(CONFIRM_LIMIT)
        .fetch_all(pool)
        .await
    {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(error = %e, "event mirror: unlisted candidates query failed");
            return;
        }
    };
    for id in ids {
        if cancel.is_cancelled() {
            out.unlisted_unconfirmed += 1;
            continue;
        }
        let url = format!("{base}/api/events/{id}");
        let outcome = match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = read_body_capped(resp, MAX_PAGE_BYTES)
                    .await
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
                detail_outcome(status, body.as_ref())
            }
            Err(e) => {
                tracing::warn!(error = %e, url = %url, "event mirror: detail fetch failed");
                DetailOutcome::Unknown
            }
        };
        match outcome {
            DetailOutcome::Present(event) => match upsert_event(pool, &event).await {
                Ok(true) => out.unlisted_refreshed += 1,
                Ok(false) => out.unlisted_unconfirmed += 1,
                Err(e) => {
                    tracing::warn!(error = %e, id = %id, "event mirror: detail upsert failed");
                    out.unlisted_unconfirmed += 1;
                }
            },
            DetailOutcome::Gone => match sqlx::query(MARK_GONE)
                .bind(&id)
                .bind(Utc::now().to_rfc3339())
                .execute(pool)
                .await
            {
                Ok(done) => out.unlisted_gone += done.rows_affected(),
                Err(e) => {
                    tracing::warn!(error = %e, id = %id, "event mirror: hiding a gone row failed");
                    out.unlisted_unconfirmed += 1;
                }
            },
            DetailOutcome::Unknown => out.unlisted_unconfirmed += 1,
        }
    }
}

async fn fetch_page(client: &crate::http::Outbound, url: &str) -> Result<Vec<Value>> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url}"))?;
    let body = read_body_capped(resp, MAX_PAGE_BYTES)
        .await
        .with_context(|| format!("reading {url}"))?;
    let body: Value = serde_json::from_slice(&body).with_context(|| format!("decoding {url}"))?;
    parse_page(&body)
}

pub fn parse_page(body: &Value) -> Result<Vec<Value>> {
    if body.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(anyhow!("events upstream returned ok!=true"));
    }
    body.get("data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("events upstream response has no data array"))
}

/// The indexed columns the serving queries filter and order on; everything
/// else the read surface needs stays in `raw`. Every field except `id` is
/// tolerant of being absent or null upstream.
#[derive(Debug, PartialEq)]
pub struct EventFields<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub start_at: Option<&'a str>,
    pub finish_at: Option<&'a str>,
    pub next_start_at: Option<&'a str>,
    pub next_finish_at: Option<&'a str>,
    pub duration_ms: Option<i64>,
    pub recurrent: bool,
    pub highlighted: bool,
    pub trending: bool,
    pub approved: bool,
    pub attending: Option<bool>,
    pub community_id: Option<&'a str>,
    pub user: Option<&'a str>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub description: &'a str,
}

fn coord(event: &Value, idx: usize) -> Option<i32> {
    event
        .get("coordinates")
        .and_then(Value::as_array)
        .and_then(|a| a.get(idx))
        .and_then(Value::as_i64)
        .map(|v| v as i32)
}

fn flag(event: &Value, key: &str) -> bool {
    event.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub fn extract_fields(event: &Value) -> Option<EventFields<'_>> {
    let id = match event.get("id").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s,
        _ => return None,
    };
    Some(EventFields {
        id,
        name: event.get("name").and_then(Value::as_str).unwrap_or(""),
        start_at: event.get("start_at").and_then(Value::as_str),
        finish_at: event.get("finish_at").and_then(Value::as_str),
        next_start_at: event.get("next_start_at").and_then(Value::as_str),
        next_finish_at: event.get("next_finish_at").and_then(Value::as_str),
        duration_ms: event.get("duration").and_then(Value::as_i64),
        recurrent: flag(event, "recurrent"),
        highlighted: flag(event, "highlighted"),
        trending: flag(event, "trending"),
        approved: flag(event, "approved"),
        attending: event.get("attending").and_then(Value::as_bool),
        community_id: event.get("community_id").and_then(Value::as_str),
        user: event.get("user").and_then(Value::as_str),
        x: coord(event, 0).or_else(|| event.get("x").and_then(Value::as_i64).map(|v| v as i32)),
        y: coord(event, 1).or_else(|| event.get("y").and_then(Value::as_i64).map(|v| v as i32)),
        description: event
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(""),
    })
}

/// Ok(false) means the row was skipped (no id), Ok(true) means upserted.
pub async fn upsert_event(pool: &PgPool, event: &Value) -> Result<bool> {
    let Some(f) = extract_fields(event) else {
        return Ok(false);
    };
    upsert_rows(pool, &[(f, event)]).await?;
    Ok(true)
}

/// One multi-row upsert per page; a later duplicate id wins as it did row by row.
/// A failed batch falls back to row-by-row so one bad row still only costs itself.
async fn upsert_page(pool: &PgPool, events: &[Value], out: &mut PassOutcome) {
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    let mut rows: Vec<(EventFields, &Value)> = Vec::with_capacity(events.len());
    let mut extracted = 0usize;
    for event in events {
        let Some(f) = extract_fields(event) else {
            continue;
        };
        extracted += 1;
        match by_id.get(f.id) {
            Some(&i) => rows[i] = (f, event),
            None => {
                by_id.insert(f.id, rows.len());
                rows.push((f, event));
            }
        }
    }
    match upsert_rows(pool, &rows).await {
        Ok(()) => {
            out.upserted += extracted;
            out.skipped_rows += events.len() - extracted;
        }
        Err(e) => {
            tracing::warn!(error = %e, "event mirror: page upsert failed; retrying row by row");
            for event in events {
                match upsert_event(pool, event).await {
                    Ok(true) => out.upserted += 1,
                    Ok(false) => out.skipped_rows += 1,
                    Err(e) => {
                        out.failed_rows += 1;
                        out.complete = false;
                        let id = event.get("id").and_then(Value::as_str).unwrap_or("?");
                        tracing::warn!(error = %e, event_id = id, "event mirror: upsert failed; row skipped");
                    }
                }
            }
        }
    }
}

async fn upsert_rows(pool: &PgPool, rows: &[(EventFields<'_>, &Value)]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    reclaim_local(pool).await?;
    sqlx::query(UPSERT)
        .bind(rows.iter().map(|(f, _)| f.id).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.name).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.start_at).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.finish_at).collect::<Vec<_>>())
        .bind(
            rows.iter()
                .map(|(f, _)| f.next_start_at)
                .collect::<Vec<_>>(),
        )
        .bind(
            rows.iter()
                .map(|(f, _)| f.next_finish_at)
                .collect::<Vec<_>>(),
        )
        .bind(rows.iter().map(|(f, _)| f.duration_ms).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.recurrent).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.highlighted).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.trending).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.approved).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.attending).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.community_id).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.user).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.x).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.y).collect::<Vec<_>>())
        .bind(rows.iter().map(|(f, _)| f.description).collect::<Vec<_>>())
        .bind(rows.iter().map(|(_, e)| (*e).clone()).collect::<Vec<_>>())
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum SweepOutcome {
    Swept(u64),
    /// The volume fuse tripped: `candidates` rows were up for deletion out
    /// of `mirrored` mirror-owned rows, and nothing was deleted.
    Refused {
        candidates: i64,
        mirrored: i64,
    },
}

/// Deletes mirrored rows last seen before `cutoff` with their local RSVPs
/// and moderation, sparing this node's own events. Refuses to delete anything when the candidate set exceeds
/// max(SWEEP_FUSE_MIN_CANDIDATES, 20% of the mirror-owned rows).
pub async fn sweep_removed(pool: &PgPool, cutoff: DateTime<Utc>) -> Result<SweepOutcome> {
    reclaim_local(pool).await?;
    let candidates: i64 = sqlx::query_scalar(SWEEP_CANDIDATES)
        .bind(cutoff.naive_utc())
        .fetch_one(pool)
        .await?;
    let mirrored: i64 = sqlx::query_scalar(MIRRORED_COUNT).fetch_one(pool).await?;
    let threshold = SWEEP_FUSE_MIN_CANDIDATES.max(mirrored / 5);
    if candidates > threshold {
        tracing::warn!(
            candidates,
            mirrored,
            threshold,
            "event mirror: sweep would delete too many rows; skipping sweep"
        );
        return Ok(SweepOutcome::Refused {
            candidates,
            mirrored,
        });
    }
    let swept: i64 = sqlx::query_scalar(SWEEP)
        .bind(cutoff.naive_utc())
        .fetch_one(pool)
        .await?;
    Ok(SweepOutcome::Swept(swept as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real captured responses from events.decentraland.org -- the active
    /// default list and the first page of the full catalogue (oldest events,
    /// which carry the most nulls).
    const ACTIVE_PAGE: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/explore_mirrors/upstream_events_active_page0.json"
    ));
    const ALL_PAGE: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/explore_mirrors/upstream_events_all_page0.json"
    ));

    fn events(fixture: &str) -> Vec<Value> {
        let body: Value = serde_json::from_str(fixture).expect("fixture is valid JSON");
        parse_page(&body).expect("fixture envelope parses")
    }

    #[test]
    fn only_a_served_event_or_a_404_settles_an_unlisted_row() {
        let served = serde_json::json!({"ok": true, "data": {"id": "e1", "deleted_by_user": true}});
        assert_eq!(
            detail_outcome(200, Some(&served)),
            DetailOutcome::Present(served["data"].clone())
        );
        assert_eq!(detail_outcome(404, None), DetailOutcome::Gone);
        let not_found = serde_json::json!({"ok": false, "error": "Not found event \"e1\""});
        assert_eq!(detail_outcome(404, Some(&not_found)), DetailOutcome::Gone);
        assert_eq!(detail_outcome(502, None), DetailOutcome::Unknown);
        assert_eq!(detail_outcome(200, None), DetailOutcome::Unknown);
        let refused = serde_json::json!({"ok": false, "error": "rate limited"});
        assert_eq!(detail_outcome(200, Some(&refused)), DetailOutcome::Unknown);
        let no_event = serde_json::json!({"ok": true, "data": null});
        assert_eq!(detail_outcome(200, Some(&no_event)), DetailOutcome::Unknown);
    }

    #[test]
    fn active_fixture_parses_all_events() {
        let evs = events(ACTIVE_PAGE);
        assert_eq!(evs.len(), 100);
        for e in &evs {
            let f = extract_fields(e).expect("every live event has an id");
            assert!(!f.id.is_empty());
            assert!(!f.name.is_empty());
            assert!(f.start_at.is_some() && f.finish_at.is_some());
            assert!(f.next_start_at.is_some() && f.next_finish_at.is_some());
            assert!(f.duration_ms.is_some());
            assert!(f.approved);
            assert!(f.x.is_some() && f.y.is_some());
        }
        let with_community = evs
            .iter()
            .filter(|e| extract_fields(e).unwrap().community_id.is_some())
            .count();
        assert!(with_community > 0 && with_community < evs.len());
        let f = extract_fields(&evs[0]).unwrap();
        let coords = evs[0]["coordinates"].as_array().unwrap();
        assert_eq!(f.x, coords[0].as_i64().map(|v| v as i32));
        assert_eq!(f.y, coords[1].as_i64().map(|v| v as i32));
    }

    #[test]
    fn all_catalog_fixture_parses_all_events() {
        let evs = events(ALL_PAGE);
        assert_eq!(evs.len(), 100);
        assert!(evs.iter().all(|e| extract_fields(e).is_some()));
        assert!(
            evs.iter()
                .all(|e| extract_fields(e).unwrap().community_id.is_none())
        );
    }

    #[test]
    fn envelope_rejects_ok_false_and_missing_data_and_an_event_without_id_is_skipped() {
        assert!(parse_page(&serde_json::json!({"ok": false, "data": []})).is_err());
        assert!(parse_page(&serde_json::json!({"ok": true})).is_err());
        assert!(parse_page(&serde_json::json!({"data": []})).is_err());
        assert!(extract_fields(&serde_json::json!({"name": "no id"})).is_none());
        assert!(extract_fields(&serde_json::json!({"id": ""})).is_none());
    }
}
