//! The upstream communities mirror: pages through an upstream social API's
//! public listing (`GET /v1/communities?limit=100&offset=N`, the public,
//! active communities an anonymous viewer sees) into `social.communities`,
//! keyed by the upstream id, the way the explore mirrors copy places and
//! events (`crate::explore::events::mirror`): a durable cursor per source
//! (`social.community_mirror_state`) so a killed pass resumes at its page
//! with the same start, and a sweep gated on a complete pass behind a volume
//! fuse.
//!
//! A mirrored community (`communities.origin = 'upstream'`) is not local
//! authority. The pass writes no owner or moderator row for it, so no wallet
//! manages it here (the upstream owner included: its `owner_address` is
//! display data, and every authority check reads `community_members`). A
//! join or leave is a local member row keyed by the upstream id, which the
//! member count adds to the upstream's; [`refuse_mirrored_writes`] answers
//! 409 naming the upstream to every other community-scoped write; and the
//! pass never overwrites a local community (a colliding id). The local
//! operator axes (`suspended`, `editors_choice`, `ranking_score`) are never
//! written by the pass, so a local suspension holds.
//!
//! What the pass copies: name, description, owner, privacy and active; the
//! upstream `membersCount` and the source land in the provenance row
//! (`social.community_mirror`); the thumbnail is the upstream CDN's
//! `/social/communities/{id}/raw-thumbnail.png` once a HEAD finds it. A
//! community the upstream stopped listing (deleted, made private) is
//! deactivated (`active = false`, reversible: a later listing reactivates
//! it) after it was missed for the grace window, never deleted. Its local
//! members stay, inert while it is inactive (every member read requires an
//! active community), and come back with it; the rows go only with the
//! community itself (`ON DELETE CASCADE`).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use chrono::{NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::SharedState;
use super::http::err_json;
use crate::Address;
use crate::explore::MirrorSource;
use crate::http::{Outbound, read_body_capped};
use crate::jobs::{Jobs, PeriodicCfg};

/// Upstream clamps `limit` to 100.
pub const PAGE: i64 = 100;
const MAX_CONSECUTIVE_PAGE_FAILURES: u32 = 3;
/// The live listing is ~150 communities; this ceiling means a misbehaving
/// upstream, not growth.
const MAX_OFFSET: i64 = 50_000;
const MAX_PAGE_BYTES: usize = 8 * 1024 * 1024;
/// Thumbnails probed per pass (only rows without a found one).
const THUMBNAIL_PROBES: i64 = 500;
/// The sweep refuses when its candidates exceed max(this, 20% of the active
/// mirrored rows): a truncated but well-formed listing must not deactivate
/// the catalogue.
const SWEEP_FUSE_MIN_CANDIDATES: i64 = 10;

/// Where a pass reads from.
#[derive(Debug, Clone)]
pub struct MirrorConfig {
    pub source: MirrorSource,
    /// The base the upstream serves thumbnails under
    /// (`{base}/social/communities/{id}/raw-thumbnail.png`).
    pub thumbnail_base: MirrorSource,
    pub interval: Duration,
}

/// One upstream community as the pass stores it.
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamCommunity {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub owner_address: String,
    pub private: bool,
    pub active: bool,
    pub members_count: i64,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct PassOutcome {
    pub upserted: usize,
    /// Rows the listing carried that could not be read (bad id or owner).
    pub skipped_rows: usize,
    /// Rows whose id names a local (non-mirrored) community; left alone.
    pub local_conflicts: usize,
    pub failed_pages: usize,
    pub thumbnails_found: usize,
    pub deactivated: u64,
    /// The communities the sweep deactivated, for the worlds that list them.
    #[serde(skip)]
    pub deactivated_ids: Vec<Uuid>,
    pub sweep_refused: bool,
    pub resumed_from: Option<i64>,
    pub cancelled: bool,
    /// Every page fetched and stored; the sweep runs only then.
    pub complete: bool,
}

pub fn parse_page(body: &Value) -> Result<(Vec<Value>, Option<i64>)> {
    let data = body
        .get("data")
        .ok_or_else(|| anyhow!("communities upstream response has no data"))?;
    let results = data
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("communities upstream response has no data.results array"))?;
    Ok((results, data.get("total").and_then(Value::as_i64)))
}

