//! The walk the marketplace chain indexes share (`nft`, `trade_events`):
//! one window at a time read from a [`LogSource`] (over JSON-RPC, every
//! filter group's address list split in chunks the endpoint accepts),
//! the window's logs and the cursor advance committed together by the
//! caller together with the cursor hash, a request budget per pass (the backpressure: a catch-up is many
//! passes, each leaving a durable `last_block`), Polygon milestone finality
//! and Ethereum confirmation depth, a reorg check of the cursor block's hash at the start of
//! every pass (a divergence withdraws the publication, then rewinds to the
//! newest recovery point still on the chain, [`recover`]), and the publish
//! marker (`folded_block`) set only when the index reaches the head within
//! the publish lag.
//!
//! `shop.chain_cursor` is one row per (indexer, network). A cursor
//! created at the indexer's start block records `full_history`; one started
//! at `MARKETPLACE_INDEX_START_BLOCK_*` (a bounded development or
//! measurement window) never publishes, and neither does one held short by
//! `MARKETPLACE_INDEX_STOP_BLOCK_*`. Readers trust an index through
//! [`is_ready`]: `folded_block = last_block` and `folded_at` at most
//! [`READY_WINDOW`] old at the caller's clock. The `nft` index's readers use
//! the publish marker alone (`owned_items::Rule`): every pass advances
//! `last_block` before it folds and publishes. Accepting work that waits on
//! an index (tip claims) uses the liveness view [`is_walked`] instead: ready,
//! or a full-history walk advancing near the head, bound to its chain and out
//! of quarantine.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use sqlx::{PgConnection, PgPool};

use crate::chain_index::rpc::{BlockHeader, RpcLog};

pub(crate) fn topic_address(topic: &str) -> Option<String> {
    let hex = topic.strip_prefix("0x")?;
    (hex.len() == 64).then(|| format!("0x{}", &hex[24..]).to_ascii_lowercase())
}

pub(crate) fn topic_u256(topic: &str) -> Option<String> {
    alloy_primitives::U256::from_str_radix(topic.strip_prefix("0x")?, 16)
        .ok()
        .map(|v| v.to_string())
}

pub const INDEXER_MANA: &str = "mana_transfers";

pub const INDEXER_COMMITTEE: &str = "committee";

pub const INDEXER_COLLECTIONS: &str = "collections";

pub const INDEXER_NFT: &str = "nft";
pub const INDEXER_SALES: &str = "sales";
pub const INDEXER_TRADE_EVENTS: &str = "trade_events";
pub const INDEXER_THIRD_PARTY_REGISTRY: &str = "third_party_registry";

/// How old a publish may be before readers stop trusting it (a stopped job).
pub const READY_WINDOW: ChronoDuration = ChronoDuration::minutes(30);

/// The two chains the marketplace indexes walk; the `network` column's spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChainNet {
    Ethereum,
    Matic,
}

impl From<super::graph::Network> for ChainNet {
    fn from(net: super::graph::Network) -> Self {
        match net {
            super::graph::Network::Ethereum => Self::Ethereum,
            super::graph::Network::Matic => Self::Matic,
        }
    }
}

impl ChainNet {
    pub const ALL: [ChainNet; 2] = [ChainNet::Ethereum, ChainNet::Matic];

    pub fn db_name(self) -> &'static str {
        match self {
            ChainNet::Ethereum => "ethereum",
            ChainNet::Matic => "matic",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ethereum" | "mainnet" => Some(ChainNet::Ethereum),
            "matic" | "polygon" => Some(ChainNet::Matic),
            _ => None,
        }
    }

    pub fn chain_id(self) -> i64 {
        match self {
            ChainNet::Ethereum => 1,
            ChainNet::Matic => 137,
        }
    }

    /// Blocks the walk stays behind the head: a reorg shallower than this
    /// never reaches the log (Ethereum finalizes in two epochs; Polygon's
    /// milestones finalize in seconds, its historical reorgs ran deeper).
    pub fn confirmations(self) -> u64 {
        match self {
            ChainNet::Ethereum => 12,
            ChainNet::Matic => 64,
        }
    }

    /// A publish needs `head - last_block <= confirmations + publish_lag`
    /// at the moment the fold committed.
    pub fn publish_lag(self) -> u64 {
        match self {
            ChainNet::Ethereum => 25,
            ChainNet::Matic => 150,
        }
    }

    /// The fewest milliseconds two blocks are apart on average over a span:
    /// `block_at_or_before` never lands after a timestamp when it divides
    /// by this.
    pub fn min_block_ms(self) -> u64 {
        match self {
            ChainNet::Ethereum => 1_000,
            ChainNet::Matic => 500,
        }
    }

    /// The most milliseconds two blocks are apart on average over a span:
    /// `time_at_most` never lands before the real block time with it (a
    /// longer halt is caught by the late-contract check with exact times).
    pub fn max_block_ms(self) -> u64 {
        match self {
            ChainNet::Ethereum => 30_000,
            ChainNet::Matic => 6_000,
        }
    }
}

/// The indexes' configuration (`MarketSettings::indexes`); every default
/// leaves the indexers off.
#[derive(Clone, Debug)]
pub struct IndexSettings {
    /// `MARKETPLACE_NFT_INDEX_ENABLED`.
    pub nft_enabled: bool,
    pub mana_transfers_enabled: bool,
    /// `MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED`.
    pub trade_events_enabled: bool,
    /// `MARKETPLACE_SALES_INDEX_ENABLED`.
    pub sales_enabled: bool,
    /// `MARKETPLACE_COMMITTEE_INDEX_ENABLED`: the Polygon curation committee.
    pub committee_enabled: bool,
    /// `MARKETPLACE_THIRD_PARTY_REGISTRY_INDEX_ENABLED`: Polygon providers and slots.
    pub third_party_registry_enabled: bool,
    /// `MARKETPLACE_COLLECTIONS_INDEX_ENABLED`: the Polygon collection factories and collections.
    pub collections_enabled: bool,
    /// `MARKETPLACE_INDEX_INTERVAL_SECS` (5; 30 under the mirror posture): pause between passes.
    pub interval: Duration,
    /// `MARKETPLACE_INDEX_REQUESTS_PER_PASS` (100): `eth_getLogs` calls one
    /// network's pass may make before it yields with its cursor durable.
    pub requests_per_pass: usize,
    /// `MARKETPLACE_INDEX_WINDOW_BLOCKS` (10,000): the widest window asked
    /// for (the strictest public backend's limit).
    pub window_blocks: u64,
    /// `MARKETPLACE_INDEX_ADDRESS_CHUNK` (1,000): addresses per request; an
    /// endpoint refusing the count halves it for the rest of the pass.
    pub address_chunk: usize,
    /// `MARKETPLACE_INDEX_START_BLOCK_{ETHEREUM,MATIC}`: begin a fresh cursor
    /// here instead of the contracts' deployment (never published).
    pub start_block_ethereum: Option<u64>,
    pub start_block_matic: Option<u64>,
    /// `MARKETPLACE_INDEX_STOP_BLOCK_{ETHEREUM,MATIC}`: index no further (never published).
    pub stop_block_ethereum: Option<u64>,
    pub stop_block_matic: Option<u64>,
    /// `MARKETPLACE_NEIGHBOURS_INTERVAL_SECS` (21,600): between two rebuilds of the item neighbours.
    pub neighbours_interval: Duration,
    /// `MARKETPLACE_COUPON_REFRESH_INTERVAL_SECS` (300): between two coupon state reads.
    pub coupon_refresh_interval: Duration,
    /// `MARKETPLACE_INDEX_CHAIN_ID_{ETHEREUM,MATIC}` (1, 137): the chain the
    /// NFT and trade-event walks of each network read. A catalogue database holds
    /// the mainnet pair or the Sepolia/Amoy pair.
    pub chain_id_ethereum: i64,
    pub chain_id_matic: i64,
}

impl Default for IndexSettings {
    fn default() -> Self {
        Self {
            nft_enabled: false,
            mana_transfers_enabled: false,
            trade_events_enabled: false,
            sales_enabled: false,
            committee_enabled: false,
            third_party_registry_enabled: false,
            collections_enabled: false,
            interval: Duration::from_secs(5),
            requests_per_pass: 100,
            window_blocks: 10_000,
            address_chunk: 1_000,
            start_block_ethereum: None,
            start_block_matic: None,
            stop_block_ethereum: None,
            stop_block_matic: None,
            neighbours_interval: Duration::from_secs(6 * 3600),
            coupon_refresh_interval: Duration::from_secs(300),
            chain_id_ethereum: ChainNet::Ethereum.chain_id(),
            chain_id_matic: ChainNet::Matic.chain_id(),
        }
    }
}

impl IndexSettings {
    /// The served chain of this network.
    pub fn chain_id(&self, net: ChainNet) -> i64 {
        match net {
            ChainNet::Ethereum => self.chain_id_ethereum,
            ChainNet::Matic => self.chain_id_matic,
        }
    }

    /// Refuses a chain that is neither the network's mainnet nor its
    /// testnet: the walk would read another network's contracts.
    pub fn check_chains(&self) -> Result<(), String> {
        for (net, allowed, var) in [
            (
                ChainNet::Ethereum,
                [1, 11_155_111],
                "MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM",
            ),
            (
                ChainNet::Matic,
                [137, 80_002],
                "MARKETPLACE_INDEX_CHAIN_ID_MATIC",
            ),
        ] {
            let id = self.chain_id(net);
            if !allowed.contains(&id) {
                return Err(format!(
                    "{var}={id}: {} indexes chain {} or {}",
                    net.db_name(),
                    allowed[0],
                    allowed[1]
                ));
            }
        }
        if self.nft_enabled
            && !matches!(
                (self.chain_id_ethereum, self.chain_id_matic),
                (1, 137) | (11_155_111, 80_002)
            )
        {
            return Err("the NFT catalogue requires the Ethereum/Polygon mainnet pair (1/137) or Sepolia/Amoy (11155111/80002)".into());
        }
        Ok(())
    }

    pub fn start_override(&self, net: ChainNet) -> Option<u64> {
        match net {
            ChainNet::Ethereum => self.start_block_ethereum,
            ChainNet::Matic => self.start_block_matic,
        }
    }

    pub fn stop_block(&self, net: ChainNet) -> Option<u64> {
        match net {
            ChainNet::Ethereum => self.stop_block_ethereum,
            ChainNet::Matic => self.stop_block_matic,
        }
    }
}

