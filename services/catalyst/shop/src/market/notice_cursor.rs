//! What the chain notification producers share: their names (the rows of
//! `shop.notification_cursor`), the cursor helpers, the replay that moves
//! the cursors back over rows an index writes again (a reorg rewind, a late
//! nft backfill), the operator rewind ([`rewind`]: the cutover seed from a
//! legacy cursor and any later move back), and the reorg-stable notification ids.
//!
//! An id keys on the log's transaction (`tx_hash`, the log's place among the
//! table's logs of that transaction, and its content), or, for `nft_event`,
//! which keeps no hash, on its token, its content and how many identical
//! logs of the token precede it. The same log mined again in another block
//! keeps its id; a different log at the same (block, log_index) gets another.

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::chain_walk::{ChainNet, INDEXER_NFT, INDEXER_SALES, INDEXER_TRADE_EVENTS};
use crate::rentals::index::INDEXER_RENTALS;

pub const TRANSFER_RECEIVED: &str = "transfer_received";
pub const ROYALTIES_EARNED: &str = "royalties_earned";
pub const RENTAL_STARTED: &str = "rental_started";
pub const RENTAL_ENDED: &str = "rental_ended";
pub const SALES_TRADED_ETHEREUM: &str = "sales:traded:ethereum";
pub const SALES_TRADED_MATIC: &str = "sales:traded:matic";
pub const SALES_ORDER_SUCCESSFUL_MATIC: &str = "sales:order_successful:matic";
pub const SALES_MINT_MATIC: &str = "sales:mint:matic";
pub const SALES_BID_ACCEPTED_MATIC: &str = "sales:bid_accepted:matic";
pub const ROYALTIES_BID_ACCEPTED_MATIC: &str = "royalties:bid_accepted:matic";
pub const BID_RECEIVED: &str = "bid_received";

/// Every producer; no other name is ever written to `shop.notification_cursor`
/// (its CHECK lists exactly these).
pub const PRODUCERS: &[&str] = &[
    TRANSFER_RECEIVED,
    ROYALTIES_EARNED,
    RENTAL_STARTED,
    RENTAL_ENDED,
    SALES_TRADED_ETHEREUM,
    SALES_TRADED_MATIC,
    SALES_ORDER_SUCCESSFUL_MATIC,
    SALES_MINT_MATIC,
    SALES_BID_ACCEPTED_MATIC,
    ROYALTIES_BID_ACCEPTED_MATIC,
    BID_RECEIVED,
];

/// How far below an index's head a replay moves the cursors back: deeper
/// rows are not announced again, and a backfill entirely below it replays nothing.
pub const REPLAY_BLOCKS: i64 = 1_000;

/// The network `producer` reads and every index it reads there.
fn reads(producer: &str) -> Option<(ChainNet, &'static [&'static str])> {
    Some(match producer {
        TRANSFER_RECEIVED => (ChainNet::Matic, &[INDEXER_NFT, INDEXER_TRADE_EVENTS]),
        BID_RECEIVED => (ChainNet::Matic, &[INDEXER_SALES, INDEXER_NFT]),
        ROYALTIES_EARNED | SALES_ORDER_SUCCESSFUL_MATIC => (ChainNet::Matic, &[INDEXER_NFT]),
        SALES_TRADED_MATIC => (ChainNet::Matic, &[INDEXER_TRADE_EVENTS]),
        SALES_MINT_MATIC | SALES_BID_ACCEPTED_MATIC | ROYALTIES_BID_ACCEPTED_MATIC => {
            (ChainNet::Matic, &[INDEXER_SALES])
        }
        SALES_TRADED_ETHEREUM => (ChainNet::Ethereum, &[INDEXER_TRADE_EVENTS]),
        RENTAL_STARTED | RENTAL_ENDED => (ChainNet::Ethereum, &[INDEXER_RENTALS]),
        _ => return None,
    })
}

/// The producers reading `indexer` on `net`.
pub fn producers_of(indexer: &str, net: ChainNet) -> Vec<&'static str> {
    PRODUCERS
        .iter()
        .copied()
        .filter(|p| reads(p).is_some_and(|(n, idx)| n == net && idx.contains(&indexer)))
        .collect()
}

fn known(producer: &str) -> sqlx::Result<()> {
    if PRODUCERS.contains(&producer) {
        Ok(())
    } else {
        Err(sqlx::Error::InvalidArgument(format!(
            "unknown notification producer {producer}"
        )))
    }
}

/// Seeds the producer's row (and its seed point, the floor of any replay)
/// once; a row [`rewind`] seeded first is kept.
pub(crate) async fn seed(
    pool: &PgPool,
    producer: &'static str,
    end_time: Option<i64>,
    block_number: i64,
    log_index: i32,
) -> sqlx::Result<()> {
    known(producer)?;
    sqlx::query(
        "INSERT INTO shop.notification_cursor (producer, end_time, block_number, log_index, seed_end_time, seed_block, seed_log_index) \
         VALUES ($1, $2, $3, $4, $2, $3, $4) ON CONFLICT (producer) DO NOTHING",
    )
    .bind(producer)
    .bind(end_time)
    .bind(block_number)
    .bind(log_index)
    .execute(pool)
    .await?;
    Ok(())
}

/// The producer's `end_time`, `block_number` and `log_index`, locked for the page.
pub(crate) async fn lock(
    conn: &mut PgConnection,
    producer: &'static str,
) -> sqlx::Result<(Option<i64>, i64, i32)> {
    known(producer)?;
    sqlx::query_as("SELECT end_time, block_number, log_index FROM shop.notification_cursor WHERE producer = $1 FOR UPDATE")
        .bind(producer)
        .fetch_one(conn)
        .await
}

pub(crate) async fn advance(
    conn: &mut PgConnection,
    producer: &'static str,
    end_time: Option<i64>,
    block_number: i64,
    log_index: i32,
) -> sqlx::Result<()> {
    known(producer)?;
    sqlx::query(
        "UPDATE shop.notification_cursor SET end_time = $2, block_number = $3, log_index = $4, \
         updated_at = timezone('UTC', now()) WHERE producer = $1",
    )
    .bind(producer)
    .bind(end_time)
    .bind(block_number)
    .bind(log_index)
    .execute(conn)
    .await?;
    Ok(())
}

/// Where an operator moves a producer's cursor back to: the first chain
/// position announced again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewindFrom {
    /// The logs of this block and later.
    Block(i64),
    /// The logs mined at or after this unix second; for `rental_ended`, the
    /// periods ending at or after it.
    Time(i64),
}

/// A `shop.notification_cursor` row: its cursor and its seed (the replay floor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, sqlx::FromRow)]
pub struct CursorRow {
    pub end_time: Option<i64>,
    pub block_number: i64,
    pub log_index: i32,
    pub seed_end_time: Option<i64>,
    pub seed_block: i64,
    pub seed_log_index: i32,
}

impl CursorRow {
    fn at(end_time: Option<i64>, block_number: i64, log_index: i32) -> Self {
        Self {
            end_time,
            block_number,
            log_index,
            seed_end_time: end_time,
            seed_block: block_number,
            seed_log_index: log_index,
        }
    }

