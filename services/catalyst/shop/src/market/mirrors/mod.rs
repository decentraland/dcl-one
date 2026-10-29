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
//! moves `last_complete_pass_at`, which every route over a mirror requires
//! ([`require_ready`]) before it answers: a mirror without a complete pass
//! answers the unavailable envelope, never an empty catalogue presented as
//! the truth. Readiness reads the marker, not the job switch: a restored or
//! read-only replica whose tables hold a complete pass serves it with
//! `MARKETPLACE_MIRRORS_ENABLED=false` (the way presence and price serve
//! their archives with the writer off); the switch only attaches the jobs,
//! and the envelope names it because that is what fills the table. The
//! configured collections subgraphs still say which networks a pass must
//! cover. The tables are a copy, so a route over them answers it stamped
//! with `as_of`, the oldest network's last complete pass ([`stamp_copies`]),
//! and the price-bearing routes follow the money rule on top
//! ([`require_priced`]): past [`price_max_age`] they answer the envelope,
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

use crate::jobs::{JobContext, Jobs, PeriodicCfg};
use crate::shared::as_of;

use super::graph::Network;
use super::{MarketplaceState, Source, unavailable};

pub const DEFAULT_INTERVAL_SECS: u64 = 300;
pub const DEFAULT_PAGES_PER_TICK: usize = 50;
/// How long a route trusts its last `mirror_state` read.
pub const READY_TTL: Duration = Duration::from_secs(10);
/// The subgraphs' page cap, the keyset step.
pub const PAGE_SIZE: i64 = 1000;

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
        "SELECT source_url, pass_started_at, cursor_updated_at, cursor_id, pages_fetched, rows_upserted, \
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
              rows_upserted, last_complete_pass_at, last_complete_pass_rows, last_error, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, timezone('UTC', now())) \
         ON CONFLICT (mirror, network) DO UPDATE SET \
             source_url = EXCLUDED.source_url, pass_started_at = EXCLUDED.pass_started_at, \
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
    .execute(pool)
    .await?;
    Ok(())
}

