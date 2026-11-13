//! The worlds mirror: one pass is the catalogue (`GET {source}/index`, one
//! document listing every world and its scene ids) written to
//! `worlds.remote_worlds`, then the scenes/files walk over the catalogue in
//! name order: `GET {source}/world/{name}/about` (the world's spawn and content
//! `baseUrl`), its paged `GET {source}/world/{name}/scenes` (every deployed scene with its
//! deployment's times, size, chain and deployer: `/about` lists only the newest) and
//! `GET {source}/world/{name}/settings` (the stored world settings and their version) for each
//! world whose advertised scene set differs from what is held, the scene entities and
//! every file they reference into the ONE blob store (hash-verified, so a
//! lying upstream cannot plant bytes under a CID), and the `worlds.worlds` +
//! `worlds.world_scenes` rows in one transaction per world. Every world the walk
//! reaches, changed or not, also gets the access setting its
//! `GET {source}/world/{name}/permissions` reports (a shared secret stored as one no secret
//! matches), and the settings `/settings` reports. Either is kept while the owner that set it on
//! this node (`access_set_here`, `settings_set_here`) still holds the name, as the source's
//! `/permissions` owner says ([`mirror_keeps_local`]). An unreadable or unknown access setting
//! keeps the stored one: it fails the sync of a changed world, so a world restricted upstream is
//! never stored open, and a world whose scenes are all held skips only the access copy. The
//! deployment and streaming grants it lists are not copied. A world whose scenes are all held
//! is still read for its settings and its `/scenes` listing, since upstream edits settings and
//! access without a deployment and its held rows take the listed times, size, chain and
//! deployer. Each such world costs a pass `/permissions`, every `/scenes` page, `/settings` and
//! a thumbnail check against the store (a fetch only when it is not held), then one transaction
//! that writes only what changed.
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
//! catalogue's trust: a row carries the listed chain and deployer only when that chain's ECDSA
//! links sign the scene and its signer is the listed deployer, else no chain and the zero
//! address; no right here is granted from either (nor from the access setting, which only
//! narrows who may enter).
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
use crate::auth::{
    AuthChain, hash_bytes_v1, is_canonical_cid, raw_cid_v1_from_sha256_hex, verify_auth_chain,
    verify_hash,
};
use crate::comms::world_identity::WorldName;
use crate::content::read_through::{DownloadSlot, Downloads};
use crate::core::settings_policy::{
    MAX_CATEGORIES, VALID_RATINGS, detect_image_format, storable_skybox_time,
    storable_thumbnail_hash,
};
use crate::core::store::{AccessSetting, SetHere, mirror_keeps_local, replace_mirrored_access};
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
/// Scenes per page of a world's `/scenes` listing, and the cap of one page's body.
const SCENES_PAGE: usize = 20;
const MAX_SCENES_PAGE_BYTES: usize = 64 * 1024 * 1024;
/// A world listing more scenes than this is refused on every pass: none of it is retired, and
/// none of its updates land.
const MAX_LISTED_SCENES: usize = 2_000;
/// One `/world/{name}/permissions` answer: the owner and a few allow-lists of addresses.
const MAX_PERMISSIONS_BYTES: usize = 256 * 1024;
/// The stored secret hash of a mirrored shared-secret world: not a bcrypt hash, so no secret
/// verifies against it (the source never publishes its hash).
const NO_SECRET_MATCHES: &str = "mirrored: the source keeps the secret";
/// One `/world/{name}/settings` answer: a handful of short columns.
const MAX_SETTINGS_BYTES: usize = 256 * 1024;
/// A world thumbnail upstream accepts at most 1 MiB (`world-settings-handler.ts`).
const MAX_THUMBNAIL_BYTES: usize = 1024 * 1024;
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
    pub fn from_config(
        config: &crate::config::Config,
        own_urls: &[String],
    ) -> Result<Option<Self>> {
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
                    let newest_timestamp_ms = scenes
                        .iter()
                        .filter_map(|s| crate::core::store::entity_timestamp_ms(s))
                        .max();
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

/// `{source}/world/{name}/permissions` (at most [`MAX_PERMISSIONS_BYTES`]); anything but a 2xx
/// JSON answer is an error.
async fn fetch_permissions(client: &crate::http::Outbound, url: &str) -> Result<Value> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        bail!("GET {url} answered {}", resp.status());
    }
    serde_json::from_slice(&read_body_capped(resp, MAX_PERMISSIONS_BYTES).await?)
        .with_context(|| format!("parsing {url}"))
}

/// The world's access setting at the source ([`parse_upstream_access`]) and the owner it names,
/// read on every pass that reaches the world, a changed one or not.
struct UpstreamAccess {
    setting: AccessSetting,
    /// `owner`, when it is an address: who holds the name, for [`mirror_keeps_local`].
    owner: Option<Address>,
}