    fn cursor(&self) -> (Option<i64>, i64, i32) {
        (self.end_time, self.block_number, self.log_index)
    }

    fn seed(&self) -> (Option<i64>, i64, i32) {
        (self.seed_end_time, self.seed_block, self.seed_log_index)
    }
}

/// What an index the producer reads holds: the time resolution reads only
/// its rows, so logs below `start_block` (when not `full_history`) cannot
/// be announced again.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, sqlx::FromRow)]
pub struct IndexCoverage {
    pub indexer: String,
    pub start_block: i64,
    pub full_history: bool,
    pub last_block: i64,
    pub last_block_time: Option<i64>,
    /// The chain its history was proven on; `None` while unbound.
    pub chain_id: Option<i64>,
    /// Its history is quarantined: not proven on the expected chain.
    pub quarantined: bool,
    /// A reorg left its checkpoint off the chain and the recovery still
    /// searches for the fork: its rows above the fork are not canonical.
    pub diverged: bool,
    /// On a dry run, how many of this index's logs lie after the resulting
    /// cursor (`after`), up to its `last_block`: an upper bound on the logs
    /// the next passes read (each producer reads only some kinds). Absent
    /// otherwise, and for `rental_ended`, which reads rental periods by end
    /// time. Counted after the transaction ends, holding no cursor row.
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs_after: Option<i64>,
}

/// What [`rewind`] did (or, on a dry run, would do).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Rewound {
    pub producer: &'static str,
    pub network: &'static str,
    /// The cursor the request resolves to: everything after it is announced.
    pub target_end_time: Option<i64>,
    pub target_block: i64,
    pub target_log_index: i32,
    /// The row before, `None` when no pass had seeded it yet.
    pub before: Option<CursorRow>,
    pub after: CursorRow,
    pub changed: bool,
    pub dry_run: bool,
    /// The indexes the producer reads that have a cursor row.
    pub indexes: Vec<IndexCoverage>,
}

#[derive(Debug, thiserror::Error)]
pub enum RewindError {
    #[error("unknown notification producer {0:?}; the producers are {PRODUCERS:?}")]
    UnknownProducer(String),
    #[error("{0} is ordered by rental end time: give a date (--since), not a block")]
    NeedsDate(&'static str),
    #[error(
        "block {0} is not a chain block: give 1 or more (block 0 or less would announce the whole index again)"
    )]
    BlockOutOfRange(i64),
    #[error(
        "time {0} is not after 1970-01-01T00:00:00Z: give a later date (an earlier one would announce the whole index again)"
    )]
    TimeOutOfRange(i64),
    #[error(
        "the {network} {indexer} index has not indexed {since} yet (its last block time is {last_block_time:?}); \
         run again once it has{}", if *.by_end_time { "" } else { ", or give a block" }
    )]
    NotReached {
        network: &'static str,
        indexer: String,
        since: i64,
        last_block_time: Option<i64>,
        /// The producer orders by end time and takes no block.
        by_end_time: bool,
    },
    #[error(
        "the {network} {indexer} index's history is not proven on chain {expected} ({why}); \
         a date resolves only through a proven history: revalidate the index, or give a block"
    )]
    Unproven {
        network: &'static str,
        indexer: String,
        expected: i64,
        why: String,
    },
    #[error("the {network} {indexer} index has no cursor row; nothing is indexed yet")]
    NoIndex {
        network: &'static str,
        indexer: &'static str,
    },
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// The log table of a chain index, with the column holding a log's block
/// time; `None` for an index no notice producer reads.
fn event_table(indexer: &str) -> Option<&'static str> {
    Some(match indexer {
        INDEXER_NFT => "shop.nft_event",
        INDEXER_TRADE_EVENTS => "shop.trade_event",
        INDEXER_SALES => "shop.sale_event",
        INDEXER_RENTALS => "shop.rental_event",
        _ => return None,
    })
}

/// The chain `indexer` must have been proven on for `producer` to read it
/// on `net`: the trade-events walk reads the chain the node configures
/// (`trade_chains`, `(ethereum, matic)`, `MARKETPLACE_INDEX_CHAIN_ID_*`),
/// except for `transfer_received`, which matches it against the nft index
/// and so runs on Polygon mainnet only; every other index reads its
/// network's mainnet.
fn read_chain(producer: &str, indexer: &str, net: ChainNet, trade_chains: (i64, i64)) -> i64 {
    match (indexer, net) {
        (INDEXER_TRADE_EVENTS, _) if producer == TRANSFER_RECEIVED => net.chain_id(),
        (INDEXER_TRADE_EVENTS, ChainNet::Ethereum) => trade_chains.0,
        (INDEXER_TRADE_EVENTS, ChainNet::Matic) => trade_chains.1,
        _ => net.chain_id(),
    }
}

/// Why `index` cannot date a rewind for a producer reading it on `expected`.
fn unproven(index: &IndexCoverage, expected: i64) -> Option<String> {
    if index.quarantined {
        Some("quarantined".into())
    } else if index.diverged {
        Some("diverged, its reorg recovery still searching".into())
    } else {
        match index.chain_id {
            Some(c) if c == expected => None,
            Some(c) => Some(format!("proven on chain {c}")),
            None => Some("unbound".into()),
        }
    }
}

/// Whether the producer's cursor orders by an end time before the chain position.
fn ends_by_time(producer: &str) -> bool {
    producer == RENTAL_ENDED
}

/// Parses an operator's `--since`: an RFC 3339 instant, a `YYYY-MM-DD` day
/// (its UTC midnight), or epoch milliseconds as a legacy cursor stores them.
/// Answers unix seconds. A bare number below 10^12 is refused: it is not a
/// millisecond time (seconds, or a legacy cursor holding a sale id). A time
/// at or before 1970-01-01T00:00:00Z is refused: it would announce every
/// indexed log again. An RFC 3339 time needs its zone (`Z` or an offset).
pub fn parse_since(s: &str) -> Result<i64, String> {
    let s = s.trim();
    let t = if let Ok(ms) = s.parse::<i64>() {
        if ms < 1_000_000_000_000 {
            return Err(format!(
                "{s} is not epoch milliseconds (10^12 or more); give a date such as 2026-11-01T00:00:00Z"
            ));
        }
        ms.div_euclid(1_000)
    } else if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s) {
        t.timestamp()
    } else if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        d.and_time(chrono::NaiveTime::MIN).and_utc().timestamp()
    } else {
        return Err(format!(
            "{s} is not a date: give RFC 3339 with its zone (2026-11-01T00:00:00Z), a day (2026-11-01) or epoch milliseconds"
        ));
    };
    if t <= 0 {
        return Err(format!(
            "{s} is not after 1970-01-01T00:00:00Z; it would announce the whole index again"
        ));
    }
    Ok(t)
}

/// An index's coverage and, for a date, the last block mined before it.
#[derive(sqlx::FromRow)]
struct Covered {
    #[sqlx(flatten)]
    coverage: IndexCoverage,
    before: Option<i64>,
}

