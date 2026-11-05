//! The worlds mirror: one pass is the catalogue (`GET {source}/index`, one
//! document listing every world and its scene ids) written to
//! `worlds.remote_worlds`, then the scenes/files walk over the catalogue in
//! name order: `GET {source}/world/{name}/about` for each world whose
//! advertised scene set differs from what is held, the scene entities and
//! every file they reference into the ONE blob store (hash-verified, so a
//! lying upstream cannot plant bytes under a CID), and the `worlds.worlds` +
//! `worlds.world_scenes` rows in one transaction per world.
//!
//! Resumable: the scenes walk persists its cursor (the last world name
//! completed) after every batch in `worlds.mirror_passes`, so a restart
//! continues there; a pass is complete when the cursor is cleared and
//! `pass_completed_at` stamped with the pass counts. Local authority: a world
//! holding a scene with `mirror_source IS NULL` (a local publish) is refused,
//! never overwritten (the write re-checks under the world row's lock, so a
//! publish landing mid-download wins too), and only scenes the mirror wrote
//! are ever retired: those of a refused world at once, since the mirror never
//! refreshes them again.
//!
//! A read-through fill ([`mirror_one`]) takes the same path with [`Verify`]: it also asks
//! `{source}/world/{name}/scenes?entity_id=<id>` for each listed scene's deployment auth chain
//! (one bounded body, the chain at most [`MAX_CHAIN_BYTES`]), and stores nothing until
//! every scene is signed (ECDSA links only, as the upstream worlds server accepts) by an address
//! the policy says may have published the world, or (`WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS`,
//! on the upstream's word) one its `/permissions` lists as a deployer of a world whose owner it
//! names as this node's NAME authority does; its rows carry that chain and signer. Of the
//! files it stores only the scene host's (the main code file and `main.crdt`): the rest are read
//! through, hash-checked, from the same source when first asked for. It also leaves alone a world
//! this node keeps itself: a `worlds.worlds` row with no `mirror_source` (a local world, scenes or
//! not) or a `blocked_since`. The scheduled pass keeps the
//! catalogue's trust: its rows carry no chain and the zero address.
//! Deletion: a complete catalogue pass that no longer lists a world stamps
//! `unlisted_at`; the next scenes pass retires its mirrored scenes. A pass
//! that would unlist more than max(50, 20%) of the catalogue is refused, so
//! a truncated-but-well-formed index cannot empty the mirror.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tokio::sync::Semaphore;

use super::ZERO_ADDRESS;
use crate::auth::{AuthChain, is_canonical_cid, verify_auth_chain, verify_hash};
use crate::comms::world_identity::WorldName;
use crate::content::read_through::{DownloadSlot, Downloads};
use crate::explore::places::scene_identity;
use crate::http::{is_safe_http_url, read_body_capped};
use crate::jobs::JobContext;
use crate::policy::Policy;
use crate::shared::identity::{Address, PermissionDecision};
use catalyst_storage::Storage;

/// The upstream index measured 4.3 MB for 1,667 worlds (2026-09-24); the
/// cap leaves an order of magnitude of growth before a pass is refused.
const MAX_INDEX_BYTES: usize = 64 * 1024 * 1024;
const MAX_ABOUT_BYTES: usize = 1024 * 1024;
const MAX_BLOB_BYTES: usize = 256 * 1024 * 1024;
/// One `/scenes?entity_id=` answer: a single scene's row, its entity included.
const MAX_SCENE_ROW_BYTES: usize = 4 * 1024 * 1024;
/// A deployment auth chain as JSON; a real one is a few links of a few hundred bytes.
const MAX_CHAIN_BYTES: usize = 16 * 1024;
/// One `/world/{name}/permissions` answer: the owner and a few allow-lists of addresses.
const MAX_PERMISSIONS_BYTES: usize = 256 * 1024;
const SWEEP_FUSE_MIN_CANDIDATES: i64 = 50;

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(6 * 3600);
pub const DEFAULT_CONCURRENCY: usize = 4;
pub const DEFAULT_BLOB_CONCURRENCY: usize = 8;

/// The validated `WORLDS_CONTENT_MIRROR_*` configuration.
#[derive(Clone, Debug)]
pub struct MirrorSettings {
    /// The upstream worlds content server, no trailing slash.
    pub source: String,
    /// Pause between passes; the v1 stack's timer ran every six hours.
    pub interval: Duration,
    /// Worlds mirrored at once (one batch; the cursor moves per batch).
    pub concurrency: usize,
    /// Files fetched at once per scene.
    pub blob_concurrency: usize,
    /// A bounded run: only the first N catalogue names (name order).
    pub limit: Option<usize>,
    /// Only these worlds (lowercased); empty means the whole catalogue.
    pub names: Vec<String>,
    /// This node's own origins: a source or a scene base URL that names one
    /// of them is a mirror loop.
    own_origins: Vec<String>,
    /// Tests only (`for_tests`): a loopback stub may stand in for the
    /// upstream. `parse` never sets it.
    allow_private_source: bool,
    /// `WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS`: a read-through fill also accepts a scene
    /// signed by a deployer the upstream lists as granted ([`Signatures::upstream_grant`]).
    pub trust_upstream_grants: bool,
    /// `WORLDS_CONTENT_MIRROR_ENABLED`: the periodic pass runs. Off, the source only serves
    /// the read-through ([`crate::WorldsState::fill_world`]): a world is mirrored when read.
    pub scheduled: bool,
}

fn origin_of(url: &url::Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url.port_or_known_default()?;
    Some(format!("{}://{host}:{port}", url.scheme()))
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")
}

impl MirrorSettings {
    /// The mirror `serve` runs: the periodic pass with `WORLDS_CONTENT_MIRROR_ENABLED`, else
    /// the read-through alone when `READ_THROUGH_ENABLED` and a `WORLDS_CONTENT_MIRROR_SOURCE`
    /// are set (worlds are mirrored one at a time as they are read, none crawled); `None`
    /// otherwise.
    pub fn from_config(config: &crate::config::Config, own_urls: &[String]) -> Result<Option<Self>> {
        let worlds = &config.worlds_content;
        let scheduled = worlds.worlds_content_mirror_enabled;
        if !scheduled
            && !(config.content.read_through_enabled
                && !worlds.worlds_content_mirror_source.trim().is_empty())
        {
            return Ok(None);
        }
        let mut settings = Self::parse(
            &worlds.worlds_content_mirror_source,
            own_urls,
            Duration::from_secs(worlds.worlds_content_mirror_interval_secs),
            worlds.worlds_content_mirror_concurrency,
            worlds.worlds_content_mirror_blob_concurrency,
            worlds.worlds_content_mirror_limit,
            &worlds.worlds_content_mirror_names,
        )
        .context("WORLDS_CONTENT_MIRROR_* configuration")?;
        settings.scheduled = scheduled;
        settings.trust_upstream_grants = worlds.worlds_read_through_trust_upstream_grants;
        Ok(Some(settings))
    }

