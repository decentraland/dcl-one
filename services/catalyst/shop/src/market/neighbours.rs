//! Item co-ownership neighbours for `/v3/catalog/suggested`, derived from the
//! nft index: for every item held with others, its top [`NEIGHBORS_PER_ITEM`]
//! candidate items by a cosine over the wallets currently holding both,
//! shrunk by the co-owner count (`sim * co / (co + 10)`), with the hoarder
//! damping `|items(wallet)|^(-1/4)` and wallets outside [2, 500] distinct
//! items left out. Anchors are every item; neighbours only approved,
//! non-social ones.
//!
//! The source differs from the old job's: it read every *paid acquisition*
//! (squid `mint` with a primary sale price, and `sale`), which the transfer
//! log does not carry; this reads *current holdings* from `shop.nft`
//! (a v2 token by its item id, a v1 token by its collection URN and
//! wearable id). The band still drops the airdrop farms the paid filter was
//! for. The table is rebuilt whole, in one transaction, every
//! `MARKETPLACE_NEIGHBOURS_INTERVAL_SECS` once the nft index is published on
//! a network, and `item_neighbors_meta` names the folds it was built from.
//! Until a build exists the rail answers the trending fallback
//! (`personalized: false`), as the old rail did for a wallet it knew nothing
//! about.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

use super::MarketplaceState;
use super::chain_walk::{ChainNet, INDEXER_NFT, ready_cursor};
use crate::jobs::{Jobs, PeriodicCfg};

pub const ALGORITHM: &str = "co-ownership-holdings-v1";
pub const NEIGHBORS_PER_ITEM: usize = 50;
pub const MIN_CO_OWNERS: u32 = 3;
pub const CO_OWNERSHIP_SHRINKAGE: f64 = 10.0;
pub const MIN_WALLET_ITEMS: usize = 2;
pub const MAX_WALLET_ITEMS: usize = 500;
/// Per-wallet scratch space in the accumulation loop, independent of the band.
const MAX_WALLET_ITEMS_BUFFER: usize = 512;
pub const DEFAULT_BLOCK_WIDTH: usize = 128;
/// Any constant; the same in every replica (the old job's key).
const REBUILD_SESSION_LOCK_KEY: i64 = 8_421_312;