/// Moves `producer`'s cursor back to `from`, and its seed with it, so the
/// next passes announce everything after that point again; the reorg-stable
/// ids keep a notice already stored from being stored or emailed twice. A
/// producer no pass has seeded yet is seeded there, and its first pass keeps
/// that seed: this is also the cutover seed from a legacy cursor. Neither
/// the cursor nor the seed ever moves forward. One transaction, holding the
/// cursor row, recorded in `ops.admin_audit` (domain `market`) as `actor`;
/// `dry_run` resolves and reports without writing.
///
/// A block below 1 or a time at or before the epoch is refused: it would
/// announce every indexed log again. A date resolves through the logs of
/// the indexes the producer reads ([`reads`]): the cursor goes after the
/// last block mined before it, and every such index must have a cursor row,
/// a history proven on the chain the producer reads it on ([`read_chain`]:
/// bound to it, not quarantined and not diverged) and already have indexed past it; the
/// coverage and the log positions come from one statement, so one snapshot
/// (`trade_chains` is the node's `(ethereum, matic)` trade-events chain).
/// For `rental_ended` the date is the
/// end-time cursor itself, refused likewise until the rentals index has
/// reached it. A block needs no index; the ones with a cursor row are
/// reported. Cost: a backward walk of each index's log by its primary key
/// from the head down to the date; a dry run also counts each index's logs
/// after the resulting cursor ([`IndexCoverage::logs_after`]), once the
/// cursor row is released.
pub async fn rewind(
    pool: &PgPool,
    producer: &str,
    from: RewindFrom,
    trade_chains: (i64, i64),
    actor: &str,
    dry_run: bool,
) -> Result<Rewound, RewindError> {
    let producer = PRODUCERS
        .iter()
        .copied()
        .find(|p| *p == producer)
        .ok_or_else(|| RewindError::UnknownProducer(producer.to_string()))?;
    let (net, indexers) = reads(producer).ok_or(RewindError::UnknownProducer(producer.into()))?;
    let network = net.db_name();
    match from {
        RewindFrom::Block(_) if ends_by_time(producer) => {
            return Err(RewindError::NeedsDate(producer));
        }
        RewindFrom::Block(b) if b < 1 => return Err(RewindError::BlockOutOfRange(b)),
        RewindFrom::Time(t) if t <= 0 => return Err(RewindError::TimeOutOfRange(t)),
        _ => {}
    }
    let ends = ends_by_time(producer);
    let mut txn = pool.begin().await?;
    // One statement, so one snapshot: each index's coverage and, for a date,
    // the last block mined before it among that index's logs.
    let mut arms = Vec::with_capacity(indexers.len());
    for (i, &indexer) in indexers.iter().enumerate() {
        let before = match from {
            RewindFrom::Time(_) if !ends => {
                let table = event_table(indexer).ok_or_else(|| {
                    sqlx::Error::InvalidArgument(format!(
                        "no log table is known for the {indexer} index"
                    ))
                })?;
                format!(
                    "(SELECT e.block_number FROM {table} e WHERE e.network = $1 AND e.block_time < $2 \
                     ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1)"
                )
            }
            _ => "NULL::bigint".to_string(),
        };
        arms.push(format!(
            "SELECT c.indexer, c.start_block, c.full_history, c.last_block, c.last_block_time, \
             c.chain_id, c.quarantined_at IS NOT NULL AS quarantined, \
             c.diverged_at IS NOT NULL AS diverged, {before} AS before, {i} AS ord \
             FROM shop.chain_cursor c WHERE c.indexer = ${} AND c.network = $1",
            i + 3
        ));
    }
    let sql = format!("{} ORDER BY ord", arms.join(" UNION ALL "));
    let mut query = sqlx::query_as::<_, Covered>(sqlx::AssertSqlSafe(sql))
        .bind(network)
        .bind(match from {
            RewindFrom::Time(t) => t,
            RewindFrom::Block(_) => 0,
        });
    for &indexer in indexers {
        query = query.bind(indexer);
    }
    let covered = query.fetch_all(&mut *txn).await?;
    if let RewindFrom::Time(t) = from {
        // A date counts only through an index proven on the chain the
        // producer reads: another history would place the cursor and its
        // seed (the replay floor) by blocks of the wrong chain.
        for &indexer in indexers {
            let Some(c) = covered.iter().find(|c| c.coverage.indexer == indexer) else {
                return Err(RewindError::NoIndex { network, indexer });
            };
            let expected = read_chain(producer, indexer, net, trade_chains);
            if let Some(why) = unproven(&c.coverage, expected) {
                return Err(RewindError::Unproven {
                    network,
                    indexer: indexer.to_string(),
                    expected,
                    why,
                });
            }
        }
        // Logs (or, for `rental_ended`, period ends) the index has not
        // reached would later be read as already handled.
        if let Some(index) = covered
            .iter()
            .map(|c| &c.coverage)
            .find(|i| i.last_block_time.is_none_or(|lt| lt < t))
        {
            return Err(RewindError::NotReached {
                network,
                indexer: index.indexer.clone(),
                since: t,
                last_block_time: index.last_block_time,
                by_end_time: ends,
            });
        }
    }
    let target = match from {
        RewindFrom::Block(b) => (None, b - 1, i32::MAX),
        RewindFrom::Time(t) if ends => (Some(t), 0, -1),
        RewindFrom::Time(_) => match covered.iter().filter_map(|c| c.before).max() {
            Some(b) => (None, b, i32::MAX),
            None => (None, 0, -1),
        },
    };
    let indexes: Vec<IndexCoverage> = covered.into_iter().map(|c| c.coverage).collect();
    let wanted = CursorRow::at(target.0, target.1, target.2);
    let inserted: Option<CursorRow> = sqlx::query_as(
        "INSERT INTO shop.notification_cursor (producer, end_time, block_number, log_index, seed_end_time, seed_block, seed_log_index) \
         VALUES ($1, $2, $3, $4, $2, $3, $4) ON CONFLICT (producer) DO NOTHING \
         RETURNING end_time, block_number, log_index, seed_end_time, seed_block, seed_log_index",
    )
    .bind(producer)
    .bind(target.0)
    .bind(target.1)
    .bind(target.2)
    .fetch_optional(&mut *txn)
    .await?;
    let (before, after) = match inserted {
        Some(row) => (None, row),
        None => {
            let row: CursorRow = sqlx::query_as(
                "SELECT end_time, block_number, log_index, seed_end_time, seed_block, seed_log_index \
                 FROM shop.notification_cursor WHERE producer = $1 FOR UPDATE",
            )
            .bind(producer)
            .fetch_one(&mut *txn)
            .await?;
            let cursor = row.cursor().min(target);
            let seed = row.seed().min(target);
            let after = CursorRow {
                end_time: cursor.0,
                block_number: cursor.1,
                log_index: cursor.2,
                seed_end_time: seed.0,
                seed_block: seed.1,
                seed_log_index: seed.2,
            };
            if after != row {
                sqlx::query(
                    "UPDATE shop.notification_cursor SET end_time = $2, block_number = $3, log_index = $4, \
                     seed_end_time = $5, seed_block = $6, seed_log_index = $7, \
                     updated_at = timezone('UTC', now()) WHERE producer = $1",
                )
                .bind(producer)
                .bind(after.end_time)
                .bind(after.block_number)
                .bind(after.log_index)
                .bind(after.seed_end_time)
                .bind(after.seed_block)
                .bind(after.seed_log_index)
                .execute(&mut *txn)
                .await?;
            }
            (Some(row), after)
        }
    };
    debug_assert!(before.is_some() || after == wanted);
    let changed = before != Some(after);
    let mut out = Rewound {
        producer,
        network,
        target_end_time: target.0,
        target_block: target.1,
        target_log_index: target.2,
        before,
        after,
        changed,
        dry_run,
        indexes,
    };
    if dry_run {
        txn.rollback().await?;
        // Counted after the rollback, so the walk holds no cursor row.
        if after.end_time.is_none() {
            for index in &mut out.indexes {
                let Some(table) = event_table(&index.indexer) else {
                    continue;
                };
                index.logs_after = Some(
                    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                        "SELECT count(*) FROM {table} WHERE network = $1 \
                         AND (block_number, log_index) > ($2, $3) AND block_number <= $4"
                    )))
                    .bind(network)
                    .bind(after.block_number)
                    .bind(after.log_index)
                    .bind(index.last_block)
                    .fetch_one(pool)
                    .await?,
                );
            }
        }
        return Ok(out);
    }
    if changed {
        let from = match from {
            RewindFrom::Block(b) => serde_json::json!({ "block": b }),
            RewindFrom::Time(t) => serde_json::json!({ "since": t }),
        };
        crate::shared::audit::record(
            &mut *txn,
            &crate::shared::audit::Entry::new(actor, "market", "notice-cursor.rewind")
                .target(Some(producer))
                .detail(serde_json::json!({
                    "from": from,
                    "before": out.before,
                    "after": out.after,
                })),
        )
        .await?;
    }
    txn.commit().await?;
    Ok(out)
}