/// `shop.chain_cursor`.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Cursor {
    pub indexer: String,
    pub network: String,
    pub start_block: i64,
    pub last_block: i64,
    pub last_block_hash: Option<String>,
    pub last_block_time: Option<i64>,
    /// The block the last fold covered, published or not: the next fold's `since`.
    pub fold_through: Option<i64>,
    pub folded_block: Option<i64>,
    pub folded_at: Option<NaiveDateTime>,
    pub full_history: bool,
    pub reorg_rewinds: i32,
    pub updated_at: NaiveDateTime,
    /// The chain the history was read from, as the source proved it; `None`
    /// on a row written before the binding, until a pass proves it.
    pub chain_id: Option<i64>,
    /// Raised by every job that takes the projection ([`claim`]).
    pub owner_generation: i64,
    pub quarantined_at: Option<NaiveDateTime>,
    pub quarantine_reason: Option<String>,
    pub rebuild_requested_at: Option<NaiveDateTime>,
    /// Its block was found on another branch than its stored hash, its
    /// publication withdrawn, while its recovery searches.
    pub diverged_at: Option<NaiveDateTime>,
}

pub(crate) const CURSOR_COLUMNS: &str = "indexer, network, start_block, last_block, last_block_hash, last_block_time, fold_through, folded_block, folded_at, full_history, reorg_rewinds, updated_at, chain_id, owner_generation, quarantined_at, quarantine_reason, rebuild_requested_at, diverged_at";

/// Runs `ANALYZE <table>` in the fold's transaction only when the fold needs
/// statistics the table does not have yet: a rebuild (`since` -1), a table
/// never analyzed, or rows changed since the last analyze past 5 % of the
/// table (at least 1,000): a catch-up window's bulk insert that autoanalyze
/// has not seen (the land_authz fold misplanned such a window for 169 s
/// without statistics). A steady-state fold leaves the table to autovacuum:
/// on a 3M-row `nft_event` the ANALYZE was 790 to 807 ms of a 797 to 819 ms
/// incremental fold, twice a minute, and its ShareUpdateExclusiveLock held
/// to commit cancelled autovacuum on the table (2026-09-25 review). `table`
/// is a schema-qualified name from code. Returns whether it analyzed.
pub async fn analyze_if_stale(
    conn: &mut PgConnection,
    schema: &str,
    table: &str,
    rebuild: bool,
) -> Result<bool, sqlx::Error> {
    let stale: bool = rebuild
        || sqlx::query_scalar(
            "SELECT COALESCE((
                 SELECT (s.last_analyze IS NULL AND s.last_autoanalyze IS NULL)
                     OR s.n_mod_since_analyze >= GREATEST(1000, 0.05 * GREATEST(c.reltuples, 0))
                 FROM pg_catalog.pg_stat_user_tables s JOIN pg_catalog.pg_class c ON c.oid = s.relid
                 WHERE s.schemaname = $1 AND s.relname = $2), true)",
        )
        .bind(schema)
        .bind(table)
        .fetch_one(&mut *conn)
        .await?;
    if stale {
        sqlx::query(sqlx::AssertSqlSafe(format!("ANALYZE {schema}.{table}")))
            .execute(&mut *conn)
            .await?;
    }
    Ok(stale)
}

pub async fn cursor<'e, E>(
    executor: E,
    indexer: &str,
    net: ChainNet,
) -> Result<Option<Cursor>, sqlx::Error>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {CURSOR_COLUMNS} FROM shop.chain_cursor WHERE indexer = $1 AND network = $2"
    )))
    .bind(indexer)
    .bind(net.db_name())
    .fetch_optional(executor)
    .await
}

/// How long ago [`touch_started`] marks a cursor advanced: just outside the
/// live window, even read from a clock [`MAX_CLOCK_SKEW`] behind the database's.
const STARTED_AGE: ChronoDuration = READY_WINDOW
    .checked_add(&MAX_CLOCK_SKEW)
    .expect("a small sum")
    .checked_add(&ChronoDuration::seconds(1))
    .expect("a small sum");

