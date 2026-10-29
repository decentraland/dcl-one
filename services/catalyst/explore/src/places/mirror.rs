//! The places and worlds catalogue mirrors: page through the upstream
//! `/api/places` and `/api/worlds` listings into `explore.place`, with the
//! cursor and pass markers of [`crate::mirror_state`] so a restart
//! resumes at the page it was on and `/api/status` can prove a complete pass.
//!
//! These mirrors write the world=false (places) and world=true (worlds)
//! `origin = 'upstream'` rows of `explore.place`; the places derived from
//! this node's own scenes (`origin = 'local'`, `places::derive`) sit beside
//! them, and a node's own worlds live in `explore.place_world_local`, which
//! nothing here writes or deletes; an upstream row of a world (or id) listed
//! there, or of a world this node publishes and blocks, is skipped. Rows age out by omission: a complete pass stamps every
//! row upstream still lists with a fresh `fetched_at`, and an upstream row is
//! deleted, with this node's likes and favorites of it, only after two
//! consecutive complete passes fail to list it (the cutoff sits two intervals
//! before the pass start, so one pagination miss never deletes a live row),
//! behind a volume fuse.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::MirrorSource;
use crate::http::read_body_capped;
use crate::mirror_state::{self, PassEnd};

pub const PLACES_MIRROR: &str = "places";
pub const WORLDS_MIRROR: &str = "worlds";

/// Upstream clamps `limit` to 100 (places getPlaceListQuery / getWorldListQuery).
const PAGE: i64 = 100;
/// A worlds pass ends after this many consecutive failed pages so an
/// unreachable upstream cannot spin the offset forever.
const MAX_CONSECUTIVE_PAGE_FAILURES: u32 = 3;
/// Hard offset ceiling; the live catalogues are ~1.7k worlds and ~24.4k
/// places, so hitting this means the upstream is misbehaving.
/// A pass that reaches it is recorded incomplete and never sweeps.
const MAX_OFFSET: i64 = 100_000;
const MAX_PAGE_BYTES: usize = 8 * 1024 * 1024;

/// The sweep refuses to delete anything when its candidate set exceeds
/// max(SWEEP_FUSE_MIN_CANDIDATES, 20% of the leg's mirrored rows): a
/// well-formed but truncated upstream response still reports complete, and
/// must not mass-delete the catalogue.
const SWEEP_FUSE_MIN_CANDIDATES: i64 = 50;

const UPSERT: &str = r#"
    INSERT INTO explore.place
        (id, base_position, title, description, creator_address, content_rating,
         categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
         raw, fetched_at, origin)
    SELECT u.id, u.base_position, u.title, u.description, u.creator_address,
           COALESCE(m.content_rating, u.content_rating),
           ARRAY(SELECT jsonb_array_elements_text(u.categories)), u.likes, u.dislikes, u.favorites,
           u.deployed_at, COALESCE(m.disabled, u.disabled), COALESCE(m.highlighted, u.highlighted),
           u.raw || COALESCE(r.overlay, '{}'::jsonb), timezone('UTC', now()), 'upstream'
    FROM unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::jsonb[], $8::int4[], $9::int4[], $10::int4[], $11::timestamp[],
                $12::boolean[], $13::boolean[], $14::jsonb[])
         AS u(id, base_position, title, description, creator_address, content_rating,
              categories, likes, dislikes, favorites, deployed_at, disabled, highlighted, raw)
    -- An operator's decision (places::moderation) outlives every pass: the
    -- overlay's non-NULL fields win over upstream's.
    LEFT JOIN explore.place_moderation m ON m.place_id = u.id
    -- and so does an operator's ranking (places::ranking): its raw keys win.
    LEFT JOIN explore.place_ranking r ON r.place_id = u.id
    -- A world this node publishes is listed from the local leg alone
    -- (places::derive), and an id stays unique across both legs.
    WHERE NOT EXISTS (
        SELECT 1 FROM explore.place_world_local l
        WHERE l.id = u.id
           OR (l.world IS TRUE AND lower(l.world_name) = lower(NULLIF(btrim(u.raw->>'world_name'), ''))))
    -- While this node blocks one, the local leg prunes its row and the world
    -- is not listed at all, as the worlds listing hides it.
      AND NOT EXISTS (
        SELECT 1 FROM worlds.worlds w
        WHERE w.mirror_source IS NULL AND w.blocked_since IS NOT NULL
          AND (w.name = u.id OR w.name = lower(NULLIF(btrim(u.raw->>'world_name'), '')))
          AND EXISTS (SELECT 1 FROM worlds.world_scenes ws
                      WHERE ws.world_name = w.name AND ws.mirror_source IS NULL))
    ON CONFLICT (id) DO UPDATE SET
        base_position   = EXCLUDED.base_position,
        title           = EXCLUDED.title,
        description     = EXCLUDED.description,
        creator_address = EXCLUDED.creator_address,
        content_rating  = EXCLUDED.content_rating,
        categories      = EXCLUDED.categories,
        likes           = EXCLUDED.likes,
        dislikes        = EXCLUDED.dislikes,
        favorites       = EXCLUDED.favorites,
        deployed_at     = EXCLUDED.deployed_at,
        disabled        = EXCLUDED.disabled,
        highlighted     = EXCLUDED.highlighted,
        raw             = EXCLUDED.raw,
        fetched_at      = timezone('UTC', now()),
        -- a listed id is the upstream's, even where a local scene derived it first
        origin          = 'upstream'
