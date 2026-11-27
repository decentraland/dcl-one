//! The two chain-derived mirrors of the stage-5 data plane:
//! `shop.collections` and `shop.items`, filled from the L1/L2
//! collections subgraphs by `collections-mirror` and `items-mirror`, one
//! `jobs::spawn_periodic` job each, one pass per configured network per tick,
//! at most [`MarketSettings::mirror_pages_per_tick`] pages of 1,000 rows a
//! tick, both whole walks by id (the items subgraph does not bump
//! `updatedAt` on mints, so there is no incremental keyset to trust;
//! `items`). The keyset (`cursor_id`; `cursor_updated_at` is kept for a
//! mirror that can use one) and the counters are persisted in
//! `shop.mirror_state` after every page, so a `kill -9`
//! resumes at that page; a page shorter than 1,000 completes the pass and
//! moves `last_complete_pass_at`, which the catalogue gate
//! ([`super::catalogue::begin`]) requires of every network the mirrors serve
//! before a route answers: a mirror without a complete pass answers the
//! unavailable envelope, never an empty catalogue presented as the truth. Readiness reads the marker, not the job switch: a restored or
//! read-only replica whose tables hold a complete pass serves it with
//! `MARKETPLACE_MIRRORS_ENABLED=false` (the way presence and price serve
//! their archives with the writer off); the switch only attaches the jobs,
//! and the envelope names it because that is what fills the table. The
//! configured collections subgraphs still say which networks a pass must
//! cover. The tables are a copy, so a route over them answers it stamped
//! with `as_of`, the oldest network's last complete pass ([`stamp_copies`]),
//! and the price-bearing routes follow the money rule on top
//! ([`price_verdict`]): past [`price_max_age`] they answer the envelope,
//! not a price the chain may no longer charge. v2 has no squid database
//! (README.md, "Data sources"); these two tables are
//! the local coverage the catalog, items, contracts and collections routes
//! need, and the economy relay's contract allowlist reads `collections` too.
//!
//! [`MarketSettings::mirror_pages_per_tick`]: super::MarketSettings

pub mod collections;
pub mod items;

use crate::shared::now_utc;
use std::sync::Arc;
use std::time::Duration;

use std::cell::Cell;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::chain_index::rpc::{Rpc, RpcError};
use crate::http::jsonrpc::RpcError as HttpRpcError;
use crate::jobs::{JobContext, Jobs, PeriodicCfg};
use crate::shared::as_of;

use super::graph::Network;
use super::{MarketplaceState, Source};

pub const DEFAULT_INTERVAL_SECS: u64 = 300;
pub const DEFAULT_PAGES_PER_TICK: usize = 50;
/// How long a route trusts its last `mirror_state` read.
pub const READY_TTL: Duration = Duration::from_secs(10);
/// The subgraphs' page cap, the keyset step.
pub const PAGE_SIZE: i64 = 1000;
/// Retry transient errors at the same keyset. An old failing pin may have
/// been pruned or orphaned, so the next tick starts a new verified pass.
const MAX_PIN_AGE: chrono::Duration = chrono::Duration::days(1);
const MAX_FAILED_PIN_AGE: chrono::Duration = chrono::Duration::hours(1);
const PAGE_FETCH_FAILURE: &str = "page fetch failed: ";
const PROOF_UNAVAILABLE: &str = "catalogue proof unavailable: ";
const PROOF_REJECTED: &str = "catalogue proof rejected: ";
/// graph-node's error for a `block: {hash}` constraint it has not indexed.
const GRAPH_UNKNOWN_BLOCK_HASH: &str = "no block with that hash found";
const MAINNET_PROOF_RETRY_WINDOW: chrono::Duration = chrono::Duration::minutes(10);
const MAINNET_MIRROR_BACKOFF: Duration = Duration::from_secs(5 * 60);

const PROOF_OUTCOMES: dcl_metrics::CounterFamily<3> = dcl_metrics::CounterFamily::new(
    "catalyst_catalogue_mirror_proof_outcomes_total",
    "Mainnet catalogue mirror proof failures and skips.",
    Some(dcl_metrics::Unit::Count),
    &["mirror", "network", "outcome"],
);
pub const FAMILIES: &[dcl_metrics::Descriptor] = &[PROOF_OUTCOMES.descriptor];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mirror {
    Collections,
    Items,
}

impl Mirror {
    pub fn name(self) -> &'static str {
        match self {
            Mirror::Collections => "collections",
            Mirror::Items => "items",
        }
    }

    pub fn source(self) -> Source {
        match self {
            Mirror::Collections => Source::CollectionsMirror,
            Mirror::Items => Source::ItemsMirror,
        }
    }

    fn interval_multiplier(self) -> u32 {
        match self {
            // The collections walk is a whole walk (about 10^4 rows) every
            // three items ticks: 15 min at the default 5 min interval.
            Mirror::Collections => 3,
            Mirror::Items => 1,
        }
    }
}