async fn upstream_access(
    client: &crate::http::Outbound,
    source: &str,
    name: &str,
) -> Result<UpstreamAccess> {
    let url = world_url(source, name, "permissions")?;
    let body = fetch_permissions(client, &url).await?;
    Ok(UpstreamAccess {
        setting: parse_upstream_access(&body)?,
        owner: body["owner"].as_str().and_then(|o| Address::parse(o).ok()),
    })
}

/// The access setting the source's `/permissions` reports (`permissions.access`, upstream's
/// `getAccessForWorld` without secrets), as this node stores it. The shared secret's hash is not
/// on the wire, so a shared-secret world is stored with [`NO_SECRET_MATCHES`]: no secret admits
/// and only the owner enters. An allow-list keeps the wallets and community ids that parse. Any
/// other answer is an error, so a world is never stored open on a doubtful reading.
fn parse_upstream_access(body: &Value) -> Result<AccessSetting> {
    let access = &body["permissions"]["access"];
    let strings = |key: &str| -> Vec<String> {
        access[key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(match access["type"].as_str() {
        Some("unrestricted") => AccessSetting::Unrestricted,
        Some("shared-secret") => AccessSetting::SharedSecret {
            secret_hash: NO_SECRET_MATCHES.to_string(),
        },
        Some("nft-ownership") => AccessSetting::NftOwnership {
            nft: access["nft"]
                .as_str()
                .filter(|nft| !nft.is_empty())
                .context("the nft-ownership access names no nft")?
                .to_string(),
        },
        Some("allow-list") => AccessSetting::AllowList {
            wallets: strings("wallets"),
            communities: strings("communities"),
        },
        other => bail!("the permissions name no known access type: {other:?}"),
    })
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
                fetch_permissions(client, url.as_str())
                    .await
                    .and_then(|body| parse_upstream_grant(&body))
                    .map_err(|e| format!("{e:#}"))
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
        let timestamp = crate::core::store::entity_timestamp_ms(entity)
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

/// One scene of a world as it is stored.
struct SceneRecord {
    cid: String,
    entity: Value,
    parcels: Vec<String>,
    size: i64,
    chain: Value,
    deployer: String,
    created_at: Option<chrono::NaiveDateTime>,
    updated_at: Option<chrono::NaiveDateTime>,
}

/// The settings a source stores for a world (`GET {source}/world/{name}/settings`, public and
/// shaped for mirrors upstream: `toSnakeCaseSettings` of `getWorldSettings`), as this node can
/// store them. The default is no answer: every column stays as it is.
#[derive(Debug, Default, PartialEq)]
struct WorldSettingsCopy {
    /// The source answered; the text columns and categories are then written, an absent key
    /// clearing its column (upstream leaves out an empty text and NULL categories).
    present: bool,
    title: Option<String>,
    description: Option<String>,
    /// One of the ratings the column accepts, else none.
    content_rating: Option<String>,
    /// The first [`MAX_CATEGORIES`] strings.
    categories: Option<Vec<String>>,
    /// The source's key, a canonical CID or the SHA-256 hex of an upload; [`mirrored_thumbnail`]
    /// turns it into the CID of the bytes held here before it is stored.
    thumbnail_hash: Option<String>,
    /// The source names a thumbnail this pass could not fetch: the stored one stays.
    thumbnail_unread: bool,
    /// `Some(None)` clears: upstream answers `null` for no fixed hour.
    skybox_time: Option<Option<i32>>,
    single_player: Option<bool>,
    show_in_places: Option<bool>,
    settings_version: Option<i64>,
}

impl WorldSettingsCopy {
    fn parse(body: &Value) -> Option<Self> {
        let body = body.as_object()?;
        let text = |key: &str| {
            body.get(key)
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .map(String::from)
        };
        Some(Self {
            present: true,
            title: text("title"),
            description: text("description"),
            content_rating: text("content_rating").filter(|r| VALID_RATINGS.contains(&r.as_str())),
            categories: body.get("categories").and_then(Value::as_array).map(|c| {
                c.iter()
                    .filter_map(Value::as_str)
                    .take(MAX_CATEGORIES)
                    .map(String::from)
                    .collect()
            }),
            thumbnail_hash: text("thumbnail_hash")
                .filter(|h| is_canonical_cid(h) || raw_cid_v1_from_sha256_hex(h).is_some()),
            thumbnail_unread: false,
            skybox_time: match body.get("skybox_time") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(t) => t.as_f64().and_then(storable_skybox_time).map(Some),
            },
            single_player: body.get("single_player").and_then(Value::as_bool),
            show_in_places: body.get("show_in_places").and_then(Value::as_bool),
            settings_version: body
                .get("settings_version")
                .and_then(Value::as_i64)
                .filter(|v| *v >= 0),
        })
    }
}

/// `{source}/world/{name}/settings`, read on every pass that reaches the world (upstream edits
/// settings and access without a deployment, bumping `settings_version`), with its thumbnail
/// resolved by [`mirrored_thumbnail`]. `None` (no answer, a refusal or a body that is not a
/// settings object) leaves the stored settings as they are.
async fn upstream_settings(
    client: &crate::http::Outbound,
    pool: &PgPool,
    storage: &Storage,
    source: &str,
    name: &str,
) -> Option<WorldSettingsCopy> {
    let url = world_url(source, name, "settings").ok()?;
    let read = async {
        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("answered {}", resp.status());
        }
        let body: Value =
            serde_json::from_slice(&read_body_capped(resp, MAX_SETTINGS_BYTES).await?)?;
        WorldSettingsCopy::parse(&body).context("not a settings object")
    };
    let mut copy = match read.await {
        Ok(copy) => copy,
        Err(e) => {
            tracing::warn!(world = %name, error = %e, "keeping the stored settings: GET {url}");
            return None;
        }
    };
    if let Some(key) = copy.thumbnail_hash.take() {
        match mirrored_thumbnail(client, pool, storage, source, name, &key).await {
            Ok(cid) => copy.thumbnail_hash = cid,
            Err(e) => {
                tracing::warn!(world = %name, thumbnail = %key, error = %e, "keeping the stored thumbnail");
                copy.thumbnail_unread = true;
            }
        }
    }
    Some(copy)
}

/// The CID under which this node holds the thumbnail the source stores under `key`, fetched
/// from `{source}/contents/{key}` when it is not held, or `None` when the source's file is no
/// image this node serves (the world then has no thumbnail here). A failed read or store is an
/// error, so the stored thumbnail stays. A SHA-256 hex key (an upload upstream,
/// served only by the source) is checked against the bytes' SHA-256 and stored under their
/// CIDv1, the key every route here serves; a row whose stored thumbnail already holds those
/// bytes keeps it without a fetch (a thumbnail over one 256 KiB chunk has a CIDv1 that is not
/// the raw-leaf CID of the digest). The bytes must be an image, as for a local write
/// ([`storable_thumbnail_hash`]).
async fn mirrored_thumbnail(
    client: &crate::http::Outbound,
    pool: &PgPool,
    storage: &Storage,
    source: &str,
    name: &str,
    key: &str,
) -> Result<Option<String>> {
    let sha256_is_key = |bytes: &[u8]| {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(bytes)) == key
    };
    let resolve = async {
        let held = |cid: String| async move {
            Ok::<_, anyhow::Error>(storage.exist(&cid).await?.then_some(cid))
        };
        let cid = if is_canonical_cid(key) {
            held(key.to_string()).await?
        } else {
            let raw = raw_cid_v1_from_sha256_hex(key).context("not a thumbnail key")?;
            match held(raw).await? {
                Some(cid) => Some(cid),
                None => {
                    let stored = sqlx::query_scalar::<_, Option<String>>(
                        "SELECT thumbnail_hash FROM worlds.worlds \
                         WHERE name = lower($1) AND mirror_source IS NOT NULL",
                    )
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
                    .flatten()
                    .filter(|cid| is_canonical_cid(cid));
                    match stored {
                        Some(cid) => match storage.retrieve(&cid).await? {
                            Some(bytes) if sha256_is_key(&bytes) => Some(cid),
                            _ => None,
                        },
                        None => None,
                    }
                }
            }
        };
        let cid = match cid {
            Some(cid) => cid,
            None => {
                let url = contents_url(source, key)?;
                let resp = client.get(&url).send().await?;
                if !resp.status().is_success() {
                    bail!("GET {url} answered {}", resp.status());
                }
                let bytes = read_body_capped(resp, MAX_THUMBNAIL_BYTES).await?;
                let cid = if is_canonical_cid(key) {
                    if !verify_hash(&bytes, key) {
                        bail!("{url}: body does not hash to {key}");
                    }
                    key.to_string()
                } else {
                    if !sha256_is_key(&bytes) {
                        bail!("{url}: body is not the SHA-256 {key}");
                    }
                    hash_bytes_v1(&bytes)
                };
                if detect_image_format(&bytes).is_none() {
                    tracing::debug!(world = %name, thumbnail = %key, "{url} is not an image");
                    return Ok(None);
                }
                storage.store(&cid, Bytes::from(bytes)).await?;
                cid
            }
        };
        Ok::<_, anyhow::Error>(storable_thumbnail_hash(storage, &cid).await)
    };
    resolve.await
}