/// Acquisitions in compressed row form, one row per wallet: wallet `w` owns
/// `[offsets[w], offsets[w + 1])`; `weights` carries the hoarder damping.
#[derive(Debug, Default)]
pub struct AcquisitionMatrix {
    pub offsets: Vec<i32>,
    pub items: Vec<i32>,
    pub weights: Vec<f32>,
    pub wallet_count: usize,
    pub item_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeighborRow {
    pub item: usize,
    pub neighbor: usize,
    pub sim: f64,
    pub support: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct CoOwnershipOptions {
    pub min_support: u32,
    pub neighbors_per_item: usize,
    pub shrinkage: f64,
    pub block_width: usize,
}

impl Default for CoOwnershipOptions {
    fn default() -> Self {
        Self {
            min_support: MIN_CO_OWNERS,
            neighbors_per_item: NEIGHBORS_PER_ITEM,
            shrinkage: CO_OWNERSHIP_SHRINKAGE,
            block_width: DEFAULT_BLOCK_WIDTH,
        }
    }
}

/// The per-item weight vector's L2 norm over the damped weights.
pub fn compute_norms(matrix: &AcquisitionMatrix) -> Vec<f64> {
    let mut norms = vec![0.0f64; matrix.item_count];
    for (i, item) in matrix.items.iter().enumerate() {
        let weight = matrix.weights[i] as f64;
        norms[*item as usize] += weight * weight;
    }
    for norm in norms.iter_mut() {
        *norm = norm.sqrt();
    }
    norms
}

/// The hoarder damping: a wallet's own dot product is `1 / sqrt(size)`.
pub fn damp(wallet_size: usize) -> f32 {
    (wallet_size.max(1) as f64).powf(-0.25) as f32
}

/// For every anchor item, the top-K candidate items most often held by the
/// same wallets, blocked over neighbour columns so the accumulator stays
/// `item_count * block_width * 8` bytes whatever the catalogue size.
pub fn build_co_ownership_neighbors(
    matrix: &AcquisitionMatrix,
    is_candidate: &[bool],
    options: CoOwnershipOptions,
) -> Vec<NeighborRow> {
    let item_count = matrix.item_count;
    let k = options.neighbors_per_item.max(1);
    let norms = compute_norms(matrix);
    let mut columns: Vec<usize> = Vec::new();
    let mut column_of = vec![-1i32; item_count];
    for item in 0..item_count {
        if is_candidate.get(item).copied().unwrap_or(false) && norms[item] > 0.0 {
            column_of[item] = columns.len() as i32;
            columns.push(item);
        }
    }
    if columns.is_empty() {
        return Vec::new();
    }
    let mut heaps = TopKHeaps::new(item_count, k);
    let width = options.block_width.max(1).min(columns.len());
    let mut dot = vec![0.0f64; item_count * width];
    let mut support = vec![0u32; item_count * width];
    let mut in_block = [0usize; MAX_WALLET_ITEMS_BUFFER];
    let mut in_block_weight = [0.0f64; MAX_WALLET_ITEMS_BUFFER];
    let mut block_start = 0usize;
    while block_start < columns.len() {
        let block_end = (block_start + width).min(columns.len());
        let block_size = block_end - block_start;
        dot.fill(0.0);
        support.fill(0);
        for wallet in 0..matrix.wallet_count {
            let from = matrix.offsets[wallet] as usize;
            let to = matrix.offsets[wallet + 1] as usize;
            let mut hits = 0usize;
            for i in from..to {
                if hits >= in_block.len() {
                    break;
                }
                let column = column_of[matrix.items[i] as usize];
                if column >= block_start as i32 && column < block_end as i32 {
                    in_block[hits] = (column as usize) - block_start;
                    in_block_weight[hits] = matrix.weights[i] as f64;
                    hits += 1;
                }
            }
            if hits == 0 {
                continue;
            }
            for i in from..to {
                let anchor = matrix.items[i] as usize;
                let anchor_weight = matrix.weights[i] as f64;
                let anchor_column = column_of[anchor] - block_start as i32;
                let base = anchor * width;
                for j in 0..hits {
                    let column = in_block[j];
                    if column as i32 == anchor_column {
                        continue;
                    }
                    dot[base + column] += anchor_weight * in_block_weight[j];
                    support[base + column] += 1;
                }
            }
        }
        for anchor in 0..item_count {
            let anchor_norm = norms[anchor];
            if anchor_norm == 0.0 {
                continue;
            }
            let base = anchor * width;
            for column in 0..block_size {
                let co = support[base + column];
                if co < options.min_support {
                    continue;
                }
                let neighbor = columns[block_start + column];
                let denominator = anchor_norm * norms[neighbor];
                if denominator == 0.0 {
                    continue;
                }
                let co_f = co as f64;
                let sim = (dot[base + column] / denominator) * (co_f / (co_f + options.shrinkage));
                if sim > 0.0 {
                    heaps.offer(anchor, neighbor, sim, co);
                }
            }
        }
        block_start += width;
    }
    heaps.drain()
}

/// One bounded min-heap per anchor, flat so there is no per-item allocation.
struct TopKHeaps {
    sims: Vec<f64>,
    neighbors: Vec<usize>,
    supports: Vec<u32>,
    sizes: Vec<usize>,
    item_count: usize,
    k: usize,
}

impl TopKHeaps {
    fn new(item_count: usize, k: usize) -> Self {
        Self {
            sims: vec![0.0; item_count * k],
            neighbors: vec![0; item_count * k],
            supports: vec![0; item_count * k],
            sizes: vec![0; item_count],
            item_count,
            k,
        }
    }

    fn offer(&mut self, anchor: usize, neighbor: usize, sim: f64, support: u32) {
        let base = anchor * self.k;
        let size = self.sizes[anchor];
        if size < self.k {
            self.sims[base + size] = sim;
            self.neighbors[base + size] = neighbor;
            self.supports[base + size] = support;
            self.sizes[anchor] = size + 1;
            if size + 1 == self.k {
                self.heapify(base);
            }
            return;
        }
        if sim <= self.sims[base] {
            return;
        }
        self.sims[base] = sim;
        self.neighbors[base] = neighbor;
        self.supports[base] = support;
        self.sift_down(base, 0, self.k);
    }

    fn drain(self) -> Vec<NeighborRow> {
        let mut rows: Vec<NeighborRow> = Vec::new();
        for anchor in 0..self.item_count {
            let size = self.sizes[anchor];
            if size == 0 {
                continue;
            }
            let base = anchor * self.k;
            let mut slice: Vec<NeighborRow> = (0..size)
                .map(|i| NeighborRow {
                    item: anchor,
                    neighbor: self.neighbors[base + i],
                    sim: self.sims[base + i],
                    support: self.supports[base + i],
                })
                .collect();
            slice.sort_by(|a, b| {
                b.sim
                    .partial_cmp(&a.sim)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.neighbor.cmp(&b.neighbor))
            });
            rows.extend(slice);
        }
        rows
    }

    fn heapify(&mut self, base: usize) {
        for i in (0..self.k / 2).rev() {
            self.sift_down(base, i, self.k);
        }
    }

    fn sift_down(&mut self, base: usize, start: usize, size: usize) {
        let mut root = start;
        loop {
            let left = 2 * root + 1;
            if left >= size {
                break;
            }
            let mut smallest = left;
            let right = left + 1;
            if right < size && self.sims[base + right] < self.sims[base + left] {
                smallest = right;
            }
            if self.sims[base + smallest] >= self.sims[base + root] {
                break;
            }
            self.sims.swap(base + root, base + smallest);
            self.neighbors.swap(base + root, base + smallest);
            self.supports.swap(base + root, base + smallest);
            root = smallest;
        }
    }
}

/// An item's identity: `(network, contract, item_id)`, the items mirror's key.
pub type ItemKey = (String, String, String);

/// Every (wallet, item) currently held, one row each, for the networks
/// given, in wallet order, wallets outside the band dropped in SQL.
const HOLDINGS_SQL: &str = "
WITH held AS (
    SELECT n.owner AS wallet, n.network, n.contract_address, n.item_id
    FROM shop.nft n
    WHERE n.network = 'matic' AND 'matic' = ANY($1) AND n.owner IS NOT NULL AND n.item_id IS NOT NULL
  UNION
    SELECT n.owner, i.network, i.contract_address, i.item_id
    FROM shop.nft n
    JOIN shop.collections c ON c.network = n.network AND c.contract_address = n.contract_address
    JOIN shop.items i ON i.urn = c.urn || ':' || n.wearable_id
    WHERE n.network = 'ethereum' AND 'ethereum' = ANY($1) AND n.owner IS NOT NULL AND n.wearable_id IS NOT NULL
),
band AS (
    SELECT wallet FROM held GROUP BY wallet HAVING count(*) BETWEEN $2 AND $3
)
SELECT h.wallet, h.network, h.contract_address, h.item_id::text AS item_id
FROM held h JOIN band b ON b.wallet = h.wallet
ORDER BY h.wallet";

/// Folds wallet-ordered holdings into the compressed row matrix as they
/// arrive: one `i32` per (wallet, item), each item key interned once, the
/// wallet compared in place, so the (wallet, item) rows never exist as row
/// objects at the same time (the old loader's streaming contract; the
/// matrix is kept whole because the accumulator makes a pass per block).
#[derive(Debug, Default)]
pub struct MatrixBuilder {
    offsets: Vec<i32>,
    items: Vec<i32>,
    index: HashMap<String, usize>,
    keys: Vec<ItemKey>,
    wallet: Option<String>,
    scratch: String,
}

impl MatrixBuilder {
    pub fn new() -> Self {
        Self {
            offsets: vec![0],
            ..Default::default()
        }
    }

