//! What every mirror in this module shares: the persisted pass marker
//! (`explore_api.mirror_passes`: a pass is complete when `completed_at` is at
//! or after `started_at`, and `cursor` is what the next one resumes from),
//! the guarded upstream fetch, and the `GET /mirrors` report.

use std::future::Future;

use anyhow::Context as _;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::NaiveDateTime;
use reqwest::header::ETAG;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Settings, Shared};
use crate::jobs::JobContext;

pub struct PassOutcome {
    pub items: i64,
    pub cursor: Option<String>,
}

/// Runs one pass under its marker: `started_at` before the body, then either
/// `completed_at` + items + cursor, or `last_error` with the marker left
/// incomplete for the report and the next pass.
pub async fn run<F, Fut>(pool: &PgPool, name: &str, ctx: &JobContext, body: F) -> anyhow::Result<()>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<PassOutcome>>,
{
    sqlx::query(
        "INSERT INTO explore_api.mirror_passes (mirror, started_at)
         VALUES ($1, timezone('UTC', now()))
         ON CONFLICT (mirror) DO UPDATE SET started_at = EXCLUDED.started_at",
    )
    .bind(name)
    .execute(pool)
    .await
    .with_context(|| format!("{name}: recording the pass start"))?;

    finish(pool, name, ctx, body().await).await
}

/// Like [`run`], for a pass whose units of work carry their own
/// `checked_at` stamp: a pass that did not complete keeps its `started_at`
/// while it is younger than `resume_window`, so the next one resumes it (the
/// body receives that start and skips every unit stamped at or after it). A
/// complete pass, or an incomplete one older than the window, starts afresh
/// at `now`. `now` is the caller's clock, a parameter so tests can move it.
pub async fn run_resumable<F, Fut>(
    pool: &PgPool,
    name: &str,
    ctx: &JobContext,
    now: NaiveDateTime,
    resume_window: std::time::Duration,
    body: F,
) -> anyhow::Result<()>
where
    F: FnOnce(NaiveDateTime) -> Fut,
    Fut: Future<Output = anyhow::Result<PassOutcome>>,
{
    let now = now
        - chrono::Duration::nanoseconds(i64::from(now.and_utc().timestamp_subsec_nanos() % 1000));
    let window = chrono::Duration::from_std(resume_window).unwrap_or(chrono::Duration::MAX);
    let resume_after = now.checked_sub_signed(window).unwrap_or(NaiveDateTime::MIN);
    let (started_at,): (NaiveDateTime,) = sqlx::query_as(
        "INSERT INTO explore_api.mirror_passes AS p (mirror, started_at)
         VALUES ($1, $2)
         ON CONFLICT (mirror) DO UPDATE SET started_at = CASE
             WHEN p.started_at IS NOT NULL
              AND p.started_at >= $3
              AND (p.completed_at IS NULL OR p.completed_at < p.started_at)
             THEN p.started_at
             ELSE EXCLUDED.started_at END
         RETURNING started_at",
    )
    .bind(name)
    .bind(now)
    .bind(resume_after)
    .fetch_one(pool)
    .await
    .with_context(|| format!("{name}: recording the pass start"))?;
    if started_at < now {
        ctx.progress(format!("{name}: resuming the pass started at {started_at}"));
        tracing::info!(mirror = name, %started_at, "resuming an incomplete mirror pass");
    }
    finish(pool, name, ctx, body(started_at).await).await
}

async fn finish(
    pool: &PgPool,
    name: &str,
    ctx: &JobContext,
    result: anyhow::Result<PassOutcome>,
) -> anyhow::Result<()> {
    match result {
        Ok(outcome) => {
            sqlx::query(
                "UPDATE explore_api.mirror_passes
                 SET passes = passes + 1, items = $2, cursor = COALESCE($3, cursor),
                     completed_at = timezone('UTC', now()), last_error = NULL
                 WHERE mirror = $1",
            )
            .bind(name)
            .bind(outcome.items)
            .bind(outcome.cursor.as_deref())
            .execute(pool)
            .await
            .with_context(|| format!("{name}: recording the pass completion"))?;
            ctx.progress(format!(
                "{name}: complete pass, {} items, cursor {}",
                outcome.items,
                outcome.cursor.as_deref().unwrap_or("-")
            ));
            tracing::info!(mirror = name, items = outcome.items, cursor = ?outcome.cursor, "mirror pass complete");
            Ok(())
        }
        Err(err) => {
            let message = format!("{err:#}");
            sqlx::query("UPDATE explore_api.mirror_passes SET last_error = $2 WHERE mirror = $1")
                .bind(name)
                .bind(&message)
                .execute(pool)
                .await
                .with_context(|| format!("{name}: recording the pass failure"))?;
            Err(err)
        }
    }
}

/// What a conditional GET of one upstream object answered.
pub enum Conditional {
    /// 304 to the validators sent.
    NotModified,
    /// 404, 403 or 410: the upstream does not hold the object (an S3 bucket
    /// answers 403 for a missing key it may not list).
    Missing(reqwest::StatusCode),
    Fetched {
        body: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
        content_type: Option<String>,
    },
}

/// Where a conditional GET may go: through the SSRF guard (every upstream
/// by default), or, for an operator's explicit opt-in to a private upstream
/// (a render rig on the tailnet), unguarded on the shared pooled client, the
/// relay's `CATALYRST_RPC_ALLOW_PRIVATE_UPSTREAM` rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Public,
    PrivateAllowed,
}