    /// `WORLDS_CONTENT_MIRROR_SOURCE` must be an https:// URL to a publicly
    /// routable host (the SSRF guard of [`crate::http`]; plaintext would let
    /// a MITM plant worlds and files) and must not name this node
    /// (`own_urls`: PUBLIC_URL, CONTENT_URL, the bind address).
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        source: &str,
        own_urls: &[String],
        interval: Duration,
        concurrency: usize,
        blob_concurrency: usize,
        limit: usize,
        names: &str,
    ) -> Result<Self> {
        Self::parse_with(
            source,
            own_urls,
            interval,
            concurrency,
            blob_concurrency,
            limit,
            names,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_with(
        source: &str,
        own_urls: &[String],
        interval: Duration,
        concurrency: usize,
        blob_concurrency: usize,
        limit: usize,
        names: &str,
        allow_private_source: bool,
    ) -> Result<Self> {
        let source = source.trim().trim_end_matches('/').to_string();
        if source.is_empty() {
            bail!(
                "WORLDS_CONTENT_MIRROR_SOURCE is empty: the mirror has nothing to do without an upstream"
            );
        }
        let parsed = url::Url::parse(&source).map_err(|e| {
            anyhow!("WORLDS_CONTENT_MIRROR_SOURCE {source:?} is not a valid URL: {e}")
        })?;
        let host = parsed.host_str().unwrap_or("");
        match parsed.scheme() {
            "https" => {}
            "http" if allow_private_source && is_loopback_host(host) => {}
            "http" => bail!(
                "WORLDS_CONTENT_MIRROR_SOURCE is http:// to {host}: a plaintext upstream lets a MITM plant \
                 worlds and files; use https://"
            ),
            other => bail!("WORLDS_CONTENT_MIRROR_SOURCE scheme {other}:// is not http(s)"),
        }
        if !allow_private_source && !is_safe_http_url(&source) {
            bail!(
                "WORLDS_CONTENT_MIRROR_SOURCE host {host} is refused by the SSRF guard (loopback, private, \
                 internal or single-label); a mirror source is an explicit public upstream, never a local URL"
            );
        }
        let own_origins: Vec<String> = own_urls
            .iter()
            .filter_map(|u| url::Url::parse(u.trim()).ok())
            .filter_map(|u| origin_of(&u))
            .collect();
        let source_origin = origin_of(&parsed).unwrap_or_default();
        if own_origins.contains(&source_origin) {
            bail!(
                "WORLDS_CONTENT_MIRROR_SOURCE {source} is this node itself; a node cannot mirror worlds from its own routes"
            );
        }
        if concurrency == 0 || blob_concurrency == 0 {
            bail!(
                "WORLDS_CONTENT_MIRROR_CONCURRENCY and WORLDS_CONTENT_MIRROR_BLOB_CONCURRENCY must be at least 1"
            );
        }
        if interval.is_zero() {
            bail!("WORLDS_CONTENT_MIRROR_INTERVAL_SECS must be at least 1");
        }
        let names: Vec<String> = names
            .split(',')
            .map(|n| n.trim().to_ascii_lowercase())
            .filter(|n| !n.is_empty())
            .collect();
        Ok(Self {
            source,
            interval,
            concurrency,
            blob_concurrency,
            limit: (limit > 0).then_some(limit),
            names,
            own_origins,
            allow_private_source,
            trust_upstream_grants: false,
            scheduled: true,
        })
    }

    /// A loopback stub as the upstream, unguarded; the guard itself is
    /// covered by `parse`'s tests.
    pub fn for_tests(source: &str) -> Self {
        Self::parse_with(source, &[], Duration::from_secs(60), 2, 4, 0, "", true).unwrap()
    }

    /// Tests only: the node's own URLs (`PUBLIC_URL`, ...) for the loop checks.
    pub fn with_own_urls(mut self, own_urls: &[String]) -> Self {
        self.own_origins = own_urls
            .iter()
            .filter_map(|u| url::Url::parse(u.trim()).ok())
            .filter_map(|u| origin_of(&u))
            .collect();
        self
    }

    pub fn allows_private_source(&self) -> bool {
        self.allow_private_source
    }

    /// Tests only: the read-through alone, as `from_config` builds it without
    /// `WORLDS_CONTENT_MIRROR_ENABLED`.
    pub fn read_through_only(mut self) -> Self {
        self.scheduled = false;
        self
    }

    /// Tests only: `WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS`.
    pub fn trusting_upstream_grants(mut self, trust: bool) -> Self {
        self.trust_upstream_grants = trust;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = (limit > 0).then_some(limit);
        self
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// A scene's `baseUrl` (from the upstream's /about) is fetched only when
    /// it is a publicly routable http(s) URL that is not this node: the
    /// upstream chooses it, so it gets the SSRF guard (the socket-level half
    /// runs in [`crate::http::Outbound`] at fetch time). A test stub's own
    /// origin passes only under `for_tests`.
    pub fn base_url_allowed(&self, base: &str) -> bool {
        let Ok(parsed) = url::Url::parse(base) else {
            return false;
        };
        let Some(origin) = origin_of(&parsed) else {
            return false;
        };
        if self.own_origins.contains(&origin) {
            return false;
        }
        if self.allow_private_source {
            let source_origin = url::Url::parse(&self.source)
                .ok()
                .and_then(|u| origin_of(&u));
            if source_origin.as_deref() == Some(origin.as_str()) {
                return true;
            }
        }
        is_safe_http_url(base)
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct CatalogueOutcome {
    pub listed: usize,
    pub unlisted: usize,
    pub complete: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScenesOutcome {
    pub processed: usize,
    pub synced: usize,
    pub unchanged: usize,
    pub refused: usize,
    pub skipped: usize,
    pub failed: usize,
    pub retired_scenes: u64,
    pub scenes: usize,
    pub new_blobs: usize,
    pub resumed_from: Option<String>,
    pub interrupted: bool,
    pub complete: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct PassSummary {
    pub catalogue: CatalogueOutcome,
    pub scenes: ScenesOutcome,
}

/// One full pass: catalogue, then the scenes walk (resumed if a cursor is
/// persisted). A catalogue failure is the pass's error; the scenes walk
/// records per-world failures and completes regardless.
pub async fn run_pass(
    client: &crate::http::Outbound,
    pool: &PgPool,
    storage: &Arc<Storage>,
    settings: &MirrorSettings,
    ctx: &JobContext,
) -> Result<PassSummary> {
    let catalogue = match catalogue_pass(client, pool, settings, ctx).await {
        Ok(out) => out,
        Err(e) => {
            record_error(pool, "catalogue", &settings.source, &format!("{e:#}")).await?;
            return Err(e);
        }
    };
    if ctx.is_cancelled() {
        return Ok(PassSummary {
            catalogue,
            scenes: ScenesOutcome {
                interrupted: true,
                ..Default::default()
            },
        });
    }
    let scenes = match scenes_pass(client, pool, storage, settings, ctx).await {
        Ok(out) => out,
        Err(e) => {
            record_error(pool, "scenes", &settings.source, &format!("{e:#}")).await?;
            return Err(e);
        }
    };
    Ok(PassSummary { catalogue, scenes })
}

async fn record_error(pool: &PgPool, mirror: &str, source: &str, error: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO worlds.mirror_passes (mirror, source, last_attempt_at, last_error)
           VALUES ($1, $2, timezone('UTC', now()), $3)
           ON CONFLICT (mirror) DO UPDATE
             SET last_attempt_at = EXCLUDED.last_attempt_at, last_error = EXCLUDED.last_error"#,
    )
    .bind(mirror)
    .bind(source)
    .bind(error)
    .execute(pool)
    .await
    .context("recording the mirror error")?;
    Ok(())
}

struct CatalogueEntry {
    name: String,
    scene_ids: Vec<String>,
    newest_timestamp_ms: Option<i64>,
}

/// The worlds an upstream `/index` document lists with at least one scene,
/// lowercased and deduplicated (name order).
fn parse_index(index: &Value) -> Vec<CatalogueEntry> {
    let mut seen = HashSet::new();
    let mut entries: Vec<CatalogueEntry> = index["data"]
        .as_array()
        .map(|worlds| {
            worlds
                .iter()
                .filter_map(|w| {
                    let name = w["name"].as_str()?.trim().to_ascii_lowercase();
                    let scenes: Vec<&Value> = w["scenes"]
                        .as_array()?
                        .iter()
                        .filter(|s| s["id"].as_str().is_some_and(is_canonical_cid))
                        .collect();
                    let scene_ids: Vec<String> = scenes
                        .iter()
                        .filter_map(|s| s["id"].as_str())
                        .map(str::to_string)
                        .collect();
                    if name.is_empty() || scene_ids.is_empty() || !seen.insert(name.clone()) {
                        return None;
                    }
                    let newest_timestamp_ms =
                        scenes.iter().filter_map(|s| s["timestamp"].as_i64()).max();
                    Some(CatalogueEntry {
                        name,
                        scene_ids,
                        newest_timestamp_ms,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

async fn catalogue_pass(
    client: &crate::http::Outbound,
    pool: &PgPool,
    settings: &MirrorSettings,
    ctx: &JobContext,
) -> Result<CatalogueOutcome> {
    ctx.progress("catalogue: fetching /index");
    let url = format!("{}/index", settings.source);
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("GET {url} answered {}", resp.status());
    }
    let body = read_body_capped(resp, MAX_INDEX_BYTES)
        .await
        .with_context(|| format!("reading {url}"))?;
    let index: Value = serde_json::from_slice(&body).with_context(|| format!("parsing {url}"))?;
    let entries = parse_index(&index);
    if entries.is_empty() {
        bail!("{url} lists no world with scenes; refusing to treat an empty catalogue as complete");
    }

    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    let scene_ids: Vec<Value> = entries
        .iter()
        .map(|e| Value::from(e.scene_ids.clone()))
        .collect();
    let newest: Vec<Option<i64>> = entries.iter().map(|e| e.newest_timestamp_ms).collect();

    let mut tx = pool.begin().await?;
    let previously_listed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM worlds.remote_worlds WHERE unlisted_at IS NULL")
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        r#"INSERT INTO worlds.remote_worlds (world_name, source, scene_ids, newest_timestamp_ms)
           SELECT t.name, $4, ARRAY(SELECT jsonb_array_elements_text(t.scene_ids)), t.newest
           FROM unnest($1::text[], $2::jsonb[], $3::bigint[]) AS t(name, scene_ids, newest)
           ON CONFLICT (world_name) DO UPDATE
             SET source = EXCLUDED.source,
                 scene_ids = EXCLUDED.scene_ids,
                 newest_timestamp_ms = EXCLUDED.newest_timestamp_ms,
                 listed_at = timezone('UTC', now()),
                 unlisted_at = NULL"#,
    )
    .bind(&names)
    .bind(&scene_ids)
    .bind(&newest)
    .bind(&settings.source)
    .execute(&mut *tx)
    .await
    .context("upserting the catalogue")?;

    let candidates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM worlds.remote_worlds WHERE unlisted_at IS NULL AND NOT (world_name = ANY($1))",
    )
    .bind(&names)
    .fetch_one(&mut *tx)
    .await?;
    let fuse = SWEEP_FUSE_MIN_CANDIDATES.max(previously_listed / 5);
    let (unlisted, complete, error) = if candidates > fuse {
        tracing::warn!(
            candidates,
            previously_listed,
            fuse,
            "catalogue pass would unlist more worlds than the fuse allows; keeping them listed"
        );
        (
            0,
            false,
            Some(format!(
                "refused to unlist {candidates} of {previously_listed} worlds (fuse {fuse})"
            )),
        )
    } else {
        let n = sqlx::query(
            r#"UPDATE worlds.remote_worlds SET unlisted_at = timezone('UTC', now())
               WHERE unlisted_at IS NULL AND NOT (world_name = ANY($1))"#,
        )
        .bind(&names)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        (n as usize, true, None)
    };
    let outcome = CatalogueOutcome {
        listed: entries.len(),
        unlisted,
        complete,
    };
    sqlx::query(
        r#"INSERT INTO worlds.mirror_passes
             (mirror, source, pass_number, pass_started_at, pass_completed_at, last_attempt_at, cursor, stats, last_error)
           VALUES ('catalogue', $1, 1, timezone('UTC', now()),
                   CASE WHEN $2 THEN timezone('UTC', now()) END,
                   timezone('UTC', now()), NULL, $3, $4)
           ON CONFLICT (mirror) DO UPDATE
             SET source = EXCLUDED.source,
                 pass_number = worlds.mirror_passes.pass_number + CASE WHEN $2 THEN 1 ELSE 0 END,
                 pass_started_at = EXCLUDED.pass_started_at,
                 pass_completed_at = COALESCE(EXCLUDED.pass_completed_at, worlds.mirror_passes.pass_completed_at),
                 last_attempt_at = EXCLUDED.last_attempt_at,
                 stats = EXCLUDED.stats,
                 last_error = EXCLUDED.last_error"#,
    )
    .bind(&settings.source)
    .bind(complete)
    .bind(serde_json::to_value(&outcome)?)
    .bind(&error)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(
        listed = outcome.listed,
        unlisted,
        complete,
        "worlds catalogue mirrored"
    );
    Ok(outcome)
}

/// A world this run did not mirror is not automatically a failure: refusing
/// to overwrite a local publish is the mirror working, an unreachable
/// upstream is the mirror broken. A refusal counts the mirrored scenes it
/// retired from beside the local publish.
enum Outcome {
    Synced {
        scenes: usize,
        new_blobs: usize,
    },
    Unchanged,
    /// A local publish, or on a read-through a local or blocked world.
    RefusedLocalPublish {
        retired: u64,
    },
    NothingUpstream(&'static str),
}

async fn scenes_pass(
    client: &crate::http::Outbound,
    pool: &PgPool,
    storage: &Arc<Storage>,
    settings: &MirrorSettings,
    ctx: &JobContext,
) -> Result<ScenesOutcome> {
    let mut out = ScenesOutcome::default();

    let resume: Option<(Option<String>, Value)> =
        sqlx::query("SELECT cursor, stats FROM worlds.mirror_passes WHERE mirror = 'scenes'")
            .fetch_optional(pool)
            .await?
            .map(|r| (r.get("cursor"), r.get("stats")));
    let mut cursor: String = match resume {
        Some((Some(cursor), stats)) => {
            if let Ok(prev) = serde_json::from_value::<ScenesOutcome>(stats) {
                out = ScenesOutcome {
                    interrupted: false,
                    complete: false,
                    ..prev
                };
            }
            out.resumed_from = Some(cursor.clone());
            tracing::info!(
                cursor,
                "resuming the worlds scenes pass from its persisted cursor"
            );
            cursor
        }
        _ => {
            sqlx::query(
                r#"INSERT INTO worlds.mirror_passes (mirror, source, pass_started_at, last_attempt_at, cursor, stats)
                   VALUES ('scenes', $1, timezone('UTC', now()), timezone('UTC', now()), '', '{}'::jsonb)
                   ON CONFLICT (mirror) DO UPDATE
                     SET source = EXCLUDED.source, pass_started_at = EXCLUDED.pass_started_at,
                         last_attempt_at = EXCLUDED.last_attempt_at, cursor = '', stats = '{}'::jsonb,
                         last_error = NULL"#,
            )
            .bind(&settings.source)
            .execute(pool)
            .await?;
            out.retired_scenes = retire_unlisted(pool).await?;
            String::new()
        }
    };

    let total: i64 = due_total(pool, settings).await?;
    loop {
        if ctx.is_cancelled() {
            out.interrupted = true;
            break;
        }
        let batch = due_batch(pool, settings, &cursor).await?;
        if batch.is_empty() {
            break;
        }
        ctx.progress(format!(
            "scenes: {}/{total} worlds, at {:?}",
            out.processed, batch[0].0
        ));
        let mut set = tokio::task::JoinSet::new();
        for (name, scene_ids) in &batch {
            let (client, pool, storage, settings) = (
                client.clone(),
                pool.clone(),
                storage.clone(),
                settings.clone(),
            );
            let (name, scene_ids) = (name.clone(), scene_ids.clone());
            set.spawn(async move {
                let result =
                    mirror_world(&client, &pool, &storage, &settings, &name, &scene_ids, None)
                        .await;
                (name, result)
            });
        }
        let mut results = Vec::with_capacity(batch.len());
        let cancelled = loop {
            tokio::select! {
                joined = set.join_next() => match joined {
                    Some(Ok(r)) => results.push(r),
                    Some(Err(e)) => results.push((String::new(), Err(anyhow!("mirror task: {e}")))),
                    None => break false,
                },
                _ = ctx.cancel.cancelled() => break true,
            }
        };
        if cancelled {
            set.abort_all();
            out.interrupted = true;
            break;
        }
        for (name, result) in results {
            out.processed += 1;
            match result {
                Ok(Outcome::Synced { scenes, new_blobs }) => {
                    out.synced += 1;
                    out.scenes += scenes;
                    out.new_blobs += new_blobs;
                    mark_world(pool, &name, None).await?;
                }
                Ok(Outcome::Unchanged) => {
                    out.unchanged += 1;
                    mark_world(pool, &name, None).await?;
                }
                Ok(Outcome::RefusedLocalPublish { retired }) => {
                    out.refused += 1;
                    out.retired_scenes += retired;
                    tracing::warn!(world = %name, "refused: published to this node; the mirror never replaces a local publish");
                    mark_world(pool, &name, Some("published locally; not mirrored")).await?;
                }
                Ok(Outcome::NothingUpstream(why)) => {
                    out.skipped += 1;
                    tracing::info!(world = %name, why, "skipped");
                    mark_world(pool, &name, Some(why)).await?;
                }
                Err(e) => {
                    out.failed += 1;
                    let error = format!("{e:#}");
                    // The same error as the last pass (an upstream that keeps serving a
                    // body that does not hash, say) is one WARN, then INFO per pass while it
                    // lasts; the row keeps the error either way (`last_error`).
                    let repeated = !name.is_empty()
                        && stored_error(pool, &name).await?.as_deref() == Some(error.as_str());
                    if repeated {
                        tracing::info!(world = %name, error = %error, "world mirror still failing with the error of the last pass");
                    } else {
                        tracing::warn!(world = %name, error = %error, "world mirror failed");
                    }
                    if !name.is_empty() {
                        mark_world(pool, &name, Some(&error)).await?;
                    }
                }
            }
        }
        cursor = batch.last().map(|(n, _)| n.clone()).unwrap_or(cursor);
        sqlx::query(
            r#"UPDATE worlds.mirror_passes
               SET cursor = $1, stats = $2, last_attempt_at = timezone('UTC', now())
               WHERE mirror = 'scenes'"#,
        )
        .bind(&cursor)
        .bind(serde_json::to_value(&out)?)
        .execute(pool)
        .await
        .context("persisting the scenes cursor")?;
    }

    if out.synced + out.failed > 0 || out.retired_scenes > 0 {
        super::index::invalidate_index_memo();
    }
    if !out.interrupted {
        out.complete = true;
        sqlx::query(
            r#"UPDATE worlds.mirror_passes
               SET cursor = NULL, pass_number = pass_number + 1,
                   pass_completed_at = timezone('UTC', now()),
                   last_attempt_at = timezone('UTC', now()),
                   stats = $1, last_error = NULL
               WHERE mirror = 'scenes'"#,
        )
        .bind(serde_json::to_value(&out)?)
        .execute(pool)
        .await?;
        ctx.progress(format!(
            "scenes pass complete: {} worlds, {} synced, {} unchanged, {} refused, {} failed",
            out.processed, out.synced, out.unchanged, out.refused, out.failed
        ));
    } else {
        tracing::info!(
            cursor,
            processed = out.processed,
            "worlds scenes pass interrupted; cursor persisted"
        );
    }
    Ok(out)
}

/// The scope of a pass: listed worlds, optionally only `names`, optionally
/// only the first `limit` in name order (a bounded, restart-stable run).
async fn due_total(pool: &PgPool, settings: &MirrorSettings) -> Result<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM (
             SELECT world_name FROM worlds.remote_worlds
             WHERE unlisted_at IS NULL
               AND (cardinality($1::text[]) = 0 OR world_name = ANY($1))
             ORDER BY world_name
             LIMIT $2) s"#,
    )
    .bind(&settings.names)
    .bind(settings.limit.map(|l| l as i64).unwrap_or(i64::MAX))
    .fetch_one(pool)
    .await?;
    Ok(n)
}

async fn due_batch(
    pool: &PgPool,
    settings: &MirrorSettings,
    cursor: &str,
) -> Result<Vec<(String, Vec<String>)>> {
    let rows = sqlx::query(
        r#"SELECT world_name, scene_ids FROM (
             SELECT world_name, scene_ids FROM worlds.remote_worlds
             WHERE unlisted_at IS NULL
               AND (cardinality($1::text[]) = 0 OR world_name = ANY($1))
             ORDER BY world_name
             LIMIT $2) s
           WHERE world_name > $3
           ORDER BY world_name
           LIMIT $4"#,
    )
    .bind(&settings.names)
    .bind(settings.limit.map(|l| l as i64).unwrap_or(i64::MAX))
    .bind(cursor)
    .bind(settings.concurrency as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get("world_name"), r.get("scene_ids")))
        .collect())
}

/// The error the last pass stored for a world, if any (`remote_worlds.last_error`).
async fn stored_error(pool: &PgPool, name: &str) -> Result<Option<String>> {
    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT last_error FROM worlds.remote_worlds WHERE world_name = $1")
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(stored.flatten())
}

async fn mark_world(pool: &PgPool, name: &str, error: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"UPDATE worlds.remote_worlds
           SET mirrored_at = CASE WHEN $2::text IS NULL THEN timezone('UTC', now()) ELSE mirrored_at END,
               last_error = $2
           WHERE world_name = $1"#,
    )
    .bind(name)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Retires what the mirror wrote for worlds a complete catalogue pass no
/// longer lists: their mirrored scenes, their world row when nothing local
/// remains, and the catalogue row. Local scenes are never touched; blobs
/// stay in the store (content-addressed, possibly shared).
async fn retire_unlisted(pool: &PgPool) -> Result<u64> {
    let mut tx = pool.begin().await?;
    let candidates: Vec<String> = sqlx::query_scalar(
        "SELECT world_name FROM worlds.remote_worlds WHERE unlisted_at IS NOT NULL",
    )
    .fetch_all(&mut *tx)
    .await?;
    // Match publish/mirror lock order before touching scenes or shared places.
    // Freeze the candidate set so a later catalogue change cannot add an
    // unlocked world to any of the deletes below.
    let locked: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM worlds.worlds WHERE name = ANY($1) ORDER BY name FOR UPDATE",
    )
    .bind(&candidates)
    .fetch_all(&mut *tx)
    .await?;
    let retired: Vec<String> = sqlx::query_scalar(
        r#"DELETE FROM worlds.world_scenes s
           USING worlds.remote_worlds r
           WHERE r.world_name = ANY($1) AND r.unlisted_at IS NOT NULL AND lower(s.world_name) = r.world_name
             AND s.mirror_source IS NOT NULL
           RETURNING lower(s.world_name)"#,
    )
    .bind(&locked)
    .fetch_all(&mut *tx)
    .await?;
    let scenes = retired.len() as u64;
    scene_identity::retire_orphans(&mut tx, &retired).await?;
    sqlx::query(
        r#"DELETE FROM worlds.worlds w
           USING worlds.remote_worlds r
           WHERE r.world_name = ANY($1) AND r.unlisted_at IS NOT NULL AND lower(w.name) = r.world_name
             AND w.mirror_source IS NOT NULL
             AND NOT EXISTS (SELECT 1 FROM worlds.world_scenes s WHERE s.world_name = w.name)"#,
    )
    .bind(&locked)
    .execute(&mut *tx)
    .await?;
    let worlds = sqlx::query(
        "DELETE FROM worlds.remote_worlds r WHERE world_name = ANY($1) AND unlisted_at IS NOT NULL AND (world_name = ANY($2) OR NOT EXISTS (SELECT 1 FROM worlds.worlds w WHERE w.name = r.world_name))",
    )
    .bind(&candidates)
    .bind(&locked)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    if worlds > 0 {
        tracing::info!(
            worlds,
            scenes,
            "retired worlds the catalogue no longer lists"
        );
    }
    Ok(scenes)
}

/// A world with a local publish holds no mirrored scene: the mirror never
/// refreshes it again, so what it wrote there before is retired and the world
/// row says local, as a publish leaves it (core::store::deploy_scene).
async fn retire_beside_local(pool: &PgPool, name: &str) -> Result<u64> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT name FROM worlds.worlds WHERE name = lower($1) FOR UPDATE")
        .bind(name)
        .execute(&mut *tx)
        .await?;
    let retired = sqlx::query(
        r#"DELETE FROM worlds.world_scenes
           WHERE world_name = lower($1) AND mirror_source IS NOT NULL
             AND EXISTS (SELECT 1 FROM worlds.world_scenes l
                         WHERE l.world_name = lower($1) AND l.mirror_source IS NULL)"#,
    )
    .bind(name)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    scene_identity::retire_orphans(&mut tx, &[name.to_lowercase()]).await?;
    sqlx::query(
        r#"UPDATE worlds.worlds SET mirror_source = NULL, updated_at = timezone('UTC', now())
           WHERE name = lower($1) AND mirror_source IS NOT NULL
             AND EXISTS (SELECT 1 FROM worlds.world_scenes l
                         WHERE l.world_name = lower($1) AND l.mirror_source IS NULL)"#,
    )
    .bind(name)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if retired > 0 {
        tracing::info!(world = %name, retired, "retired the mirrored scenes beside a local publish");
    }
    Ok(retired)
}

/// What a read-through fill holds a mirrored world to beyond a pass's hash checks: every scene
/// signed by an address [`Policy::may_have_published`] allows, every download under the bound
/// the content read-through shares.
pub struct Verify<'a> {
    pub policy: &'a Policy,
    pub downloads: &'a Downloads,
}

/// Read-through: one world this node holds no scene of, mirrored now through the single-world
/// path a pass uses (every file hash-verified, a local publish never overwritten) with every
/// scene's signature and signer checked before anything is stored. `true` when scenes landed.
pub async fn mirror_one(
    client: &crate::http::Outbound,
    pool: &PgPool,
    storage: &Arc<Storage>,
    settings: &MirrorSettings,
    name: &str,
    verify: &Verify<'_>,
) -> Result<bool> {
    let synced = matches!(
        mirror_world(client, pool, storage, settings, name, &[], Some(verify)).await?,
        Outcome::Synced { .. }
    );
    if synced {
        super::index::invalidate_index_memo();
    }
    Ok(synced)
}

/// `{source}/world/{name}/{tail}` with the name as one percent-encoded path segment, so no
/// name can add a segment, a query or a fragment to the upstream request.
fn world_url(source: &str, name: &str, tail: &str) -> Result<String> {
    if matches!(name, "" | "." | "..") {
        bail!("{name:?} is not a world name");
    }
    let mut url = url::Url::parse(source).with_context(|| format!("{source} is not a URL"))?;
    url.path_segments_mut()
        .map_err(|_| anyhow!("{source} cannot carry a path"))?
        .pop_if_empty()
        .extend(["world", name, tail]);
    Ok(url.into())
}

/// Who published each scene of the world a read-through fills, asked of the upstream one scene
/// at a time.
struct Signatures<'a> {
    verify: &'a Verify<'a>,
    world: WorldName,
    scenes_url: url::Url,
    /// `{source}/world/{name}/permissions` when upstream grants are trusted.
    permissions_url: Option<url::Url>,
    /// That answer, asked at most once per fill.
    grant: tokio::sync::OnceCell<Result<UpstreamGrant, String>>,
}

/// What the upstream worlds server says of a world's deployment rights: its owner and the
/// addresses on the deployment allow-list.
#[derive(Debug, PartialEq)]
struct UpstreamGrant {
    owner: Address,
    deployers: Vec<Address>,
}

/// `{"owner": "0x..", "permissions": {"deployment": {"type": "allow-list", "wallets": [..]}}}`;
/// any other deployment type grants no one.
fn parse_upstream_grant(body: &Value) -> Result<UpstreamGrant> {
    let owner = body["owner"]
        .as_str()
        .context("the permissions name no owner")?;
    let owner = Address::parse(owner).map_err(|e| anyhow!("the permissions owner: {e}"))?;
    let deployment = &body["permissions"]["deployment"];
    let deployers = if deployment["type"].as_str() == Some("allow-list") {
        deployment["wallets"]
            .as_array()
            .context("the deployment allow-list has no wallets")?
            .iter()
            .filter_map(|w| w.as_str().and_then(|w| Address::parse(w).ok()))
            .collect()
    } else {
        Vec::new()
    };
    Ok(UpstreamGrant { owner, deployers })
}

/// A scene's chain and the signer the policy allowed.
struct Signed {
    chain: AuthChain,
    json: Value,
    signer: Address,
    /// The scene's size as the upstream lists it: a read-through fetches its files on demand,
    /// so this is the figure `/scenes` reports until a pass fetches them.
    size: i64,
}

impl<'a> Signatures<'a> {
    fn new(verify: &'a Verify<'a>, settings: &MirrorSettings, name: &str) -> Result<Self> {
        let source = settings.source.as_str();
        let permissions_url = match settings.trust_upstream_grants {
            true => Some(url::Url::parse(&world_url(source, name, "permissions")?)?),
            false => None,
        };
        Ok(Self {
            verify,
            world: WorldName::parse(name)?,
            scenes_url: url::Url::parse(&world_url(source, name, "scenes")?)?,
            permissions_url,
            grant: tokio::sync::OnceCell::new(),
        })
    }

    /// A signer the NAME holder did not sign as may still have been granted the world upstream.
    /// That grant is the upstream worlds server's word, not a signature this node checks (the
    /// owner's signed grant request is not published): it is trusted only under
    /// `WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS`, only for a read-through fill (a local
    /// deployment keeps the policy's own grants), and only when the owner the upstream names is
    /// the one this node's own NAME authority names. Anything else refuses, an unreachable or
    /// malformed permissions answer included.
    async fn upstream_grant(&self, client: &crate::http::Outbound, signer: &Address) -> Result<()> {
        let Some(url) = &self.permissions_url else {
            bail!("upstream grants are not trusted (WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS)");
        };
        let grant = self
            .grant
            .get_or_init(|| async {
                let fetched: Result<UpstreamGrant> = async {
                    let resp = client
                        .get(url.as_str())
                        .send()
                        .await
                        .with_context(|| format!("GET {url}"))?;
                    if !resp.status().is_success() {
                        bail!("GET {url} answered {}", resp.status());
                    }
                    let body: Value = serde_json::from_slice(
                        &read_body_capped(resp, MAX_PERMISSIONS_BYTES).await?,
                    )
                    .with_context(|| format!("parsing {url}"))?;
                    parse_upstream_grant(&body)
                }
                .await;
                fetched.map_err(|e| format!("{e:#}"))
            })
            .await
            .as_ref()
            .map_err(|e| anyhow!("{e}"))?;
        if !grant.deployers.contains(signer) {
            bail!("{signer} is not on the upstream deployment allow-list");
        }
        match self
            .verify
            .policy
            .may_have_published(&self.world, &grant.owner)
            .await
        {
            PermissionDecision::Allowed => Ok(()),
            PermissionDecision::Refused(why) => bail!(
                "the upstream names {} as the owner, which this node's NAME authority does not: {why}",
                grant.owner
            ),
        }
    }

    /// The chain the upstream lists for `cid` (`?entity_id=<cid>&limit=1`: one body of at most
    /// [`MAX_SCENE_ROW_BYTES`], the chain itself at most [`MAX_CHAIN_BYTES`], nothing else of it
    /// kept) and its signer, refused unless the policy says the signer may have published this
    /// world. Asked before the entity is fetched.
    async fn signer(&self, client: &crate::http::Outbound, cid: &str) -> Result<Signed> {
        if !is_canonical_cid(cid) {
            bail!("{cid:?} is not a canonical content id");
        }
        let mut url = self.scenes_url.clone();
        url.query_pairs_mut()
            .append_pair("entity_id", cid)
            .append_pair("limit", "1");
        let resp = client
            .get(url.as_str())
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            bail!("GET {url} answered {}", resp.status());
        }
        let (json, size) = {
            let listing: Value =
                serde_json::from_slice(&read_body_capped(resp, MAX_SCENE_ROW_BYTES).await?)
                    .with_context(|| format!("parsing {url}"))?;
            let row = listing["scenes"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["entityId"] == cid))
                .with_context(|| format!("{} lists no deployment of scene {cid}", self.world))?;
            let size = serde_json::to_vec(&row["deploymentAuthChain"])?.len();
            if size > MAX_CHAIN_BYTES {
                bail!("the auth chain of scene {cid} is {size} bytes, over {MAX_CHAIN_BYTES}");
            }
            let listed_size = match &row["size"] {
                Value::String(s) => s.parse().ok(),
                other => other.as_i64(),
            };
            (
                row["deploymentAuthChain"].clone(),
                listed_size.unwrap_or(0).max(0),
            )
        };
        let chain: AuthChain = serde_json::from_value(json.clone())
            .with_context(|| format!("the auth chain of scene {cid} is malformed"))?;
        let signer = chain
            .first()
            .map(|link| Address::parse(&link.payload))
            .with_context(|| format!("the auth chain of scene {cid} is empty"))??;
        match self
            .verify
            .policy
            .may_have_published(&self.world, &signer)
            .await
        {
            PermissionDecision::Allowed => Ok(Signed {
                chain,
                json,
                signer,
                size,
            }),
            PermissionDecision::Refused(why) => match self.upstream_grant(client, &signer).await {
                Ok(()) => Ok(Signed {
                    chain,
                    json,
                    signer,
                    size,
                }),
                Err(grant) => bail!(
                    "{signer} may not have published scene {cid} to {}: {why}; not granted upstream: {grant:#}",
                    self.world
                ),
            },
        }
    }

    /// The fetched entity is a scene of this world and the chain signs its id with ECDSA links
    /// only, ephemeral keys judged at the entity's timestamp. The upstream worlds server accepts
    /// no contract-wallet (EIP-1654) link, so one refuses here; the peer validator content sync
    /// runs would take it on the peer's word.
    async fn check(&self, cid: &str, entity: &Value, signed: &Signed) -> Result<()> {
        if entity["type"].as_str() != Some("scene") {
            bail!("entity {cid} is not a scene");
        }
        let declared = entity["metadata"]["worldConfiguration"]["name"]
            .as_str()
            .unwrap_or_default();
        if !declared.eq_ignore_ascii_case(self.world.key()) {
            bail!(
                "scene {cid} names the world {declared:?}, not {}",
                self.world
            );
        }
        let timestamp = entity["timestamp"]
            .as_i64()
            .with_context(|| format!("entity {cid} has no timestamp"))?;
        verify_auth_chain(&signed.chain, cid, Some(timestamp))
            .with_context(|| format!("the auth chain of scene {cid} does not verify"))?;
        Ok(())
    }
}