"#;

const RETIRE_DERIVED: &str =
    "DELETE FROM explore.place WHERE raw->>'source' = 'content' AND origin = 'upstream'";

/// A place a binary from before `origin` derived from this node's own
/// scenes (it may still run against a migrated database, and the default
/// makes the row 'upstream'): its `raw.local` marker, the rule
/// schemas/migrations/0007 ran once, reclassifies it before anything here
/// could overwrite, retire or sweep it.
const RECLAIM_LOCAL: &str = "UPDATE explore.place SET origin = 'local' WHERE raw->>'local' = 'true' AND origin = 'upstream'";

async fn reclaim_local(pool: &PgPool) -> Result<()> {
    let n = sqlx::query(RECLAIM_LOCAL)
        .execute(pool)
        .await?
        .rows_affected();
    if n > 0 {
        tracing::info!(
            rows = n,
            "places mirror: reclassified local places an older binary derived"
        );
    }
    Ok(())
}

/// Deletes the rows the whole-catalogue content derive produced; a complete
/// places pass replaces them with the upstream catalogue. The rows derived
/// from this node's own scenes (`origin = 'local'`, `places::derive`) stay
/// beside it.
pub(crate) async fn retire_derived(pool: &PgPool) -> Result<u64> {
    reclaim_local(pool).await?;
    Ok(sqlx::query(RETIRE_DERIVED)
        .execute(pool)
        .await?
        .rows_affected())
}

/// Parse before binding, never through `::timestamptz`: a mirrored row carries
/// a third-party string, and a cast error there fails the whole page instead of
/// dropping the one unusable field.
pub(crate) fn parse_mirror_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = raw?;
    match DateTime::parse_from_rfc3339(raw) {
        Ok(d) => Some(d.with_timezone(&Utc)),
        Err(_) => {
            tracing::debug!(value = raw, "mirror: unparseable deployed_at dropped");
            None
        }
    }
}

#[derive(Debug, Default)]
pub struct PassOutcome {
    pub upserted: usize,
    pub skipped_rows: usize,
    pub failed_rows: usize,
    pub failed_pages: usize,
    pub swept: u64,
    pub retired: u64,
    /// The offset this pass resumed from, when a persisted cursor was found.
    pub resumed_from: Option<i64>,
    /// Shutdown interrupted the pass; the cursor stays for the next run.
    pub cancelled: bool,
    /// True only when every page fetched and every row upserted cleanly; the
    /// delete sweep is gated on it so a flaky cycle can never wipe rows it
    /// merely failed to see.
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leg {
    Places,
    Worlds,
}

impl Leg {
    fn world_clause(self) -> &'static str {
        match self {
            Leg::Places => "world IS FALSE",
            Leg::Worlds => "world IS TRUE",
        }
    }
}

/// One mirrored row as the upsert binds it.
#[derive(Debug, Clone)]
pub struct MirrorRow {
    pub id: String,
    pub base_position: String,
    pub title: String,
    pub description: String,
    pub creator_address: Option<String>,
    pub content_rating: Option<String>,
    pub categories: Vec<String>,
    pub likes: i32,
    pub dislikes: i32,
    pub favorites: i32,
    pub deployed_at: Option<NaiveDateTime>,
    pub disabled: bool,
    pub highlighted: bool,
    pub raw: Value,
}