/// The cursor a pass starts from: the persisted row for this source, a fresh
/// one when the source changed (the old keyset means nothing against a
/// different subgraph) or there is none.
async fn cursor_for(
    pool: &PgPool,
    mirror: Mirror,
    network: Network,
    url: &str,
) -> Result<Cursor, sqlx::Error> {
    Ok(match load(pool, mirror, network).await? {
        Some(c) if c.source_url == url => c,
        Some(c) => {
            tracing::warn!(mirror = mirror.name(), network = network.db_name(), old = %c.source_url, "mirror source changed; the cursor restarts");
            Cursor::fresh(mirror, network, url)
        }
        None => Cursor::fresh(mirror, network, url),
    })
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
    F: Fn(Option<i64>, Option<String>) -> FF,
    FF: std::future::Future<Output = Result<Page, String>>,
    U: Fn(Vec<Value>) -> UF,
    UF: std::future::Future<Output = Result<i64, sqlx::Error>>,
{
    let pool = &state.pool;
    let mut c = cursor_for(pool, mirror, network, url).await?;
    if !c.in_progress() {
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
        let page = match fetch(c.cursor_updated_at, c.cursor_id.clone()).await {
            Ok(p) => p,
            Err(e) => {
                c.last_error = Some(e.clone());
                save(pool, &c).await?;
                anyhow::bail!(
                    "{} mirror ({}): page after {:?}: {e}",
                    mirror.name(),
                    network.db_name(),
                    c.cursor_id
                );
            }
        };
        let fetched = page.rows.len() as i64;
        let short = fetched < PAGE_SIZE;
        let upserted = match upsert(page.rows).await {
            Ok(n) => n,
            Err(e) => {
                c.last_error = Some(format!("upsert failed: {e}"));
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
            PeriodicCfg::every(
                state.settings.market.mirror_interval * mirror.interval_multiplier(),
            )
            .with_jitter(Duration::from_secs(30))
            .with_error_backoff(Duration::from_secs(3600)),
            move |ctx| {
                let st = st.clone();
                async move { run_tick(&st, mirror, &ctx).await }
            },
        );
    }
}

/// The copy `mirror` serves: the oldest last complete pass over the
/// configured networks, whichever process wrote it; `None` while a network
/// has none, or when no subgraph is configured (nothing says which networks
/// a pass must cover). Cached 10 s.
pub async fn ready(
    state: &MarketplaceState,
    mirror: Mirror,
) -> Result<Option<NaiveDateTime>, sqlx::Error> {
    let networks = state.settings.collection_networks();
    if networks.is_empty() {
        return Ok(None);
    }
    state
        .mirror_ready
        .get_or_fetch(mirror, || async move {
            let done: Vec<(String, NaiveDateTime)> = sqlx::query_as(
                "SELECT network, last_complete_pass_at FROM shop.mirror_state WHERE mirror = $1 AND last_complete_pass_at IS NOT NULL",
            )
            .bind(mirror.name())
            .fetch_all(&state.pool)
            .await?;
            Ok::<_, sqlx::Error>(oldest_pass(&networks, &done))
        })
        .await
}

/// The copy the tables hold with no subgraph configured: the oldest last
/// complete pass of the networks `shop.mirror_state` records; `None` when
/// no pass ever completed. Cached 10 s beside [`ready`] (the configuration
/// is fixed for the process, so one key serves either).
async fn recorded(
    state: &MarketplaceState,
    mirror: Mirror,
) -> Result<Option<NaiveDateTime>, sqlx::Error> {
    state
        .mirror_ready
        .get_or_fetch(mirror, || async move {
            sqlx::query_scalar::<_, Option<NaiveDateTime>>(
                "SELECT min(last_complete_pass_at) FROM shop.mirror_state WHERE mirror = $1",
            )
            .bind(mirror.name())
            .fetch_one(&state.pool)
            .await
        })
        .await
}

fn oldest_pass(networks: &[Network], done: &[(String, NaiveDateTime)]) -> Option<NaiveDateTime> {
    networks
        .iter()
        .map(|n| {
            done.iter()
                .find(|(d, _)| d == n.db_name())
                .map(|(_, at)| *at)
        })
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .min()
}

/// The gate a mirror-served route passes first: `Ok` to read the table
/// (and the answer is stamped with the copy's `as_of`), `Err` the envelope
/// to answer instead. A descriptive read serves any copy the tables hold:
/// with no subgraph configured, the networks `shop.mirror_state` records
/// ([`recorded`]).
pub async fn require_ready(state: &MarketplaceState, mirror: Mirror) -> Result<(), Response> {
    ready_at(state, mirror, Accept::Recorded).await.map(|_| ())
}

/// Which copy a gate accepts when no collections subgraph is configured.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Accept {
    /// None: prices need the configured networks' pass.
    Configured,
    /// The copy of the networks the tables record.
    Recorded,
}

async fn ready_at(
    state: &MarketplaceState,
    mirror: Mirror,
    accept: Accept,
) -> Result<NaiveDateTime, Response> {
    if state.settings.collection_networks().is_empty() {
        let recorded = match accept {
            Accept::Configured => None,
            Accept::Recorded => recorded(state, mirror).await.map_err(|e| {
                tracing::error!(error = %e, "mirror_state read failed");
                super::internal_server_error()
            })?,
        };
        if let Some(at) = recorded {
            note_copy(state, mirror, at);
            return Ok(at);
        }
        return Err(unavailable(
            Source::CollectionsSubgraph(Network::Matic),
            &format!(
                "no collections subgraph is configured, so the {} mirror has nothing to walk",
                mirror.name()
            ),
        ));
    }
    match ready(state, mirror).await {
        Ok(Some(at)) => {
            note_copy(state, mirror, at);
            Ok(at)
        }
        Ok(None) if !state.settings.market.mirrors_enabled => Err(unavailable(
            mirror.source(),
            &format!(
                "the {} mirror has no complete pass in this database and the mirror job is not enabled on this node",
                mirror.name()
            ),
        )),
        Ok(None) => Err(unavailable(
            mirror.source(),
            &format!(
                "the {} mirror has no complete pass yet for every configured network",
                mirror.name()
            ),
        )),
        Err(e) => {
            tracing::error!(error = %e, "mirror_state read failed");
            Err(super::internal_server_error())
        }
    }
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
    if age > budget.as_secs() as i64 {
        return Err(format!(
            "the items mirror's last complete pass is {age} s old, past the {} s freshness budget of the prices it carries (the items-mirror job is off or failing)",
            budget.as_secs()
        ));
    }
    Ok(())
}

/// The gate of a route whose answer carries item prices (the minter
/// `price` and `available`, a price sort or filter): the items mirror's
/// pass over every configured network, within the money budget
/// ([`price_verdict`]) whether or not this node runs the job: a frozen copy
/// (a restored replica, a fixture) holds prices of its age all the same.
pub async fn require_priced(state: &MarketplaceState) -> Result<(), Response> {
    let oldest = ready_at(state, Mirror::Items, Accept::Configured).await?;
    price_verdict(oldest, Utc::now(), price_max_age(&state.settings))
        .map_err(|why| unavailable(Mirror::Items.source(), &why))
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
fn stale_after(settings: &super::Settings, mirror: Mirror) -> Duration {
    as_of::stale_after(
        settings
            .market
            .mirror_interval
            .saturating_mul(mirror.interval_multiplier()),
    )
}

fn note_copy(state: &MarketplaceState, mirror: Mirror, at: NaiveDateTime) {
    let budget = chrono::Duration::from_std(stale_after(&state.settings, mirror))
        .unwrap_or(chrono::Duration::MAX);
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

/// Router middleware: an answer whose handler read a mirror through
/// [`require_ready`] carries that copy's `as_of` (the oldest, when it read
/// both) and `stale_after_secs`; every other answer passes as it came. A
/// HEAD runs as its GET so its length is the stamped answer's.
pub async fn stamp_copies(request: Request, next: Next) -> Response {
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
            as_of::stamp(response, as_of.and_utc(), stale_after).await
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

    #[test]
    fn the_copy_is_as_old_as_its_oldest_network_and_needs_every_one() {
        let at = |s: i64| chrono::DateTime::from_timestamp(s, 0).unwrap().naive_utc();
        let both = [Network::Ethereum, Network::Matic];
        let done = vec![
            ("ethereum".to_string(), at(200)),
            ("matic".to_string(), at(100)),
        ];
        assert_eq!(oldest_pass(&both, &done), Some(at(100)));
        assert_eq!(oldest_pass(&both, &done[..1]), None, "matic has no pass");
        assert_eq!(oldest_pass(&[Network::Ethereum], &done), Some(at(200)));
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

    /// Prices need the configured networks' pass: with no subgraph there is
    /// none, whatever the tables hold (a descriptive read serves their copy,
    /// `tests::without_a_subgraph_descriptive_reads_serve_the_recorded_copy`).
    #[tokio::test]
    async fn an_unconfigured_mirror_never_prices() {
        let state = crate::market::tests::support::state_without_db();
        assert_eq!(ready(&state, Mirror::Items).await.unwrap(), None);
        let err = require_priced(&state).await.unwrap_err();
        assert_eq!(err.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    /// The switch attaches the job; the marker decides the reads, so a
    /// replica with a complete pass in its tables serves with the job off.
    #[tokio::test]
    async fn a_complete_pass_serves_with_the_mirror_job_off() {
        let settings = super::super::Settings {
            matic_collections_subgraph_url: Some("https://subgraph.example/matic".into()),
            ..super::super::Settings::for_tests()
        };
        let Some(state) = crate::market::tests::support::state_with_db(settings).await else {
            return;
        };
        let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
            .lock()
            .await;
        assert!(!state.settings.market.mirrors_enabled);
        let url = format!("https://subgraph.example/{}", uuid::Uuid::new_v4().simple());
        sqlx::query("DELETE FROM shop.mirror_state WHERE mirror = 'items' AND network = 'matic'")
            .execute(&state.pool)
            .await
            .unwrap();
        let err = require_ready(&state, Mirror::Items).await.unwrap_err();
        assert_eq!(
            err.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "no pass in the tables"
        );
        let mut c = Cursor::fresh(Mirror::Items, Network::Matic, &url);
        c.last_complete_pass_at = Some(now_utc());
        save(&state.pool, &c).await.unwrap();
        state.mirror_ready.clear();
        assert!(
            require_ready(&state, Mirror::Items).await.is_ok(),
            "the marker, not the switch, decides"
        );
        sqlx::query("DELETE FROM shop.mirror_state WHERE mirror = 'items' AND network = 'matic'")
            .execute(&state.pool)
            .await
            .unwrap();
    }
}