/// One `(mirror, network)` row of `shop.mirror_state`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub mirror: Mirror,
    pub network: Network,
    pub source_url: String,
    pub chain_id: Option<i64>,
    pub pass_block_hash: Option<String>,
    pub pass_started_at: Option<NaiveDateTime>,
    pub cursor_updated_at: Option<i64>,
    pub cursor_id: Option<String>,
    pub pages_fetched: i64,
    pub rows_upserted: i64,
    pub last_complete_pass_at: Option<NaiveDateTime>,
    pub last_complete_pass_rows: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
}

impl Cursor {
    fn fresh(mirror: Mirror, network: Network, source_url: &str) -> Self {
        Self {
            mirror,
            network,
            source_url: source_url.to_string(),
            chain_id: None,
            pass_block_hash: None,
            pass_started_at: None,
            cursor_updated_at: None,
            cursor_id: None,
            pages_fetched: 0,
            rows_upserted: 0,
            last_complete_pass_at: None,
            last_complete_pass_rows: None,
            last_error: None,
            updated_at: None,
        }
    }

    fn restart_pagination(&mut self) {
        self.pass_started_at = None;
        self.pass_block_hash = None;
        self.cursor_id = None;
        self.cursor_updated_at = None;
    }

    /// A pass started and not completed since.
    pub fn in_progress(&self) -> bool {
        match (self.pass_started_at, self.last_complete_pass_at) {
            (Some(_), None) => true,
            (Some(s), Some(c)) => c < s,
            _ => false,
        }
    }
}

pub async fn load(
    pool: &PgPool,
    mirror: Mirror,
    network: Network,
) -> Result<Option<Cursor>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT source_url, chain_id, pass_block_hash, pass_started_at, cursor_updated_at, cursor_id, pages_fetched, rows_upserted, \
                last_complete_pass_at, last_complete_pass_rows, last_error, updated_at \
         FROM shop.mirror_state WHERE mirror = $1 AND network = $2",
    )
    .bind(mirror.name())
    .bind(network.db_name())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Cursor {
        mirror,
        network,
        source_url: r.get("source_url"),
        chain_id: r.get("chain_id"),
        pass_block_hash: r.get("pass_block_hash"),
        pass_started_at: r.get("pass_started_at"),
        cursor_updated_at: r.get("cursor_updated_at"),
        cursor_id: r.get("cursor_id"),
        pages_fetched: r.get("pages_fetched"),
        rows_upserted: r.get("rows_upserted"),
        last_complete_pass_at: r.get("last_complete_pass_at"),
        last_complete_pass_rows: r.get("last_complete_pass_rows"),
        last_error: r.get("last_error"),
        updated_at: r.get("updated_at"),
    }))
}

pub async fn save(pool: &PgPool, c: &Cursor) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO shop.mirror_state \
             (mirror, network, source_url, pass_started_at, cursor_updated_at, cursor_id, pages_fetched, \
              rows_upserted, last_complete_pass_at, last_complete_pass_rows, last_error, chain_id, pass_block_hash, marks_item_rows, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, timezone('UTC', now())) \
         ON CONFLICT (mirror, network) DO UPDATE SET \
             source_url = EXCLUDED.source_url, marks_item_rows = EXCLUDED.marks_item_rows, chain_id = EXCLUDED.chain_id, pass_block_hash = EXCLUDED.pass_block_hash, pass_started_at = EXCLUDED.pass_started_at, \
             cursor_updated_at = EXCLUDED.cursor_updated_at, cursor_id = EXCLUDED.cursor_id, \
             pages_fetched = EXCLUDED.pages_fetched, rows_upserted = EXCLUDED.rows_upserted, \
             last_complete_pass_at = EXCLUDED.last_complete_pass_at, \
             last_complete_pass_rows = EXCLUDED.last_complete_pass_rows, \
             last_error = EXCLUDED.last_error, updated_at = EXCLUDED.updated_at",
    )
    .bind(c.mirror.name())
    .bind(c.network.db_name())
    .bind(&c.source_url)
    .bind(c.pass_started_at)
    .bind(c.cursor_updated_at)
    .bind(&c.cursor_id)
    .bind(c.pages_fetched)
    .bind(c.rows_upserted)
    .bind(c.last_complete_pass_at)
    .bind(c.last_complete_pass_rows)
    .bind(&c.last_error)
    .bind(c.chain_id)
    .bind(&c.pass_block_hash)
    .bind(c.mirror == Mirror::Items && (c.pass_block_hash.is_some() || c.chain_id == Some(super::chain_walk::ChainNet::from(c.network).testnet_chain_id())))
    .execute(pool)
    .await?;
    Ok(())
}

fn catalogue_proof_rpc(
    state: &MarketplaceState,
    net: super::chain_walk::ChainNet,
) -> Option<String> {
    state.catalogue_rpc_url(net).or_else(|| {
        if net == super::chain_walk::ChainNet::Matic
            && state.settings.market.indexes.serves_mainnet(net)
        {
            state.polygon_mainnet_rpc_url()
        } else {
            None
        }
    })
}