/// The sleeps before each retry of one page fetch. One 25 s read timeout in
/// a 250-page places pass used to fail the whole pass into the job's backoff.
const PAGE_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];

/// One page, retried over [`PAGE_RETRY_DELAYS`] while the failure is one the
/// upstream or the path to it mends on its own ([`retryable_page_error`]);
/// any other error, and the last retry's, is the caller's. Both passes call
/// this inside a `select!` on the cancel token, which covers the sleeps.
async fn fetch_page(client: &crate::http::Outbound, url: &str) -> Result<Value> {
    let mut delays = PAGE_RETRY_DELAYS.iter();
    loop {
        match fetch_page_once(client, url).await {
            Ok(body) => return Ok(body),
            Err(e) => match delays.next() {
                Some(delay) if retryable_page_error(&e) => {
                    tracing::info!(
                        attempt = PAGE_RETRY_DELAYS.len() - delays.len(),
                        wait_s = delay.as_secs(),
                        url = %url,
                        error = %format!("{e:#}"),
                        "places upstream: page fetch failed; retrying"
                    );
                    tokio::time::sleep(*delay).await;
                }
                _ => return Err(e),
            },
        }
    }
}

/// A timeout, a connect failure or a 5xx anywhere in the cause chain.
fn retryable_page_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        if let Some(e) = cause.downcast_ref::<crate::http::SendError>() {
            return e.is_timeout() || e.is_connect();
        }
        if let Some(e) = cause.downcast_ref::<reqwest::Error>() {
            return e.is_timeout()
                || e.is_connect()
                || e.status().is_some_and(|s| s.is_server_error());
        }
        false
    })
}

/// The process user agent (`catalyst/<version>`) identifies this node's
/// traffic upstream; the old crawler string claimed decentraland.org.
async fn fetch_page_once(client: &crate::http::Outbound, url: &str) -> Result<Value> {
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
    serde_json::from_slice(&body).with_context(|| format!("decoding {url}"))
}

/// The upstream envelope is `{"total": N, "ok": true, "data": [row, ...]}`.
pub fn parse_page(body: &Value) -> Result<Vec<Value>> {
    if body.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(anyhow!("upstream returned ok!=true"));
    }
    body.get("data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("upstream response has no data array"))
}

fn first_str<'a>(place: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| place.get(*k).and_then(Value::as_str))
}