/// `None` for a row the pass cannot store: no uuid id, no well-formed owner
/// or no name.
pub fn extract(row: &Value) -> Option<UpstreamCommunity> {
    let id = Uuid::parse_str(row.get("id")?.as_str()?).ok()?;
    let owner = Address::parse(row.get("ownerAddress")?.as_str()?).ok()?;
    let name = row.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(UpstreamCommunity {
        id,
        name: name.to_string(),
        description: row
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        owner_address: owner.as_str().to_string(),
        private: row.get("privacy").and_then(Value::as_str) == Some("private"),
        active: row.get("active").and_then(Value::as_bool).unwrap_or(true),
        members_count: row
            .get("membersCount")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0),
    })
}

/// The upstream thumbnail URL of a community.
pub fn upstream_thumbnail(base: &MirrorSource, id: Uuid) -> String {
    super::thumbnail_url(base.base(), &id.to_string())
}

/// Communities copied in: inserted new, or updated only where the row is
/// already mirrored (a local community with the same id is left as is and
/// gets no provenance row). `updated_at` moves only when a copied field
/// changed. One statement: the provenance rows reference the communities
/// the same statement wrote (the FK is checked at its end).
const UPSERT: &str = r#"
    WITH input AS (
        SELECT * FROM unnest($1::uuid[], $2::text[], $3::text[], $4::text[], $5::boolean[],
                             $6::boolean[], $7::int8[])
            AS u(id, name, description, owner_address, private, active, members_count)
    ), written AS (
        INSERT INTO social.communities AS c
            (id, name, description, owner_address, private, active, created_at, updated_at, origin)
        SELECT id, name, description, owner_address, private, active, $8, $8, 'upstream' FROM input
        ON CONFLICT (id) DO UPDATE SET
            name          = EXCLUDED.name,
            description   = EXCLUDED.description,
            owner_address = EXCLUDED.owner_address,
            private       = EXCLUDED.private,
            active        = EXCLUDED.active,
            updated_at    = CASE
                WHEN (c.name, c.description, c.owner_address, c.private, c.active)
                     IS DISTINCT FROM (EXCLUDED.name, EXCLUDED.description,
                                       EXCLUDED.owner_address, EXCLUDED.private, EXCLUDED.active)
                THEN EXCLUDED.updated_at ELSE c.updated_at END
        WHERE c.origin = 'upstream'
        RETURNING c.id
    )
    INSERT INTO social.community_mirror (community_id, source_url, members_count, fetched_at)
    SELECT i.id, $9, i.members_count, $8 FROM input i JOIN written w ON w.id = i.id
    ON CONFLICT (community_id) DO UPDATE SET
        source_url    = EXCLUDED.source_url,
        members_count = EXCLUDED.members_count,
        fetched_at    = EXCLUDED.fetched_at
"#;

/// Upserts one page; returns how many rows landed (the rest named local
/// communities).
pub async fn upsert_page(
    pool: &PgPool,
    source: &str,
    rows: &[UpstreamCommunity],
    now: NaiveDateTime,
) -> sqlx::Result<u64> {
    let mut by_id: HashMap<Uuid, usize> = HashMap::new();
    let mut unique: Vec<&UpstreamCommunity> = Vec::with_capacity(rows.len());
    for r in rows {
        match by_id.get(&r.id) {
            Some(&i) => unique[i] = r,
            None => {
                by_id.insert(r.id, unique.len());
                unique.push(r);
            }
        }
    }
    if unique.is_empty() {
        return Ok(0);
    }
    let res = sqlx::query(UPSERT)
        .bind(unique.iter().map(|r| r.id).collect::<Vec<_>>())
        .bind(unique.iter().map(|r| r.name.as_str()).collect::<Vec<_>>())
        .bind(
            unique
                .iter()
                .map(|r| r.description.as_str())
                .collect::<Vec<_>>(),
        )
        .bind(
            unique
                .iter()
                .map(|r| r.owner_address.as_str())
                .collect::<Vec<_>>(),
        )
        .bind(unique.iter().map(|r| r.private).collect::<Vec<_>>())
        .bind(unique.iter().map(|r| r.active).collect::<Vec<_>>())
        .bind(unique.iter().map(|r| r.members_count).collect::<Vec<_>>())
        .bind(now)
        .bind(source)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

struct PassStart {
    started_at: NaiveDateTime,
    offset: i64,
    rows: i64,
    resumed: bool,
}

async fn begin_pass(pool: &PgPool, source: &str, now: NaiveDateTime) -> sqlx::Result<PassStart> {
    let row: Option<(Option<NaiveDateTime>, Option<i64>, i64)> = sqlx::query_as(
        "SELECT pass_started_at, next_offset, pass_rows \
         FROM social.community_mirror_state WHERE source_url = $1",
    )
    .bind(source)
    .fetch_optional(pool)
    .await?;
    if let Some((Some(started_at), Some(offset), rows)) = row {
        return Ok(PassStart {
            started_at,
            offset,
            rows,
            resumed: true,
        });
    }
    sqlx::query(
        "INSERT INTO social.community_mirror_state \
           (source_url, pass_started_at, next_offset, pass_rows, updated_at) \
         VALUES ($1, $2, 0, 0, $2) \
         ON CONFLICT (source_url) DO UPDATE SET \
           pass_started_at = EXCLUDED.pass_started_at, next_offset = 0, pass_rows = 0, \
           updated_at = EXCLUDED.updated_at",
    )
    .bind(source)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(PassStart {
        started_at: now,
        offset: 0,
        rows: 0,
        resumed: false,
    })
}

async fn advance(pool: &PgPool, source: &str, next_offset: i64, rows: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE social.community_mirror_state \
         SET next_offset = $2, pass_rows = $3, updated_at = $4 WHERE source_url = $1",
    )
    .bind(source)
    .bind(next_offset)
    .bind(rows)
    .bind(Utc::now().naive_utc())
    .execute(pool)
    .await?;
    Ok(())
}