    pub fn push(&mut self, wallet: &str, network: &str, contract: &str, item: &str) {
        if self.wallet.as_deref() != Some(wallet) {
            if self.wallet.is_some() {
                self.offsets.push(self.items.len() as i32);
            }
            self.wallet = Some(wallet.to_string());
        }
        self.scratch.clear();
        for (i, part) in [network, contract, item].into_iter().enumerate() {
            if i > 0 {
                self.scratch.push('\0');
            }
            self.scratch.push_str(part);
        }
        let id = match self.index.get(self.scratch.as_str()) {
            Some(id) => *id,
            None => {
                let id = self.keys.len();
                self.index.insert(self.scratch.clone(), id);
                self.keys
                    .push((network.to_string(), contract.to_string(), item.to_string()));
                id
            }
        };
        self.items.push(id as i32);
    }

    pub fn pairs(&self) -> usize {
        self.items.len()
    }

    /// The matrix (weights from each wallet's size) and the item keys by id.
    pub fn finish(mut self) -> (AcquisitionMatrix, Vec<ItemKey>) {
        if self.wallet.is_some() {
            self.offsets.push(self.items.len() as i32);
        }
        let wallet_count = self.offsets.len() - 1;
        let mut weights = vec![0.0f32; self.items.len()];
        for wallet in 0..wallet_count {
            let (from, to) = (
                self.offsets[wallet] as usize,
                self.offsets[wallet + 1] as usize,
            );
            weights[from..to].fill(damp(to - from));
        }
        let item_count = self.keys.len();
        (
            AcquisitionMatrix {
                offsets: self.offsets,
                items: self.items,
                weights,
                wallet_count,
                item_count,
            },
            self.keys,
        )
    }
}

/// The matrix of `rows` (wallet-ordered): [`MatrixBuilder`] over a slice.
pub fn build_matrix(rows: &[(String, ItemKey)]) -> (AcquisitionMatrix, Vec<ItemKey>) {
    let mut builder = MatrixBuilder::new();
    for (wallet, (network, contract, item)) in rows {
        builder.push(wallet, network, contract, item);
    }
    builder.finish()
}

/// The longest the holdings scan may stream before the rebuild gives up (the
/// old loader's scan deadline; the statement's own timeout bounds the SQL).
pub const HOLDINGS_SCAN_DEADLINE: Duration = Duration::from_secs(900);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RebuildOutcome {
    pub skipped: Option<String>,
    pub wallets: usize,
    pub items: usize,
    pub rows: usize,
}

/// One rebuild under the session advisory lock (replicas take turns): read
/// the holdings of every published network, compute off the runtime, swap
/// the table whole in one transaction with its meta row.
pub async fn rebuild(pool: &PgPool) -> anyhow::Result<RebuildOutcome> {
    let now = Utc::now();
    let mut networks: Vec<String> = Vec::new();
    let mut folded: HashMap<ChainNet, i64> = HashMap::new();
    for net in ChainNet::ALL {
        if let Some(c) = ready_cursor(pool, INDEXER_NFT, net, now).await {
            networks.push(net.db_name().to_string());
            folded.insert(net, c.last_block);
        }
    }
    rebuild_from(
        pool,
        &networks,
        folded.get(&ChainNet::Matic).copied(),
        folded.get(&ChainNet::Ethereum).copied(),
    )
    .await
}

/// [`rebuild`] over the given networks (the tests pass them directly).
pub async fn rebuild_from(
    pool: &PgPool,
    networks: &[String],
    matic_block: Option<i64>,
    ethereum_block: Option<i64>,
) -> anyhow::Result<RebuildOutcome> {
    if networks.is_empty() {
        return Ok(RebuildOutcome {
            skipped: Some("no network's nft index is published".into()),
            ..Default::default()
        });
    }
    let mut guard = pool.acquire().await?;
    let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(REBUILD_SESSION_LOCK_KEY)
        .fetch_one(&mut *guard)
        .await?;
    if !held {
        return Ok(RebuildOutcome {
            skipped: Some("another replica is rebuilding".into()),
            ..Default::default()
        });
    }
    let outcome = rebuild_locked(pool, networks, matic_block, ethereum_block).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(REBUILD_SESSION_LOCK_KEY)
        .execute(&mut *guard)
        .await;
    outcome
}

async fn rebuild_locked(
    pool: &PgPool,
    networks: &[String],
    matic_block: Option<i64>,
    ethereum_block: Option<i64>,
) -> anyhow::Result<RebuildOutcome> {
    let builder = {
        use futures::TryStreamExt;
        let started = std::time::Instant::now();
        let mut builder = MatrixBuilder::new();
        let mut tx = pool.begin().await?;
        sqlx::query("SET LOCAL statement_timeout = '900s'")
            .execute(&mut *tx)
            .await?;
        {
            let mut stream = sqlx::query(HOLDINGS_SQL)
                .bind(networks)
                .bind(MIN_WALLET_ITEMS as i64)
                .bind(MAX_WALLET_ITEMS as i64)
                .fetch(&mut *tx);
            while let Some(r) = stream.try_next().await? {
                builder.push(
                    r.try_get::<&str, _>("wallet")?,
                    r.try_get::<&str, _>("network")?,
                    r.try_get::<&str, _>("contract_address")?,
                    r.try_get::<&str, _>("item_id")?,
                );
                if builder.pairs().is_multiple_of(65_536)
                    && started.elapsed() > HOLDINGS_SCAN_DEADLINE
                {
                    anyhow::bail!(
                        "the holdings scan passed {} s after {} rows",
                        HOLDINGS_SCAN_DEADLINE.as_secs(),
                        builder.pairs()
                    );
                }
            }
        }
        tx.commit().await?;
        builder
    };
    let candidates: std::collections::HashSet<ItemKey> = sqlx::query("SELECT network, contract_address, item_id::text AS item_id FROM shop.items WHERE search_is_collection_approved AND emote_outcome_type IS NULL")
        .fetch_all(pool)
        .await?
        .iter()
        .map(|r| (r.get("network"), r.get("contract_address"), r.get("item_id")))
        .collect();
    let (neighbours, keys, wallets) = tokio::task::spawn_blocking(move || {
        let (matrix, keys) = builder.finish();
        let is_candidate: Vec<bool> = keys.iter().map(|k| candidates.contains(k)).collect();
        (
            build_co_ownership_neighbors(&matrix, &is_candidate, CoOwnershipOptions::default()),
            keys,
            matrix.wallet_count,
        )
    })
    .await?;
    type Columns = (
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<f32>,
        Vec<i32>,
        Vec<i16>,
    );
    let mut cols: Columns = Default::default();
    let mut rank: HashMap<usize, i16> = HashMap::new();
    for row in &neighbours {
        let r = rank.entry(row.item).or_insert(0);
        *r += 1;
        let (a, b) = (&keys[row.item], &keys[row.neighbor]);
        cols.0.push(a.0.clone());
        cols.1.push(a.1.clone());
        cols.2.push(a.2.clone());
        cols.3.push(b.0.clone());
        cols.4.push(b.1.clone());
        cols.5.push(b.2.clone());
        cols.6.push((row.sim.min(1.0)) as f32);
        cols.7.push(row.support as i32);
        cols.8.push(*r);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '900s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM shop.item_neighbors")
        .execute(&mut *tx)
        .await?;
    for start in (0..cols.0.len()).step_by(5_000) {
        let end = (start + 5_000).min(cols.0.len());
        sqlx::query(
            "INSERT INTO shop.item_neighbors (network, contract_address, item_id, neighbor_network, neighbor_contract_address, neighbor_item_id, sim, support, rank) \
             SELECT a, b, c::numeric, d, e, f::numeric, g, h, i FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[], $7::real[], $8::int[], $9::smallint[]) AS u(a, b, c, d, e, f, g, h, i)",
        )
        .bind(&cols.0[start..end])
        .bind(&cols.1[start..end])
        .bind(&cols.2[start..end])
        .bind(&cols.3[start..end])
        .bind(&cols.4[start..end])
        .bind(&cols.5[start..end])
        .bind(&cols.6[start..end])
        .bind(&cols.7[start..end])
        .bind(&cols.8[start..end])
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO shop.item_neighbors_meta (id, built_at, algorithm, wallets, items, rows_written, source_folded_block_matic, source_folded_block_ethereum) \
         VALUES (1, timezone('UTC', now()), $1, $2, $3, $4, $5, $6) \
         ON CONFLICT (id) DO UPDATE SET built_at = EXCLUDED.built_at, algorithm = EXCLUDED.algorithm, wallets = EXCLUDED.wallets, items = EXCLUDED.items, \
             rows_written = EXCLUDED.rows_written, source_folded_block_matic = EXCLUDED.source_folded_block_matic, source_folded_block_ethereum = EXCLUDED.source_folded_block_ethereum",
    )
    .bind(ALGORITHM)
    .bind(wallets as i32)
    .bind(keys.len() as i32)
    .bind(neighbours.len() as i32)
    .bind(matic_block)
    .bind(ethereum_block)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(RebuildOutcome {
        skipped: None,
        wallets,
        items: keys.len(),
        rows: neighbours.len(),
    })
}

/// Attaches `item-neighbours` with the nft index: a first run 5 min after
/// start, then every `MARKETPLACE_NEIGHBOURS_INTERVAL_SECS`.
pub fn spawn_job(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    if !state.settings.market.indexes.nft_enabled {
        return;
    }
    let pool = state.pool.clone();
    let mut cfg = PeriodicCfg::every(
        state
            .settings
            .market
            .indexes
            .neighbours_interval
            .max(Duration::from_secs(600)),
    );
    cfg = cfg
        .with_jitter(Duration::from_secs(300))
        .with_error_backoff(Duration::from_secs(6 * 3600));
    jobs.spawn_periodic_leased("item-neighbours", cfg, move |ctx| {
        let pool = pool.clone();
        async move {
            let outcome = rebuild(&pool).await?;
            ctx.progress(match &outcome.skipped {
                Some(why) => format!("skipped: {why}"),
                None => format!(
                    "{} neighbour rows over {} items from {} wallets",
                    outcome.rows, outcome.items, outcome.wallets
                ),
            });
            Ok(())
        }
    });
}

/// The meta row, for `/v1/indexes`.
pub async fn status(pool: &PgPool) -> Value {
    match sqlx::query("SELECT built_at, algorithm, wallets, items, rows_written FROM shop.item_neighbors_meta WHERE id = 1").fetch_optional(pool).await {
        Ok(Some(r)) => json!({
            "builtAt": r.get::<chrono::NaiveDateTime, _>("built_at").and_utc().timestamp_millis(),
            "algorithm": r.get::<String, _>("algorithm"),
            "wallets": r.get::<i32, _>("wallets"), "items": r.get::<i32, _>("items"), "rows": r.get::<i32, _>("rows_written"),
        }),
        _ => Value::Null,
    }
}

/// A scored neighbour of the request's seeds: its wire id, the summed
/// similarity and the seed that contributed most.
#[derive(Clone, Debug, PartialEq)]
pub struct Scored {
    pub id: String,
    pub score: f64,
    pub seed: String,
}

/// The neighbours of `seeds` (wire `contract-itemId` ids on their network),
/// minus `exclude`, strongest first, at most `limit`. Empty when no build
/// exists.
pub async fn neighbours_of(
    pool: &PgPool,
    seeds: &[(String, String, String)],
    exclude: &[String],
    limit: i64,
) -> Result<Vec<Scored>, sqlx::Error> {
    let weighted: Vec<(String, String, String, f64)> = seeds
        .iter()
        .cloned()
        .map(|(n, c, i)| (n, c, i, 1.0))
        .collect();
    weighted_neighbours_of(pool, &weighted, exclude, limit).await
}

/// [`neighbours_of`] with a weight per seed (how much that signal is worth:
/// what the avatar wears outranks a holding): a neighbour scores the sum of
/// its similarities times its seeds' weights, and its seed is the one that
/// contributed most.
pub async fn weighted_neighbours_of(
    pool: &PgPool,
    seeds: &[(String, String, String, f64)],
    exclude: &[String],
    limit: i64,
) -> Result<Vec<Scored>, sqlx::Error> {
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut nets = Vec::with_capacity(seeds.len());
    let mut contracts = Vec::with_capacity(seeds.len());
    let mut items = Vec::with_capacity(seeds.len());
    let mut weights = Vec::with_capacity(seeds.len());
    for (n, c, i, w) in seeds {
        nets.push(n.clone());
        contracts.push(c.clone());
        items.push(i.clone());
        weights.push(*w);
    }
    let rows = sqlx::query(
        "WITH seed AS (SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $6::float8[]) AS s(network, contract_address, item_id, weight)) \
         SELECT nb.neighbor_contract_address || '-' || nb.neighbor_item_id::text AS id, sum(nb.sim * s.weight)::float8 AS score, \
                (array_agg(nb.contract_address || '-' || nb.item_id::text ORDER BY nb.sim * s.weight DESC))[1] AS seed \
         FROM seed s JOIN shop.item_neighbors nb \
           ON nb.network = s.network AND nb.contract_address = s.contract_address AND nb.item_id = s.item_id::numeric \
         WHERE NOT (nb.neighbor_contract_address || '-' || nb.neighbor_item_id::text) = ANY($4) \
         GROUP BY 1 ORDER BY 2 DESC, 1 LIMIT $5",
    )
    .bind(&nets)
    .bind(&contracts)
    .bind(&items)
    .bind(exclude)
    .bind(limit)
    .bind(&weights)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| Scored {
            id: r.get("id"),
            score: r.get("score"),
            seed: r.get("seed"),
        })
        .collect())
}