fn int(place: &Value, key: &str) -> i32 {
    place.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

/// A place row: only an id is required, every other column tolerates absence.
pub fn extract_place(place: &Value) -> Option<MirrorRow> {
    let id = match place.get("id").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s,
        _ => return None,
    };
    Some(MirrorRow {
        id: id.to_string(),
        base_position: place
            .get("base_position")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("0,0")
            .to_string(),
        title: first_str(place, &["title", "name"])
            .unwrap_or("")
            .to_string(),
        description: place
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        creator_address: first_str(place, &["owner", "creator_address"])
            .map(|s| s.to_lowercase())
            .filter(|s| !s.is_empty()),
        content_rating: place
            .get("content_rating")
            .and_then(Value::as_str)
            .map(str::to_string),
        categories: place
            .get("categories")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        likes: int(place, "likes"),
        dislikes: int(place, "dislikes"),
        favorites: int(place, "favorites"),
        deployed_at: parse_mirror_timestamp(place.get("deployed_at").and_then(Value::as_str))
            .map(|d| d.naive_utc()),
        disabled: place
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        highlighted: place
            .get("highlighted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        raw: place.clone(),
    })
}

/// One full places pass. Strict: a page that fails to fetch or upsert ends the
/// pass with an error and leaves the cursor, so the next run (after the job's
/// backoff) resumes at that page rather than starting over.
pub async fn run_places_pass(
    client: &crate::http::Outbound,
    pool: &PgPool,
    source: &MirrorSource,
    interval: Duration,
    cancel: &CancellationToken,
) -> Result<PassOutcome> {
    let base = source.base();
    reclaim_local(pool).await?;
    let start = mirror_state::begin_pass(pool, PLACES_MIRROR, base).await?;
    let mut out = PassOutcome {
        complete: true,
        resumed_from: start.resumed.then_some(start.offset),
        upserted: start.rows as usize,
        ..Default::default()
    };
    let mut offset = start.offset;
    let mut ceiling_error: Option<String> = None;
    loop {
        if cancel.is_cancelled() {
            out.cancelled = true;
            out.complete = false;
            return Ok(out);
        }
        let url = format!("{base}/api/places?limit={PAGE}&offset={offset}");
        let body = tokio::select! {
            _ = cancel.cancelled() => {
                out.cancelled = true;
                out.complete = false;
                return Ok(out);
            }
            fetched = fetch_page(client, &url) => match fetched {
                Ok(body) => body,
                Err(e) => {
                    let _ = mirror_state::record_error(pool, PLACES_MIRROR, &format!("{e:#}")).await;
                    return Err(e);
                }
            },
        };
        let data = match parse_page(&body) {
            Ok(data) => data,
            Err(e) => {
                let _ = mirror_state::record_error(
                    pool,
                    PLACES_MIRROR,
                    &format!("offset {offset}: {e:#}"),
                )
                .await;
                bail!("places upstream at offset={offset}: {e}");
            }
        };
        let total = body.get("total").and_then(Value::as_i64).unwrap_or(0);
        if data.is_empty() {
            break;
        }
        let rows = dedupe(data.iter().filter_map(extract_place));
        out.skipped_rows += data.len() - rows.len();
        if let Err(e) = upsert_rows(pool, &rows).await {
            let _ =
                mirror_state::record_error(pool, PLACES_MIRROR, &format!("offset {offset}: {e:#}"))
                    .await;
            return Err(e);
        }
        out.upserted += rows.len();
        offset += data.len() as i64;
        mirror_state::advance(pool, PLACES_MIRROR, offset, out.upserted as i64).await?;
        if offset >= total || (data.len() as i64) < PAGE {
            break;
        }
        if offset >= MAX_OFFSET {
            tracing::warn!(
                offset,
                total,
                "places mirror: offset ceiling reached; ending pass incomplete"
            );
            out.complete = false;
            ceiling_error = Some(format!(
                "offset ceiling {MAX_OFFSET} reached with upstream total {total}"
            ));
            break;
        }
    }
    if out.complete && out.upserted > 0 {
        out.retired = retire_derived(pool).await?;
        out.swept = sweep_after_pass(pool, Leg::Places, start.started_at, interval).await;
    }
    // The marker's row count is the distinct rows this pass stamped, not the
    // page lengths summed: an id repeated across two upstream pages (seen
    // live: 24,420 upstream rows, 24,419 distinct) is upserted twice but is
    // one row of the table.
    let stamped: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM explore.place WHERE world IS FALSE AND fetched_at >= $1",
    )
    .bind(start.started_at.naive_utc())
    .fetch_one(pool)
    .await?;
    mirror_state::finish_pass(
        pool,
        PLACES_MIRROR,
        &PassEnd {
            complete: out.complete,
            rows: stamped,
            swept: out.swept as i64,
            error: ceiling_error,
        },
    )
    .await?;
    Ok(out)
}

/// The indexed columns the serving queries filter and order on; every other
/// served field (image, created_at, like_rate, is_private, ranking, ...) is
/// read from `raw`, which stores the upstream row verbatim. Everything except
/// the identity pair tolerates being absent or null upstream, and unknown keys
/// pass through untouched inside raw.
#[derive(Debug, Deserialize)]
pub struct WorldFields {
    pub id: String,
    pub world_name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub content_rating: Option<String>,
    #[serde(default)]
    pub base_position: Option<String>,
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    pub likes: Option<i32>,
    #[serde(default)]
    pub dislikes: Option<i32>,
    #[serde(default)]
    pub favorites: Option<i32>,
    #[serde(default)]
    pub deployed_at: Option<String>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub highlighted: Option<bool>,
}

/// A world is identified by its name, so a row without an id or a world_name
/// is unservable and yields None -- skipped, not fatal. The stored raw always
/// carries world=true: the row's provenance is the worlds listing, and the
/// generated `world` column computes from that key.
pub fn extract_world_fields(world: &Value) -> Option<(WorldFields, Value)> {
    let typed: WorldFields = serde_json::from_value(world.clone()).ok()?;
    if typed.id.trim().is_empty() || typed.world_name.trim().is_empty() {
        return None;
    }
    let mut raw = world.clone();
    if raw.get("world").and_then(Value::as_bool) != Some(true) {
        raw["world"] = Value::Bool(true);
    }
    Some((typed, raw))
}