/// Where a checked scene's entity file is when the world is stored.
enum EntityFile {
    /// Already in the store.
    Stored,
    /// Fetched (and on a read-through checked), stored from memory.
    Held(Bytes),
}

/// Whether this node keeps `name` itself, so a read-through must not write it: a local world
/// (no `mirror_source`, with scenes or not) or a blocked one.
async fn kept_here(pool: &PgPool, name: &str) -> Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM worlds.worlds WHERE name = lower($1) \
           AND (mirror_source IS NULL OR blocked_since IS NOT NULL))",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .with_context(|| format!("checking whether {name} is kept here"))
}

fn holds_every_scene(upstream: &[String], held: &[String]) -> bool {
    !upstream.is_empty()
        && upstream.len() == held.len()
        && upstream.iter().all(|cid| held.contains(cid))
}

async fn mirror_world(
    client: &crate::http::Outbound,
    pool: &PgPool,
    storage: &Arc<Storage>,
    settings: &MirrorSettings,
    name: &str,
    catalogue_scene_ids: &[String],
    verify: Option<&Verify<'_>>,
) -> Result<Outcome> {
    let source = settings.source.as_str();
    let locally_published: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM worlds.world_scenes WHERE world_name = lower($1) AND mirror_source IS NULL)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .with_context(|| format!("checking local publish for {name}"))?;
    if locally_published {
        let retired = retire_beside_local(pool, name).await?;
        return Ok(Outcome::RefusedLocalPublish { retired });
    }
    if verify.is_some() && kept_here(pool, name).await? {
        return Ok(Outcome::RefusedLocalPublish { retired: 0 });
    }

    let held: Vec<String> = sqlx::query_scalar(
        "SELECT entity_id FROM worlds.world_scenes WHERE world_name = lower($1) AND mirror_source IS NOT NULL",
    )
    .bind(name)
    .fetch_all(pool)
    .await
    .with_context(|| format!("reading the scenes held for {name}"))?;
    if holds_every_scene(catalogue_scene_ids, &held) {
        // Every file, not only the entities: a read-through fill leaves all but the scene
        // host's to be read through, and the pass fetches them.
        let files: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT c->>'hash' FROM worlds.world_scenes s, \
               jsonb_array_elements(CASE WHEN jsonb_typeof(s.entity->'content') = 'array' \
                                         THEN s.entity->'content' ELSE '[]'::jsonb END) c \
             WHERE s.world_name = lower($1) AND s.mirror_source IS NOT NULL AND c->>'hash' IS NOT NULL",
        )
        .bind(name)
        .fetch_all(pool)
        .await
        .with_context(|| format!("reading the files of the scenes held for {name}"))?;
        let wanted: Vec<&str> = catalogue_scene_ids
            .iter()
            .chain(&files)
            .map(String::as_str)
            .collect();
        let present = storage.exist_multiple(&wanted).await?;
        if wanted
            .iter()
            .all(|hash| present.get(*hash).copied().unwrap_or(false))
        {
            return Ok(Outcome::Unchanged);
        }
    }

    let url = world_url(source, name, "about")?;
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        return Ok(Outcome::NothingUpstream(
            "upstream /about did not answer 2xx",
        ));
    }
    let about: Value = serde_json::from_slice(&read_body_capped(resp, MAX_ABOUT_BYTES).await?)
        .with_context(|| format!("parsing {url}"))?;
    let refs = scene_refs(&about);
    if refs.is_empty() {
        return Ok(Outcome::NothingUpstream("upstream world has no scenes"));
    }

    let skybox_time = about["configurations"]["skybox"]["fixedHour"]
        .as_f64()
        .map(|t| t as i32);
    let single_player = about["comms"]["adapter"].as_str() == Some("fixed-adapter:offline:offline");
    let upstream_spawn = about["spawnCoordinates"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(String::from);

    let signatures = match verify {
        Some(verify) => Some(Signatures::new(verify, settings, name)?),
        None => None,
    };
    let downloads = verify.map(|v| v.downloads);

    // Every scene entity fetched and, on a read-through, its signature and signer checked
    // before any file of the world is stored; the bytes checked are the bytes stored.
    let mut scenes = Vec::with_capacity(refs.len());
    for (cid, base) in refs {
        if !settings.base_url_allowed(&base) {
            bail!("scene {cid} of {name} names a baseUrl this node will not fetch: {base}");
        }
        let signed = match &signatures {
            Some(signatures) => Some(signatures.signer(client, &cid).await?),
            None => None,
        };
        // The slot ends with this scene's checks: holding it to the store could wait on itself
        // across the scenes of one world.
        let (entity_bytes, fetched, _slot) =
            fetch_checked(client, storage, &base, &cid, downloads).await?;
        let entity: Value =
            serde_json::from_slice(&entity_bytes).with_context(|| format!("parse entity {cid}"))?;
        if let (Some(signatures), Some(signed)) = (&signatures, &signed) {
            signatures.check(&cid, &entity, signed).await?;
        }
        let file = if fetched {
            EntityFile::Held(entity_bytes)
        } else {
            EntityFile::Stored
        };
        scenes.push((cid, base, file, entity, signed));
    }

    let mut new_blobs = 0usize;
    let mut scene_base_spawn: Option<String> = None;
    let mut records: Vec<(String, Value, Vec<String>, i64, Value, String)> = Vec::new();
    let blob_sem = Arc::new(Semaphore::new(settings.blob_concurrency));
    for (cid, base, file, entity, signed) in scenes {
        match file {
            EntityFile::Stored => {}
            EntityFile::Held(bytes) => {
                storage.store(&cid, bytes).await?;
                new_blobs += 1;
            }
        }

        // A read-through stores only what the scene host reads from the blob store (the main
        // code file and main.crdt); every other file is read through, hash-checked, from the
        // source on its first `/contents/{hash}` (`content::read_through`), so a large world
        // answers its /about in seconds. A pass fetches every file of the worlds it syncs,
        // those a fill left to be read through included.
        let main = entity["metadata"]["main"].as_str().unwrap_or_default();
        let hashes: Vec<String> = entity["content"]
            .as_array()
            .map(|content| {
                content
                    .iter()
                    .filter(|c| {
                        signed.is_none()
                            || c["file"]
                                .as_str()
                                .is_some_and(|f| f == main || f == "main.crdt")
                    })
                    .filter_map(|c| c["hash"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let mut set = tokio::task::JoinSet::new();
        for hash in hashes {
            let permit = blob_sem.clone().acquire_owned().await?;
            let (client, base, storage) = (client.clone(), base.clone(), storage.clone());
            let downloads = downloads.cloned();
            set.spawn(async move {
                let _permit = permit;
                let (bytes, fetched) =
                    fetch_blob(&client, &storage, &base, &hash, downloads.as_ref()).await?;
                Ok::<_, anyhow::Error>((bytes.len() as i64, fetched))
            });
        }
        let mut size: i64 = 0;
        while let Some(joined) = set.join_next().await {
            let (len, fetched) = joined.context("blob fetch task")??;
            size += len;
            new_blobs += fetched as usize;
        }

        let parcels: Vec<String> = entity["pointers"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| p.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        if scene_base_spawn.is_none() {
            scene_base_spawn = entity["metadata"]["scene"]["base"]
                .as_str()
                .map(String::from);
        }
        let (chain, deployer) = match signed {
            Some(signed) => {
                size = signed.size;
                (signed.json, signed.signer.as_str().to_string())
            }
            None => (Value::Array(Vec::new()), ZERO_ADDRESS.to_string()),
        };
        records.push((cid, entity, parcels, size, chain, deployer));
    }

    let mut tx = pool.begin().await.context("begin tx")?;
    let upserted = sqlx::query(
        r#"INSERT INTO worlds.worlds
             (name, spawn_coordinates, skybox_time, single_player, blocked_since, mirror_source, updated_at)
           VALUES ($1, COALESCE($2, $5), $3, $4, NULL, $6, timezone('UTC', now()))
           ON CONFLICT (name) DO UPDATE
             SET spawn_coordinates = COALESCE($2, worlds.worlds.spawn_coordinates, $5),
                 skybox_time = EXCLUDED.skybox_time,
                 single_player = EXCLUDED.single_player,
                 settings_version = worlds.worlds.settings_version
                   + CASE WHEN (worlds.worlds.skybox_time, worlds.worlds.single_player)
                          IS DISTINCT FROM (EXCLUDED.skybox_time, EXCLUDED.single_player)
                          THEN 1 ELSE 0 END,
                 mirror_source = EXCLUDED.mirror_source,
                 updated_at = timezone('UTC', now())
             WHERE NOT $7
                OR (worlds.worlds.mirror_source IS NOT NULL AND worlds.worlds.blocked_since IS NULL)"#,
    )
    .bind(name)
    .bind(&upstream_spawn)
    .bind(skybox_time)
    .bind(single_player)
    .bind(&scene_base_spawn)
    .bind(source)
    .bind(verify.is_some())
    .execute(&mut *tx)
    .await
    .with_context(|| format!("upsert world {name}"))?;
    if upserted.rows_affected() == 0 {
        // Kept here since the check above: the transaction rolls back on drop.
        return Ok(Outcome::RefusedLocalPublish { retired: 0 });
    }
    // The upsert holds the world row, which a local publish takes first
    // (core::store::deploy_scene), so this fresh read sees any publish that
    // landed during the download and none can land before the commit.
    let published_meanwhile: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM worlds.world_scenes WHERE world_name = lower($1) AND mirror_source IS NULL)",
    )
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    .with_context(|| format!("re-checking local publish for {name}"))?;
    if published_meanwhile {
        tx.rollback().await.context("rollback tx")?;
        let retired = retire_beside_local(pool, name).await?;
        return Ok(Outcome::RefusedLocalPublish { retired });
    }

    let cids: Vec<&str> = records.iter().map(|r| r.0.as_str()).collect();
    let entities: Vec<Value> = records.iter().map(|r| r.1.clone()).collect();
    let parcels: Vec<Value> = records.iter().map(|r| Value::from(r.2.clone())).collect();
    let sizes: Vec<i64> = records.iter().map(|r| r.3).collect();
    let chains: Vec<Value> = records.iter().map(|r| r.4.clone()).collect();
    let deployers: Vec<&str> = records.iter().map(|r| r.5.as_str()).collect();
    // A pass's unsigned row never replaces the chain and signer a read-through stored.
    sqlx::query(
        r#"INSERT INTO worlds.world_scenes
             (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source)
           SELECT $1, t.cid, t.chain::json, t.entity, t.deployer,
                  ARRAY(SELECT jsonb_array_elements_text(t.parcels)), t.size, $8
           FROM unnest($2::text[], $3::jsonb[], $4::jsonb[], $5::bigint[], $6::jsonb[], $7::text[])
                AS t(cid, entity, parcels, size, chain, deployer)
           ON CONFLICT (world_name, entity_id) DO UPDATE
             SET entity = EXCLUDED.entity, parcels = EXCLUDED.parcels, size = EXCLUDED.size,
                 deployment_auth_chain = CASE
                   WHEN json_array_length(EXCLUDED.deployment_auth_chain) > 0
                   THEN EXCLUDED.deployment_auth_chain
                   ELSE worlds.world_scenes.deployment_auth_chain END,
                 deployer = CASE
                   WHEN json_array_length(EXCLUDED.deployment_auth_chain) > 0
                   THEN EXCLUDED.deployer
                   ELSE worlds.world_scenes.deployer END,
                 mirror_source = EXCLUDED.mirror_source, updated_at = timezone('UTC', now())"#,
    )
    .bind(name)
    .bind(&cids)
    .bind(&entities)
    .bind(&parcels)
    .bind(&sizes)
    .bind(&chains)
    .bind(&deployers)
    .bind(source)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("upsert {} scenes of {name}", records.len()))?;

    sqlx::query(
        r#"DELETE FROM worlds.world_scenes
           WHERE world_name = lower($1) AND entity_id <> ALL($2::text[])
             AND mirror_source IS NOT NULL"#,
    )
    .bind(name)
    .bind(&cids)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("retire the scenes {name} no longer lists upstream"))?;
    let world = name.to_lowercase();
    // /about lists newest first. Apply the overlap rule in deployment order:
    // a replacement must inherit its predecessor before a later scene takes
    // a parcel that predecessor vacated. CID breaks timestamp ties stably.
    let mut chronological: Vec<_> = records.iter().collect();
    chronological.sort_by(|a, b| {
        a.1["timestamp"]
            .as_i64()
            .unwrap_or_default()
            .cmp(&b.1["timestamp"].as_i64().unwrap_or_default())
            .then_with(|| a.0.cmp(&b.0))
    });
    for record in chronological {
        scene_identity::assign(&mut tx, &world, &record.0, &record.2)
            .await
            .with_context(|| format!("storage identity of {} in {name}", record.0))?;
    }
    scene_identity::retire_orphans(&mut tx, std::slice::from_ref(&world))
        .await
        .with_context(|| format!("retire the storage identities of {name}"))?;
    tx.commit().await.context("commit tx")?;

    Ok(Outcome::Synced {
        scenes: records.len(),
        new_blobs,
    })
}

/// `(entity id, baseUrl)` per `scenesUrn` entry:
/// `urn:decentraland:entity:<cid>?=&baseUrl=<url>`.
fn scene_refs(about: &Value) -> Vec<(String, String)> {
    about["configurations"]["scenesUrn"]
        .as_array()
        .map(|urns| {
            urns.iter()
                .filter_map(|u| u.as_str())
                .map(|urn| {
                    let after = urn
                        .split_once("urn:decentraland:entity:")
                        .map(|(_, b)| b)
                        .unwrap_or(urn);
                    let (cid, query) = after.split_once('?').unwrap_or((after, ""));
                    let base = query
                        .split('&')
                        .find_map(|kv| kv.strip_prefix("baseUrl="))
                        .map(String::from)
                        .unwrap_or_default();
                    (cid.trim().to_string(), base)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The blob from the store, or fetched from `{base_url}{hash}`, verified
/// against its CID and stored, its download slot held until then. The bool
/// says whether it was fetched.
async fn fetch_blob(
    client: &crate::http::Outbound,
    storage: &Storage,
    base_url: &str,
    hash: &str,
    downloads: Option<&Downloads>,
) -> Result<(Bytes, bool)> {
    let (bytes, fetched, slot) = fetch_checked(client, storage, base_url, hash, downloads).await?;
    if fetched {
        storage.store(hash, bytes.clone()).await?;
    }
    drop(slot);
    Ok((bytes, fetched))
}

/// [`fetch_blob`] without the store: the bytes from the store, or fetched and verified against
/// their CID under a [`Downloads`] slot, and its byte budget, when given; the caller holds the
/// slot as long as the bytes count against the bound. The bool says whether they were fetched,
/// so are not stored yet.
async fn fetch_checked(
    client: &crate::http::Outbound,
    storage: &Storage,
    base_url: &str,
    hash: &str,
    downloads: Option<&Downloads>,
) -> Result<(Bytes, bool, Option<DownloadSlot>)> {
    if !is_canonical_cid(hash) {
        bail!("{hash:?} is not a canonical content id");
    }
    if let Some(bytes) = storage.retrieve(hash).await? {
        return Ok((bytes, false, None));
    }
    let mut slot = match downloads {
        Some(downloads) => Some(downloads.slot().await.map_err(|e| anyhow!(e))?),
        None => None,
    };
    let url = format!("{base_url}{hash}");
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("GET {url} answered {}", resp.status());
    }
    if let Some(slot) = &mut slot {
        let len = resp
            .content_length()
            .unwrap_or(MAX_BLOB_BYTES as u64)
            .min(MAX_BLOB_BYTES as u64);
        slot.hold(len).await.map_err(|e| anyhow!(e))?;
    }
    let bytes = read_body_capped(resp, MAX_BLOB_BYTES)
        .await
        .with_context(|| format!("reading {url}"))?;
    if !verify_hash(&bytes, hash) {
        bail!("{url}: body does not hash to {hash}");
    }
    Ok((Bytes::from(bytes), true, slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_world_is_unchanged_only_when_it_holds_exactly_the_upstream_scenes() {
        let cids = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert!(holds_every_scene(&cids(&["a", "b"]), &cids(&["b", "a"])));
        assert!(!holds_every_scene(&cids(&["a", "b"]), &cids(&["a"])));
        assert!(!holds_every_scene(&cids(&["a"]), &cids(&["a", "stale"])));
        assert!(!holds_every_scene(&cids(&["new"]), &cids(&["old"])));
        assert!(!holds_every_scene(&[], &[]));
    }

    #[test]
    fn an_upstream_grant_is_its_owner_and_the_deployment_allow_list_only() {
        let owner = "0x00000000000000000000000000000000000000aa";
        let deployer = "0x00000000000000000000000000000000000000BB";
        let grant = parse_upstream_grant(&json!({
            "owner": owner,
            "permissions": {
                "deployment": {"type": "allow-list", "wallets": [deployer, "not an address"]},
                "streaming": {"type": "allow-list", "wallets": ["0x00000000000000000000000000000000000000cc"]}
            }
        }))
        .unwrap();
        assert_eq!(grant.owner, Address::parse(owner).unwrap());
        assert_eq!(grant.deployers, vec![Address::parse(deployer).unwrap()]);
        let unrestricted = parse_upstream_grant(&json!({
            "owner": owner, "permissions": {"deployment": {"type": "unrestricted"}}
        }))
        .unwrap();
        assert!(unrestricted.deployers.is_empty());
        assert!(parse_upstream_grant(&json!({"permissions": {}})).is_err());
    }

    #[test]
    fn the_read_through_alone_mirrors_worlds_as_read_and_runs_no_pass() {
        let mirror = |args: &[&str]| {
            let mut argv = vec!["x", "--database-url", "postgres:///c"];
            argv.extend_from_slice(args);
            let config = crate::config::Config::parse_for_tests(&argv).unwrap();
            MirrorSettings::from_config(&config, &[]).unwrap()
        };
        let source = "https://worlds.example.org";
        assert!(mirror(&[]).is_none());
        assert!(mirror(&["--worlds-content-mirror-source", source]).is_none());
        assert!(mirror(&["--read-through-enabled", "true"]).is_none());
        let read_through = mirror(&[
            "--read-through-enabled",
            "true",
            "--worlds-content-mirror-source",
            source,
        ])
        .unwrap();
        assert!(!read_through.scheduled);
        assert!(!read_through.trust_upstream_grants);
        assert_eq!(read_through.source, source);
        let trusting = mirror(&[
            "--read-through-enabled",
            "true",
            "--worlds-content-mirror-source",
            source,
            "--worlds-read-through-trust-upstream-grants",
            "true",
        ])
        .unwrap();
        assert!(trusting.trust_upstream_grants);
        let scheduled = mirror(&[
            "--worlds-content-mirror-enabled",
            "true",
            "--worlds-content-mirror-source",
            source,
        ])
        .unwrap();
        assert!(scheduled.scheduled);
        let config = crate::config::Config::parse_for_tests(&[
            "x",
            "--database-url",
            "postgres:///c",
            "--read-through-enabled",
            "true",
            "--worlds-content-mirror-source",
            "http://10.0.0.1",
        ])
        .unwrap();
        assert!(MirrorSettings::from_config(&config, &[]).is_err());
    }

    #[test]
    fn a_world_name_is_one_encoded_segment_of_the_upstream_url() {
        assert_eq!(
            world_url("http://up.example/base/", "rt.dcl.eth", "about").unwrap(),
            "http://up.example/base/world/rt.dcl.eth/about"
        );
        assert_eq!(
            world_url("https://up.example", "rt.dcl.eth\\about?0#f/x", "about").unwrap(),
            "https://up.example/world/rt.dcl.eth%5Cabout%3F0%23f%2Fx/about"
        );
        for refused in ["", ".", ".."] {
            assert!(world_url("https://up.example", refused, "about").is_err());
        }
    }

    #[test]
    fn scene_refs_split_the_urn_into_id_and_base_url() {
        let about = json!({"configurations": {"scenesUrn": [
            "urn:decentraland:entity:bafkreicid?=&baseUrl=https://up.example/contents/",
            "urn:decentraland:entity:bafkreiother",
            42
        ]}});
        assert_eq!(
            scene_refs(&about),
            vec![
                (
                    "bafkreicid".to_string(),
                    "https://up.example/contents/".to_string()
                ),
                ("bafkreiother".to_string(), String::new()),
            ]
        );
        assert!(scene_refs(&json!({})).is_empty());
    }

    #[test]
    fn the_index_keeps_worlds_with_canonical_scene_ids_lowercased_and_sorted() {
        let index = json!({"data": [
            {"name": "Zed.dcl.eth", "scenes": [
                {"id": "bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa", "timestamp": 5},
                {"id": "not-a-cid", "timestamp": 9}
            ]},
            {"name": "empty.dcl.eth", "scenes": []},
            {"name": "zed.dcl.eth", "scenes": [{"id": "bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa"}]},
            {"name": "alpha", "scenes": [{"id": "QmcoQSrVoi8CKSwiRyJ3MPYyN1AUiLjHiAtYCUGoBr8JM4", "timestamp": 7}]},
            {"scenes": [{"id": "bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa"}]}
        ]});
        let entries = parse_index(&index);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zed.dcl.eth"]);
        assert_eq!(
            entries[1].scene_ids,
            vec!["bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa"]
        );
        assert_eq!(
            entries[1].newest_timestamp_ms,
            Some(5),
            "the dropped non-CID scene does not vote"
        );
        assert_eq!(entries[0].newest_timestamp_ms, Some(7));
        assert!(parse_index(&json!({"data": "nope"})).is_empty());
    }

    fn parse(source: &str, own: &[&str]) -> Result<MirrorSettings> {
        let own: Vec<String> = own.iter().map(|s| s.to_string()).collect();
        MirrorSettings::parse(source, &own, Duration::from_secs(60), 4, 8, 0, "")
    }

    #[test]
    fn the_source_must_be_public_https_and_never_this_node() {
        let ok = parse(
            "https://worlds-content-server.decentraland.org/",
            &["https://v2.example.org"],
        )
        .unwrap();
        assert_eq!(ok.source, "https://worlds-content-server.decentraland.org");
        assert!(!ok.allows_private_source());
        for private in [
            "http://127.0.0.1:9/",
            "https://10.0.0.5",
            "https://localhost:9",
            "https://[::1]/",
            "https://router/",
        ] {
            let err = parse(private, &[]).unwrap_err().to_string();
            assert!(
                err.contains("SSRF guard") || err.contains("plaintext"),
                "{private}: {err}"
            );
        }
        assert!(
            parse("http://worlds.example.org", &[])
                .unwrap_err()
                .to_string()
                .contains("plaintext")
        );
        assert!(parse("ftp://x", &[]).is_err());
        assert!(parse("   ", &[]).unwrap_err().to_string().contains("empty"));
        let err = parse(
            "https://v2.example.org/",
            &["https://v2.example.org:443/content/"],
        )
        .unwrap_err();
        assert!(err.to_string().contains("this node itself"), "{err}");
        assert!(
            MirrorSettings::parse(
                "https://up.example",
                &[],
                Duration::from_secs(60),
                0,
                8,
                0,
                ""
            )
            .is_err()
        );
        // The loopback is admitted only through the test constructor.
        let dev = MirrorSettings::for_tests("http://127.0.0.1:9/");
        assert!(dev.allows_private_source());
        assert_eq!(dev.source, "http://127.0.0.1:9");
    }

    #[test]
    fn names_and_limit_bound_a_run() {
        let s = MirrorSettings::parse(
            "https://up.example",
            &[],
            Duration::from_secs(60),
            4,
            8,
            25,
            " Foo.dcl.eth, bar ,,",
        )
        .unwrap();
        assert_eq!(s.limit, Some(25));
        assert_eq!(s.names, vec!["foo.dcl.eth", "bar"]);
        assert_eq!(MirrorSettings::for_tests("https://up.example").limit, None);
    }

    #[test]
    fn scene_base_urls_are_the_source_or_public_and_never_this_node() {
        let s = parse("https://up.example", &["https://v2.example.org"]).unwrap();
        assert!(s.base_url_allowed("https://up.example/contents/"));
        assert!(s.base_url_allowed("https://peer.decentraland.org/content/contents/"));
        assert!(
            !s.base_url_allowed("https://v2.example.org/contents/"),
            "a mirror loop"
        );
        assert!(
            !s.base_url_allowed("http://169.254.169.254/latest/"),
            "the SSRF guard"
        );
        assert!(!s.base_url_allowed("http://10.0.0.1/contents/"));
        assert!(!s.base_url_allowed(""));
        assert!(
            !s.base_url_allowed("https://up.example.internal/contents/"),
            "the source's own host is not exempt"
        );
        let dev = MirrorSettings::for_tests("http://127.0.0.1:9")
            .with_own_urls(&["http://127.0.0.1:5210".to_string()]);
        assert!(
            dev.base_url_allowed("http://127.0.0.1:9/contents/"),
            "the test stub itself"
        );
        assert!(!dev.base_url_allowed("http://127.0.0.1:5210/contents/"));
        assert!(
            !dev.base_url_allowed("http://127.0.0.1:10/contents/"),
            "another loopback port is not the stub"
        );
    }
    #[tokio::test]
    async fn unlisting_locks_the_world_before_scenes_and_cannot_retire_a_concurrent_publish() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("unlist-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let mut seed = pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1,'https://up.example')",
        )
        .bind(&world)
        .execute(&mut *seed)
        .await
        .unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes (world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size,mirror_source) VALUES ($1,'old','[]','{}','owner',ARRAY['0,0'],1,'https://up.example')").bind(&world).execute(&mut *seed).await.unwrap();
        let place = scene_identity::assign(&mut seed, &world, "old", &["0,0".into()])
            .await
            .unwrap()
            .unwrap();
        sqlx::query("INSERT INTO worlds.remote_worlds (world_name,source,scene_ids,unlisted_at) VALUES ($1,'https://up.example',ARRAY['old'],now())").bind(&world).execute(&mut *seed).await.unwrap();
        seed.commit().await.unwrap();

        let mut publish = pool.begin().await.unwrap();
        sqlx::query("SELECT name FROM worlds.worlds WHERE name=$1 FOR UPDATE")
            .bind(&world)
            .execute(&mut *publish)
            .await
            .unwrap();
        let publisher: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *publish)
            .await
            .unwrap();
        let retiring = tokio::spawn({
            let pool = pool.clone();
            async move { retire_unlisted(&pool).await }
        });
        // Wait for actual lock contention, not a scheduler sleep that could
        // let either implementation win the race accidentally.
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))")
                    .bind(publisher).fetch_one(&pool).await.unwrap();
                if blocked { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("the unlisting transaction waits for the publisher's world lock");
        sqlx::query("SET LOCAL lock_timeout='500ms'")
            .execute(&mut *publish)
            .await
            .unwrap();
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name=$1")
            .bind(&world)
            .execute(&mut *publish)
            .await
            .expect("retirement cannot hold the scene while waiting for its world");
        sqlx::query("INSERT INTO worlds.world_scenes (world_name,entity_id,deployment_auth_chain,entity,deployer,parcels,size) VALUES ($1,'new','[]','{}','owner',ARRAY['0,0'],1)").bind(&world).execute(&mut *publish).await.unwrap();
        assert_eq!(
            scene_identity::assign(&mut publish, &world, "new", &["0,0".into()])
                .await
                .unwrap(),
            Some(place)
        );
        sqlx::query("UPDATE worlds.worlds SET mirror_source=NULL WHERE name=$1")
            .bind(&world)
            .execute(&mut *publish)
            .await
            .unwrap();
        publish.commit().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), retiring)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let binding: (String, bool) = sqlx::query_as(
            "SELECT entity_id,active FROM explore.world_scene_place WHERE identity=$1",
        )
        .bind(place)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(binding, ("new".into(), true));
        sqlx::query("DELETE FROM worlds.worlds WHERE name=$1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.world_scene_place WHERE identity=$1")
            .bind(place)
            .execute(&pool)
            .await
            .unwrap();
    }
}