/// A cursor move-back worked out before the index's rows change.
#[derive(Debug, PartialEq, Eq)]
pub struct Replay {
    /// Each producer and the block it moves back to just after.
    moves: Vec<(&'static str, i64)>,
    /// `rental_ended`'s end-time floor: the earliest rent it may have missed.
    rental_since: Option<i64>,
}

/// Plans moving the cursors of the producers reading `indexer` on `net`
/// back to just after `from_block`, for rows written again up to
/// `max_block`. Each producer is clamped by the lowest head of the indexes
/// it reads ([`reads`]), which its cursor never passes: nothing when
/// `max_block` is [`REPLAY_BLOCKS`] or more below it, never further back than that.
/// Reads only, so it runs before the rows are deleted without holding the
/// cursor rows.
pub async fn plan_replay(
    conn: &mut PgConnection,
    indexer: &str,
    net: ChainNet,
    from_block: i64,
    max_block: i64,
) -> sqlx::Result<Option<Replay>> {
    let mut moves = Vec::new();
    for producer in producers_of(indexer, net) {
        let indexes = reads(producer).map_or(&[][..], |(_, idx)| idx);
        let head: Option<i64> = sqlx::query_scalar(
            "SELECT min(last_block) FROM shop.chain_cursor WHERE indexer = ANY($1) AND network = $2",
        )
        .bind(indexes)
        .bind(net.db_name())
        .fetch_one(&mut *conn)
        .await?;
        match head {
            Some(head) if max_block > head - REPLAY_BLOCKS => {
                moves.push((producer, from_block.max(head - REPLAY_BLOCKS)));
            }
            _ => {}
        }
    }
    if moves.is_empty() {
        return Ok(None);
    }
    let rental_since = match moves.iter().find(|(p, _)| *p == RENTAL_ENDED) {
        Some(&(_, from)) => sqlx::query_scalar(
            "SELECT min(block_time) FROM shop.rental_event WHERE network = $1 AND kind = 'rented' AND block_number > $2",
        )
        .bind(net.db_name())
        .bind(from)
        .fetch_one(&mut *conn)
        .await?,
        None => None,
    };
    Ok(Some(Replay {
        moves,
        rental_since,
    }))
}

impl Replay {
    /// Moves the cursors back, never below their seed; the ids keep the
    /// replay from notifying twice. Locks the cursor rows until commit.
    pub async fn apply(self, conn: &mut PgConnection) -> sqlx::Result<()> {
        for (producer, from) in self.moves {
            sqlx::query(
            "UPDATE shop.notification_cursor SET \
                 block_number = CASE WHEN ($2::bigint, 2147483647) > (seed_block, seed_log_index) THEN $2 ELSE seed_block END, \
                 log_index = CASE WHEN ($2::bigint, 2147483647) > (seed_block, seed_log_index) THEN 2147483647 ELSE seed_log_index END, \
                 updated_at = timezone('UTC', now()) \
             WHERE producer = $1 AND end_time IS NULL \
               AND (block_number, log_index) > ($2::bigint, 2147483647) \
               AND (block_number, log_index) > (seed_block, seed_log_index)",
            )
            .bind(producer)
            .bind(from)
            .execute(&mut *conn)
            .await?;
        }
        if let Some(since) = self.rental_since {
            sqlx::query(
                "UPDATE shop.notification_cursor SET \
                     end_time = CASE WHEN ($2::bigint, 0::bigint, -1) > (seed_end_time, seed_block, seed_log_index) THEN $2 ELSE seed_end_time END, \
                     block_number = CASE WHEN ($2::bigint, 0::bigint, -1) > (seed_end_time, seed_block, seed_log_index) THEN 0 ELSE seed_block END, \
                     log_index = CASE WHEN ($2::bigint, 0::bigint, -1) > (seed_end_time, seed_block, seed_log_index) THEN -1 ELSE seed_log_index END, \
                     updated_at = timezone('UTC', now()) \
                 WHERE producer = $1 \
                   AND (end_time, block_number, log_index) > ($2::bigint, 0::bigint, -1) \
                   AND (end_time, block_number, log_index) > (seed_end_time, seed_block, seed_log_index)",
            )
            .bind(RENTAL_ENDED)
            .bind(since)
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }
}

/// [`plan_replay`] then [`Replay::apply`], for rows added in place (a backfill).
pub async fn replay(
    conn: &mut PgConnection,
    indexer: &str,
    net: ChainNet,
    from_block: i64,
    max_block: i64,
) -> sqlx::Result<()> {
    if let Some(r) = plan_replay(conn, indexer, net, from_block, max_block).await? {
        r.apply(conn).await?;
    }
    Ok(())
}

/// A chain notification's id: `kind` and `network` over the log's key
/// ([`tx_log_key`] or [`token_log_key`]).
pub fn notification_id(kind: &str, network: &str, key: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("{kind}:{network}:{key}").as_bytes(),
    )
}

/// SQL for the key of a log of `shop.{table}` aliased `a`: its transaction,
/// its place among the table's logs of it, and `content`.
pub fn tx_log_key(table: &str, a: &str, content: &str) -> String {
    format!(
        "({a}.tx_hash || ':' || (SELECT count(*) FROM shop.{table} o \
             WHERE o.network = {a}.network AND o.block_number = {a}.block_number \
               AND o.tx_hash = {a}.tx_hash AND o.log_index < {a}.log_index) || ':' || {content})"
    )
}

/// SQL for the key of a `shop.nft_event` log aliased `a`: its token, its
/// content and how many identical logs of the token precede it.
pub fn token_log_key(a: &str) -> String {
    format!(
        "({a}.contract_address || ':' || {a}.token_id || ':' || {a}.kind \
          || ':' || coalesce(lower({a}.from_address), '') || ':' || coalesce(lower({a}.to_address), '') \
          || ':' || coalesce({a}.amount::text, '') || ':' || (SELECT count(*) FROM shop.nft_event o \
             WHERE o.network = {a}.network AND o.contract_address = {a}.contract_address \
               AND o.token_id = {a}.token_id AND o.kind = {a}.kind \
               AND lower(o.from_address) IS NOT DISTINCT FROM lower({a}.from_address) \
               AND lower(o.to_address) IS NOT DISTINCT FROM lower({a}.to_address) \
               AND o.amount IS NOT DISTINCT FROM {a}.amount \
               AND (o.block_number, o.log_index) < ({a}.block_number, {a}.log_index)))"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::chain_walk::INDEXER_MANA;

    #[test]
    fn the_producers_are_the_listed_names() {
        for p in PRODUCERS {
            assert!(reads(p).is_some(), "{p} lists its indexes");
        }
        assert!(producers_of(INDEXER_MANA, ChainNet::Matic).is_empty());
        assert_eq!(
            producers_of(INDEXER_TRADE_EVENTS, ChainNet::Matic),
            [TRANSFER_RECEIVED, SALES_TRADED_MATIC]
        );
        assert_eq!(
            producers_of(INDEXER_SALES, ChainNet::Matic),
            [
                SALES_MINT_MATIC,
                SALES_BID_ACCEPTED_MATIC,
                ROYALTIES_BID_ACCEPTED_MATIC,
                BID_RECEIVED
            ]
        );
        assert!(producers_of(INDEXER_SALES, ChainNet::Ethereum).is_empty());
        let mut sorted = PRODUCERS.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), PRODUCERS.len());
    }