/// Marks the cursor of a walk `owner` just started, standing at `at`, as
/// advanced [`STARTED_AGE`] ago, unless it advanced later: [`is_walked`] reads
/// it as behind, not abandoned and not live, until its first window really
/// lands. The block, its time and the history flag are unchanged, and nothing
/// publishes earlier. A fenced write, like any other: a cursor no longer at
/// `at`, or one `owner` no longer holds, is left as it is.
pub async fn touch_started(
    pool: &PgPool,
    owner: &Owner,
    at: (i64, Option<&str>),
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let set = sqlx::query(
        "UPDATE shop.chain_cursor \
         SET updated_at = GREATEST(updated_at, timezone('UTC', now()) - make_interval(secs => $5)) \
         WHERE indexer = $1 AND network = $2 AND last_block = $3 AND last_block_hash IS NOT DISTINCT FROM $4",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(at.0)
    .bind(at.1)
    .bind(STARTED_AGE.num_seconds() as f64)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if set != 1 {
        return Err(owner
            .fenced("the checkpoint a started walk read moved".into())
            .into());
    }
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Whether the walk of one lease acquisition's generation already touched its
/// cursor ([`touch_started`]): set only once that touch committed, so a
/// failed or fenced one is tried again on the next pass, and a new
/// generation starts untouched.
#[derive(Clone, Debug, Default)]
pub struct Started(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Started {
    pub fn done(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn mark(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Owners compare by what they hold, not by whether their walk touched yet.
impl PartialEq for Started {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Started {}

/// [`touch_started`] once per generation `owner` holds, for its cursor at
/// `at`: only once its head reads answered, and not while it is held at its
/// stop block, so a walk with its RPC down or held there keeps its age. Marked
/// only after the fenced touch committed; a failed one is tried again on the
/// next pass.
pub(crate) async fn touch_once(
    pool: &PgPool,
    owner: &Owner,
    at: (i64, Option<&str>),
    stop: Option<u64>,
) -> anyhow::Result<()> {
    if !held_at_stop(at.0, stop) && !owner.started.done() {
        touch_started(pool, owner, at).await?;
        owner.started.mark();
    }
    Ok(())
}

/// One projection as the job that walks it holds it: the cursor row
/// (`indexer`, `net`), the chain its history is bound to and the generation
/// [`claim`] granted. Every transaction that writes the projection ends with
/// [`Owner::check`], so a job that lost the projection commits nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub indexer: &'static str,
    pub net: ChainNet,
    pub chain_id: i64,
    pub generation: i64,
    /// Whether this generation's walk touched its cursor at its start.
    pub started: Started,
}

/// A source that serves another chain than the index reads: the pass stops
/// before any decision about the data.
#[derive(Debug, thiserror::Error)]
#[error("the endpoint serves chain {served}, not {expected}: refusing to index it")]
pub struct WrongChain {
    pub served: i64,
    pub expected: i64,
}

/// A write refused because its job no longer owns the projection, or the
/// projection is in quarantine: the transaction is rolled back.
#[derive(Debug, thiserror::Error)]
#[error("{indexer} {network}: {reason}; the write is rolled back")]
pub struct Fenced {
    pub indexer: &'static str,
    pub network: &'static str,
    pub reason: String,
}

/// A stable read that found the block a projection's cursor stands on (or
/// extends) on another branch than its stored hash: evidence that its
/// history diverged, which withdraws its publication and starts its
/// recovery. `requests` are the reads spent until then.
#[derive(Debug, thiserror::Error)]
#[error("block {block} is {chain} on the chain, but the cursor stored {stored}")]
pub struct Diverged {
    pub block: u64,
    pub stored: String,
    pub chain: String,
    pub requests: usize,
}

/// What lanes spent: what they charged the walk's log budget (a window's
/// [`Window::charge`], at least its requests, plus the headers and times
/// they read) and the
/// requests they sent (its `requests`, plus the same reads). Both are equal
/// over an RPC source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spent {
    pub charge: usize,
    pub requests: usize,
}

impl Spent {
    /// A read that charges what it sends (a header, the block times).
    pub fn read(&mut self, requests: usize) {
        self.charge += requests;
        self.requests += requests;
    }

    /// A window: its charge, and never less than the requests it sent (a
    /// lane pays the headers it reads, an anchor included), and its requests.
    pub fn window(&mut self, w: &Window) {
        self.charge += w.charge.max(w.requests);
        self.requests += w.requests;
    }
}

impl Owner {
    /// Locks the cursor row until the caller's commit and refuses unless the
    /// row still carries this generation, is bound to this chain and is not
    /// in quarantine. Call it last, right before the commit, so the row lock
    /// lasts only as long as the commit.
    pub async fn check(&self, conn: &mut PgConnection) -> anyhow::Result<()> {
        let row: Option<(i64, Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT owner_generation, chain_id, quarantine_reason FROM shop.chain_cursor \
             WHERE indexer = $1 AND network = $2 FOR UPDATE",
        )
        .bind(self.indexer)
        .bind(self.net.db_name())
        .fetch_optional(conn)
        .await?;
        let reason = match row {
            None => "its cursor row is gone".to_string(),
            Some((generation, _, _)) if generation != self.generation => format!(
                "another job took it (generation {generation}; this job holds {})",
                self.generation
            ),
            Some((_, _, Some(why))) => format!("it is in quarantine ({why})"),
            Some((_, chain, None)) if chain != Some(self.chain_id) => {
                format!("it is bound to chain {chain:?}, not {}", self.chain_id)
            }
            Some(_) => return Ok(()),
        };
        Err(self.fenced(reason).into())
    }

    fn fenced(&self, reason: String) -> Fenced {
        Fenced {
            indexer: self.indexer,
            network: self.net.db_name(),
            reason,
        }
    }
}

/// Grants the calling job a new generation of the projection (`indexer`,
/// `net`) and returns it, creating the row when there is none: at
/// `start_block - 1` and bound to `chain_id`, the chain the source proved.
/// Run it on the session that holds the job's lease ([`Holdings::owner`]),
/// so the grant belongs to the lease the job holds.
pub async fn claim(
    conn: &mut PgConnection,
    indexer: &str,
    net: ChainNet,
    start_block: u64,
    full_history: bool,
    chain_id: i64,
) -> sqlx::Result<i64> {
    use sqlx::Connection;
    let mut tx = conn.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '2000ms'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '1500ms'")
        .execute(&mut *tx)
        .await?;
    let generation: i64 = sqlx::query_scalar(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, full_history, chain_id, owner_generation) \
         VALUES ($1, $2, $3, $3 - 1, $4, $5, 1) \
         ON CONFLICT (indexer, network) DO UPDATE SET owner_generation = shop.chain_cursor.owner_generation + 1 \
         RETURNING owner_generation",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(start_block as i64)
    .bind(full_history)
    .bind(chain_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(generation)
}

/// The projections one job holds: each is claimed once per acquisition of
/// the job's lease (once per process without leases) and its generation kept
/// until a write is fenced, when the next pass claims it again.
#[derive(Default)]
pub struct Holdings {
    held: parking_lot::Mutex<HashMap<Projection, Held>>,
}

/// A projection's key: (`indexer`, network).
type Projection = (&'static str, ChainNet);

/// A held projection: (lease acquisition, generation, whether its walk
/// touched its cursor).
type Held = (u64, i64, Started);

impl Holdings {
    /// The owner of `ix`'s projection for this pass, bound to `ix`'s chain
    /// (which the caller proved the source serves): the generation granted
    /// for the current lease acquisition, claimed now if there is none.
    pub async fn owner(
        &self,
        pool: &PgPool,
        lease: Option<&crate::jobs::LeaseControl>,
        ix: &dyn ChainIndex,
        settings: &IndexSettings,
    ) -> anyhow::Result<Owner> {
        let (indexer, net, chain_id) = (ix.indexer(), ix.net(), ix.chain_id());
        let acquisition = lease.map_or(0, |l| l.acquisition());
        let held = self.held.lock().get(&(indexer, net)).cloned();
        let (generation, started) = match held {
            Some((at, generation, started)) if at == acquisition => (generation, started),
            _ => {
                let start = settings.start_override(net).unwrap_or(ix.start_block());
                let full_history = start <= ix.start_block();
                let generation = match lease {
                    Some(lease) => lease
                        .with_connection(move |conn| {
                            Box::pin(claim(conn, indexer, net, start, full_history, chain_id))
                        })
                        .await
                        .context("claiming the projection on the lease's session")?,
                    None => {
                        let mut conn = pool.acquire().await?;
                        claim(&mut conn, indexer, net, start, full_history, chain_id).await?
                    }
                };
                let started = Started::default();
                self.held
                    .lock()
                    .insert((indexer, net), (acquisition, generation, started.clone()));
                (generation, started)
            }
        };
        Ok(Owner {
            indexer,
            net,
            chain_id,
            generation,
            started,
        })
    }

    /// Drops a projection whose write was fenced: the next pass claims it
    /// again, which only succeeds while the job still holds its lease.
    pub fn forget(&self, indexer: &'static str, net: ChainNet) {
        self.held.lock().remove(&(indexer, net));
    }
}

/// The points a projection keeps in `shop.chain_checkpoint`: the newest 64
/// committed window ends, a reorg's recovery points.
pub const CHECKPOINTS_KEPT: i64 = 64;

/// Moves `last_block` from the checkpoint (`expected`, `expected_hash`) to
/// `header` in the caller's transaction (the one that wrote the window's
/// rows), and records `header` as a recovery point (dropping any past
/// `expected`), keeping the newest [`CHECKPOINTS_KEPT`]; a cursor someone
/// else moved fails the transaction
/// instead of skipping or repeating a range. `header` must have been read in
/// the window's bracket, after a predecessor read that proved `expected`.
/// The caller ends the transaction with [`Owner::check`].
pub async fn advance(
    conn: &mut PgConnection,
    indexer: &str,
    net: ChainNet,
    expected: (i64, Option<&str>),
    header: &BlockHeader,
) -> Result<(), sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = $5, last_block_hash = $6, last_block_time = $7, \
             updated_at = timezone('UTC', now()) \
         WHERE indexer = $1 AND network = $2 AND last_block = $3 AND last_block_hash IS NOT DISTINCT FROM $4 \
           AND last_block < $5",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(expected.0)
    .bind(expected.1)
    .bind(header.number as i64)
    .bind(&header.hash)
    .bind(header.timestamp)
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    // A point past the checkpoint this window extends belongs to no proven
    // prefix: it goes before the insert (defense in depth; the points an
    // older binary left are cleared by the operator, DATABASE.md "Deploys").
    sqlx::query(
        "DELETE FROM shop.chain_checkpoint WHERE indexer = $1 AND network = $2 AND block_number > $3",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(expected.0)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO shop.chain_checkpoint (indexer, network, block_number, block_hash, block_time) \
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (indexer, network, block_number) \
         DO UPDATE SET block_hash = EXCLUDED.block_hash, block_time = EXCLUDED.block_time",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(header.number as i64)
    .bind(&header.hash)
    .bind(header.timestamp)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "DELETE FROM shop.chain_checkpoint WHERE indexer = $1 AND network = $2 AND block_number < \
           (SELECT block_number FROM shop.chain_checkpoint WHERE indexer = $1 AND network = $2 \
            ORDER BY block_number DESC OFFSET $3 LIMIT 1)",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(CHECKPOINTS_KEPT - 1)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Rewinds the checkpoint (`from`, `from_hash`) to `to` in the caller's
/// transaction (which also deletes the log past `to`), standing on `keep`
/// (the recovery point's hash and time) or on no hash, and drops the
/// recovery points past `to`: the publish marker goes with it, so readers
/// fall back until the refold publishes. A cursor no longer at that
/// checkpoint fails the transaction. It locks the cursor row, so the caller
/// runs it after the notification cursors' moves; the caller ends the
/// transaction with [`Owner::check`].
pub async fn rewind(
    conn: &mut PgConnection,
    indexer: &str,
    net: ChainNet,
    from: (i64, Option<&str>),
    to: i64,
    keep: Option<(&str, i64)>,
) -> Result<(), sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = $5, last_block_hash = $6, last_block_time = $7, \
             fold_through = NULL, folded_block = NULL, folded_at = NULL, diverged_at = NULL, \
             reorg_rewinds = reorg_rewinds + 1, updated_at = timezone('UTC', now()) \
         WHERE indexer = $1 AND network = $2 AND last_block = $3 AND last_block_hash IS NOT DISTINCT FROM $4 \
           AND last_block >= $5",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(from.0)
    .bind(from.1)
    .bind(to)
    .bind(keep.map(|k| k.0))
    .bind(keep.map(|k| k.1))
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    sqlx::query(
        "DELETE FROM shop.chain_checkpoint WHERE indexer = $1 AND network = $2 AND block_number > $3",
    )
    .bind(indexer)
    .bind(net.db_name())
    .bind(to)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Clears the divergence mark of the projection standing at `at`, whose
/// block a pass found on its stored hash again, in a fenced transaction of
/// its own. A cursor no longer at `at` is fenced.
async fn undiverge(pool: &PgPool, owner: &Owner, at: (i64, Option<&str>)) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let set = sqlx::query(
        "UPDATE shop.chain_cursor SET diverged_at = NULL \
         WHERE indexer = $1 AND network = $2 AND last_block = $3 AND last_block_hash IS NOT DISTINCT FROM $4",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(at.0)
    .bind(at.1)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if set != 1 {
        return Err(owner
            .fenced("the checkpoint a pass proved again moved".into())
            .into());
    }
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Withdraws the publication of the projection standing at `at`, in a
/// transaction of its own, before a divergence seen there is searched, and
/// marks it diverged (`diverged_at`): its readers fall back at once, and the
/// ones that use its height without its publication skip it, while its rows,
/// its recovery points and its notification cursors stay as they are until
/// the search decides. `updated_at` stays too: a withdrawal is no progress
/// ([`is_walked`]). A cursor no longer at `at` is fenced.
pub(crate) async fn withdraw(
    pool: &PgPool,
    owner: &Owner,
    at: (i64, Option<&str>),
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let set = sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL, \
             diverged_at = COALESCE(diverged_at, timezone('UTC', now())) \
         WHERE indexer = $1 AND network = $2 AND last_block = $3 AND last_block_hash IS NOT DISTINCT FROM $4",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(at.0)
    .bind(at.1)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if set != 1 {
        return Err(owner
            .fenced("the checkpoint a divergence was seen at moved".into())
            .into());
    }
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Records the block a completed fold covered (the next fold's `since`),
/// on the row still at that block; a publish is [`mark_folded`]'s.
pub async fn record_fold(pool: &PgPool, owner: &Owner, block: i64) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE shop.chain_cursor SET fold_through = $3 WHERE indexer = $1 AND network = $2 AND last_block = $3")
        .bind(owner.indexer)
        .bind(owner.net.db_name())
        .bind(block)
        .execute(&mut *tx)
        .await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Sets `folded_block` to the block a completed fold covered, only on the
/// row still at that block, with its whole history walked and not in
/// quarantine.
pub async fn mark_folded(
    pool: &PgPool,
    owner: &Owner,
    block: i64,
    now: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE shop.chain_cursor SET folded_block = last_block, folded_at = $4 \
         WHERE indexer = $1 AND network = $2 AND last_block = $3 AND full_history AND last_block_hash IS NOT NULL \
           AND fold_through = $3 AND quarantine_reason IS NULL",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(block)
    .bind(now.naive_utc())
    .execute(&mut *tx)
    .await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(updated.rows_affected() == 1)
}

/// Withdraws the publish of the row still at `block`: a fold that may not
/// publish, or one held short of the head.
pub async fn unpublish(pool: &PgPool, owner: &Owner, block: i64) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL WHERE indexer = $1 AND network = $2 AND last_block = $3")
        .bind(owner.indexer)
        .bind(owner.net.db_name())
        .bind(block)
        .execute(&mut *tx)
        .await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

/// The mainnet chain ID of this network.
pub fn mainnet_chain(net: ChainNet) -> i64 {
    net.chain_id()
}

/// The readiness rule, at the caller's clock, for a projection read on
/// chain `chain_id`: a whole history folded through its last block within
/// [`READY_WINDOW`], bound to that chain and not in quarantine.
pub fn is_ready_on(c: &Cursor, chain_id: i64, now: DateTime<Utc>) -> bool {
    fresh_fold_on(c, chain_id, now) == Some(c.last_block)
}

/// The published height for a reader that bounds every event query by that
/// height. A forward walk may already have committed the next window.
pub(super) fn fresh_fold_on(c: &Cursor, chain_id: i64, now: DateTime<Utc>) -> Option<i64> {
    let age = now.naive_utc().signed_duration_since(c.folded_at?);
    c.folded_block.filter(|folded| {
        c.full_history
            && *folded <= c.last_block
            && c.chain_id == Some(chain_id)
            && c.quarantined_at.is_none()
            && age >= ChronoDuration::zero()
            && age <= READY_WINDOW
    })
}

/// [`is_ready_on`] for the indexes that read mainnet only: every index but
/// `trade_events`, whose readers pass the chain they configured.
pub fn is_ready(c: &Cursor, now: DateTime<Utc>) -> bool {
    ChainNet::parse(&c.network).is_some_and(|net| is_ready_on(c, mainnet_chain(net), now))
}

/// [`is_ready`] as SQL over a `shop.chain_cursor` alias, for statements
/// that decide per row with a bound `now` (`now_p`).
pub fn ready_sql(alias: &str, now_p: &str) -> String {
    ready_sql_on(
        alias,
        now_p,
        mainnet_chain(ChainNet::Ethereum),
        mainnet_chain(ChainNet::Matic),
    )
}

/// [`is_ready_on`] as SQL, the expected chain per network given as integers
/// (`trade_events`, which may read a testnet).
pub fn ready_sql_on(alias: &str, now_p: &str, ethereum_chain: i64, matic_chain: i64) -> String {
    format!(
        "({alias}.full_history AND {alias}.folded_block = {alias}.last_block \
          AND {alias}.chain_id = CASE {alias}.network WHEN 'ethereum' THEN {ethereum_chain}::bigint WHEN 'matic' THEN {matic_chain}::bigint END \
          AND {alias}.quarantined_at IS NULL \
          AND {alias}.folded_at <= {now_p} AND {alias}.folded_at >= {now_p} - interval '30 minutes')"
    )
}

/// The publish marker alone as SQL, for the fail-closed readers of an index
/// whose every pass advances `last_block` before it folds and publishes
/// (`trade_events`; the `nft` readers' `owned_items::Rule::Fresh`): the whole
/// history walked, a fold published (`folded_block`, cleared by every
/// rewind, divergence, unpublish, quarantine and rebuild) within
/// [`READY_WINDOW`] of the caller's clock, bound to the expected chain and
/// out of quarantine, without `folded_block = last_block`, which a pass
/// breaks for as long as it walks and folds. `folded_at` is the walker's
/// clock: one up to [`MAX_CLOCK_SKEW`] ahead of the caller's still counts.
pub fn published_sql_on(alias: &str, now_p: &str, ethereum_chain: i64, matic_chain: i64) -> String {
    format!(
        "({alias}.full_history AND {alias}.folded_block IS NOT NULL \
          AND {alias}.chain_id = CASE {alias}.network WHEN 'ethereum' THEN {ethereum_chain}::bigint WHEN 'matic' THEN {matic_chain}::bigint END \
          AND {alias}.quarantined_at IS NULL \
          AND {alias}.folded_at <= {now_p} + interval '{skew} seconds' \
          AND {alias}.folded_at >= {now_p} - interval '{window} seconds')",
        skew = MAX_CLOCK_SKEW.num_seconds(),
        window = READY_WINDOW.num_seconds(),
    )
}

/// How far ahead of the caller's clock the database's `updated_at` may be and
/// still count as advanced.
pub(crate) const MAX_CLOCK_SKEW: ChronoDuration = ChronoDuration::minutes(10);

/// A liveness heuristic for whether cursor `c` serves its index at `now`:
/// ready, or bound to its network's mainnet chain, out of quarantine, not
/// diverged, with its whole history walked, advanced within [`READY_WINDOW`] and its last block
/// under `horizon` old. It does not measure the walk's speed
/// or publish lag. An `updated_at` ahead of `now` (written by the database
/// clock) counts as advanced up to [`MAX_CLOCK_SKEW`], and not past it.
pub fn is_walked(c: &Cursor, now: DateTime<Utc>, horizon: ChronoDuration) -> bool {
    if is_ready(c, now) {
        return true;
    }
    // As readiness asks: bound to the chain its readers expect, out of
    // quarantine and not diverged (its recovery still searching); otherwise
    // no claim it takes can settle yet.
    let bound =
        ChainNet::parse(&c.network).is_some_and(|net| c.chain_id == Some(mainnet_chain(net)));
    if !bound || c.quarantined_at.is_some() || c.diverged_at.is_some() {
        return false;
    }
    let advanced = now.naive_utc().signed_duration_since(c.updated_at);
    if advanced < -MAX_CLOCK_SKEW {
        return false;
    }
    let near_head = c
        .last_block_time
        .is_some_and(|t| now.timestamp() - t <= horizon.num_seconds());
    c.full_history && advanced <= READY_WINDOW && near_head
}

/// `Some(cursor)` when `indexer`, read on chain `chain_id`, may answer for
/// `net` at `now`; a database error reads as not ready (the envelope, never
/// a guess).
pub async fn ready_cursor_on(
    pool: &PgPool,
    indexer: &str,
    net: ChainNet,
    chain_id: i64,
    now: DateTime<Utc>,
) -> Option<Cursor> {
    match cursor(pool, indexer, net).await {
        Ok(Some(c)) if is_ready_on(&c, chain_id, now) => Some(c),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(error = %e, indexer, network = net.db_name(), "cannot read shop.chain_cursor; treating the index as not ready");
            None
        }
    }
}

/// [`ready_cursor_on`] for the indexes that read mainnet only.
pub async fn ready_cursor(
    pool: &PgPool,
    indexer: &str,
    net: ChainNet,
    now: DateTime<Utc>,
) -> Option<Cursor> {
    ready_cursor_on(pool, indexer, net, mainnet_chain(net), now).await
}

pub use crate::chain_index::logs::{
    LogGroup, LogSource, Read, RpcSource, Window, log_position, requests_for,
};

/// The newest block whose time is surely at or before `t`, from an exact
/// anchor at or after it: `anchor.number - (anchor.timestamp - t) /
/// min_block`, never later than the real one.
pub fn block_at_or_before(net: ChainNet, anchor: (u64, i64), t: i64) -> u64 {
    let (block, time) = anchor;
    if t >= time {
        return block;
    }
    let blocks = ((time - t) as u128 * 1000).div_ceil(net.min_block_ms() as u128) as u64;
    block.saturating_sub(blocks)
}

/// A time the chain surely had not passed at `block`, from an exact anchor
/// at or before it: `anchor.timestamp + (block - anchor.number) * max_block`.
pub fn time_at_most(net: ChainNet, anchor: (u64, i64), block: u64) -> i64 {
    let (at, time) = anchor;
    time + (block.saturating_sub(at) as i64).saturating_mul(net.max_block_ms() as i64) / 1000
}

/// A pass's outcome, for the job trace, the tests and the measurement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassOutcome {
    pub network: &'static str,
    pub head: u64,
    pub from_block: i64,
    pub indexed_to: i64,
    pub requests: usize,
    pub logs: usize,
    pub rows_written: usize,
    /// The budget ran out (or shutdown came) before the target.
    pub paused: bool,
    pub reorg_rewound_to: Option<i64>,
    pub folded: bool,
    pub published: bool,
    pub elapsed_ms: u64,
    /// The cursor is in quarantine, with its reason: the pass left it alone.
    pub quarantined: Option<String>,
    /// The pass rebuilt the projection from its start block, as asked.
    pub rebuilt: bool,
}

impl PassOutcome {
    pub fn blocks(&self) -> i64 {
        (self.indexed_to - self.from_block + 1).max(0)
    }

    /// `blocks/s` and `rows/s` of the pass, the catch-up measure.
    pub fn rates(&self) -> (f64, f64) {
        let secs = (self.elapsed_ms.max(1) as f64) / 1000.0;
        (self.blocks() as f64 / secs, self.rows_written as f64 / secs)
    }

    pub fn summary(&self) -> String {
        let (bps, rps) = self.rates();
        format!(
            "{}: blocks {}..{} of head {} ({} requests, {} logs, {} rows, {:.0} blocks/s, {:.0} rows/s){}{}{}{}{}",
            self.network,
            self.from_block,
            self.indexed_to,
            self.head,
            self.requests,
            self.logs,
            self.rows_written,
            bps,
            rps,
            if self.paused {
                "; paused at the budget"
            } else {
                ""
            },
            if self.folded { "; folded" } else { "" },
            if self.published { "; published" } else { "" },
            if self.rebuilt {
                "; rebuilt from its start"
            } else {
                ""
            },
            self.quarantined
                .as_ref()
                .map(|why| format!("; in quarantine: {why}"))
                .unwrap_or_default(),
        )
    }
}

/// Whether a cursor at `last_block` is held at its stop block, with nothing left to walk.
fn held_at_stop(last_block: i64, stop: Option<u64>) -> bool {
    stop.is_some_and(|s| last_block >= s as i64)
}

/// Clamp the chain's safe head to an optional operator stop block.
pub fn target_block(safe: u64, stop: Option<u64>) -> u64 {
    stop.map_or(safe, |s| s.min(safe))
}

/// Whether a fold at `indexed` may be published against a head read after it.
pub fn within_publish_lag(net: ChainNet, head: u64, indexed: u64) -> bool {
    head.saturating_sub(indexed) <= net.confirmations() + net.publish_lag()
}

/// One indexer's side of the walk: where it starts, what it asks for, how
/// it writes a window, how it folds, and how it undoes a reorg.
#[async_trait::async_trait]
pub trait ChainIndex: Send + Sync {
    fn indexer(&self) -> &'static str;
    fn net(&self) -> ChainNet;
    /// The chain the index reads on `net`: mainnet for every index but
    /// `trade_events`, which may read a testnet.
    fn chain_id(&self) -> i64 {
        mainnet_chain(self.net())
    }
    /// The first block a full history reads (the contracts' deployment).
    fn start_block(&self) -> u64;
    /// The filter groups of window `[from, to]`; `anchor` is an exact
    /// (block, time) at or before `from` for time-bounded address sets.
    async fn groups(
        &self,
        pool: &PgPool,
        from: u64,
        to: u64,
        anchor: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>>;
    /// Decodes and inserts one window's logs in the cursor's transaction;
    /// the rows actually written (duplicates dropped).
    async fn write(
        &self,
        conn: &mut PgConnection,
        logs: &[RpcLog],
        times: &HashMap<i64, i64>,
    ) -> anyhow::Result<usize>;
    /// Record filter-specific coverage in the same transaction as the cursor advance.
    async fn record_coverage(
        &self,
        _conn: &mut PgConnection,
        _previous: (i64, Option<&str>),
        _end: &BlockHeader,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    /// Deletes the log past `to` and refolds what it touched, in the
    /// transaction that rewinds the cursor.
    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()>;
    /// Undoes everything the projection derived down to `to`, for an
    /// operator's rebuild from its start: by default its reorg rewind. An
    /// index that keeps derived state its rewind does not recompute drops it
    /// here.
    async fn clear(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        self.rewind(conn, to).await
    }
    /// Folds the keys touched past `since` (`None`: rebuild) up to `upto`;
    /// whether the result may be published (nothing still owed). A fold
    /// that may not be published clears its cursor's `folded_block` in the
    /// transaction that commits its rows: the pass clears it only after it
    /// reads the head again, and that read can fail. Every transaction it
    /// commits ends with `owner.check`.
    async fn fold(
        &self,
        pool: &PgPool,
        since: Option<i64>,
        upto: i64,
        owner: &Owner,
    ) -> anyhow::Result<bool>;
    /// Work walked before the main range under the same budget (the nft
    /// backfill lane), read from the pass's source: what it charged the
    /// budget and the requests it sent ([`Spent`]).
    /// Every transaction it commits ends with `owner.check`. With the
    /// cursor's block and stored hash (`cursor`), each read is bracketed by
    /// that block's header, which must stay that hash: a stable bracket on
    /// another branch fails it with [`Diverged`], writing nothing.
    #[allow(
        clippy::too_many_arguments,
        reason = "the pass's source, anchor, budget, chunk, owner and cursor"
    )]
    async fn lanes(
        &self,
        _pool: &PgPool,
        _source: &dyn LogSource,
        _anchor: (u64, i64),
        _budget: usize,
        _chunk: &mut usize,
        _owner: &Owner,
        _cursor: Option<(u64, &str)>,
    ) -> anyhow::Result<Spent> {
        Ok(Spent::default())
    }
    /// One optional historical log window after publication, using spare log budget.
    async fn after_publish(
        &self,
        _pool: &PgPool,
        _source: &dyn LogSource,
        _owner: &Owner,
        _window_blocks: u64,
        _remaining: usize,
    ) -> anyhow::Result<usize> {
        Ok(0)
    }
    /// Forgets what the index keeps in memory between its calls (the nft
    /// walk's collection set and pending contracts) after a pass failed past
    /// it: the next pass reloads it from the database.
    fn abandon(&self) {}
    /// The start of a pass that does not run [`ChainIndex::lanes`] (a shared
    /// walk): what the index keeps in memory is reloaded, as lanes does first.
    fn begin_pass(&self) {}
    /// After each window's rows and cursor advance committed, in every walk:
    /// a test holds a commit's return here. Test builds only.
    #[cfg(test)]
    async fn window_committed(&self) {}
    /// Whether this pass must walk alone, with its lanes (the nft walk owing
    /// a backfill): a shared walk runs no lanes.
    async fn walks_alone(&self, _pool: &PgPool) -> anyhow::Result<bool> {
        Ok(false)
    }
}

/// Whether the pass may walk the cursor, after [`bind`].
enum Bound {
    Walk,
    Quarantined(String),
}

/// Proves the cursor's history on the owner's chain before the pass touches
/// it.
/// - A cursor in quarantine is left alone.
/// - A cursor bound to another chain goes to quarantine.
/// - A cursor written before the binding (`chain_id` NULL) is adopted when
///   it has no history yet, or when its stored last-block hash equals the
///   source's at that height; otherwise it goes to quarantine.
///
/// A source that does not answer fails the pass, with no decision about the
/// data. Adoption and quarantine are compare-and-sets on the owner's
/// generation and the cursor's checkpoint.
async fn bind(
    pool: &PgPool,
    source: &dyn LogSource,
    owner: &Owner,
    cur: &Cursor,
    out: &mut PassOutcome,
) -> anyhow::Result<Bound> {
    if let Some(why) = &cur.quarantine_reason {
        return Ok(Bound::Quarantined(why.clone()));
    }
    match cur.chain_id {
        Some(chain) if chain == owner.chain_id => return Ok(Bound::Walk),
        Some(chain) => {
            let why = format!(
                "its history is bound to chain {chain}; this node indexes chain {}",
                owner.chain_id
            );
            quarantine(pool, owner, cur, &why).await?;
            return Ok(Bound::Quarantined(why));
        }
        None => {}
    }
    let checkpoint = cur.last_block_hash.as_deref();
    let proof = if cur.last_block < cur.start_block {
        Ok(())
    } else if let Some(stored) = checkpoint {
        let header = source
            .header(cur.last_block as u64)
            .await
            .context("the cursor block's header, to prove its history")?;
        out.requests += header.requests;
        if header.value.hash == stored {
            Ok(())
        } else {
            Err(format!(
                "block {} is {} on chain {}, but its history stored {stored}",
                cur.last_block, header.value.hash, owner.chain_id
            ))
        }
    } else {
        Err(format!(
            "block {} has no stored hash to prove its history on chain {}",
            cur.last_block, owner.chain_id
        ))
    };
    match proof {
        Ok(()) => {
            let adopted = sqlx::query(
                "UPDATE shop.chain_cursor SET chain_id = $6 \
                 WHERE indexer = $1 AND network = $2 AND owner_generation = $3 AND chain_id IS NULL \
                   AND quarantine_reason IS NULL AND last_block = $4 AND last_block_hash IS NOT DISTINCT FROM $5",
            )
            .bind(owner.indexer)
            .bind(owner.net.db_name())
            .bind(owner.generation)
            .bind(cur.last_block)
            .bind(checkpoint)
            .bind(owner.chain_id)
            .execute(pool)
            .await?
            .rows_affected();
            if adopted != 1 {
                return Err(owner
                    .fenced("its cursor moved or was taken before the binding".into())
                    .into());
            }
            tracing::info!(
                indexer = owner.indexer,
                network = owner.net.db_name(),
                chain_id = owner.chain_id,
                block = cur.last_block,
                "marketplace index: bound the cursor's history to its chain"
            );
            Ok(Bound::Walk)
        }
        Err(why) => {
            quarantine(pool, owner, cur, &why).await?;
            Ok(Bound::Quarantined(why))
        }
    }
}

/// Sets the cursor apart (not walked, rewound or published) when it is
/// still at the checkpoint the pass read and the owner still holds it.
async fn quarantine(pool: &PgPool, owner: &Owner, cur: &Cursor, why: &str) -> anyhow::Result<()> {
    let updated = sqlx::query(
        "UPDATE shop.chain_cursor SET quarantined_at = timezone('UTC', now()), quarantine_reason = $6, \
             folded_block = NULL, folded_at = NULL \
         WHERE indexer = $1 AND network = $2 AND owner_generation = $3 AND quarantine_reason IS NULL \
           AND last_block = $4 AND last_block_hash IS NOT DISTINCT FROM $5",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(owner.generation)
    .bind(cur.last_block)
    .bind(cur.last_block_hash.as_deref())
    .bind(why)
    .execute(pool)
    .await?
    .rows_affected();
    if updated != 1 {
        return Err(owner
            .fenced("its cursor moved or was taken before the quarantine".into())
            .into());
    }
    tracing::error!(
        indexer = owner.indexer,
        network = owner.net.db_name(),
        reason = why,
        "marketplace index: the cursor is in quarantine; an operator revalidates or rebuilds it"
    );
    Ok(())
}

/// Rebuilds the projection from its start block, as an operator asked
/// (`rebuild_requested_at`), in one transaction:
/// 1. the notification producers' replay is planned from the state before
///    the cleanup;
/// 2. the projection's own cleanup runs down to `start_block - 1`, and the
///    producers' cursors move back;
/// 3. the cursor restarts there, bound to the owner's chain, unpublished and
///    out of quarantine, with the request cleared; its row is locked only
///    for this last step and the commit.
///
/// A rebuild keeps the projection's chain and start block. It is refused,
/// with the history kept and the cursor in quarantine, when the cursor is
/// bound to another chain, starts at another block, or has a history with no
/// binding that its checkpoint cannot prove on the owner's chain.
#[allow(
    clippy::too_many_arguments,
    reason = "the pass's source, index, settings, owner, cursor and outcome"
)]
async fn rebuild(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    owner: &Owner,
    cur: &Cursor,
    out: &mut PassOutcome,
) -> anyhow::Result<Option<String>> {
    let start = settings
        .start_override(owner.net)
        .unwrap_or(ix.start_block()) as i64;
    let full_history = start <= ix.start_block() as i64;
    let mut refusal = match cur.chain_id {
        Some(chain) if chain != owner.chain_id => Some(format!(
            "its history is from chain {chain}, and this node indexes chain {}",
            owner.chain_id
        )),
        _ if cur.start_block != start => Some(format!(
            "its history starts at block {}, and this node's walk starts at {start}",
            cur.start_block
        )),
        _ => None,
    };
    // A history with no binding is cleared only once its checkpoint is
    // proven on the owner's chain: it may be another chain's, whose
    // notification cursors a rebuild here would replay. A source that does
    // not answer fails the pass and decides nothing.
    if refusal.is_none() && cur.chain_id.is_none() && cur.last_block >= cur.start_block {
        refusal = match cur.last_block_hash.as_deref() {
            None => Some(format!(
                "block {} has no stored hash to prove its history on chain {}",
                cur.last_block, owner.chain_id
            )),
            Some(stored) => {
                let header = source
                    .header(cur.last_block as u64)
                    .await
                    .context("the cursor block's header, to prove the history a rebuild clears")?;
                out.requests += header.requests;
                (header.value.hash != stored).then(|| {
                    format!(
                        "block {} is {} on chain {}, but its history stored {stored}",
                        cur.last_block, header.value.hash, owner.chain_id
                    )
                })
            }
        };
    }
    // The request, the owner and the checkpoint the pass read, as they stand.
    let expected = Some((
        owner.generation,
        true,
        cur.last_block,
        cur.last_block_hash.clone(),
    ));
    let read = |lock: &'static str| {
        sqlx::AssertSqlSafe(format!(
            "SELECT owner_generation, rebuild_requested_at IS NOT NULL, last_block, last_block_hash \
             FROM shop.chain_cursor WHERE indexer = $1 AND network = $2{lock}"
        ))
    };
    if let Some(why) = refusal {
        let why = format!(
            "rebuild refused: {why}; a rebuild keeps the projection's chain and start block, \
             and a database for another chain or start begins empty"
        );
        let set = sqlx::query(
            "UPDATE shop.chain_cursor SET quarantined_at = COALESCE(quarantined_at, timezone('UTC', now())), \
                 quarantine_reason = $6, rebuild_requested_at = NULL, folded_block = NULL, folded_at = NULL \
             WHERE indexer = $1 AND network = $2 AND owner_generation = $3 AND rebuild_requested_at IS NOT NULL \
               AND last_block = $4 AND last_block_hash IS NOT DISTINCT FROM $5",
        )
        .bind(owner.indexer)
        .bind(owner.net.db_name())
        .bind(owner.generation)
        .bind(cur.last_block)
        .bind(cur.last_block_hash.as_deref())
        .bind(&why)
        .execute(pool)
        .await?
        .rows_affected();
        if set != 1 {
            return Err(owner
                .fenced("its rebuild, or the checkpoint it read, changed".into())
                .into());
        }
        tracing::error!(indexer = owner.indexer, network = owner.net.db_name(), reason = %why, "marketplace index: refused a rebuild; the cursor is in quarantine");
        return Ok(Some(why));
    }
    let mut tx = pool.begin().await?;
    // Checked without a lock first, so a stale owner does no cleanup. The
    // cursor row is locked last, after the cleanup and the producers' cursor
    // moves, so it is held only for the commit; every writer takes the
    // notification cursor rows before the chain cursor row.
    let held: Option<(i64, bool, i64, Option<String>)> = sqlx::query_as(read(""))
        .bind(owner.indexer)
        .bind(owner.net.db_name())
        .fetch_optional(&mut *tx)
        .await?;
    if held != expected {
        return Err(owner
            .fenced("its rebuild, or the checkpoint it read, changed".into())
            .into());
    }
    let to = start - 1;
    let replay =
        super::notice_cursor::plan_replay(&mut tx, owner.indexer, owner.net, to, cur.last_block)
            .await?;
    ix.clear(&mut tx, to)
        .await
        .context("the projection's cleanup for its rebuild")?;
    if let Some(replay) = replay {
        replay.apply(&mut tx).await?;
    }
    let held: Option<(i64, bool, i64, Option<String>)> = sqlx::query_as(read(" FOR UPDATE"))
        .bind(owner.indexer)
        .bind(owner.net.db_name())
        .fetch_optional(&mut *tx)
        .await?;
    if held != expected {
        return Err(owner
            .fenced("its rebuild, or the checkpoint it read, changed".into())
            .into());
    }
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = $3, last_block_hash = NULL, last_block_time = NULL, \
             fold_through = NULL, folded_block = NULL, folded_at = NULL, full_history = $4, chain_id = $5, \
             quarantined_at = NULL, quarantine_reason = NULL, rebuild_requested_at = NULL, diverged_at = NULL, \
             updated_at = timezone('UTC', now()) \
         WHERE indexer = $1 AND network = $2",
    )
    .bind(owner.indexer)
    .bind(owner.net.db_name())
    .bind(to)
    .bind(full_history)
    .bind(owner.chain_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM shop.chain_checkpoint WHERE indexer = $1 AND network = $2")
        .bind(owner.indexer)
        .bind(owner.net.db_name())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::warn!(
        indexer = owner.indexer,
        network = owner.net.db_name(),
        from = cur.last_block,
        to,
        "marketplace index: rebuilt the projection from its start block, as an operator asked"
    );
    Ok(None)
}

/// One pass as a job runs it:
/// 1. the source's chain checked against `ix`'s (an endpoint serving another
///    chain fails the pass, with no decision about the data);
/// 2. the projection's owner for the job's current lease acquisition;
/// 3. [`run_pass`].
///
/// A fenced write drops the holding, so the next pass claims the projection
/// again, which only succeeds while the job still holds its lease.
#[allow(
    clippy::too_many_arguments,
    reason = "the pass's inputs, as run_pass takes them, plus the job's lease and holdings"
)]
pub async fn job_pass(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    holdings: &Holdings,
    lease: Option<&crate::jobs::LeaseControl>,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    progress: &(dyn Fn(String) + Send + Sync),
) -> anyhow::Result<PassOutcome> {
    job_pass_with(
        pool,
        source,
        ix,
        settings,
        holdings,
        lease,
        Budget::fresh(settings),
        cancel,
        progress,
    )
    .await
}

/// [`job_pass`] spending at most `budget`: a shared walk's member that fell
/// back goes on alone with what it had left.
#[allow(
    clippy::too_many_arguments,
    reason = "job_pass's inputs and the budget"
)]
pub async fn job_pass_with(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    holdings: &Holdings,
    lease: Option<&crate::jobs::LeaseControl>,
    budget: Budget,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    progress: &(dyn Fn(String) + Send + Sync),
) -> anyhow::Result<PassOutcome> {
    super::ensure_chain(source, ix.chain_id()).await?;
    let owner = holdings.owner(pool, lease, ix, settings).await?;
    let outcome = run_pass_with(pool, source, ix, settings, &owner, budget, cancel, progress).await;
    match outcome {
        Ok(mut o) => {
            o.requests += 1;
            Ok(o)
        }
        Err(e) => {
            if e.chain().any(|c| c.downcast_ref::<Fenced>().is_some()) {
                holdings.forget(ix.indexer(), ix.net());
            }
            Err(e)
        }
    }
}

/// What a logical pass may still spend on log reads, and whether its first
/// window (which runs even over the budget, so every pass progresses) was
/// already read. A member of a shared walk carries it into its fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub remaining: usize,
    pub first_window_used: bool,
}

impl Budget {
    /// A pass's whole budget, `requests_per_pass`.
    pub fn fresh(settings: &IndexSettings) -> Self {
        Self {
            remaining: settings.requests_per_pass.max(1),
            first_window_used: false,
        }
    }
}

/// One pass of `ix` over `net`, read from `source`, as `owner` (the
/// projection's generation, which every transaction checks before its
/// commit):
/// 1. an operator's rebuild request;
/// 2. the history's binding to the owner's chain ([`bind`]);
/// 3. the reorg check;
/// 4. the backfill lanes;
/// 5. the windows from the cursor to the confirmed head under the request
///    budget, each committed with its cursor advance;
/// 6. once caught up, the fold and the publish ([`close`]).
pub async fn run_pass(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    owner: &Owner,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    progress: &(dyn Fn(String) + Send + Sync),
) -> anyhow::Result<PassOutcome> {
    run_pass_with(
        pool,
        source,
        ix,
        settings,
        owner,
        Budget::fresh(settings),
        cancel,
        progress,
    )
    .await
}

/// [`run_pass`] spending at most `budget`.
#[allow(
    clippy::too_many_arguments,
    reason = "run_pass's inputs and the budget"
)]
pub async fn run_pass_with(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    owner: &Owner,
    budget: Budget,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    progress: &(dyn Fn(String) + Send + Sync),
) -> anyhow::Result<PassOutcome> {
    let outcome = walk(pool, source, ix, settings, owner, budget, cancel, progress).await;
    if outcome.is_err() {
        ix.abandon();
    }
    outcome
}