async fn record_error(pool: &PgPool, source: &str, error: &str) {
    let _ = sqlx::query(
        "UPDATE social.community_mirror_state SET last_error = $2, updated_at = $3 \
         WHERE source_url = $1",
    )
    .bind(source)
    .bind(error)
    .bind(Utc::now().naive_utc())
    .execute(pool)
    .await;
}

async fn finish_pass(pool: &PgPool, source: &str, out: &PassOutcome) -> sqlx::Result<()> {
    let error = (!out.complete).then(|| {
        format!(
            "{} failed pages{}",
            out.failed_pages,
            if out.cancelled { ", cancelled" } else { "" }
        )
    });
    sqlx::query(
        "UPDATE social.community_mirror_state SET \
           next_offset = NULL, pass_rows = 0, \
           last_pass_at = $2, last_pass_complete = $3, last_pass_rows = $4, \
           last_pass_deactivated = $5, last_error = $6, \
           last_complete_pass_at = CASE WHEN $3 THEN $2 ELSE last_complete_pass_at END, \
           updated_at = $2 \
         WHERE source_url = $1",
    )
    .bind(source)
    .bind(Utc::now().naive_utc())
    .bind(out.complete)
    .bind(out.upserted as i64)
    .bind(out.deactivated as i64)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_page(client: &Outbound, url: &str) -> Result<(Vec<Value>, Option<i64>)> {
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

/// One full pass. `grace`: a mirrored community last listed before the
/// pass start minus this is deactivated by a complete pass (the periodic
/// job uses two intervals, so one flaky page never hides a community).
pub async fn run_pass(
    client: &Outbound,
    pool: &PgPool,
    cfg: &MirrorConfig,
    grace: Duration,
    cancel: &CancellationToken,
) -> Result<PassOutcome> {
    let base = cfg.source.base();
    let start = begin_pass(pool, base, Utc::now().naive_utc()).await?;
    let mut out = PassOutcome {
        complete: true,
        resumed_from: start.resumed.then_some(start.offset),
        upserted: start.rows as usize,
        ..Default::default()
    };
    let mut offset = start.offset;
    let mut failures = 0u32;
    loop {
        if cancel.is_cancelled() {
            out.cancelled = true;
            out.complete = false;
            return Ok(out);
        }
        if offset >= MAX_OFFSET {
            tracing::warn!(
                offset,
                "community mirror: offset ceiling reached; ending pass"
            );
            out.complete = false;
            break;
        }
        let url = format!("{base}/v1/communities?limit={PAGE}&offset={offset}");
        let fetched = tokio::select! {
            _ = cancel.cancelled() => {
                out.cancelled = true;
                out.complete = false;
                return Ok(out);
            }
            fetched = fetch_page(client, &url) => fetched,
        };
        match fetched {
            Ok((rows, total)) => {
                failures = 0;
                let listed = rows.len() as i64;
                let parsed: Vec<UpstreamCommunity> = rows.iter().filter_map(extract).collect();
                out.skipped_rows += rows.len() - parsed.len();
                let landed = upsert_page(pool, base, &parsed, Utc::now().naive_utc()).await?;
                let unique = parsed.iter().map(|r| r.id).collect::<HashSet<_>>().len();
                out.local_conflicts += unique.saturating_sub(landed as usize);
                out.upserted += landed as usize;
                offset += PAGE;
                advance(pool, base, offset, out.upserted as i64).await?;
                if listed < PAGE || total.is_some_and(|t| offset >= t) {
                    break;
                }
            }
            Err(e) => {
                out.failed_pages += 1;
                out.complete = false;
                failures += 1;
                tracing::warn!(error = %e, url = %url, "community mirror: page fetch failed");
                record_error(pool, base, &format!("{e:#}")).await;
                if failures >= MAX_CONSECUTIVE_PAGE_FAILURES {
                    break;
                }
                offset += PAGE;
                advance(pool, base, offset, out.upserted as i64).await?;
            }
        }
    }

    out.thumbnails_found = probe_thumbnails(client, pool, &cfg.thumbnail_base, cancel).await;

    if out.complete && out.upserted > 0 {
        let grace = chrono::Duration::from_std(grace).unwrap_or(chrono::Duration::hours(2));
        let cutoff = start.started_at - grace;
        match sweep_unlisted(pool, cutoff).await? {
            SweepOutcome::Deactivated(ids) => {
                out.deactivated = ids.len() as u64;
                out.deactivated_ids = ids;
            }
            SweepOutcome::Refused { .. } => out.sweep_refused = true,
        }
    }
    finish_pass(pool, base, &out).await?;
    Ok(out)
}

/// HEADs the upstream thumbnail of the mirrored rows that have none yet;
/// a 2xx image answer stores its URL. Anything else leaves the row for the
/// next pass (the read path answers no thumbnail meanwhile).
async fn probe_thumbnails(
    client: &Outbound,
    pool: &PgPool,
    base: &MirrorSource,
    cancel: &CancellationToken,
) -> usize {
    let ids: Vec<Uuid> = match sqlx::query_scalar(
        "SELECT community_id FROM social.community_mirror WHERE thumbnail_url IS NULL \
         ORDER BY community_id LIMIT $1",
    )
    .bind(THUMBNAIL_PROBES)
    .fetch_all(pool)
    .await
    {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(error = %e, "community mirror: thumbnail candidates query failed");
            return 0;
        }
    };
    let mut found = 0;
    for id in ids {
        if cancel.is_cancelled() {
            break;
        }
        let url = upstream_thumbnail(base, id);
        let is_image = match client.request(Method::HEAD, &url).send().await {
            Ok(resp) => {
                resp.status().is_success()
                    && resp
                        .headers()
                        .get(axum::http::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|ct| ct.starts_with("image/"))
            }
            Err(e) => {
                tracing::debug!(error = %e, url = %url, "community mirror: thumbnail probe failed");
                false
            }
        };
        if is_image
            && sqlx::query(
                "UPDATE social.community_mirror SET thumbnail_url = $2 WHERE community_id = $1",
            )
            .bind(id)
            .bind(&url)
            .execute(pool)
            .await
            .is_ok()
        {
            found += 1;
        }
    }
    found
}

#[derive(Debug, PartialEq, Eq)]
pub enum SweepOutcome {
    Deactivated(Vec<Uuid>),
    Refused { candidates: i64, mirrored: i64 },
}

/// Deactivates the active mirrored communities last listed before `cutoff`,
/// keeping their local members, behind the volume fuse. Local communities
/// are never candidates.
pub async fn sweep_unlisted(pool: &PgPool, cutoff: NaiveDateTime) -> sqlx::Result<SweepOutcome> {
    let (candidates, mirrored): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE m.fetched_at < $1), count(*) \
         FROM social.community_mirror m JOIN social.communities c ON c.id = m.community_id \
         WHERE c.active AND c.origin = 'upstream'",
    )
    .bind(cutoff)
    .fetch_one(pool)
    .await?;
    let threshold = SWEEP_FUSE_MIN_CANDIDATES.max(mirrored / 5);
    if candidates > threshold {
        tracing::warn!(
            candidates,
            mirrored,
            threshold,
            "community mirror: sweep would deactivate too many communities; skipped"
        );
        return Ok(SweepOutcome::Refused {
            candidates,
            mirrored,
        });
    }
    let deactivated = sqlx::query_scalar(
        "UPDATE social.communities c SET active = false, updated_at = $2 \
         FROM social.community_mirror m \
         WHERE m.community_id = c.id AND m.fetched_at < $1 AND c.active AND c.origin = 'upstream' \
         RETURNING c.id",
    )
    .bind(cutoff)
    .bind(Utc::now().naive_utc())
    .fetch_all(pool)
    .await?;
    Ok(SweepOutcome::Deactivated(deactivated))
}