/// `{source}/contents/{key}`.
fn contents_url(source: &str, key: &str) -> Result<String> {
    let mut url = url::Url::parse(source).with_context(|| format!("{source} is not a URL"))?;
    url.path_segments_mut()
        .map_err(|_| anyhow!("{source} cannot carry a path"))?
        .pop_if_empty()
        .extend(["contents", key]);
    Ok(url.into())
}

/// Writes a source's settings answer onto a mirrored row that is not disabled, unless the owner
/// that set its settings here still holds the name ([`mirror_keeps_local`], with the source's
/// `/permissions` owner). A thumbnail the pass could not fetch keeps the stored one. `settings_version` never goes back: it takes the source's counter, or this row's
/// plus one when a copied column changes and the source's is not ahead, so a consumer ordering
/// by it never sees an update out of order (a row bumped by earlier ingests can be ahead of its
/// source). No write when nothing changes.
async fn apply_settings(
    conn: &mut sqlx::PgConnection,
    world: &WorldName,
    copy: &WorldSettingsCopy,
    source_owner: Option<&Address>,
) -> Result<()> {
    let name = world.key();
    if mirror_keeps_local(conn, world, SetHere::Settings, source_owner)
        .await
        .with_context(|| format!("settings of {name}"))?
        != Some(false)
    {
        return Ok(());
    }
    sqlx::query(
        r#"WITH cur AS (
             SELECT name, title, description, content_rating, categories, thumbnail_hash,
                    skybox_time, single_player, show_in_places
             FROM worlds.worlds
             WHERE name = $1 AND mirror_source IS NOT NULL AND blocked_since IS NULL
           ), n AS (
             SELECT name,
                    $2::text AS title,
                    $3::text AS description,
                    $4::text AS content_rating,
                    $5::text[] AS categories,
                    CASE WHEN $12 THEN thumbnail_hash ELSE $6::text END AS thumbnail_hash,
                    CASE WHEN $8 THEN $7::integer ELSE skybox_time END AS skybox_time,
                    COALESCE($9, single_player) AS single_player,
                    COALESCE($10, show_in_places) AS show_in_places,
                    COALESCE($11::bigint, 0) AS source_version
             FROM cur
           ), d AS (
             SELECT n.*,
                    (cur.title, cur.description, cur.content_rating, cur.categories,
                     cur.thumbnail_hash, cur.skybox_time, cur.single_player, cur.show_in_places)
                      IS DISTINCT FROM
                    (n.title, n.description, n.content_rating, n.categories, n.thumbnail_hash,
                     n.skybox_time, n.single_player, n.show_in_places) AS changed
             FROM n JOIN cur USING (name)
           )
           UPDATE worlds.worlds w
           SET title = d.title, description = d.description, content_rating = d.content_rating,
               categories = d.categories, thumbnail_hash = d.thumbnail_hash,
               skybox_time = d.skybox_time, single_player = d.single_player,
               show_in_places = d.show_in_places,
               settings_version = GREATEST(d.source_version,
                                           w.settings_version + CASE WHEN d.changed THEN 1 ELSE 0 END),
               updated_at = timezone('UTC', now())
           FROM d
           WHERE w.name = d.name AND (d.changed OR w.settings_version < d.source_version)"#,
    )
    .bind(name)
    .bind(&copy.title)
    .bind(&copy.description)
    .bind(&copy.content_rating)
    .bind(&copy.categories)
    .bind(&copy.thumbnail_hash)
    .bind(copy.skybox_time.flatten())
    .bind(copy.skybox_time.is_some())
    .bind(copy.single_player)
    .bind(copy.show_in_places)
    .bind(copy.settings_version)
    .bind(copy.thumbnail_unread)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("settings of {name}"))?;
    Ok(())
}