/// What a pass's opening steps came to.
pub(crate) enum Opening {
    /// The pass ended there: a rebuild, a quarantine or a reorg rewind.
    Ended(PassOutcome),
    /// The cursor stands at its checkpoint, proven on the chain.
    Walk(Box<Cursor>, PassOutcome),
    /// The cursor block is on another branch than its stored hash: the
    /// publication is withdrawn, and the recovery is left to a pass alone.
    Diverged(PassOutcome),
}

/// The opening of a pass as `owner`: the cursor, an operator's rebuild, the
/// binding to the owner's chain and the reorg check, whose header
/// `at_checkpoint` gives when a shared walk already read it. A cursor block
/// on another branch withdraws the publication first; then the opening
/// recovers ([`recover`]) when `recover_here`, and ends
/// [`Opening::Diverged`] otherwise (a shared walk, whose members' steps run
/// under a deadline, leaves the search to each one's pass alone).
#[allow(
    clippy::too_many_arguments,
    reason = "the pass's inputs, the shared walk's header and where it recovers"
)]
pub(crate) async fn open(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    owner: &Owner,
    started: std::time::Instant,
    at_checkpoint: Option<&BlockHeader>,
    recover_here: bool,
) -> anyhow::Result<Opening> {
    let net = ix.net();
    let name = ix.indexer();
    anyhow::ensure!(
        owner.indexer == name && owner.net == net,
        "the owner holds {} {}, not {name} {}",
        owner.indexer,
        owner.net.db_name(),
        net.db_name()
    );
    let cur = cursor(pool, name, net)
        .await?
        .context("the chain cursor row is missing (claiming the projection creates it)")?;
    let configured = settings.start_override(net).unwrap_or(ix.start_block());
    if cur.start_block != configured as i64 {
        tracing::info!(
            indexer = name,
            network = net.db_name(),
            stored = cur.start_block,
            configured,
            "chain cursor keeps its stored start block"
        );
    }
    let mut out = PassOutcome {
        network: net.db_name(),
        from_block: cur.last_block + 1,
        indexed_to: cur.last_block,
        ..Default::default()
    };
    if cur.rebuild_requested_at.is_some() {
        match rebuild(pool, source, ix, settings, owner, &cur, &mut out).await? {
            None => {
                out.rebuilt = true;
                out.indexed_to = cur.start_block - 1;
            }
            Some(why) => out.quarantined = Some(why),
        }
        out.elapsed_ms = started.elapsed().as_millis() as u64;
        return Ok(Opening::Ended(out));
    }
    if let Bound::Quarantined(why) = bind(pool, source, owner, &cur, &mut out).await? {
        out.quarantined = Some(why);
        out.elapsed_ms = started.elapsed().as_millis() as u64;
        return Ok(Opening::Ended(out));
    }
    if cur.last_block >= cur.start_block {
        let stored = cur.last_block_hash.as_deref().unwrap_or("");
        let header = match at_checkpoint.filter(|h| h.number as i64 == cur.last_block) {
            Some(h) => h.clone(),
            None => {
                let header = source
                    .header(cur.last_block as u64)
                    .await
                    .context("eth_getBlockByNumber at the cursor")?;
                out.requests += header.requests;
                header.value
            }
        };
        if header.hash != stored {
            tracing::warn!(indexer = name, network = net.db_name(), block = cur.last_block, stored, chain = %header.hash, "marketplace index: the cursor block's hash changed (a reorg past the confirmation depth); its publication is withdrawn");
            withdraw(
                pool,
                owner,
                (cur.last_block, cur.last_block_hash.as_deref()),
            )
            .await?;
            if !recover_here {
                out.elapsed_ms = started.elapsed().as_millis() as u64;
                return Ok(Opening::Diverged(out));
            }
            recover(pool, source, ix, owner, &cur, &mut out).await?;
            out.elapsed_ms = started.elapsed().as_millis() as u64;
            return Ok(Opening::Ended(out));
        }
        if cur.diverged_at.is_some() {
            // The chain flipped back before any rewind: the cursor is proven
            // again, so its readers may use its height again.
            undiverge(
                pool,
                owner,
                (cur.last_block, cur.last_block_hash.as_deref()),
            )
            .await?;
        }
    }
    Ok(Opening::Walk(Box::new(cur), out))
}