    #[test]
    fn ids_differ_by_kind_network_and_key() {
        let id = notification_id("traded", "matic", "0xab:0:0x01");
        assert_eq!(id, notification_id("traded", "matic", "0xab:0:0x01"));
        assert_ne!(id, notification_id("traded", "matic", "0xab:1:0x01"));
        assert_ne!(id, notification_id("traded", "ethereum", "0xab:0:0x01"));
        assert_ne!(id, notification_id("royalty", "matic", "0xab:0:0x01"));
    }

    /// Every listed name passes the table's CHECK; the replay floor is the
    /// seed, a deep backfill replays nothing, and planning holds no lock.
    #[tokio::test]
    async fn the_cursor_table_takes_every_producer_and_replays_keep_their_seed() {
        let Some(db) = crate::market::transfer_notices::tests::PrivateDb::create("noticecur").await
        else {
            return;
        };
        let pool = &db.pool;
        for &p in PRODUCERS {
            let end = (p == RENTAL_ENDED).then_some(1_000);
            seed(pool, p, end, 5_000, 3).await.unwrap();
        }
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.notification_cursor")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(rows, PRODUCERS.len() as i64);
        for name in ["Transfer Received", "sales:traded:base"] {
            assert!(
                sqlx::query("INSERT INTO shop.notification_cursor (producer, block_number, log_index, seed_block, seed_log_index) VALUES ($1, 0, 0, 0, 0)")
                    .bind(name)
                    .execute(pool)
                    .await
                    .is_err(),
                "{name}"
            );
        }
        let check: String = sqlx::query_scalar(
            "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = 'notification_cursor_producer_known'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut listed: Vec<&str> = check.split('\'').skip(1).step_by(2).collect();
        listed.sort();
        let mut known = PRODUCERS.to_vec();
        known.sort();
        assert_eq!(listed, known, "the CHECK lists exactly PRODUCERS: {check}");
        assert!(seed(pool, "sales:traded:base", None, 0, 0).await.is_err());
        let mut conn = pool.acquire().await.unwrap();
        assert!(lock(&mut conn, "sales:traded:base").await.is_err());
        assert!(
            advance(&mut conn, "sales:traded:base", None, 0, 0)
                .await
                .is_err()
        );
        drop(conn);
        crate::market::transfer_notices::tests::publish(
            pool,
            INDEXER_NFT,
            "matic",
            9_000,
            1_700_000_000,
        )
        .await;
        let cursor = |p: &'static str| async move {
            sqlx::query_as::<_, (i64, i32)>(
                "SELECT block_number, log_index FROM shop.notification_cursor WHERE producer = $1",
            )
            .bind(p)
            .fetch_one(pool)
            .await
            .unwrap()
        };
        advance(
            &mut pool.acquire().await.unwrap(),
            TRANSFER_RECEIVED,
            None,
            8_900,
            0,
        )
        .await
        .unwrap();

        // A backfill chunk entirely at or below head - REPLAY_BLOCKS is history.
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            plan_replay(&mut tx, INDEXER_NFT, ChainNet::Matic, 10, 8_000)
                .await
                .unwrap(),
            None
        );
        tx.rollback().await.unwrap();