/// A wallet's held items (the seeds its holdings give), most recently
/// received first, at most `limit`.
pub async fn held_items(
    pool: &PgPool,
    wallet: &str,
    limit: i64,
) -> Result<Vec<(String, String, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT network, contract_address, item_id FROM ( \
             SELECT DISTINCT ON (network, contract_address, item_id) network, contract_address, item_id::text AS item_id, transferred_at FROM ( \
                 SELECT n.network, n.contract_address, n.item_id, n.transferred_at FROM shop.nft n \
                 WHERE n.owner = $1 AND n.network = 'matic' AND n.item_id IS NOT NULL \
               UNION ALL \
                 SELECT i.network, i.contract_address, i.item_id, n.transferred_at FROM shop.nft n \
                 JOIN shop.collections c ON c.network = n.network AND c.contract_address = n.contract_address \
                 JOIN shop.items i ON i.urn = c.urn || ':' || n.wearable_id \
                 WHERE n.owner = $1 AND n.network = 'ethereum' AND n.wearable_id IS NOT NULL \
             ) h ORDER BY network, contract_address, item_id, transferred_at DESC \
         ) d ORDER BY transferred_at DESC LIMIT $2",
    )
    .bind(wallet.to_ascii_lowercase())
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get("network"),
                r.get("contract_address"),
                r.get("item_id"),
            )
        })
        .collect())
}