/// The recovery of a projection whose cursor `cur` diverged (its block, or
/// the block a window extended, is on another branch than its stored hash),
/// once its publication was withdrawn. Its recovery points below the cursor
/// are compared newest first with the chain's headers, outside any
/// transaction; the newest still on the chain is the one rewound to, keeping
/// its hash. With none recorded, or every one compared and none on the
/// chain, the rewind goes to the start block. A header read that fails ends
/// the recovery with nothing rewound (the next pass searches again); the
/// reads count in the pass but are not cut by its budget, so a slow search
/// still ends in one pass. The rewind's transaction runs the notification
/// cursors' moves before the cursor row's lock, as every writer does.
pub(crate) async fn recover(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    owner: &Owner,
    cur: &Cursor,
    out: &mut PassOutcome,
) -> anyhow::Result<()> {
    let (name, net) = (ix.indexer(), ix.net());
    let candidates: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT block_number, block_hash, block_time FROM shop.chain_checkpoint \
         WHERE indexer = $1 AND network = $2 AND block_number < $3 AND block_number >= $4 \
         ORDER BY block_number DESC LIMIT $5",
    )
    .bind(name)
    .bind(net.db_name())
    .bind(cur.last_block)
    .bind(cur.start_block)
    .bind(CHECKPOINTS_KEPT)
    .fetch_all(pool)
    .await?;
    let candidate_count = candidates.len();
    let mut probed = 0usize;
    let mut found = None;
    for (block, hash, time) in candidates {
        probed += 1;
        let header = source
            .header(block as u64)
            .await
            .with_context(|| format!("the header at recovery point {block}"))?;
        out.requests += header.requests;
        if header.value.hash == hash {
            found = Some((block, hash, time));
            break;
        }
    }
    let (to, keep) = match &found {
        Some((block, hash, time)) => (*block, Some((hash.as_str(), *time))),
        None => (cur.start_block - 1, None),
    };
    let mut tx = pool.begin().await?;
    // Checked without a lock first, so a stale owner does no cleanup; the
    // cursor row is locked by the rewind, after the producers' cursors.
    let held: Option<(i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT owner_generation, last_block, last_block_hash FROM shop.chain_cursor WHERE indexer = $1 AND network = $2",
    )
    .bind(name)
    .bind(net.db_name())
    .fetch_optional(&mut *tx)
    .await?;
    if held
        != Some((
            owner.generation,
            cur.last_block,
            cur.last_block_hash.clone(),
        ))
    {
        return Err(owner
            .fenced("the checkpoint its recovery read moved".into())
            .into());
    }
    let replay = super::notice_cursor::plan_replay(&mut tx, name, net, to, cur.last_block).await?;
    ix.rewind(&mut tx, to)
        .await
        .context("undoing the reorged range")?;
    if let Some(replay) = replay {
        replay.apply(&mut tx).await?;
    }
    rewind(
        &mut tx,
        name,
        net,
        (cur.last_block, cur.last_block_hash.as_deref()),
        to,
        keep,
    )
    .await
    .context("rewinding the cursor from the checkpoint it was read at")?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    tracing::warn!(
        indexer = name,
        network = net.db_name(),
        from = cur.last_block,
        rewound_to = to,
        candidates = candidate_count,
        probed,
        bounded = found.is_some(),
        "marketplace index: recovered from a reorg past the confirmation depth"
    );
    out.reorg_rewound_to = Some(to);
    Ok(())
}