pub fn extract_world(world: &Value) -> Option<MirrorRow> {
    let (w, raw) = extract_world_fields(world)?;
    Some(MirrorRow {
        id: w.id,
        base_position: w.base_position.unwrap_or_else(|| "0,0".to_string()),
        title: w.title.unwrap_or_default(),
        description: w.description.unwrap_or_default(),
        creator_address: w.owner.map(|s| s.to_lowercase()).filter(|s| !s.is_empty()),
        content_rating: w.content_rating,
        categories: w.categories.unwrap_or_default(),
        likes: w.likes.unwrap_or(0),
        dislikes: w.dislikes.unwrap_or(0),
        favorites: w.favorites.unwrap_or(0),
        deployed_at: parse_mirror_timestamp(w.deployed_at.as_deref()).map(|d| d.naive_utc()),
        disabled: w.disabled.unwrap_or(false),
        highlighted: w.highlighted.unwrap_or(false),
        raw,
    })
}

/// Ok(false) means the row was skipped (unservable), Ok(true) means upserted.
pub async fn upsert_world(pool: &PgPool, world: &Value) -> Result<bool> {
    let Some(row) = extract_world(world) else {
        return Ok(false);
    };
    upsert_rows(pool, &[row]).await?;
    Ok(true)
}

/// One full worlds pass. Fail-open per page and per row: an error logs, counts,
/// and the pass moves on; a pass that skipped anything is incomplete and never
/// sweeps.
pub async fn run_worlds_pass(
    client: &crate::http::Outbound,
    pool: &PgPool,
    source: &MirrorSource,
    interval: Duration,
    cancel: &CancellationToken,
) -> Result<PassOutcome> {
    let base = source.base();
    reclaim_local(pool).await?;
    let start = mirror_state::begin_pass(pool, WORLDS_MIRROR, base).await?;
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
            tracing::warn!(offset, "worlds mirror: offset ceiling reached; ending pass");
            out.complete = false;
            break;
        }
        let url = format!("{base}/api/worlds?limit={PAGE}&offset={offset}");
        let fetched = tokio::select! {
            _ = cancel.cancelled() => {
                out.cancelled = true;
                out.complete = false;
                return Ok(out);
            }
            fetched = fetch_page(client, &url) => fetched.and_then(|b| parse_page(&b)),
        };
        match fetched {
            Ok(worlds) => {
                consecutive_failures = 0;
                if worlds.is_empty() {
                    break;
                }
                let count = worlds.len() as i64;
                upsert_page_fail_open(pool, &worlds, extract_world, &mut out).await;
                offset += PAGE;
                mirror_state::advance(pool, WORLDS_MIRROR, offset, out.upserted as i64).await?;
                if count < PAGE {
                    break;
                }
            }
            Err(e) => {
                out.failed_pages += 1;
                out.complete = false;
                consecutive_failures += 1;
                tracing::warn!(error = %e, url = %url, "worlds mirror: page fetch failed; skipped");
                let _ = mirror_state::record_error(pool, WORLDS_MIRROR, &format!("{e:#}")).await;
                if consecutive_failures >= MAX_CONSECUTIVE_PAGE_FAILURES {
                    tracing::warn!(
                        consecutive_failures,
                        "worlds mirror: too many consecutive page failures; ending pass"
                    );
                    break;
                }
                offset += PAGE;
                mirror_state::advance(pool, WORLDS_MIRROR, offset, out.upserted as i64).await?;
            }
        }
    }
    if out.complete && out.upserted > 0 {
        out.swept = sweep_after_pass(pool, Leg::Worlds, start.started_at, interval).await;
    }
    mirror_state::finish_pass(
        pool,
        WORLDS_MIRROR,
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

/// A later duplicate id wins, as it did row by row.
fn dedupe(rows: impl Iterator<Item = MirrorRow>) -> Vec<MirrorRow> {
    let mut by_id: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<MirrorRow> = Vec::new();
    for row in rows {
        match by_id.get(&row.id) {
            Some(&i) => out[i] = row,
            None => {
                by_id.insert(row.id.clone(), out.len());
                out.push(row);
            }
        }
    }
    out
}

/// One multi-row upsert per page; a failed batch falls back to row-by-row so
/// one bad row still only costs itself.
async fn upsert_page_fail_open(
    pool: &PgPool,
    page: &[Value],
    extract: fn(&Value) -> Option<MirrorRow>,
    out: &mut PassOutcome,
) {
    let rows = dedupe(page.iter().filter_map(extract));
    let extracted = page.iter().filter(|v| extract(v).is_some()).count();
    match upsert_rows(pool, &rows).await {
        Ok(()) => {
            out.upserted += extracted;
            out.skipped_rows += page.len() - extracted;
        }
        Err(e) => {
            tracing::warn!(error = %e, "mirror: page upsert failed; retrying row by row");
            for value in page {
                match extract(value) {
                    None => out.skipped_rows += 1,
                    Some(row) => match upsert_rows(pool, std::slice::from_ref(&row)).await {
                        Ok(()) => out.upserted += 1,
                        Err(e) => {
                            out.failed_rows += 1;
                            out.complete = false;
                            tracing::warn!(error = %e, id = %row.id, "mirror: upsert failed; row skipped");
                        }
                    },
                }
            }
        }
    }
}

pub async fn upsert_rows(pool: &PgPool, rows: &[MirrorRow]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    sqlx::query(UPSERT)
        .bind(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>())
        .bind(
            rows.iter()
                .map(|r| r.base_position.as_str())
                .collect::<Vec<_>>(),
        )
        .bind(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>())
        .bind(
            rows.iter()
                .map(|r| r.description.as_str())
                .collect::<Vec<_>>(),
        )
        .bind(
            rows.iter()
                .map(|r| r.creator_address.as_deref())
                .collect::<Vec<_>>(),
        )
        .bind(
            rows.iter()
                .map(|r| r.content_rating.as_deref())
                .collect::<Vec<_>>(),
        )
        .bind(
            rows.iter()
                .map(|r| Value::from(r.categories.clone()))
                .collect::<Vec<_>>(),
        )
        .bind(rows.iter().map(|r| r.likes).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.dislikes).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.favorites).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.deployed_at).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.disabled).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.highlighted).collect::<Vec<_>>())
        .bind(rows.iter().map(|r| r.raw.clone()).collect::<Vec<_>>())
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