#[derive(Debug, thiserror::Error)]
pub(super) enum CatalogueProofError {
    #[error("catalogue proof RPC unavailable")]
    RpcUnavailable,
    #[error("catalogue proof RPC returned status {0}")]
    RpcStatus(u16),
    #[error("{0}")]
    Disagreement(String),
    #[error("{0}")]
    GraphUnavailable(String),
}

impl From<RpcError> for CatalogueProofError {
    fn from(error: RpcError) -> Self {
        match error {
            RpcError::Provider(HttpRpcError::Status(status)) => Self::RpcStatus(status),
            _ => Self::RpcUnavailable,
        }
    }
}

fn block_hash(value: &Value) -> Option<&str> {
    value.as_str().filter(|hash| {
        hash.len() == 66
            && hash.starts_with("0x")
            && hash[2..].bytes().all(|b| b.is_ascii_hexdigit())
    })
}

/// Pin the pass below the graph head on the configured chain. Mainnet also
/// bounds the pin by the RPC head because the two providers can lag independently.
/// Persisting the hash with the keyset lets later ticks resume without RPC.
pub(super) async fn catalogue_pass_hash(
    state: &MarketplaceState,
    net: super::chain_walk::ChainNet,
    url: &str,
) -> Result<String, CatalogueProofError> {
    use super::chain_walk::{LogSource, RpcSource, WrongChain};
    use CatalogueProofError::{Disagreement, GraphUnavailable, RpcUnavailable};
    use serde_json::json;
    let head = super::readers::query(state, url, "{ _meta { block { number } } }", json!({}))
        .await
        .map_err(GraphUnavailable)?;
    let graph_head = head["_meta"]["block"]["number"]
        .as_u64()
        .ok_or_else(|| GraphUnavailable("collections graph has no block number".into()))?;
    let endpoint = catalogue_proof_rpc(state, net).ok_or(RpcUnavailable)?;
    let source = RpcSource::new(Rpc::at(&state.http, &endpoint).with_strict_status());
    let indexes = &state.settings.market.indexes;
    super::ensure_chain(&source, indexes.chain_id(net))
        .await
        .map_err(|error| {
            if error.is::<WrongChain>() {
                Disagreement(error.to_string())
            } else {
                error
                    .downcast::<RpcError>()
                    .map_or(RpcUnavailable, CatalogueProofError::from)
            }
        })?;
    let head = if indexes.serves_mainnet(net) {
        graph_head.min(source.head().await?.value)
    } else {
        graph_head
    };
    let number = head.saturating_sub(net.confirmations());
    // graph-node answers `hash: null` to a `_meta` pinned by block number, so
    // the graph is asked for the configured chain's block at that height by
    // hash: a graph on another chain has no such block, or places it elsewhere.
    let header = source.header(number).await?.value;
    let data = match super::readers::query(state, url,
        "query CatalogueBlock($hash: Bytes!) { _meta(block: {hash: $hash}) { block { number hash } } }",
        json!({"hash":header.hash})).await
    {
        Ok(data) => data,
        Err(error) if error.contains(GRAPH_UNKNOWN_BLOCK_HASH) => {
            return Err(Disagreement(format!(
                "collections graph block {number} differs from the configured chain"
            )));
        }
        Err(error) => return Err(GraphUnavailable(error)),
    };
    let block = &data["_meta"]["block"];
    let hash = block_hash(&block["hash"]).ok_or_else(|| {
        GraphUnavailable("collections graph has no block hash or returned an invalid hash".into())
    })?;
    // A missing height is incomplete graph data; only a different one disagrees.
    let height = block["number"]
        .as_u64()
        .ok_or_else(|| GraphUnavailable("collections graph has no block number".into()))?;
    if height != number || !header.hash.eq_ignore_ascii_case(hash) {
        return Err(Disagreement(format!(
            "collections graph block {number} differs from the configured chain"
        )));
    }
    Ok(header.hash)
}

#[derive(Debug, thiserror::Error)]
pub(super) enum PageReadError {
    #[error("{0}")]
    Fetch(String),
    #[error("collections graph page has no block hash")]
    MissingHash,
    #[error("collections graph page differs from the pinned block")]
    HashMismatch,
}

pub(super) async fn read_catalogue_page(
    state: &MarketplaceState,
    url: &str,
    query: &str,
    from: Option<String>,
    hash: Option<String>,
) -> Result<Value, PageReadError> {
    let data = super::readers::query(state, url, query, serde_json::json!({
        "idFrom": from.unwrap_or_default(), "first": PAGE_SIZE,
        "verifyChain": hash.is_some(), "block": hash.as_ref().map(|h| serde_json::json!({"hash":h}))
    })).await.map_err(PageReadError::Fetch)?;
    if let Some(expected) = hash {
        let actual = data["_meta"]["block"]["hash"]
            .as_str()
            .ok_or(PageReadError::MissingHash)?;
        if !actual.eq_ignore_ascii_case(&expected) {
            return Err(PageReadError::HashMismatch);
        }
    }
    Ok(data)
}