/// The latest head and the safe head of `net` (Polygon's finalized block,
/// Ethereum's head less its confirmations), with Polygon's finalized header.
pub(crate) async fn safe_head(
    source: &dyn LogSource,
    net: ChainNet,
    out: &mut PassOutcome,
) -> anyhow::Result<(u64, u64, Option<BlockHeader>)> {
    let head = source.head().await.context("eth_blockNumber")?;
    out.requests += head.requests;
    let head = head.value;
    let (safe, finalized) = match net {
        ChainNet::Matic => {
            let finalized = source.finalized().await.context("Polygon finalized head")?;
            out.requests += finalized.requests;
            (finalized.value.number, Some(finalized.value))
        }
        ChainNet::Ethereum => (head.saturating_sub(net.confirmations()), None),
    };
    anyhow::ensure!(safe <= head, "safe head exceeds latest head");
    Ok((head, safe, finalized))
}

/// Commits one window of `ix`: its rows and the cursor's advance from
/// `expected` to `header`, in one transaction fenced by `owner`.
pub(crate) async fn commit_window(
    pool: &PgPool,
    ix: &dyn ChainIndex,
    owner: &Owner,
    expected: (i64, Option<&str>),
    header: &BlockHeader,
    logs: &[RpcLog],
    times: &HashMap<i64, i64>,
) -> anyhow::Result<usize> {
    let mut tx = pool.begin().await?;
    let written = ix.write(&mut tx, logs, times).await?;
    advance(&mut tx, ix.indexer(), ix.net(), expected, header)
        .await
        .context("advancing the cursor")?;
    ix.record_coverage(&mut tx, expected, header).await?;
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(written)
}