/// The upstream `id` is mirrored from (its provenance row's source), or
/// `None` for a local community or no community at all.
pub async fn mirrored_from(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<String>> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT c.origin, m.source_url FROM social.communities c \
         LEFT JOIN social.community_mirror m ON m.community_id = c.id WHERE c.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some((origin, source)) if origin == "upstream" => {
            Some(source.unwrap_or_else(|| "its upstream".to_string()))
        }
        _ => None,
    })
}

/// The 409 body of a refused write on a community mirrored from `upstream`.
pub fn mirrored_message(upstream: &str) -> String {
    format!(
        "This community is mirrored from {upstream} and is read-only on this node; manage it there"
    )
}

/// The community a write path names (`/v1/communities/{id}[/...]`), when
/// that segment is a uuid, and whether the write is a join or a leave
/// (`POST .../members`, `DELETE .../members/{address}`), the one local
/// interaction a mirrored community takes.
fn written_community(method: &Method, path: &str) -> Option<(Uuid, bool)> {
    let rest = path.strip_prefix("/v1/communities/")?;
    let mut segments = rest.split('/');
    let id = Uuid::parse_str(segments.next()?).ok()?;
    let tail: Vec<&str> = segments.collect();
    let membership = match (method, tail.as_slice()) {
        (&Method::POST, ["members"]) => true,
        (&Method::DELETE, ["members", address]) => !address.is_empty(),
        _ => false,
    };
    Some((id, membership))
}