/// Deletes the leg's mirrored rows last seen before `cutoff`, unless the
/// candidate set trips the volume fuse. Rows derived from this node's own
/// scenes (`origin = 'local'`, or an older binary's `raw.local`) are neither
/// candidates nor counted. The local
/// likes, favorites and their counters of a deleted row go in the same
/// statement, unless a local world keeps the id.
pub async fn sweep_removed(pool: &PgPool, leg: Leg, cutoff: DateTime<Utc>) -> Result<SweepOutcome> {
    reclaim_local(pool).await?;
    let world = leg.world_clause();
    let candidates: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM explore.place WHERE {world} AND origin = 'upstream' AND fetched_at < $1"
    )))
    .bind(cutoff.naive_utc())
    .fetch_one(pool)
    .await?;
    let mirrored: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM explore.place WHERE {world} AND origin = 'upstream'"
    )))
    .fetch_one(pool)
    .await?;
    let threshold = SWEEP_FUSE_MIN_CANDIDATES.max(mirrored / 5);
    if candidates > threshold {
        tracing::warn!(
            ?leg,
            candidates,
            mirrored,
            threshold,
            "mirror: sweep would delete too many rows; skipping sweep"
        );
        return Ok(SweepOutcome::Refused {
            candidates,
            mirrored,
        });
    }
    let swept: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH gone AS ( \
             DELETE FROM explore.place WHERE {world} AND origin = 'upstream' AND fetched_at < $1 \
             RETURNING id \
         ), orphaned AS ( \
             SELECT id FROM gone \
             WHERE NOT EXISTS (SELECT 1 FROM explore.place_world_local w WHERE w.id = gone.id) \
         ), favorites AS ( \
             DELETE FROM explore.user_favorites f USING orphaned o WHERE f.entity_id = o.id \
         ), likes AS ( \
             DELETE FROM explore.user_likes l USING orphaned o WHERE l.entity_id = o.id \
         ), counts AS ( \
             DELETE FROM explore.place_interaction_counts c USING orphaned o WHERE c.entity_id = o.id \
         ) \
         SELECT count(*) FROM gone"
    )))
    .bind(cutoff.naive_utc())
    .fetch_one(pool)
    .await?;
    Ok(SweepOutcome::Swept(swept as u64))
}