/// A GET of one object through the SSRF guard (unless `reach` says the
/// operator allowed a private upstream) and the body cap, with the
/// validators of the copy held (`If-None-Match`, `If-Modified-Since`) when
/// there is one. Any status other than 2xx, 304 and the missing ones is an
/// error naming the host and path only.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_conditional(
    client: &crate::http::Outbound,
    settings: &Settings,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    cap: usize,
    timeout: std::time::Duration,
    reach: Reach,
) -> anyhow::Result<Conditional> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(e) = etag.and_then(|e| reqwest::header::HeaderValue::from_str(e).ok()) {
        headers.insert(reqwest::header::IF_NONE_MATCH, e);
    }
    if let Some(m) = last_modified.and_then(|m| reqwest::header::HeaderValue::from_str(m).ok()) {
        headers.insert(reqwest::header::IF_MODIFIED_SINCE, m);
    }
    let resp = match reach {
        Reach::Public => {
            let target = settings.guarded_url(url)?;
            client
                .get(target)
                .timeout(timeout)
                .headers(headers)
                .send()
                .await
                .with_context(|| format!("{}: request failed", shown(url)))?
        }
        Reach::PrivateAllowed => {
            let target = reqwest::Url::parse(url).context("source is not a URL")?;
            anyhow::ensure!(
                matches!(target.scheme(), "http" | "https"),
                "{}: only http(s) sources are allowed",
                shown(url)
            );
            client
                .raw()
                .get(target)
                .timeout(timeout)
                .headers(headers)
                .send()
                .await
                .map_err(|e| {
                    anyhow::anyhow!("{}: request failed: {}", shown(url), e.without_url())
                })?
        }
    };
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Conditional::NotModified);
    }
    if matches!(status.as_u16(), 403 | 404 | 410) {
        return Ok(Conditional::Missing(status));
    }
    if !status.is_success() {
        anyhow::bail!("{} answered {status}", shown(url));
    }
    let header = |name: reqwest::header::HeaderName| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let (etag, last_modified, content_type) = (
        header(ETAG),
        header(reqwest::header::LAST_MODIFIED),
        header(reqwest::header::CONTENT_TYPE),
    );
    let body = crate::http::read_body_capped(resp, cap)
        .await
        .with_context(|| format!("{}: reading the body", shown(url)))?;
    Ok(Conditional::Fetched {
        body,
        etag,
        last_modified,
        content_type,
    })
}

pub struct Fetched {
    pub status: reqwest::StatusCode,
    pub etag: Option<String>,
    pub body: Vec<u8>,
}

/// One upstream call: the URL through the SSRF guard, the body through the
/// cap. `Ok` for 2xx and for 304 (empty body); any other status is an error.
pub async fn fetch(
    client: &crate::http::Outbound,
    settings: &Settings,
    method: reqwest::Method,
    url: &str,
    json: Option<&Value>,
    headers: &[(&str, &str)],
    cap: usize,
) -> anyhow::Result<Fetched> {
    let target = settings.guarded_url(url)?;
    let mut req = client
        .request(method, target)
        .header("accept", "application/json");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    if let Some(body) = json {
        req = req.json(body);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("{}: request failed", shown(url)))?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Fetched {
            status,
            etag: None,
            body: Vec::new(),
        });
    }
    if !status.is_success() {
        anyhow::bail!("{} answered {status}", shown(url));
    }
    let etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = crate::http::read_body_capped(resp, cap)
        .await
        .with_context(|| format!("{}: reading the body", shown(url)))?;
    Ok(Fetched { status, etag, body })
}

/// The form of an upstream URL that error texts carry: host and path, no
/// scheme, credentials or query. The texts land in `last_error` on
/// `/mirrors` and in logs; the full URL is configuration.
pub fn shown(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(u) => format!("{}{}", u.host_str().unwrap_or("?"), u.path()),
        Err(_) => "<unparseable url>".to_string(),
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PassRow {
    pub mirror: String,
    pub passes: i64,
    pub items: i64,
    pub cursor: Option<String>,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassReport {
    #[serde(flatten)]
    pub row: PassRow,
    /// `completed_at` at or after `started_at`: the last pass ran to the end.
    pub complete: bool,
    /// Seconds since the last complete pass; `None` before the first.
    pub age_secs: Option<i64>,
}

pub async fn report(pool: &PgPool) -> sqlx::Result<Vec<PassReport>> {
    let rows: Vec<PassRow> = sqlx::query_as(
        "SELECT mirror, passes, items, cursor, started_at, completed_at, last_error
         FROM explore_api.mirror_passes ORDER BY mirror",
    )
    .fetch_all(pool)
    .await?;
    let now = chrono::Utc::now().naive_utc();
    Ok(rows
        .into_iter()
        .map(|row| {
            let complete = match (row.started_at, row.completed_at) {
                (Some(started), Some(completed)) => completed >= started,
                _ => false,
            };
            let age_secs = row.completed_at.map(|c| (now - c).num_seconds());
            PassReport {
                row,
                complete,
                age_secs,
            }
        })
        .collect())
}

/// `GET /mirrors`: the pass markers, plus the in-memory grid stamp the map
/// routes serve from.
pub async fn status(State(state): State<Shared>) -> Response {
    match report(&state.pool).await {
        Ok(mirrors) => (
            StatusCode::OK,
            [("cache-control", "no-store")],
            Json(json!({
                "mirrors": mirrors,
                "mapGridLastUpdatedAt": state.map.last_updated_at(),
                "mapGridReady": state.map.is_ready(),
                "satelliteRegions": state.satellite.region_count(),
                "satelliteGeneration": state.satellite.generation(),
                "worldManifest": state.world_manifest.snapshot().map(|h| json!({
                    "blob": h.cid, "bytes": h.body.len(), "occupied": h.counts.occupied,
                    "roads": h.counts.roads, "empty": h.counts.empty, "fetchedAt": h.fetched_at,
                })),
            })),
        )
            .into_response(),
        Err(err) => super::internal_error("reading explore_api.mirror_passes", err),
    }
}