/// A keyset resumes only for its source and binding. A legacy unbound
/// mainnet copy keeps its publication. A testnet pin without a chain ID
/// belongs to the first verified pass, which binds the row on completion.
pub(super) async fn cursor_for(
    pool: &PgPool,
    mirror: Mirror,
    network: Network,
    url: &str,
    chain_id: Option<i64>,
    pin: bool,
) -> anyhow::Result<Cursor> {
    let net = super::chain_walk::ChainNet::from(network);
    let mainnet = chain_id == Some(net.chain_id());
    let fresh = || Cursor {
        chain_id: chain_id.filter(|_| mainnet),
        ..Cursor::fresh(mirror, network, url)
    };
    let Some(mut c) = load(pool, mirror, network).await? else {
        return Ok(fresh());
    };
    if (chain_id.is_none() && c.chain_id.is_some())
        || (!mainnet && !pin && c.in_progress() && c.pass_block_hash.is_some())
    {
        anyhow::bail!("a verified catalogue mirror requires its chain RPC to continue");
    }
    if c.chain_id
        .zip(chain_id)
        .is_some_and(|(old, new)| old != new)
    {
        anyhow::bail!("catalogue mirror is already bound to a different chain");
    }
    if c.chain_id.is_none()
        && c.pass_block_hash.is_some()
        && chain_id != Some(net.testnet_chain_id())
    {
        anyhow::bail!("an unbound pinned catalogue pass requires its testnet chain");
    }
    if c.source_url != url {
        tracing::warn!(mirror = mirror.name(), network = network.db_name(), old = %c.source_url, "mirror source changed; the cursor restarts");
        return Ok(fresh());
    }
    let resume_mainnet = mainnet
        && c.chain_id == chain_id
        && c.in_progress()
        && (c.pages_fetched > 0
            || c.last_error.as_deref().is_some_and(|error| {
                error.starts_with(PROOF_UNAVAILABLE) || error.starts_with(PROOF_REJECTED)
            }));
    if mainnet {
        c.chain_id = chain_id;
    }
    if pin && c.pass_block_hash.is_none() && !resume_mainnet {
        // A new proof must start at page one. Descriptive readers can still
        // serve the previous complete copy until the replacement publishes.
        c.restart_pagination();
    }
    Ok(c)
}

async fn pin_new_pass(
    state: &MarketplaceState,
    c: &mut Cursor,
    net: super::chain_walk::ChainNet,
) -> anyhow::Result<()> {
    let error = match catalogue_pass_hash(state, net, &c.source_url).await {
        Ok(hash) => {
            c.pass_block_hash = Some(hash);
            c.last_error = None;
            save(&state.pool, c).await?;
            return Ok(());
        }
        Err(error) => error,
    };
    let previous = c.last_error.as_deref().unwrap_or_default();
    if state.settings.market.indexes.serves_mainnet(net) {
        if let CatalogueProofError::RpcStatus(status @ (401 | 403)) = &error {
            tracing::error!(
                mirror = c.mirror.name(),
                network = net.db_name(),
                status,
                "catalogue proof RPC authentication failed"
            );
            PROOF_OUTCOMES
                .handle([c.mirror.name(), net.db_name(), "rpc_auth_failed"])
                .increment(1);
        }
        match &error {
            CatalogueProofError::RpcUnavailable | CatalogueProofError::RpcStatus(_)
                if !previous.starts_with(PROOF_REJECTED) =>
            {
                let now = now_utc();
                let retrying = previous.starts_with(PROOF_UNAVAILABLE);
                if retrying
                    && c.pass_started_at
                        .is_some_and(|start| now - start >= MAINNET_PROOF_RETRY_WINDOW)
                {
                    tracing::warn!(mirror = c.mirror.name(), network = net.db_name(), %error,
                        "catalogue proof skipped: rpc unavailable");
                    PROOF_OUTCOMES
                        .handle([
                            c.mirror.name(),
                            net.db_name(),
                            "proof_skipped_rpc_unavailable",
                        ])
                        .increment(1);
                    return Ok(());
                }
                if !retrying {
                    c.pass_started_at = Some(now);
                }
                c.last_error = Some(format!("{PROOF_UNAVAILABLE}{error}"));
                save(&state.pool, c).await?;
            }
            CatalogueProofError::RpcUnavailable | CatalogueProofError::RpcStatus(_) => {
                // A transport failure cannot supersede prior conflicting evidence.
            }
            CatalogueProofError::Disagreement(_) => {
                tracing::error!(mirror = c.mirror.name(), network = net.db_name(), %error,
                    "catalogue proof rejected");
                PROOF_OUTCOMES
                    .handle([c.mirror.name(), net.db_name(), "proof_rejected"])
                    .increment(1);
                c.last_error = Some(format!("{PROOF_REJECTED}{error}"));
                save(&state.pool, c).await?;
            }
            CatalogueProofError::GraphUnavailable(_) => {
                tracing::warn!(mirror = c.mirror.name(), network = net.db_name(), %error,
                    "catalogue proof graph unavailable");
                PROOF_OUTCOMES
                    .handle([c.mirror.name(), net.db_name(), "graph_unavailable"])
                    .increment(1);
                if !previous.starts_with(PROOF_REJECTED) && !previous.starts_with(PROOF_UNAVAILABLE)
                {
                    c.last_error = Some(error.to_string());
                    save(&state.pool, c).await?;
                }
            }
        }
    } else {
        c.last_error = Some(error.to_string());
        save(&state.pool, c).await?;
    }
    Err(error.into())
}