async fn sweep_after_pass(
    pool: &PgPool,
    leg: Leg,
    pass_start: DateTime<Utc>,
    interval: Duration,
) -> u64 {
    let grace = interval
        .checked_mul(2)
        .and_then(|d| chrono::Duration::from_std(d).ok())
        .unwrap_or_else(|| chrono::Duration::hours(2));
    match sweep_removed(pool, leg, pass_start - grace).await {
        Ok(SweepOutcome::Swept(n)) => n,
        Ok(SweepOutcome::Refused { .. }) => 0,
        Err(e) => {
            tracing::warn!(error = %e, ?leg, "mirror: sweep failed");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Real captured response from places.decentraland.org/api/worlds.
    const WORLDS_PAGE: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/explore_mirrors/worlds_page.json"
    ));

    /// A loopback upstream: `/slow` never answers, `/down` is a 503, `/bad` is not JSON.
    async fn stub_upstream() -> String {
        let app = axum::Router::new()
            .route("/slow", axum::routing::get(std::future::pending::<()>))
            .route(
                "/down",
                axum::routing::get(|| async { axum::http::StatusCode::SERVICE_UNAVAILABLE }),
            )
            .route("/bad", axum::routing::get(|| async { "not json" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn a_page_is_retried_on_timeout_connect_failure_and_5xx_only() {
        let base = stub_upstream().await;
        let client = crate::http::Outbound::for_tests();

        let timeout = client
            .get(format!("{base}/slow"))
            .timeout(Duration::from_millis(50))
            .send()
            .await
            .map(|_| ())
            .with_context(|| "GET /slow".to_string())
            .unwrap_err();
        assert!(retryable_page_error(&timeout), "{timeout:#}");

        let refused_port = {
            let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            taken.local_addr().unwrap().port()
        };
        let connect = fetch_page_once(&client, &format!("http://127.0.0.1:{refused_port}/"))
            .await
            .unwrap_err();
        assert!(retryable_page_error(&connect), "{connect:#}");

        let down = fetch_page_once(&client, &format!("{base}/down"))
            .await
            .unwrap_err();
        assert!(retryable_page_error(&down), "{down:#}");

        let bad = fetch_page_once(&client, &format!("{base}/bad"))
            .await
            .unwrap_err();
        assert!(
            !retryable_page_error(&bad),
            "a decode failure is not the upstream's to mend: {bad:#}"
        );

        let guarded = anyhow::Error::from(crate::http::SendError::Refused("private target".into()))
            .context("GET http://10.0.0.1/");
        assert!(
            !retryable_page_error(&guarded),
            "the SSRF guard's refusal never retries"
        );
        assert!(!retryable_page_error(&anyhow!(
            "upstream returned ok!=true"
        )));
    }

    fn fixture() -> Value {
        serde_json::from_str(WORLDS_PAGE).expect("fixture is valid JSON")
    }

    #[test]
    fn fixture_page_is_a_real_upstream_envelope() {
        let body = fixture();
        assert!(body["total"].as_i64().unwrap() > 1000, "{}", body["total"]);
        let rows = parse_page(&body).expect("fixture envelope parses");
        assert_eq!(rows.len(), 5);
        for row in &rows {
            let (w, raw) = extract_world_fields(row).expect("real upstream rows parse");
            assert!(!w.id.is_empty());
            assert!(!w.world_name.is_empty());
            assert_eq!(raw["world"], json!(true));
            assert_eq!(&raw["world_name"], &row["world_name"]);
        }
    }

    #[test]
    fn known_fixture_row_maps_field_for_field() {
        let (w, raw) =
            extract_world_fields(&parse_page(&fixture()).unwrap()[0]).expect("row 0 parses");
        assert_eq!(w.id, "metadynelabs.dcl.eth");
        assert_eq!(w.world_name, "MetadyneLabs.dcl.eth");
        assert_eq!(w.title.as_deref(), Some("Metadyne Labs - Rat Scape"));
        assert_eq!(
            w.owner.as_deref(),
            Some("0x6adf75e49bac21abab9adb9266d2cc6d90abd31a")
        );
        assert_eq!(w.content_rating.as_deref(), Some("RP"));
        assert_eq!(w.base_position.as_deref(), Some("0,0"));
        assert_eq!(w.categories.as_deref(), Some(&[][..]));
        assert_eq!(w.likes, Some(94));
        assert_eq!(w.dislikes, Some(4));
        assert_eq!(w.favorites, Some(3));
        assert_eq!(w.disabled, Some(false));
        assert_eq!(w.highlighted, Some(false));
        assert_eq!(w.deployed_at.as_deref(), Some("2024-08-08T11:50:58.807Z"));
        assert!(
            raw["image"]
                .as_str()
                .unwrap()
                .starts_with("https://worlds-content-server.decentraland.org/contents/")
        );
        assert_eq!(raw["like_score"], json!(0.85771024));
        assert_eq!(raw["created_at"], json!("2023-10-17T16:31:57.766Z"));
        let row = extract_world(&parse_page(&fixture()).unwrap()[0]).unwrap();
        assert_eq!(
            row.deployed_at
                .map(|d| d
                    .and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                .as_deref(),
            Some("2024-08-08T11:50:58.807Z")
        );
    }

    #[test]
    fn unused_and_unknown_fields_do_not_break_the_parse() {
        let (w, _) = extract_world_fields(&json!({
            "id": "a.dcl.eth",
            "world_name": "a.dcl.eth",
            "settings_version": null,
            "skybox_time": null,
            "some_future_field": {"nested": [1, 2, 3]},
            "likes": null,
            "categories": null
        }))
        .expect("nulls and unknown keys are tolerated");
        assert_eq!(w.likes, None);
        assert_eq!(w.categories, None);
        assert_eq!(w.title, None);
    }

    #[test]
    fn an_unparseable_deployed_at_drops_the_field_instead_of_failing_the_page() {
        let row = extract_world(&json!({
            "id": "a.dcl.eth",
            "world_name": "a.dcl.eth",
            "deployed_at": "not-a-date"
        }))
        .expect("the row is still servable");
        assert_eq!(
            row.deployed_at, None,
            "one unusable timestamp must not abort the whole mirror page"
        );
        assert_eq!(
            parse_mirror_timestamp(Some("2024-08-08T11:50:58.807Z"))
                .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
                .as_deref(),
            Some("2024-08-08T11:50:58.807Z")
        );
    }

    #[test]
    fn a_row_without_a_world_name_is_skipped_and_raw_always_carries_the_world_flag() {
        assert!(extract_world_fields(&json!({"id": "x.dcl.eth"})).is_none());
        assert!(extract_world_fields(&json!({"id": "x.dcl.eth", "world_name": ""})).is_none());
        assert!(extract_world_fields(&json!({"id": "x.dcl.eth", "world_name": "   "})).is_none());
        assert!(extract_world_fields(&json!({"id": "", "world_name": "x.dcl.eth"})).is_none());
        let (_, raw) = extract_world_fields(
            &json!({"id": "a.dcl.eth", "world_name": "a.dcl.eth", "world": false}),
        )
        .unwrap();
        assert_eq!(raw["world"], json!(true));
        let (_, raw) = extract_world_fields(&json!({
            "id": "a.dcl.eth",
            "world_name": "a.dcl.eth",
            "exclude_from_ranking": true
        }))
        .unwrap();
        assert_eq!(raw["exclude_from_ranking"], json!(true));
    }

    #[test]
    fn envelope_rejects_ok_false_and_missing_data() {
        assert!(parse_page(&json!({"ok": false, "data": []})).is_err());
        assert!(parse_page(&json!({"ok": true})).is_err());
        assert!(parse_page(&json!({"data": []})).is_err());
    }

    #[test]
    fn a_place_row_reads_owner_or_creator_and_needs_only_an_id() {
        let row = extract_place(&json!({
            "id": "p1",
            "name": "Genesis Plaza",
            "owner": "0xABC",
            "categories": ["poi", 7],
            "likes": 40,
            "deployed_at": "2024-08-08T11:50:58.807Z",
            "highlighted": true
        }))
        .unwrap();
        assert_eq!(row.title, "Genesis Plaza");
        assert_eq!(row.base_position, "0,0");
        assert_eq!(row.creator_address.as_deref(), Some("0xabc"));
        assert_eq!(row.categories, vec!["poi"]);
        assert_eq!(row.likes, 40);
        assert!(row.highlighted);
        assert!(row.deployed_at.is_some());
        assert!(extract_place(&json!({"name": "no id"})).is_none());
        assert!(extract_place(&json!({"id": ""})).is_none());
    }

    #[test]
    fn a_later_duplicate_id_wins() {
        let rows = dedupe(
            [
                json!({"id": "a", "title": "first"}),
                json!({"id": "b"}),
                json!({"id": "a", "title": "second"}),
            ]
            .iter()
            .filter_map(extract_place),
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "second");
    }
}