/// Router middleware: a mutating request on a mirrored community's routes
/// (members, bans, places, posts, requests, the community itself; client or
/// federated envelope) answers 409 naming the upstream before any handler
/// runs, so no local write lands on data another node is the authority for;
/// a join or a leave passes, to the handlers that keep it a local member
/// row. A lookup fault answers 500, never lets the write through.
pub async fn refuse_mirrored_writes(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    let Some((id, membership)) = written_community(request.method(), request.uri().path()) else {
        return next.run(request).await;
    };
    if membership {
        return next.run(request).await;
    }
    match mirrored_from(&state.pool, id).await {
        Ok(None) => next.run(request).await,
        Ok(Some(upstream)) => err_json(StatusCode::CONFLICT, mirrored_message(&upstream)),
        Err(e) => {
            tracing::error!(error = %e, "community mirror: provenance lookup failed");
            err_json(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    }
}

/// Attaches the periodic, leased mirror job (`COMMUNITIES_MIRROR_UPSTREAM`):
/// the first pass right away (jittered), then every interval, backing off
/// to an hour after a failed pass.
/// A deactivated community closes the worlds listing it (`state`'s hook).
pub fn spawn_job(state: SharedState, client: Outbound, cfg: MirrorConfig, jobs: &Jobs) {
    let pool = state.pool.clone();
    tracing::info!(
        upstream = cfg.source.base(),
        interval_secs = cfg.interval.as_secs(),
        "communities: mirroring from upstream"
    );
    let periodic = PeriodicCfg::every(cfg.interval)
        .with_jitter(Duration::from_secs(30))
        .with_error_backoff(Duration::from_secs(3600));
    jobs.spawn_periodic_leased("communities-mirror", periodic, move |ctx| {
        let (client, pool, cfg, state) = (client.clone(), pool.clone(), cfg.clone(), state.clone());
        async move {
            let grace = cfg.interval.saturating_mul(2);
            let out = run_pass(&client, &pool, &cfg, grace, &ctx.cancel).await?;
            for id in &out.deactivated_ids {
                state.community_closed_to_worlds(crate::shared::identity::CommunityId::from_uuid(
                    *id,
                ));
            }
            ctx.progress(format!(
                "upserted {} deactivated {} failed_pages {} complete {}",
                out.upserted, out.deactivated, out.failed_pages, out.complete
            ));
            tracing::info!(?out, "communities mirrored from upstream");
            if !out.complete && !out.cancelled {
                anyhow::bail!(
                    "communities pass incomplete: {} failed pages",
                    out.failed_pages
                );
            }
            Ok(())
        }
    });
}

#[cfg(test)]
mod tests;