        // Planning locks nothing: a producer still takes its row meanwhile.
        let mut tx = pool.begin().await.unwrap();
        let plan = plan_replay(&mut tx, INDEXER_NFT, ChainNet::Matic, 10, 8_500)
            .await
            .unwrap()
            .unwrap();
        let mut other = pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM shop.notification_cursor WHERE producer = $1 FOR UPDATE NOWAIT")
            .bind(TRANSFER_RECEIVED)
            .execute(&mut *other)
            .await
            .expect("the planned replay holds no cursor row");
        other.rollback().await.unwrap();
        plan.apply(&mut tx).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            cursor(TRANSFER_RECEIVED).await,
            (8_000, i32::MAX),
            "back to head - REPLAY_BLOCKS"
        );

        // Seeded 500 blocks below the head: the replay stops at the seed.
        sqlx::query(
            "UPDATE shop.notification_cursor SET block_number = 8950, seed_block = 8500, seed_log_index = 4 WHERE producer = $1",
        )
        .bind(ROYALTIES_EARNED)
        .execute(pool)
        .await
        .unwrap();
        replay(
            &mut pool.acquire().await.unwrap(),
            INDEXER_NFT,
            ChainNet::Matic,
            10,
            8_999,
        )
        .await
        .unwrap();
        assert_eq!(cursor(ROYALTIES_EARNED).await, (8_500, 4));
        assert_eq!(
            cursor(SALES_TRADED_MATIC).await,
            (5_000, 3),
            "another index's producer is not moved"
        );
        db.drop().await;
    }

    /// `transfer_received` reads up to the lower of the nft and trade heads,
    /// so a reorg of the lagging trade index replays it by that head.
    #[tokio::test]
    async fn replays_clamp_each_producer_by_the_index_it_tracks() {
        let Some(db) = crate::market::transfer_notices::tests::PrivateDb::create("noticeidx").await
        else {
            return;
        };
        let pool = &db.pool;
        let publish = |indexer: &'static str, block: i64| async move {
            crate::market::transfer_notices::tests::publish(
                pool,
                indexer,
                "matic",
                block,
                1_700_000_000,
            )
            .await;
        };
        let cursor = |p: &'static str| async move {
            sqlx::query_as::<_, (i64, i32)>(
                "SELECT block_number, log_index FROM shop.notification_cursor WHERE producer = $1",
            )
            .bind(p)
            .fetch_one(pool)
            .await
            .unwrap()
        };
        for p in [TRANSFER_RECEIVED, SALES_TRADED_MATIC] {
            seed(pool, p, None, 0, -1).await.unwrap();
        }
        // run_once never passes min(nft head, trade head) = 3000.
        let mut conn = pool.acquire().await.unwrap();
        advance(&mut conn, TRANSFER_RECEIVED, None, 2_999, 0)
            .await
            .unwrap();
        advance(&mut conn, SALES_TRADED_MATIC, None, 2_999, 0)
            .await
            .unwrap();
        drop(conn);
        publish(INDEXER_NFT, 9_000).await;
        publish(INDEXER_TRADE_EVENTS, 3_000).await;

        // Below head - REPLAY_BLOCKS of both indexes: history.
        replay(
            &mut pool.acquire().await.unwrap(),
            INDEXER_TRADE_EVENTS,
            ChainNet::Matic,
            10,
            1_500,
        )
        .await
        .unwrap();
        assert_eq!(cursor(TRANSFER_RECEIVED).await, (2_999, 0));
        assert_eq!(cursor(SALES_TRADED_MATIC).await, (2_999, 0));

        // The trade index lags the nft one by 6000 blocks; its reorg at its
        // head rewinds both producers reading it.
        replay(
            &mut pool.acquire().await.unwrap(),
            INDEXER_TRADE_EVENTS,
            ChainNet::Matic,
            2_990,
            2_999,
        )
        .await
        .unwrap();
        assert_eq!(cursor(TRANSFER_RECEIVED).await, (2_990, i32::MAX));
        assert_eq!(cursor(SALES_TRADED_MATIC).await, (2_990, i32::MAX));

        // An nft reorg near its own head is above the transfer cursor.
        replay(
            &mut pool.acquire().await.unwrap(),
            INDEXER_NFT,
            ChainNet::Matic,
            8_950,
            8_990,
        )
        .await
        .unwrap();
        assert_eq!(cursor(TRANSFER_RECEIVED).await, (2_990, i32::MAX));
        db.drop().await;
    }

    #[test]
    fn since_takes_dates_days_and_legacy_milliseconds() {
        assert_eq!(parse_since("2023-11-14T22:13:20Z"), Ok(1_700_000_000));
        assert_eq!(parse_since("2023-11-15T00:13:20+02:00"), Ok(1_700_000_000));
        assert_eq!(parse_since("2023-11-14"), Ok(1_699_920_000));
        assert_eq!(parse_since(" 1700000000999 "), Ok(1_700_000_000));
        // Seconds, or a legacy cursor holding a sale id, are not a time.
        assert!(parse_since("1700000000").is_err());
        assert!(parse_since("48213").is_err());
        assert!(parse_since("yesterday").is_err());
        // At or before the epoch would announce the whole index again.
        for s in [
            "1970-01-01",
            "1970-01-01T00:00:00Z",
            "1969-12-31T23:59:59Z",
            "0001-01-01",
            "-1700000000000",
            "-1",
        ] {
            assert!(parse_since(s).is_err(), "{s}");
        }
        assert_eq!(parse_since("1970-01-01T00:00:01Z"), Ok(1));
        // A time without its zone is ambiguous.
        assert!(parse_since("2023-11-14T22:13:20").is_err());
        assert!(parse_since("2023-11-14 22:13:20").is_err());
    }

    /// A rewind waits for a page holding the producer's row, then lowers
    /// the cursor that page advanced: the page's progress does not undo it.
    #[tokio::test]
    async fn a_rewind_waits_for_a_running_page_and_then_applies() {
        let Some(db) =
            crate::market::transfer_notices::tests::PrivateDb::create("noticelock").await
        else {
            return;
        };
        let pool = db.pool.clone();
        seed(&pool, SALES_TRADED_MATIC, None, 500, 0).await.unwrap();
        let mut page = pool.begin().await.unwrap();
        lock(&mut page, SALES_TRADED_MATIC).await.unwrap();
        let task = tokio::spawn({
            let pool = pool.clone();
            async move {
                rewind(
                    &pool,
                    SALES_TRADED_MATIC,
                    RewindFrom::Block(100),
                    (1, 137),
                    "t",
                    false,
                )
                .await
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(!task.is_finished(), "the rewind waits for the page");
        advance(&mut page, SALES_TRADED_MATIC, None, 900, 2)
            .await
            .unwrap();
        page.commit().await.unwrap();
        let out = tokio::time::timeout(std::time::Duration::from_secs(10), task)
            .await
            .expect("the rewind runs once the page commits")
            .unwrap()
            .unwrap();
        assert_eq!(
            out.before.map(|r| r.cursor()),
            Some((None, 900, 2)),
            "it read the page's cursor"
        );
        assert_eq!(out.after, CursorRow::at(None, 99, i32::MAX));
        let mut conn = pool.acquire().await.unwrap();
        assert_eq!(
            lock(&mut conn, SALES_TRADED_MATIC).await.unwrap(),
            (None, 99, i32::MAX)
        );
        drop(conn);
        db.drop().await;
    }

    /// A date counts only through histories proven on the chain the
    /// producer reads: an unbound, quarantined, diverged or other-chain cursor with a
    /// plausible `last_block_time` and logs before the date is refused, and
    /// neither the cursor nor its seed moves (nor is a producer seeded). A
    /// trade_events history counts on the chain the node configures, except
    /// for `transfer_received` (Polygon mainnet). `--from-block` needs no
    /// proven index.
    #[tokio::test]
    async fn a_date_resolves_only_through_a_history_proven_on_the_read_chain() {
        let Some(db) =
            crate::market::transfer_notices::tests::PrivateDb::create("noticeproof").await
        else {
            return;
        };
        let pool = &db.pool;
        const T0: i64 = 1_700_000_000;
        for b in [100, 200, 300] {
            sqlx::query(
                "INSERT INTO shop.trade_event (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, tx_hash) \
                 VALUES ('matic', $1, 0, $2, '0x540fb08edb56aae562864b390542c97f562825ba', 'signature_cancelled', '0x00000000000000000000000000000000000000a1', '0xsig', '0xtx')",
            )
            .bind(b)
            .bind(T0 + b)
            .execute(pool)
            .await
            .unwrap();
        }
        crate::market::transfer_notices::tests::publish(
            pool,
            INDEXER_TRADE_EVENTS,
            "matic",
            9_000,
            T0 + 9_000,
        )
        .await;
        crate::market::transfer_notices::tests::publish(
            pool,
            INDEXER_NFT,
            "matic",
            9_000,
            T0 + 9_000,
        )
        .await;
        seed(pool, SALES_TRADED_MATIC, None, 9_000, 7)
            .await
            .unwrap();
        let row = |p: &'static str| async move {
            sqlx::query_as::<_, CursorRow>(
                "SELECT end_time, block_number, log_index, seed_end_time, seed_block, seed_log_index \
                 FROM shop.notification_cursor WHERE producer = $1",
            )
            .bind(p)
            .fetch_optional(pool)
            .await
            .unwrap()
        };
        let set = |sql: &'static str| async move {
            sqlx::query(sql).execute(pool).await.unwrap();
        };
        let date = RewindFrom::Time(T0 + 250);
        let seeded = Some(CursorRow::at(None, 9_000, 7));
        let refused = |r: Result<Rewound, RewindError>, why: &str| match r {
            Err(RewindError::Unproven { why: w, .. }) => assert_eq!(w, why),
            other => panic!("expected an unproven refusal ({why}), got {other:?}"),
        };

        // Unbound, quarantined, or proven on another chain: nothing moves.
        let trade = "UPDATE shop.chain_cursor SET chain_id = NULL WHERE indexer = 'trade_events' AND network = 'matic'";
        set(trade).await;
        refused(
            rewind(pool, SALES_TRADED_MATIC, date, (1, 137), "t", false).await,
            "unbound",
        );
        set("UPDATE shop.chain_cursor SET chain_id = 137, quarantined_at = timezone('UTC', now()), quarantine_reason = 'test' \
             WHERE indexer = 'trade_events' AND network = 'matic'")
        .await;
        refused(
            rewind(pool, SALES_TRADED_MATIC, date, (1, 137), "t", false).await,
            "quarantined",
        );
        set("UPDATE shop.chain_cursor SET quarantined_at = NULL, quarantine_reason = NULL, diverged_at = timezone('UTC', now()) \
             WHERE indexer = 'trade_events' AND network = 'matic'")
        .await;
        refused(
            rewind(pool, SALES_TRADED_MATIC, date, (1, 137), "t", false).await,
            "diverged, its reorg recovery still searching",
        );
        set(
            "UPDATE shop.chain_cursor SET chain_id = 80002, diverged_at = NULL \
             WHERE indexer = 'trade_events' AND network = 'matic'",
        )
        .await;
        refused(
            rewind(pool, SALES_TRADED_MATIC, date, (1, 137), "t", false).await,
            "proven on chain 80002",
        );
        assert_eq!(
            row(SALES_TRADED_MATIC).await,
            seeded,
            "cursor and seed unchanged"
        );
        // A dry run refuses alike; a producer no pass seeded stays unseeded.
        refused(
            rewind(pool, SALES_TRADED_MATIC, date, (1, 137), "t", true).await,
            "proven on chain 80002",
        );
        refused(
            rewind(pool, TRANSFER_RECEIVED, date, (1, 80_002), "t", false).await,
            "proven on chain 80002",
        );
        assert_eq!(
            row(TRANSFER_RECEIVED).await,
            None,
            "no seed from another chain"
        );

        // On a stack whose trade walk reads Amoy, that history dates the
        // trade producer, and only it.
        let out = rewind(pool, SALES_TRADED_MATIC, date, (1, 80_002), "t", false)
            .await
            .unwrap();
        assert_eq!(out.after, CursorRow::at(None, 200, i32::MAX));
        assert_eq!(out.indexes[0].chain_id, Some(80_002));

        // A block needs no proven index: it only reports them.
        set(trade).await;
        let out = rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Block(150),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(out.after, CursorRow::at(None, 149, i32::MAX));
        assert_eq!(out.indexes.len(), 2);
        db.drop().await;
    }

    /// The operator rewind: a producer no pass seeded is seeded at the
    /// resolved point and its first pass keeps it (the cutover seed); a
    /// date resolves after the last block mined before it across the
    /// indexes the producer reads, and only once they indexed past it; a
    /// block rewind lowers the cursor and its seed; nothing moves forward;
    /// a dry run writes nothing; each change is audited.
    #[tokio::test]
    async fn rewinds_seed_and_lower_the_floor_by_block_and_date() {
        let Some(db) = crate::market::transfer_notices::tests::PrivateDb::create("noticerw").await
        else {
            return;
        };
        let pool = &db.pool;
        const T0: i64 = 1_700_000_000;
        let log = |table: &'static str, block: i64| async move {
            let sql = match table {
                "nft" => {
                    "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
                          VALUES ('matic', $1, 0, $2, 'transfer', '0x00000000000000000000000000000000000c011e', '0x00000000000000000000000000000000000c011e', $1, \
                          '0x00000000000000000000000000000000000000a1', '0x00000000000000000000000000000000000000b0')"
                }
                _ => {
                    "INSERT INTO shop.trade_event (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, tx_hash) \
                      VALUES ('matic', $1, 0, $2, '0x540fb08edb56aae562864b390542c97f562825ba', 'signature_cancelled', '0x00000000000000000000000000000000000000a1', '0xsig', '0xtx')"
                }
            };
            sqlx::query(sql)
                .bind(block)
                .bind(T0 + block)
                .execute(pool)
                .await
                .unwrap();
        };
        for b in [100, 200, 300] {
            log("nft", b).await;
        }
        log("trade", 250).await;
        let publish = |indexer: &'static str, network: &'static str| async move {
            crate::market::transfer_notices::tests::publish(
                pool,
                indexer,
                network,
                9_000,
                T0 + 9_000,
            )
            .await;
        };
        let row = |p: &'static str| async move {
            sqlx::query_as::<_, CursorRow>(
                "SELECT end_time, block_number, log_index, seed_end_time, seed_block, seed_log_index \
                 FROM shop.notification_cursor WHERE producer = $1",
            )
            .bind(p)
            .fetch_optional(pool)
            .await
            .unwrap()
        };
        let at = |e: Option<i64>, b: i64, l: i32| CursorRow::at(e, b, l);

        assert!(matches!(
            rewind(
                pool,
                "sales:traded:base",
                RewindFrom::Block(1),
                (1, 137),
                "t",
                false
            )
            .await,
            Err(RewindError::UnknownProducer(_))
        ));
        assert!(matches!(
            rewind(
                pool,
                TRANSFER_RECEIVED,
                RewindFrom::Time(T0 + 150),
                (1, 137),
                "t",
                false
            )
            .await,
            Err(RewindError::NoIndex { .. })
        ));
        publish(INDEXER_NFT, "matic").await;
        publish(INDEXER_TRADE_EVENTS, "matic").await;
        assert!(matches!(
            rewind(
                pool,
                TRANSFER_RECEIVED,
                RewindFrom::Time(T0 + 9_001),
                (1, 137),
                "t",
                false
            )
            .await,
            Err(RewindError::NotReached { .. })
        ));
        assert_eq!(
            row(TRANSFER_RECEIVED).await,
            None,
            "a refusal writes nothing"
        );

        // The cutover seed, before the first pass: after block 200, the last
        // one mined before the date on either index (the trade log at 250 is not).
        let out = rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Time(T0 + 201),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(out.before, None);
        assert_eq!(out.after, at(None, 200, i32::MAX));
        assert_eq!(out.indexes.len(), 2);
        seed(pool, TRANSFER_RECEIVED, None, 9_000, 0).await.unwrap();
        assert_eq!(
            row(TRANSFER_RECEIVED).await,
            Some(at(None, 200, i32::MAX)),
            "the first pass keeps the operator's seed"
        );
        // The trade log counts for a producer reading both indexes.
        let out = rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Time(T0 + 251),
            (1, 137),
            "t",
            true,
        )
        .await
        .unwrap();
        assert!(!out.changed, "not forward: {out:?}");

        // A pass seeded at the head, then progressed: a date rewind lowers
        // the cursor and the seed.
        seed(pool, SALES_TRADED_MATIC, None, 9_000, i32::MAX)
            .await
            .unwrap();
        let out = rewind(
            pool,
            SALES_TRADED_MATIC,
            RewindFrom::Time(T0 + 260),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(out.after, at(None, 250, i32::MAX));
        assert_eq!(row(SALES_TRADED_MATIC).await, Some(at(None, 250, i32::MAX)));

        // A block rewind, below the seed too.
        advance(
            &mut pool.acquire().await.unwrap(),
            TRANSFER_RECEIVED,
            None,
            8_000,
            0,
        )
        .await
        .unwrap();
        let out = rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Block(50),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            out.before,
            Some(CursorRow {
                block_number: 8_000,
                log_index: 0,
                ..at(None, 200, i32::MAX)
            })
        );
        assert_eq!(row(TRANSFER_RECEIVED).await, Some(at(None, 49, i32::MAX)));
        // Forward is a no-op; a dry run writes nothing.
        let out = rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Block(500),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert!(!out.changed);
        let out = rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Block(1),
            (1, 137),
            "t",
            true,
        )
        .await
        .unwrap();
        assert!(out.changed && out.dry_run);
        assert_eq!(out.after, at(None, 0, i32::MAX));
        // The logs after block 0 up to each head: nft 100, 200, 300; trade 250.
        let counts: Vec<_> = out.indexes.iter().map(|i| i.logs_after).collect();
        assert_eq!(counts, [Some(3), Some(1)]);
        assert_eq!(row(TRANSFER_RECEIVED).await, Some(at(None, 49, i32::MAX)));
        // Before the first block, or at or before the epoch: the whole index.
        for from in [
            RewindFrom::Block(0),
            RewindFrom::Block(-5),
            RewindFrom::Time(0),
            RewindFrom::Time(-62_135_596_800),
        ] {
            let err = rewind(pool, TRANSFER_RECEIVED, from, (1, 137), "t", true)
                .await
                .unwrap_err();
            assert!(
                matches!(
                    err,
                    RewindError::BlockOutOfRange(_) | RewindError::TimeOutOfRange(_)
                ),
                "{from:?}: {err}"
            );
        }

        // rental_ended takes a date as its end-time cursor, never a block.
        publish(INDEXER_RENTALS, "ethereum").await;
        assert!(matches!(
            rewind(
                pool,
                RENTAL_ENDED,
                RewindFrom::Block(10),
                (1, 137),
                "t",
                false
            )
            .await,
            Err(RewindError::NeedsDate(_))
        ));
        seed(pool, RENTAL_ENDED, Some(T0 + 9_000), 9_000, i32::MAX)
            .await
            .unwrap();
        // An end time the rentals index has not reached would skip the
        // periods ending before it.
        let err = rewind(
            pool,
            RENTAL_ENDED,
            RewindFrom::Time(T0 + 9_001),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, RewindError::NotReached { .. })
                && !err.to_string().contains("give a block"),
            "{err}"
        );
        let out = rewind(
            pool,
            RENTAL_ENDED,
            RewindFrom::Time(T0),
            (1, 137),
            "t",
            true,
        )
        .await
        .unwrap();
        assert_eq!(out.indexes[0].logs_after, None, "periods, not logs");
        rewind(
            pool,
            RENTAL_ENDED,
            RewindFrom::Time(T0),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(row(RENTAL_ENDED).await, Some(at(Some(T0), 0, -1)));

        // A block needs no index: with no sales cursor row, the block
        // rewind seeds the producer and reports no coverage; a date is refused.
        assert!(matches!(
            rewind(
                pool,
                SALES_MINT_MATIC,
                RewindFrom::Time(T0 + 100),
                (1, 137),
                "t",
                false
            )
            .await,
            Err(RewindError::NoIndex { .. })
        ));
        let out = rewind(
            pool,
            SALES_MINT_MATIC,
            RewindFrom::Block(120),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert!(out.indexes.is_empty());
        assert_eq!(row(SALES_MINT_MATIC).await, Some(at(None, 119, i32::MAX)));

        let audited: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ops.admin_audit WHERE domain = 'market' AND action = 'notice-cursor.rewind'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(audited, 5, "the five changes; no refusal, no-op or dry run");
        db.drop().await;
    }

    /// A rewind announces the logs after it once; a second rewind over the
    /// same logs stores and emails nothing again (the ids are stable).
    #[tokio::test]
    async fn a_rewound_producer_announces_again_without_duplicates() {
        use crate::market::transfer_notices::{self, Pass};
        let Some(db) =
            crate::market::transfer_notices::tests::PrivateDb::create("noticeagain").await
        else {
            return;
        };
        let pool = &db.pool;
        for block in [100_i64, 101, 102] {
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address) \
                 VALUES ('matic', $1, 0, 1700000000 + $1, 'transfer', '0x00000000000000000000000000000000000c011e', '0x00000000000000000000000000000000000c011e', $1, \
                 '0x00000000000000000000000000000000000000a1', '0x00000000000000000000000000000000000000b0')",
            )
            .bind(block)
            .execute(pool)
            .await
            .unwrap();
        }
        for indexer in [INDEXER_NFT, INDEXER_TRADE_EVENTS] {
            crate::market::transfer_notices::tests::publish(
                pool,
                indexer,
                "matic",
                110,
                1_700_000_110,
            )
            .await;
        }
        let counts = || async {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT (SELECT count(*) FROM social.notifications), (SELECT count(*) FROM social.notification_deliveries)",
            )
            .fetch_one(pool)
            .await
            .unwrap()
        };
        assert_eq!(
            transfer_notices::run_once(pool, 137).await.unwrap(),
            Pass::Emitted(0),
            "history is not announced"
        );
        rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Time(1_700_000_101),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            transfer_notices::run_once(pool, 137).await.unwrap(),
            Pass::Emitted(2)
        );
        assert_eq!(counts().await, (2, 2));
        rewind(
            pool,
            TRANSFER_RECEIVED,
            RewindFrom::Block(100),
            (1, 137),
            "t",
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            transfer_notices::run_once(pool, 137).await.unwrap(),
            Pass::Emitted(1)
        );
        assert_eq!(counts().await, (3, 3), "only block 100 is new");
        db.drop().await;
    }
}