#[allow(
    clippy::too_many_arguments,
    reason = "run_pass's inputs and the budget"
)]
async fn walk(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    owner: &Owner,
    budget: Budget,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    progress: &(dyn Fn(String) + Send + Sync),
) -> anyhow::Result<PassOutcome> {
    let started = std::time::Instant::now();
    let net = ix.net();
    let name = ix.indexer();
    let (cur, mut out) = match open(pool, source, ix, settings, owner, started, None, true).await? {
        Opening::Ended(out) | Opening::Diverged(out) => return Ok(out),
        Opening::Walk(cur, out) => (cur, out),
    };
    let (head, safe_head, _) = safe_head(source, net, &mut out).await?;
    out.head = head;
    let stop = settings.stop_block(net);
    let target = target_block(safe_head, stop);
    touch_once(
        pool,
        owner,
        (cur.last_block, cur.last_block_hash.as_deref()),
        stop,
    )
    .await?;
    let anchor = match cur.last_block_time {
        Some(t) => (cur.last_block.max(0) as u64, t),
        None => {
            let h = source
                .header(cur.last_block.max(0) as u64)
                .await
                .context("the time anchor")?;
            out.requests += h.requests;
            (h.value.number, h.value.timestamp)
        }
    };
    // What is left, not at least one: a fallback with nothing left reads
    // nothing (`Budget::fresh` keeps a whole pass at one at least).
    let total = budget.remaining;
    let mut chunk = settings.address_chunk.max(1);
    // The lanes read below the cursor, each window bracketed against the
    // cursor block's stored hash; one on another branch is a divergence.
    let durable = cur
        .last_block_hash
        .as_deref()
        .filter(|_| cur.last_block >= cur.start_block)
        .map(|h| (cur.last_block as u64, h));
    let spent = match ix
        .lanes(pool, source, anchor, total, &mut chunk, owner, durable)
        .await
    {
        Ok(spent) => spent,
        Err(e) if e.downcast_ref::<Diverged>().is_some() => {
            out.requests += e.downcast_ref::<Diverged>().map_or(0, |d| d.requests);
            // Lanes bracket only a cursor with a hash (`durable`).
            let (block, hash) = durable.expect("a lane divergence is about the cursor's hash");
            let at = (block as i64, hash.to_string());
            return diverged(pool, source, ix, owner, at, out, started).await;
        }
        Err(e) => return Err(e),
    };
    // The budget pays what the lanes charged; the pass reports what they sent.
    let mut log_requests = spent.charge;
    out.requests += spent.requests;
    let mut next = (cur.last_block + 1) as u64;
    let mut expected = (cur.last_block, cur.last_block_hash.clone());
    while next <= target {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            out.paused = true;
            break;
        }
        let to = (next + settings.window_blocks.max(1) - 1).min(target);
        let groups = ix.groups(pool, next, to, anchor).await?;
        let need = requests_for(&groups, chunk).max(1);
        if log_requests + need > total && (log_requests > 0 || budget.first_window_used) {
            out.paused = true;
            break;
        }
        // The bracket: the header at `to` before and after the logs, their
        // times, the header at the window's actual end and the one at the
        // checkpoint it extends. Under an endpoint that answers from one
        // branch between two equal reads at `to`, the end's hash names the
        // branch the logs came from, and the predecessor proves the window
        // extends the committed prefix.
        let before = source.header(to).await?;
        out.requests += before.requests;
        let window = source
            .window(next, to, &groups, &mut chunk)
            .await
            .with_context(|| format!("eth_getLogs [{next}, {to}]"))?;
        out.requests += window.requests;
        log_requests += window.charge;
        let times = source
            .block_times(&window.logs)
            .await
            .context("block times")?;
        out.requests += times.requests;
        let times = times.value;
        let reached = if window.reached == to {
            None
        } else {
            let reached = source.header(window.reached).await?;
            out.requests += reached.requests;
            Some(reached.value)
        };
        let predecessor = match expected.1.as_deref() {
            Some(_) => {
                let p = source.header(expected.0 as u64).await?;
                out.requests += p.requests;
                Some(p.value)
            }
            None => None,
        };
        let after = source.header(to).await?;
        out.requests += after.requests;
        anyhow::ensure!(
            before.value == after.value,
            "chain changed while fetching window"
        );
        if let (Some(p), Some(hash)) = (&predecessor, expected.1.as_deref())
            && p.hash != hash
        {
            // A stable bracket that finds the checkpoint on another branch:
            // evidence about the cursor, not a read error.
            let at = (expected.0, hash.to_string());
            return diverged(pool, source, ix, owner, at, out, started).await;
        }
        let header = reached.unwrap_or(after.value);
        // Accepted, not yet committed (the commit may still fail).
        source.accepted(&crate::chain_index::portal::compare::AcceptedView {
            chain_id: ix.chain_id() as u64,
            from: next,
            reached: window.reached,
            reached_hash: &header.hash,
            groups: &groups,
            logs: &window.logs,
            times: &times,
        });
        let written = commit_window(
            pool,
            ix,
            owner,
            (expected.0, expected.1.as_deref()),
            &header,
            &window.logs,
            &times,
        )
        .await?;
        #[cfg(test)]
        ix.window_committed().await;
        out.logs += window.logs.len();
        out.rows_written += written;
        out.indexed_to = window.reached as i64;
        expected = (window.reached as i64, Some(header.hash.clone()));
        next = window.reached + 1;
        progress(format!(
            "{} {}: indexed to {} of {} ({} requests this pass)",
            name,
            net.db_name(),
            window.reached,
            target,
            out.requests
        ));
    }
    if out.paused || (out.indexed_to as u64) < target {
        out.elapsed_ms = started.elapsed().as_millis() as u64;
        return Ok(out);
    }
    close(
        pool,
        source,
        ix,
        settings,
        owner,
        Close {
            fold_since: cur.fold_through,
            remaining: total.saturating_sub(log_requests),
        },
        &mut out,
    )
    .await?;
    out.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(out)
}

/// A divergence a pass alone found mid-walk (a window's predecessor, or a
/// lane's bracket, on another branch than the checkpoint `at`): the
/// publication withdrawn, then the recovery from the cursor as it stands,
/// which must still be `at`. The pass ends there, its index's memory dropped.
async fn diverged(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    owner: &Owner,
    at: (i64, String),
    mut out: PassOutcome,
    started: std::time::Instant,
) -> anyhow::Result<PassOutcome> {
    tracing::warn!(indexer = ix.indexer(), network = ix.net().db_name(), block = at.0, stored = %at.1, "marketplace index: a checkpoint the walk extends is on another branch; its publication is withdrawn");
    withdraw(pool, owner, (at.0, Some(at.1.as_str()))).await?;
    let cur = cursor(pool, ix.indexer(), ix.net())
        .await?
        .context("the chain cursor row is missing")?;
    if (cur.last_block, cur.last_block_hash.as_deref()) != (at.0, Some(at.1.as_str())) {
        return Err(owner
            .fenced("the checkpoint a divergence was seen at moved".into())
            .into());
    }
    ix.abandon();
    recover(pool, source, ix, owner, &cur, &mut out).await?;
    out.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(out)
}

pub struct Close {
    pub fold_since: Option<i64>,
    pub remaining: usize,
}

/// The close of a pass that reached its target at `out.indexed_to`: the fold
/// of what was touched since `fold_since`, its record, a fresh read of the
/// heads, and the publish (or its withdrawal) against them.
pub(crate) async fn close(
    pool: &PgPool,
    source: &dyn LogSource,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
    owner: &Owner,
    close: Close,
    out: &mut PassOutcome,
) -> anyhow::Result<()> {
    let net = ix.net();
    let stop = settings.stop_block(net);
    let may_publish = ix
        .fold(pool, close.fold_since, out.indexed_to, owner)
        .await
        .context("folding")?;
    record_fold(pool, owner, out.indexed_to).await?;
    out.folded = true;
    let latest = source
        .head()
        .await
        .context("eth_blockNumber after the fold")?;
    out.requests += latest.requests;
    let latest = latest.value;
    let latest_safe = match net {
        ChainNet::Matic => {
            let finalized = source
                .finalized()
                .await
                .context("Polygon finalized head after fold")?;
            out.requests += finalized.requests;
            finalized.value.number
        }
        ChainNet::Ethereum => latest.saturating_sub(net.confirmations()),
    };
    let held_short = stop.is_some_and(|s| s < latest_safe);
    if may_publish
        && !held_short
        && out.indexed_to >= 0
        && out.indexed_to as u64 <= latest_safe
        && within_publish_lag(net, latest, out.indexed_to as u64)
    {
        out.published = mark_folded(pool, owner, out.indexed_to, Utc::now()).await?;
    } else {
        unpublish(pool, owner, out.indexed_to).await?;
    }
    if out.published && close.remaining > 0 {
        match ix
            .after_publish(
                pool,
                source,
                owner,
                settings.window_blocks.max(1),
                close.remaining,
            )
            .await
        {
            Ok(requests) => out.requests += requests,
            Err(e)
                if e.chain()
                    .any(|cause| cause.downcast_ref::<Fenced>().is_some()) =>
            {
                return Err(e);
            }
            Err(e) => {
                tracing::warn!(indexer = ix.indexer(), error = %e, "post-publication repair will retry")
            }
        }
    }
    Ok(())
}