/// What one page of a walk yields: the rows and the keyset after them.
pub(super) struct Page {
    pub rows: Vec<Value>,
    pub last_updated_at: Option<i64>,
    pub last_id: Option<String>,
}

/// The shared walk: page from the cursor, upsert, persist, until a short
/// page (the pass completes) or the tick's page budget is spent. `fetch`
/// asks the subgraph for the page after `(cursor_updated_at, cursor_id)`,
/// `upsert` writes it and answers how many rows landed.
#[allow(
    clippy::too_many_arguments,
    reason = "the mirror identity, the cursor mode and the two per-mirror callbacks"
)]
pub(super) async fn walk<F, FF, U, UF>(
    state: &MarketplaceState,
    ctx: &JobContext,
    mirror: Mirror,
    network: Network,
    url: &str,
    incremental: bool,
    fetch: F,
    upsert: U,
) -> anyhow::Result<()>
where
    F: Fn(Option<i64>, Option<String>, Option<String>) -> FF,
    FF: std::future::Future<Output = Result<Page, PageReadError>>,
    U: Fn(Vec<Value>, i32, bool) -> UF,
    UF: std::future::Future<Output = Result<i64, sqlx::Error>>,
{
    let pool = &state.pool;
    let net = super::chain_walk::ChainNet::from(network);
    let indexes = &state.settings.market.indexes;
    let configured = indexes.chain_id(net);
    let configured_i32 = crate::market::casts::to_i32(configured)?;
    let pin = catalogue_proof_rpc(state, net).is_some();
    let mainnet = indexes.serves_mainnet(net);
    let binding = (mainnet || pin).then_some(configured);
    let mut c = cursor_for(pool, mirror, network, url, binding, pin).await?;
    if c.in_progress()
        && c.pass_block_hash.is_some()
        && c.pass_started_at
            .is_some_and(|start| now_utc() - start >= MAX_PIN_AGE)
    {
        tracing::warn!(
            mirror = mirror.name(),
            network = network.db_name(),
            "catalogue pass exceeded its pin age"
        );
        c.restart_pagination();
    }
    if !c.in_progress() {
        c.pass_block_hash = None;
        c.pass_started_at = Some(now_utc());
        c.pages_fetched = 0;
        c.rows_upserted = 0;
        c.last_error = None;
        if !incremental {
            c.cursor_updated_at = None;
            c.cursor_id = None;
        }
        save(pool, &c).await?;
        tracing::info!(
            mirror = mirror.name(),
            network = network.db_name(),
            "mirror pass started"
        );
    } else {
        tracing::info!(mirror = mirror.name(), network = network.db_name(), cursor = ?c.cursor_id, "mirror pass resumes at the persisted cursor");
    }
    if pin && c.pass_block_hash.is_none() && c.pages_fetched == 0 {
        pin_new_pass(state, &mut c, net).await?;
    }
    // Keep the elapsed outage window if the first unpinned page also fails.
    let unpinned_retry = if pin && c.pass_block_hash.is_none() && c.pages_fetched == 0 {
        PROOF_UNAVAILABLE
    } else {
        ""
    };
    let budget = state.settings.market.mirror_pages_per_tick.max(1);
    for _ in 0..budget {
        if ctx.is_cancelled() {
            ctx.progress(format!(
                "{}/{}: paused at the persisted cursor (shutdown)",
                mirror.name(),
                network.db_name()
            ));
            return Ok(());
        }
        let page = match fetch(
            c.cursor_updated_at,
            c.cursor_id.clone(),
            c.pass_block_hash.clone(),
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                let repeated_failure = c
                    .last_error
                    .as_deref()
                    .is_some_and(|previous| previous.starts_with(PAGE_FETCH_FAILURE));
                c.last_error = Some(match &e {
                    PageReadError::Fetch(error) => {
                        format!("{unpinned_retry}{PAGE_FETCH_FAILURE}{error}")
                    }
                    _ => e.to_string(),
                });
                if c.pass_block_hash.is_some()
                    && (matches!(e, PageReadError::MissingHash | PageReadError::HashMismatch)
                        || (repeated_failure
                            && c.pass_started_at
                                .is_some_and(|start| now_utc() - start >= MAX_FAILED_PIN_AGE)))
                {
                    tracing::warn!(
                        mirror = mirror.name(),
                        network = network.db_name(),
                        "graph cannot serve the pinned block; restarting catalogue pass"
                    );
                    c.restart_pagination();
                }
                save(pool, &c).await?;
                anyhow::bail!(
                    "{} mirror ({}): page after {:?}: {e}",
                    mirror.name(),
                    network.db_name(),
                    c.cursor_id
                );
            }
        };
        let fetched = crate::market::casts::len_i64(page.rows.len());
        let short = fetched < PAGE_SIZE;
        let upserted = match upsert(page.rows, configured_i32, pin && !mainnet).await {
            Ok(n) => n,
            Err(e) => {
                c.last_error = Some(format!("{unpinned_retry}upsert failed: {e}"));
                save(pool, &c).await?;
                return Err(anyhow::Error::new(e).context(format!(
                    "{} mirror ({}): upsert",
                    mirror.name(),
                    network.db_name()
                )));
            }
        };
        c.pages_fetched += 1;
        c.rows_upserted += upserted;
        c.last_error = None;
        if let Some(u) = page.last_updated_at {
            c.cursor_updated_at = Some(u);
        }
        if let Some(id) = page.last_id {
            c.cursor_id = Some(id);
        }
        if short {
            c.chain_id = binding;
            c.last_complete_pass_at = Some(now_utc());
            c.last_complete_pass_rows = Some(c.rows_upserted);
            if !incremental {
                c.cursor_updated_at = None;
                c.cursor_id = None;
            }
        }
        save(pool, &c).await?;
        ctx.progress(format!(
            "{}/{}: {} pages, {} rows{}",
            mirror.name(),
            network.db_name(),
            c.pages_fetched,
            c.rows_upserted,
            if short { " (complete)" } else { "" }
        ));
        if short {
            tracing::info!(
                mirror = mirror.name(),
                network = network.db_name(),
                pages = c.pages_fetched,
                rows = c.rows_upserted,
                "mirror pass complete"
            );
            return Ok(());
        }
    }
    tracing::info!(
        mirror = mirror.name(),
        network = network.db_name(),
        pages = c.pages_fetched,
        "mirror page budget spent; continuing next tick"
    );
    Ok(())
}