/// Whether a neighbours build exists (the personalised rail's source).
pub async fn built(pool: &PgPool) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM shop.item_neighbors_meta WHERE id = 1)",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wallets as item-index lists, folded with the hoarder damping.
    fn matrix(wallets: &[&[usize]], item_count: usize) -> AcquisitionMatrix {
        let mut offsets = vec![0i32];
        let mut items: Vec<i32> = Vec::new();
        let mut weights: Vec<f32> = Vec::new();
        for wallet in wallets {
            let damping = damp(wallet.len());
            for item in *wallet {
                items.push(*item as i32);
                weights.push(damping);
            }
            offsets.push(items.len() as i32);
        }
        AcquisitionMatrix {
            offsets,
            items,
            weights,
            wallet_count: wallets.len(),
            item_count,
        }
    }

    fn options(min_support: u32) -> CoOwnershipOptions {
        CoOwnershipOptions {
            min_support,
            ..Default::default()
        }
    }

    #[test]
    fn wallets_holding_both_items_push_each_towards_the_other() {
        let m = matrix(&[&[0, 1], &[0, 1], &[0, 1]], 3);
        let rows = build_co_ownership_neighbors(&m, &[true, true, true], options(3));
        assert!(rows.iter().any(|r| r.item == 0 && r.neighbor == 1));
        assert!(rows.iter().any(|r| r.item == 1 && r.neighbor == 0));
        assert!(rows.iter().all(|r| r.item != 2 && r.neighbor != 2));
        for row in &rows {
            assert_eq!(row.support, 3);
            assert!(row.sim > 0.0 && row.sim <= 1.0, "{row:?}");
        }
    }

    #[test]
    fn a_thin_pair_is_dropped_rather_than_ranked() {
        let m = matrix(&[&[0, 1], &[0, 1]], 2);
        assert!(build_co_ownership_neighbors(&m, &[true, true], options(3)).is_empty());
        assert!(!build_co_ownership_neighbors(&m, &[true, true], options(2)).is_empty());
    }

    #[test]
    fn support_shrinks_the_cosine_towards_zero() {
        let thin = matrix(&[&[0, 1], &[0, 1], &[0, 1]], 2);
        let thick = matrix(&vec![&[0usize, 1usize][..]; 60], 2);
        let thin_sim = build_co_ownership_neighbors(&thin, &[true, true], options(3))[0].sim;
        let thick_sim = build_co_ownership_neighbors(&thick, &[true, true], options(3))[0].sim;
        assert!(thick_sim > thin_sim, "{thick_sim} vs {thin_sim}");
    }

    #[test]
    fn a_non_candidate_can_anchor_but_never_be_a_neighbour() {
        let m = matrix(&[&[0, 1], &[0, 1], &[0, 1]], 2);
        let rows = build_co_ownership_neighbors(&m, &[false, true], options(3));
        assert!(rows.iter().any(|r| r.item == 0 && r.neighbor == 1));
        assert!(rows.iter().all(|r| r.neighbor != 0));
    }

    #[test]
    fn blocking_over_columns_does_not_change_the_result() {
        let wallets: Vec<Vec<usize>> = (0..12)
            .map(|w| vec![w % 5, (w + 1) % 5, (w + 2) % 5])
            .collect();
        let borrowed: Vec<&[usize]> = wallets.iter().map(|w| w.as_slice()).collect();
        let m = matrix(&borrowed, 5);
        let candidates = [true; 5];
        let whole = build_co_ownership_neighbors(
            &m,
            &candidates,
            CoOwnershipOptions {
                block_width: 64,
                ..options(2)
            },
        );
        let blocked = build_co_ownership_neighbors(
            &m,
            &candidates,
            CoOwnershipOptions {
                block_width: 1,
                ..options(2)
            },
        );
        assert_eq!(whole.len(), blocked.len());
        for (a, b) in whole.iter().zip(blocked.iter()) {
            assert_eq!((a.item, a.neighbor), (b.item, b.neighbor));
            assert!((a.sim - b.sim).abs() < 1e-12, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn each_anchor_keeps_at_most_k_neighbours_strongest_first() {
        let wallets: Vec<Vec<usize>> = (1..6).map(|item| vec![0, item, item + 5]).collect();
        let borrowed: Vec<&[usize]> = wallets.iter().map(|w| w.as_slice()).collect();
        let m = matrix(&borrowed, 11);
        let rows = build_co_ownership_neighbors(
            &m,
            &[true; 11],
            CoOwnershipOptions {
                neighbors_per_item: 2,
                ..options(1)
            },
        );
        let anchor_rows: Vec<&NeighborRow> = rows.iter().filter(|r| r.item == 0).collect();
        assert_eq!(anchor_rows.len(), 2);
        assert!(anchor_rows[0].sim >= anchor_rows[1].sim);
    }

    #[test]
    fn the_hoarder_damping_makes_a_wallets_own_dot_product_one_over_sqrt_of_its_size() {
        for size in [1usize, 4, 16, 100] {
            let w = damp(size) as f64;
            assert!(
                (w * w * size as f64 - (size as f64).sqrt()).abs() < 1e-3,
                "{size}"
            );
        }
    }

    #[test]
    fn the_matrix_groups_consecutive_wallet_rows() {
        let k = |i: &str| ("matic".to_string(), "0xc".to_string(), i.to_string());
        let rows = vec![
            ("a".to_string(), k("1")),
            ("a".to_string(), k("2")),
            ("b".to_string(), k("2")),
        ];
        let (m, keys) = build_matrix(&rows);
        assert_eq!(keys, vec![k("1"), k("2")]);
        assert_eq!(
            (m.wallet_count, m.item_count, m.offsets.clone()),
            (2, 2, vec![0, 2, 3])
        );
        assert_eq!(m.items, vec![0, 1, 1]);
        assert!((m.weights[0] - damp(2)).abs() < 1e-6 && (m.weights[2] - 1.0).abs() < 1e-6);
    }

    /// Not a contract: `CATALYST_NEIGHBOURS_BENCH_DATABASE_URL` names a
    /// disposable database whose `shop.nft` holds the holdings to
    /// rebuild over (METRICS.md, "Item neighbours rebuild"). Prints the wall
    /// time and the process's peak RSS.
    #[tokio::test]
    #[ignore]
    async fn bench_the_rebuild_over_a_populated_nft_table() {
        let Ok(url) = std::env::var("CATALYST_NEIGHBOURS_BENCH_DATABASE_URL") else {
            return;
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let hwm = || {
            std::fs::read_to_string("/proc/self/status")
                .unwrap()
                .lines()
                .find(|l| l.starts_with("VmHWM"))
                .unwrap_or("")
                .to_string()
        };
        println!("before: {}", hwm());
        let started = std::time::Instant::now();
        let out = super::rebuild_from(&pool, &["matic".to_string()], Some(1), None)
            .await
            .unwrap();
        println!(
            "rebuild: {:?} in {:.1} s; after: {}",
            out,
            started.elapsed().as_secs_f64(),
            hwm()
        );
    }
}