/// The owner of `ix`'s projection for a test that walks or folds it outside
/// a job: claimed on the pool, as a process without leases claims it.
#[cfg(test)]
pub(crate) async fn test_owner(
    pool: &PgPool,
    ix: &dyn ChainIndex,
    settings: &IndexSettings,
) -> Owner {
    Holdings::default()
        .owner(pool, None, ix, settings)
        .await
        .expect("claiming the projection")
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_topic_decoders_preserve_prefix_width_and_integer_boundaries() {
        let address = format!("0x{}{}", "F".repeat(24), "AB".repeat(20));
        assert_eq!(
            super::topic_address(&address),
            Some(format!("0x{}", "ab".repeat(20)))
        );
        assert_eq!(super::topic_address(&address[2..]), None);
        assert_eq!(super::topic_address("0x01"), None);
        assert_eq!(super::topic_u256("0x0"), Some("0".into()));
        assert_eq!(super::topic_u256("0xFF"), Some("255".into()));
        assert_eq!(
            super::topic_u256(&format!("0x{}", "f".repeat(64))),
            Some(alloy_primitives::U256::MAX.to_string())
        );
        assert_eq!(super::topic_u256("0x"), Some("0".into()));
        for invalid in ["ff", "0xgg"] {
            assert_eq!(super::topic_u256(invalid), None);
        }
        assert_eq!(super::topic_u256(&format!("0x1{}", "0".repeat(64))), None);
    }

    use super::*;

    #[test]
    fn each_network_indexes_its_mainnet_or_its_testnet_only() {
        let defaults = IndexSettings::default();
        assert_eq!(
            (
                defaults.chain_id(ChainNet::Ethereum),
                defaults.chain_id(ChainNet::Matic)
            ),
            (1, 137)
        );
        assert!(defaults.check_chains().is_ok());
        let testnet = IndexSettings {
            chain_id_ethereum: 11_155_111,
            chain_id_matic: 80_002,
            ..defaults.clone()
        };
        assert!(testnet.check_chains().is_ok());
        let mixed = IndexSettings {
            nft_enabled: true,
            chain_id_matic: 80_002,
            ..defaults.clone()
        };
        assert!(mixed.check_chains().is_err());
        for (ethereum, matic) in [(137, 137), (1, 1), (1, 80_001), (80_002, 80_002), (1, 0)] {
            let wrong = IndexSettings {
                chain_id_ethereum: ethereum,
                chain_id_matic: matic,
                ..defaults.clone()
            };
            assert!(wrong.check_chains().is_err(), "{ethereum} / {matic}");
        }
    }

    fn cursor_row(last: i64, folded: Option<i64>, at: Option<DateTime<Utc>>, full: bool) -> Cursor {
        Cursor {
            indexer: INDEXER_NFT.into(),
            network: "matic".into(),
            start_block: 1,
            last_block: last,
            last_block_hash: None,
            last_block_time: None,
            fold_through: folded,
            folded_block: folded,
            folded_at: at.map(|t| t.naive_utc()),
            full_history: full,
            reorg_rewinds: 0,
            updated_at: Utc::now().naive_utc(),
            chain_id: Some(137),
            owner_generation: 1,
            quarantined_at: None,
            quarantine_reason: None,
            rebuild_requested_at: None,
            diverged_at: None,
        }
    }

    #[test]
    fn a_walk_is_held_only_once_it_reaches_its_stop_block() {
        assert!(!held_at_stop(35, Some(36)));
        assert!(held_at_stop(36, Some(36)));
        assert!(!held_at_stop(1_000, None));
    }

    #[test]
    fn a_walk_serves_while_it_publishes_or_nears_the_head() {
        let now = Utc::now();
        let hour = ChronoDuration::hours(1);
        let walking = |full: bool, advanced: ChronoDuration, behind: ChronoDuration| Cursor {
            last_block_time: Some((now - behind).timestamp()),
            updated_at: (now - advanced).naive_utc(),
            ..cursor_row(100, None, None, full)
        };
        let minutes = ChronoDuration::minutes;
        assert!(
            is_walked(&walking(true, minutes(1), minutes(5)), now, hour),
            "near the head"
        );
        assert!(
            is_walked(&walking(true, minutes(-5), minutes(5)), now, hour),
            "a database clock ahead"
        );
        assert!(
            !is_walked(&walking(true, minutes(-120), minutes(5)), now, hour),
            "an updated_at too far in the future is no sign of life"
        );
        assert!(
            !is_walked(&walking(true, minutes(1), minutes(120)), now, hour),
            "a catch-up too far behind to publish in time"
        );
        assert!(
            !is_walked(&walking(false, minutes(1), minutes(5)), now, hour),
            "a walk without its whole history never publishes"
        );
        assert!(
            !is_walked(&walking(true, minutes(120), minutes(5)), now, hour),
            "a walk that stopped"
        );
        assert!(
            is_walked(
                &Cursor {
                    updated_at: (now - minutes(120)).naive_utc(),
                    ..cursor_row(100, Some(100), Some(now - minutes(1)), true)
                },
                now,
                hour
            ),
            "caught up and publishing"
        );
    }

    #[test]
    fn readiness_needs_a_whole_history_folded_at_its_cursor_and_recently() {
        let now = Utc::now();
        assert!(is_ready(
            &cursor_row(100, Some(100), Some(now - ChronoDuration::minutes(5)), true),
            now
        ));
        assert!(
            !is_ready(
                &cursor_row(
                    100,
                    Some(100),
                    Some(now - ChronoDuration::minutes(5)),
                    false
                ),
                now
            ),
            "a bounded window never publishes"
        );
        assert!(!is_ready(&cursor_row(101, Some(100), Some(now), true), now));
        assert!(!is_ready(
            &cursor_row(
                100,
                Some(100),
                Some(now - ChronoDuration::minutes(31)),
                true
            ),
            now
        ));
        assert!(!is_ready(&cursor_row(100, None, None, true), now));
        assert!(!is_ready(
            &cursor_row(100, Some(100), Some(now + ChronoDuration::seconds(5)), true),
            now
        ));
    }

    #[test]
    fn the_time_bounds_are_conservative_both_ways() {
        // Polygon at 2 s a block: 3,600 s before the anchor is 1,800 blocks back;
        // the lower bound divides by 0.5 s and goes 7,200 back.
        let anchor = (1_000_000u64, 1_700_000_000i64);
        assert_eq!(
            block_at_or_before(ChainNet::Matic, anchor, 1_700_000_000 - 3_600),
            1_000_000 - 7_200
        );
        assert_eq!(
            block_at_or_before(ChainNet::Matic, anchor, 1_800_000_000),
            1_000_000,
            "a time after the anchor is the anchor"
        );
        assert_eq!(
            block_at_or_before(ChainNet::Matic, (10, 1_000), 0),
            0,
            "never below genesis"
        );
        // 1,000 blocks after the anchor are at most 6,000 s later on Polygon.
        assert_eq!(
            time_at_most(ChainNet::Matic, anchor, 1_001_000),
            1_700_000_000 + 6_000
        );
        assert_eq!(
            time_at_most(ChainNet::Ethereum, anchor, 1_000_100),
            1_700_000_000 + 3_000
        );
    }

    #[test]
    fn the_target_keeps_the_confirmation_depth_and_the_stop_block() {
        assert_eq!(target_block(1_000, None), 1_000);
        assert_eq!(target_block(1_000, Some(500)), 500);
        assert_eq!(target_block(0, None), 0);
        assert!(within_publish_lag(ChainNet::Ethereum, 1_000, 963));
        assert!(!within_publish_lag(ChainNet::Ethereum, 1_000, 962));
    }

    #[test]
    fn the_ready_sql_is_the_rust_rule() {
        let sql = ready_sql("c", "$9");
        assert!(
            sql.contains("c.full_history")
                && sql.contains("c.folded_block = c.last_block")
                && sql.contains("interval '30 minutes'"),
            "{sql}"
        );
        assert_eq!(
            READY_WINDOW,
            ChronoDuration::minutes(30),
            "ready_sql spells the window out"
        );
    }

    #[test]
    fn rates_divide_by_the_elapsed_time() {
        let o = PassOutcome {
            network: "matic",
            from_block: 101,
            indexed_to: 200,
            rows_written: 50,
            elapsed_ms: 2_000,
            ..Default::default()
        };
        assert_eq!(o.blocks(), 100);
        assert_eq!(o.rates(), (50.0, 25.0));
        assert!(o.summary().contains("50 blocks/s"), "{}", o.summary());
    }

    /// A rebuild or a never-analyzed table analyzes; a steady-state fold over
    /// a table whose statistics are current does not.
    #[tokio::test]
    async fn a_fold_analyzes_only_on_a_rebuild_or_stale_statistics() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let schema = format!("an_{}", uuid::Uuid::new_v4().simple());
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}; CREATE TABLE {schema}.ev (n int); INSERT INTO {schema}.ev SELECT generate_series(1, 5000)")))
            .execute(&pool)
            .await
            .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        assert!(
            super::analyze_if_stale(&mut conn, &schema, "ev", false)
                .await
                .unwrap(),
            "never analyzed"
        );
        assert!(
            crate::shared::test_wait::until(async || {
                sqlx::query_scalar("SELECT last_analyze IS NOT NULL FROM pg_stat_user_tables WHERE schemaname = $1 AND relname = 'ev'")
                    .bind(&schema)
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap()
            })
            .await,
            "the ANALYZE never showed in pg_stat_user_tables"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO {schema}.ev SELECT generate_series(1, 10)"
        )))
        .execute(&mut *conn)
        .await
        .unwrap();
        assert!(
            !super::analyze_if_stale(&mut conn, &schema, "ev", false)
                .await
                .unwrap(),
            "10 new rows in 5,010: the steady state keeps autovacuum's statistics"
        );
        assert!(
            super::analyze_if_stale(&mut conn, &schema, "ev", true)
                .await
                .unwrap(),
            "a rebuild always analyzes"
        );
        drop(conn);
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await
            .unwrap();
    }
}