/// One tick of `mirror`: every configured network in turn; a failed network
/// is recorded on its row and the others still run, the tick then fails so
/// the periodic backoff applies.
pub async fn run_tick(
    state: &MarketplaceState,
    mirror: Mirror,
    ctx: &JobContext,
) -> anyhow::Result<()> {
    let mut failures = Vec::new();
    for network in state.settings.collection_networks() {
        let Some(url) = network.url(&state.index) else {
            continue;
        };
        // A network the collections index owns is not mirrored (its pages
        // would not be written either: `collections::upsert`).
        if let Some(net) = super::chain_walk::ChainNet::parse(network.db_name()) {
            let mut conn = state.pool.acquire().await?;
            if super::collections_projection::owned(&mut conn, net).await? {
                tracing::info!(
                    mirror = mirror.name(),
                    network = network.db_name(),
                    "the collections index owns this network's rows; the mirror skips it"
                );
                continue;
            }
        }
        let result = match mirror {
            Mirror::Collections => collections::run_pass(state, ctx, network, url).await,
            Mirror::Items => items::run_pass(state, ctx, network, url).await,
        };
        if let Err(e) = result {
            tracing::warn!(mirror = mirror.name(), network = network.db_name(), error = %e, "mirror pass failed");
            failures.push(format!("{}: {e}", network.db_name()));
        }
        if ctx.is_cancelled() {
            break;
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{} mirror: {}", mirror.name(), failures.join("; "))
    }
}

fn periodic_config(state: &MarketplaceState, mirror: Mirror) -> PeriodicCfg {
    let proven_mainnet = state
        .settings
        .collection_networks()
        .into_iter()
        .any(|network| {
            let net = super::chain_walk::ChainNet::from(network);
            state.settings.market.indexes.serves_mainnet(net)
                && catalogue_proof_rpc(state, net).is_some()
        });
    let backoff = if proven_mainnet {
        MAINNET_MIRROR_BACKOFF
    } else {
        Duration::from_secs(3600)
    };
    PeriodicCfg::every(state.settings.market.mirror_interval * mirror.interval_multiplier())
        .with_jitter(Duration::from_secs(30))
        .with_error_backoff(backoff)
}

pub fn spawn_jobs(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    if !state.settings.market.mirrors_enabled {
        tracing::info!(
            "MARKETPLACE_MIRRORS_ENABLED=false: the collections and items mirrors are off; the catalog routes serve the last complete pass the tables hold, the unavailable envelope without one"
        );
        return;
    }
    if state.settings.collection_networks().is_empty() {
        tracing::warn!(
            "MARKETPLACE_MIRRORS_ENABLED=true but no collections subgraph is configured: nothing to mirror"
        );
        return;
    }
    for mirror in [Mirror::Collections, Mirror::Items] {
        let st = state.clone();
        jobs.spawn_periodic_leased(
            format!("{}-mirror", mirror.name()),
            periodic_config(state, mirror),
            move |ctx| {
                let st = st.clone();
                async move { run_tick(&st, mirror, &ctx).await }
            },
        );
    }
}

/// Whether every pass in a mirror's `rows` that names its chain names the
/// one this node serves for that network (an unbound legacy pass counts).
pub(crate) fn bound(state: &MarketplaceState, rows: &[PassRow]) -> bool {
    let pair = crate::land_authz::land_network(&state.settings.eth_network)
        .map(super::nft_index::Catalogue::from_land);
    rows.iter().all(|(network, _, chain)| {
        chain.is_none_or(|id| {
            super::chain_walk::ChainNet::parse(network)
                .and_then(|net| pair.map(|pair| pair.chain_id(net)))
                == Some(id)
        })
    })
}

/// One network's row of a mirror's `shop.mirror_state`: its last complete
/// pass and the chain the pass was proven on.
pub(crate) type PassRow = (String, Option<NaiveDateTime>, Option<i64>);

/// The source and message a mirror without the pass a read needs answers:
/// none configured (`configured` false: nothing says which networks a pass
/// must cover), the job off on this node, or a configured network without a
/// complete pass.
pub(crate) fn not_ready_reason(
    state: &MarketplaceState,
    mirror: Mirror,
    configured: bool,
) -> (Source, String) {
    if !configured {
        return (
            Source::CollectionsSubgraph(Network::Matic),
            format!(
                "no collections subgraph is configured, so the {} mirror has nothing to walk",
                mirror.name()
            ),
        );
    }
    if !state.settings.market.mirrors_enabled {
        return (
            mirror.source(),
            format!(
                "the {} mirror has no complete pass in this database and the mirror job is not enabled on this node",
                mirror.name()
            ),
        );
    }
    (
        mirror.source(),
        format!(
            "the {} mirror has no complete pass yet for every configured network",
            mirror.name()
        ),
    )
}

/// The money budget of the item prices the items copy carries: four items
/// intervals, never under the MANA spot's 30 min.
pub fn price_max_age(settings: &super::Settings) -> Duration {
    super::trades::max_age(
        settings
            .market
            .mirror_interval
            .saturating_mul(Mirror::Items.interval_multiplier()),
    )
}

/// `Err` (why) when the items copy, last completed at `oldest`, is past the
/// money budget at `now`.
pub fn price_verdict(
    oldest: NaiveDateTime,
    now: DateTime<Utc>,
    budget: Duration,
) -> Result<(), String> {
    let age = (now.naive_utc() - oldest).num_seconds();
    if i64::try_from(budget.as_secs()).is_ok_and(|budget| age > budget) {
        return Err(format!(
            "the items mirror's last complete pass is {age} s old, past the {} s freshness budget of the prices it carries (the items-mirror job is off or failing)",
            budget.as_secs()
        ));
    }
    Ok(())
}

/// The copies a request's handler read ([`stamp_copies`]): the oldest
/// pass, and when the first of them goes stale.
#[derive(Clone, Copy)]
struct Read {
    as_of: NaiveDateTime,
    stale_at: NaiveDateTime,
}

tokio::task_local! {
    static COPY_AS_OF: Cell<Option<Read>>;
}

/// A copy goes stale (`as_of::stale_after`) after twice its mirror's
/// interval (the collections walk's three items intervals).
pub fn stale_after(settings: &super::Settings, mirror: Mirror) -> Duration {
    as_of::stale_after(
        settings
            .market
            .mirror_interval
            .saturating_mul(mirror.interval_multiplier()),
    )
}

/// A mirror copy read by the catalogue gate ([`super::catalogue::begin`]),
/// last completed at `at`.
pub(crate) fn note_copy_at(state: &MarketplaceState, mirror: Mirror, at: NaiveDateTime) {
    note_read(at, stale_after(&state.settings, mirror));
}

/// The collections index's published fold, read by the catalogue gate: it
/// goes stale past [`super::chain_walk::READY_WINDOW`], when a price may no
/// longer be read from it.
pub(crate) fn note_index_copy(folded_at: NaiveDateTime) {
    note_read(
        folded_at,
        super::chain_walk::READY_WINDOW
            .to_std()
            .unwrap_or(Duration::MAX),
    );
}

/// Records, for [`stamp_copies`], a copy read at `at` that goes stale after
/// `stale_after`.
fn note_read(at: NaiveDateTime, stale_after: Duration) {
    let budget = chrono::Duration::from_std(stale_after).unwrap_or(chrono::Duration::MAX);
    let stale_at = at.checked_add_signed(budget).unwrap_or(NaiveDateTime::MAX);
    let _ = COPY_AS_OF.try_with(|c| {
        c.set(Some(match c.get() {
            Some(seen) => Read {
                as_of: seen.as_of.min(at),
                stale_at: seen.stale_at.min(stale_at),
            },
            None => Read {
                as_of: at,
                stale_at,
            },
        }))
    });
}

/// Router middleware: an answer whose handler read a mirror or the index
/// through the catalogue gate carries that copy's `as_of` (the oldest, when it read
/// both) and `stale_after_secs`; every other answer passes as it came. A
/// `/lambdas/*` answer carries them as the `x-as-of` and
/// `x-stale-after-secs` headers only: its body is lamb2's, which has no such
/// keys (decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:50-53).
/// A HEAD runs as its GET so its length is the stamped answer's.
pub async fn stamp_copies(request: Request, next: Next) -> Response {
    let headers_only = request.uri().path().starts_with("/lambdas/");
    let (request, head) = as_of::as_get(request);
    let (response, read) = COPY_AS_OF
        .scope(Cell::new(None), async move {
            let response = next.run(request).await;
            (response, COPY_AS_OF.with(Cell::get))
        })
        .await;
    let response = match read {
        Some(Read { as_of, stale_at }) => {
            let stale_after = (stale_at - as_of).to_std().unwrap_or_default();
            if headers_only {
                as_of::stamp_headers(response, as_of.and_utc(), stale_after)
            } else {
                as_of::stamp(response, as_of.and_utc(), stale_after).await
            }
        }
        None => response,
    };
    if head {
        as_of::head_of(response).await
    } else {
        response
    }
}

/// `/v1/trades-mirror`'s sibling: the pass markers of both mirrors.
pub async fn status(pool: &PgPool) -> Result<Vec<Cursor>, sqlx::Error> {
    let mut out = Vec::new();
    for mirror in [Mirror::Collections, Mirror::Items] {
        for network in [Network::Ethereum, Network::Matic] {
            if let Some(c) = load(pool, mirror, network).await? {
                out.push(c);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_is_in_progress_between_its_start_and_its_completion() {
        let mut c = Cursor::fresh(Mirror::Items, Network::Matic, "https://x.example/graphql");
        assert!(!c.in_progress());
        let t0 = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        c.pass_started_at = Some(t0);
        assert!(c.in_progress());
        c.last_complete_pass_at = Some(t0 + chrono::Duration::seconds(5));
        assert!(!c.in_progress());
        c.pass_started_at = Some(t0 + chrono::Duration::seconds(10));
        assert!(c.in_progress());
        assert_eq!(Mirror::Collections.source(), Source::CollectionsMirror);
        assert_eq!(Mirror::Items.name(), "items");
    }

    #[tokio::test]
    async fn mainnet_proof_retries_fit_within_the_default_price_budget() {
        let mut state = crate::market::tests::support::state_without_db();
        let settings = &mut Arc::get_mut(&mut state).unwrap().settings;
        settings.polygon_rpc_url = Some("https://fixture.invalid".into());
        settings.matic_collections_subgraph_url = Some("https://fixture.invalid".into());
        let schedule = periodic_config(&state, Mirror::Items);
        let cap = schedule.wait_after_failure(u32::MAX);
        assert_eq!(cap, Duration::from_secs(300));
        assert!(
            schedule.interval + MAINNET_PROOF_RETRY_WINDOW.to_std().unwrap() + cap
                < price_max_age(&state.settings)
        );
        Arc::get_mut(&mut state).unwrap().settings.polygon_rpc_url = None;
        assert_eq!(
            periodic_config(&state, Mirror::Items).wait_after_failure(u32::MAX),
            Duration::from_secs(3600)
        );
    }

    /// The money rule over the prices the items copy carries, on a fixed
    /// clock: four items intervals, floored at 30 min.
    #[test]
    fn a_price_older_than_its_budget_is_not_served() {
        let mut settings = super::super::Settings::for_tests();
        settings.market.mirror_interval = Duration::from_secs(300);
        assert_eq!(price_max_age(&settings), Duration::from_secs(30 * 60));
        settings.market.mirror_interval = Duration::from_secs(900);
        let budget = price_max_age(&settings);
        assert_eq!(budget, Duration::from_secs(3600));
        let now = chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let ago = |s: i64| (now - chrono::Duration::seconds(s)).naive_utc();
        assert!(price_verdict(ago(3600), now, budget).is_ok());
        let why = price_verdict(ago(3601), now, budget).unwrap_err();
        assert!(
            why.contains("3601 s old") && why.contains("3600 s freshness budget"),
            "{why}"
        );
    }

    /// The switch attaches the job; the marker decides the reads, so a
    /// replica with a complete pass in its tables serves with the job off.
    #[tokio::test]
    async fn a_complete_pass_serves_with_the_mirror_job_off() {
        use crate::market::catalogue::{self, Access, Purpose};
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let state = super::super::MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            super::super::Settings {
                matic_collections_subgraph_url: Some("https://subgraph.example/matic".into()),
                ..super::super::Settings::for_tests()
            },
        );
        assert!(!state.settings.market.mirrors_enabled);
        let read = || {
            catalogue::begin(
                &state,
                Purpose::Describe,
                &[Mirror::Items],
                None,
                Access::ReadOnly,
            )
        };
        let err = read().await.err().unwrap();
        assert_eq!(
            err.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "no pass in the tables"
        );
        let mut c = Cursor::fresh(
            Mirror::Items,
            Network::Matic,
            "https://subgraph.example/matic",
        );
        c.last_complete_pass_at = Some(now_utc());
        save(&pool, &c).await.unwrap();
        assert!(read().await.is_ok(), "the marker, not the switch, decides");
        pool.close().await;
    }
}