/// Brings the held rows of a world whose scene set is unchanged to what its listing says: the
/// deploy times and size, and on a row without a chain the listed chain and deployer once they
/// check ([`listed_signature`]). Rows mirrored before these were kept, and a listing that gave
/// none then, are filled here; a row the listing does not name is left as it is.
async fn refresh_listed_scenes(
    conn: &mut sqlx::PgConnection,
    name: &str,
    listed: &[ListedScene],
) -> Result<()> {
    let held: Vec<(String, Value, bool)> = sqlx::query_as(
        "SELECT entity_id, entity, CASE WHEN json_typeof(deployment_auth_chain) = 'array' \
           THEN json_array_length(deployment_auth_chain) = 0 ELSE true END \
         FROM worlds.world_scenes WHERE world_name = lower($1) AND mirror_source IS NOT NULL",
    )
    .bind(name)
    .fetch_all(&mut *conn)
    .await
    .with_context(|| format!("reading the scenes held for {name}"))?;
    let mut cids = Vec::new();
    let mut sizes = Vec::new();
    let mut chains = Vec::new();
    let mut deployers = Vec::new();
    let mut created = Vec::new();
    let mut updated = Vec::new();
    for (cid, entity, unsigned) in held {
        let Some(scene) = listed.iter().find(|scene| scene.id == cid) else {
            continue;
        };
        let (chain, deployer) = match unsigned {
            true => listed_signature(scene, &entity).unzip(),
            false => (None, None),
        };
        cids.push(cid);
        sizes.push(scene.size);
        chains.push(chain);
        deployers.push(deployer);
        created.push(scene.created_at);
        updated.push(scene.updated_at);
    }
    sqlx::query(
        r#"UPDATE worlds.world_scenes s
           SET size = COALESCE(t.size, s.size),
               deployment_auth_chain = COALESCE(t.chain::json, s.deployment_auth_chain),
               deployer = COALESCE(t.deployer, s.deployer),
               created_at = COALESCE(t.created, s.created_at),
               updated_at = COALESCE(t.updated, t.created, s.updated_at)
           FROM unnest($2::text[], $3::bigint[], $4::jsonb[], $5::text[], $6::timestamp[],
                       $7::timestamp[]) AS t(cid, size, chain, deployer, created, updated)
           WHERE s.world_name = lower($1) AND s.entity_id = t.cid AND s.mirror_source IS NOT NULL
             AND (t.chain IS NOT NULL
                  OR (s.size, s.created_at, s.updated_at) IS DISTINCT FROM
                     (COALESCE(t.size, s.size), COALESCE(t.created, s.created_at),
                      COALESCE(t.updated, t.created, s.updated_at)))"#,
    )
    .bind(name)
    .bind(&cids)
    .bind(&sizes)
    .bind(&chains)
    .bind(&deployers)
    .bind(&created)
    .bind(&updated)
    .execute(&mut *conn)
    .await
    .with_context(|| format!("refreshing the scenes of {name}"))?;
    Ok(())
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
            // The scenes are current. The access setting and the world settings change upstream
            // without a deployment, and the listing carries what rows written before it was kept
            // (or while it gave none) lack: the deploy times, size, chain and deployer.
            // An unreadable access setting keeps the stored one and the rest of the sync.
            let access = upstream_access(client, source, name)
                .await
                .inspect_err(|e| {
                    tracing::warn!(world = %name, error = %e, "keeping the stored access setting");
                })
                .ok();
            let listed = listed_scenes(client, source, name).await?;
            let world_settings = upstream_settings(client, pool, storage, source, name).await;
            let world = WorldName::parse(name)?;
            let source_owner = access.as_ref().and_then(|a| a.owner.as_ref());
            let mut tx = pool.begin().await.context("begin tx")?;
            if let Some(access) = &access {
                replace_mirrored_access(&mut tx, &world, &access.setting, source_owner)
                    .await
                    .with_context(|| format!("access setting of {name}"))?;
            }
            if let Some(copy) = &world_settings {
                apply_settings(&mut tx, &world, copy, source_owner).await?;
            }
            refresh_listed_scenes(&mut tx, name, &listed).await?;
            tx.commit().await.context("commit tx")?;
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
    let Some((_, base)) = scene_refs(&about).first().cloned() else {
        return Ok(Outcome::NothingUpstream("upstream world has no scenes"));
    };
    // `/about` lists the newest scene only; `/scenes` lists them all, under the same `baseUrl`.
    // The listing decides what is retired below, so any doubt about it stops this world's sync.
    let listed = listed_scenes(client, source, name).await?;
    if listed.is_empty() {
        return Ok(Outcome::NothingUpstream("upstream world has no scenes"));
    }
    // A scene held here and missing from the listing is retired, with its storage identity. One
    // the source still lists by id means the listing moved under the pages (a redeploy shifts
    // every later row), so the world waits for the next pass instead.
    for cid in held
        .iter()
        .filter(|cid| !listed.iter().any(|scene| &scene.id == *cid))
    {
        if source_lists_scene(client, source, name, cid).await? {
            bail!("the scene listing of {name} changed while it was read ({cid} is still listed)");
        }
    }
    // The world's settings as the source stores them, not as its `/about` derives them from the
    // newest scene: a fixed hour or offline adapter there may be the scene's own.
    let world_settings = upstream_settings(client, pool, storage, source, name).await;
    // Without that answer a new row takes the fixed hour and offline adapter the source's
    // `/about` serves (its stored value whenever it has one, `worlds-manager.ts` `getWorldInfo`),
    // so this node's `/about` answers the same until a later pass reads the settings.
    let about_skybox_time = about["configurations"]["skybox"]["fixedHour"]
        .as_f64()
        .and_then(storable_skybox_time);
    let about_single_player =
        about["comms"]["adapter"].as_str() == Some("fixed-adapter:offline:offline");
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
    let mut scenes = Vec::with_capacity(listed.len());
    for listed in listed {
        let cid = listed.id.clone();
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
        scenes.push((listed, file, entity, signed));
    }
    // Asked once the scenes check, so a refused read-through asks nothing more.
    let access = upstream_access(client, source, name).await?;

    let mut new_blobs = 0usize;
    let mut scene_base_spawn: Option<String> = None;
    let mut records: Vec<SceneRecord> = Vec::new();
    let blob_sem = Arc::new(Semaphore::new(settings.blob_concurrency));
    for (listed, file, entity, signed) in scenes {
        let cid = listed.id.clone();
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
        // The deployment's own size, chain and signer as the source lists them; a pass keeps the
        // chain only when it signs this scene, a read-through the one it checked.
        let (chain, deployer) = match signed {
            Some(signed) => {
                size = signed.size;
                (signed.json, signed.signer.as_str().to_string())
            }
            None => {
                size = listed.size.unwrap_or(size);
                listed_signature(&listed, &entity)
                    .unwrap_or_else(|| (Value::Array(Vec::new()), ZERO_ADDRESS.to_string()))
            }
        };
        records.push(SceneRecord {
            cid,
            entity,
            parcels,
            size,
            chain,
            deployer,
            created_at: listed.created_at,
            updated_at: listed.updated_at,
        });
    }

    let mut tx = pool.begin().await.context("begin tx")?;
    // A new row takes the source's settings (the `/about` fallback without them); a held one
    // keeps its settings here and takes the answer through `apply_settings` below.
    let copied = world_settings.unwrap_or_default();
    let (skybox_time, single_player) = match copied.present {
        true => (copied.skybox_time.flatten(), copied.single_player),
        false => (about_skybox_time, Some(about_single_player)),
    };
    let upserted = sqlx::query(
        r#"INSERT INTO worlds.worlds AS w
             (name, spawn_coordinates, title, description, content_rating, categories,
              thumbnail_hash, skybox_time, single_player, show_in_places, settings_version,
              blocked_since, mirror_source, updated_at)
           VALUES ($1, COALESCE($2, $3), $6, $7, $8, $9::text[], $10, $11, COALESCE($12, false),
                   COALESCE($13, true), COALESCE($14, 0), NULL, $4, timezone('UTC', now()))
           ON CONFLICT (name) DO UPDATE
             SET spawn_coordinates = COALESCE($2, w.spawn_coordinates, $3),
                 mirror_source = EXCLUDED.mirror_source,
                 updated_at = timezone('UTC', now())
             WHERE NOT $5
                OR (w.mirror_source IS NOT NULL AND w.blocked_since IS NULL)"#,
    )
    .bind(name)
    .bind(&upstream_spawn)
    .bind(&scene_base_spawn)
    .bind(source)
    .bind(verify.is_some())
    .bind(&copied.title)
    .bind(&copied.description)
    .bind(&copied.content_rating)
    .bind(&copied.categories)
    .bind(&copied.thumbnail_hash)
    .bind(skybox_time)
    .bind(single_player)
    .bind(copied.show_in_places)
    .bind(copied.settings_version)
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
    let world = WorldName::parse(name)?;
    replace_mirrored_access(&mut tx, &world, &access.setting, access.owner.as_ref())
        .await
        .with_context(|| format!("access setting of {name}"))?;
    // A held row keeps its settings without an answer: the `/about` values mix the newest
    // scene's own with the stored ones, so writing them would flap with each failed read.
    if copied.present {
        apply_settings(&mut tx, &world, &copied, access.owner.as_ref()).await?;
    }

    let cids: Vec<&str> = records.iter().map(|r| r.cid.as_str()).collect();
    let entities: Vec<Value> = records.iter().map(|r| r.entity.clone()).collect();
    let parcels: Vec<Value> = records
        .iter()
        .map(|r| Value::from(r.parcels.clone()))
        .collect();
    let sizes: Vec<i64> = records.iter().map(|r| r.size).collect();
    let chains: Vec<Value> = records.iter().map(|r| r.chain.clone()).collect();
    let deployers: Vec<&str> = records.iter().map(|r| r.deployer.as_str()).collect();
    let created: Vec<Option<chrono::NaiveDateTime>> =
        records.iter().map(|r| r.created_at).collect();
    let updated: Vec<Option<chrono::NaiveDateTime>> =
        records.iter().map(|r| r.updated_at).collect();
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
    // Each row keeps its deployment's times upstream (`/about` takes the newest, `/scenes` pages
    // by them, `/worlds` sorts by them); a row the listing gave none keeps this node's.
    sqlx::query(
        r#"UPDATE worlds.world_scenes s
           SET created_at = COALESCE(t.created, s.created_at),
               updated_at = COALESCE(t.updated, t.created, s.updated_at)
           FROM unnest($2::text[], $3::timestamp[], $4::timestamp[]) AS t(cid, created, updated)
           WHERE s.world_name = lower($1) AND s.entity_id = t.cid
             AND (t.created IS NOT NULL OR t.updated IS NOT NULL)"#,
    )
    .bind(name)
    .bind(&cids)
    .bind(&created)
    .bind(&updated)
    .execute(&mut *tx)
    .await
    .with_context(|| format!("deploy times of the scenes of {name}"))?;

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
    // Apply the overlap rule in deployment order:
    // a replacement must inherit its predecessor before a later scene takes
    // a parcel that predecessor vacated. CID breaks timestamp ties stably.
    let mut chronological: Vec<_> = records.iter().collect();
    chronological.sort_by(|a, b| {
        let ts =
            |entity: &Value| crate::core::store::entity_timestamp_number(entity).unwrap_or(0.0);
        ts(&a.entity)
            .total_cmp(&ts(&b.entity))
            .then_with(|| a.cid.cmp(&b.cid))
    });
    for record in chronological {
        scene_identity::assign(&mut tx, &world, &record.cid, &record.parcels)
            .await
            .with_context(|| format!("storage identity of {} in {name}", record.cid))?;
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

/// One row of the source's `/world/{name}/scenes` listing: the scene id and what its deployment
/// stored there (`toWorldScene`). A pass keeps the times, and the chain and deployer once they
/// check ([`listed_signature`]); a read-through keeps the times and its own checked chain.
struct ListedScene {
    id: String,
    /// `createdAt` and `updatedAt` in UTC; `None` when absent or not RFC 3339.
    created_at: Option<chrono::NaiveDateTime>,
    updated_at: Option<chrono::NaiveDateTime>,
    deployer: Option<String>,
    /// `deploymentAuthChain`, `Null` when absent or over [`MAX_CHAIN_BYTES`].
    chain: Value,
    /// `size` (a decimal string upstream), `None` unless a non-negative integer.
    size: Option<i64>,
}

impl ListedScene {
    fn from_row(id: &str, row: &Value) -> Self {
        let time = |key: &str| {
            row[key]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.naive_utc())
        };
        let chain = &row["deploymentAuthChain"];
        let chain = match serde_json::to_vec(chain) {
            Ok(bytes) if bytes.len() <= MAX_CHAIN_BYTES => chain.clone(),
            _ => Value::Null,
        };
        let size = match &row["size"] {
            Value::String(s) => s.parse().ok(),
            other => other.as_i64(),
        };
        Self {
            id: id.to_string(),
            created_at: time("createdAt"),
            updated_at: time("updatedAt"),
            deployer: row["deployer"].as_str().map(String::from),
            chain,
            size: size.filter(|s| *s >= 0),
        }
    }
}

/// The listed chain and deployer of a scene a pass stores, kept only when the chain is ECDSA
/// links that sign `entity`'s id (ephemeral keys judged at its timestamp) and its signer is the
/// listed deployer; `None` keeps the pass's empty chain and zero address. Nothing grants rights
/// from these columns, so a checked copy changes no authority here.
fn listed_signature(listed: &ListedScene, entity: &Value) -> Option<(Value, String)> {
    let deployer = Address::parse(listed.deployer.as_deref()?).ok()?;
    let chain: AuthChain = serde_json::from_value(listed.chain.clone()).ok()?;
    if Address::parse(&chain.first()?.payload).ok()? != deployer {
        return None;
    }
    let timestamp = crate::core::store::entity_timestamp_ms(entity)?;
    verify_auth_chain(&chain, &listed.id, Some(timestamp)).ok()?;
    Some((listed.chain.clone(), deployer.as_str().to_string()))
}

/// Every scene `{source}/world/{name}/scenes` lists, newest first by publication (it pages
/// oldest first by `createdAt`). Any refusal, a page without its `scenes`, a repeated id or a
/// count other than the listing's `total` is an error.
async fn listed_scenes(
    client: &crate::http::Outbound,
    source: &str,
    name: &str,
) -> Result<Vec<ListedScene>> {
    let listing = url::Url::parse(&world_url(source, name, "scenes")?)?;
    let mut scenes: Vec<ListedScene> = Vec::new();
    let mut offset = 0usize;
    let mut total;
    loop {
        let mut url = listing.clone();
        url.query_pairs_mut()
            .append_pair("limit", &SCENES_PAGE.to_string())
            .append_pair("offset", &offset.to_string());
        let resp = client
            .get(url.as_str())
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        if !status.is_success() {
            bail!("GET {url} answered {status}");
        }
        let page: Value =
            serde_json::from_slice(&read_body_capped(resp, MAX_SCENES_PAGE_BYTES).await?)
                .with_context(|| format!("parsing {url}"))?;
        total = page["total"]
            .as_u64()
            .with_context(|| format!("{url} answered no total"))? as usize;
        if total > MAX_LISTED_SCENES {
            bail!("{name} lists {total} scenes, over {MAX_LISTED_SCENES}");
        }
        let rows = page["scenes"]
            .as_array()
            .with_context(|| format!("{url} answered no scenes"))?;
        for row in rows {
            let id = row["entityId"]
                .as_str()
                .filter(|id| is_canonical_cid(id))
                .with_context(|| format!("{url} lists a scene without a canonical entityId"))?;
            if scenes.iter().any(|listed| listed.id == id) {
                bail!("{url} lists scene {id} twice");
            }
            scenes.push(ListedScene::from_row(id, row));
        }
        offset += rows.len();
        if rows.is_empty() || offset >= total {
            break;
        }
    }
    if scenes.len() != total {
        bail!(
            "the scene listing of {name} names {} scenes of its total {total}",
            scenes.len()
        );
    }
    scenes.reverse();
    Ok(scenes)
}

/// Whether `{source}/world/{name}/scenes?entity_id=<cid>` still lists the scene.
async fn source_lists_scene(
    client: &crate::http::Outbound,
    source: &str,
    name: &str,
    cid: &str,
) -> Result<bool> {
    let mut url = url::Url::parse(&world_url(source, name, "scenes")?)?;
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
    let listing: Value =
        serde_json::from_slice(&read_body_capped(resp, MAX_SCENE_ROW_BYTES).await?)
            .with_context(|| format!("parsing {url}"))?;
    let rows = listing["scenes"]
        .as_array()
        .with_context(|| format!("{url} answered no scenes"))?;
    Ok(rows.iter().any(|row| row["entityId"] == cid))
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

    /// `permissions.access` as upstream's `removeSecrets` answers it, stored as this node can:
    /// a shared secret no secret matches, an allow-list as listed, anything else refused.
    #[test]
    fn upstream_access_is_copied_or_refused() {
        let access =
            |value: Value| parse_upstream_access(&json!({"permissions": {"access": value}}));
        assert_eq!(
            access(json!({"type": "unrestricted"})).unwrap(),
            AccessSetting::Unrestricted
        );
        let secret = access(json!({"type": "shared-secret"})).unwrap();
        let AccessSetting::SharedSecret { secret_hash } = &secret else {
            panic!("{secret:?}");
        };
        assert!(!bcrypt::verify("", secret_hash).unwrap_or(false));
        assert_eq!(
            access(json!({"type": "nft-ownership", "nft": "urn:x"})).unwrap(),
            AccessSetting::NftOwnership {
                nft: "urn:x".into()
            }
        );
        assert_eq!(
            access(json!({"type": "allow-list", "wallets": ["0xA", 1], "communities": ["c"]}))
                .unwrap(),
            AccessSetting::AllowList {
                wallets: vec!["0xA".into()],
                communities: vec!["c".into()],
            }
        );
        for refused in [
            json!({"type": "nft-ownership"}),
            json!({"type": "members-only"}),
            json!(null),
        ] {
            assert!(access(refused.clone()).is_err(), "{refused}");
        }
    }

    /// `toSnakeCaseSettings` leaves out empty text and NULL categories and answers
    /// `skybox_time: null`; only what the columns accept is copied.
    #[test]
    fn upstream_settings_are_copied_as_the_columns_accept_them() {
        let copy = WorldSettingsCopy::parse(&json!({
            "title": "Globe", "description": "", "content_rating": "X",
            "categories": ["art", 1, "games"], "skybox_time": null, "single_player": true,
            "thumbnail_hash": "bafkrei/../x", "access_type": "unrestricted", "settings_version": 7
        }))
        .unwrap();
        assert_eq!(
            copy,
            WorldSettingsCopy {
                present: true,
                title: Some("Globe".into()),
                categories: Some(vec!["art".into(), "games".into()]),
                skybox_time: Some(None),
                single_player: Some(true),
                settings_version: Some(7),
                ..WorldSettingsCopy::default()
            }
        );
        let copy = WorldSettingsCopy::parse(&json!({
            "content_rating": "T", "skybox_time": 43200.0, "settings_version": -1,
            "categories": vec!["c"; 25], "thumbnail_hash": "a".repeat(64)
        }))
        .unwrap();
        assert_eq!(copy.content_rating.as_deref(), Some("T"));
        assert_eq!(copy.skybox_time, Some(Some(43200)));
        assert_eq!(copy.settings_version, None);
        assert_eq!(copy.categories.unwrap().len(), MAX_CATEGORIES);
        assert_eq!(copy.thumbnail_hash, Some("a".repeat(64)));
        // A thumbnail key is a canonical CID or a lowercase SHA-256 hex, nothing else.
        let thumbnail = |key: &str| {
            WorldSettingsCopy::parse(&json!({"thumbnail_hash": key}))
                .unwrap()
                .thumbnail_hash
        };
        let cid = crate::auth::hash_bytes_v1(b"thumbnail");
        assert_eq!(thumbnail(&cid), Some(cid.clone()));
        for refused in ["abc", &"A".repeat(64), &"g".repeat(64), &"a".repeat(65)] {
            assert_eq!(thumbnail(refused), None, "{refused}");
        }
        assert_eq!(WorldSettingsCopy::parse(&json!([])), None);
    }

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
