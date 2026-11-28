//! The private legacy order build. The NFT cursor fences both source writers
//! and these transactions. Only publication writes the live order rows.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use sqlx::{PgConnection, PgPool};

use super::{ChainNet, STANDS_SQL, write_marker};
use crate::market::chain_walk::{self, INDEXER_NFT, Owner};

pub const REBUILD_ROWS_PER_STEP: i64 = 4096;
const STEP_TIMEOUT: Duration = Duration::from_secs(15);
const SWAP_TIMEOUT: Duration = Duration::from_secs(90);
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const STALLED_SECONDS: i32 = 1800;
const END_LOG: i32 = i32::MAX;
const PUBLISH_FAILURE_RESERVE: Duration = Duration::from_secs(1);
type Position = (i64, i32);

pub(crate) struct RebuildLane {
    rows: AtomicI64,
    step_timeout: Duration,
    publish_timeout: Duration,
}

impl Default for RebuildLane {
    fn default() -> Self {
        Self {
            rows: AtomicI64::new(REBUILD_ROWS_PER_STEP),
            step_timeout: STEP_TIMEOUT,
            publish_timeout: SWAP_TIMEOUT,
        }
    }
}

impl RebuildLane {
    #[cfg(test)]
    pub(super) fn with_timeouts(step_timeout: Duration, publish_timeout: Duration) -> Self {
        Self {
            step_timeout,
            publish_timeout,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(super) fn rows(&self) -> i64 {
        self.rows.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildStep {
    /// No work available at the current NFT checkpoint.
    Idle,
    Batched,
    Finished,
}

#[derive(sqlx::FromRow)]
struct Build {
    phase: String,
    upto: i64,
    upto_hash: String,
    anchor_block: i64,
    anchor_hash: String,
    reorg_rewinds: i32,
    rebuilds: i32,
    visible_through: Option<i64>,
    visible_hash: Option<String>,
    target: i64,
    scan_block: i64,
    scan_log: i32,
    step_block: i64,
    step_log: i32,
    repair_contract: Option<String>,
    repair_token: Option<String>,
    repair_revision: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct Checkpoint {
    last: i64,
    hash: String,
    reorg_rewinds: i32,
    rebuilds: i32,
    opening_hash: Option<String>,
    anchor_hash: Option<String>,
    anchor_pruned: bool,
}

struct Token<'a> {
    contract: &'a str,
    id: &'a str,
}

const OPEN_SQL: &str = "
INSERT INTO shop.legacy_order_rebuild (network, upto, upto_hash, anchor_block,
    anchor_hash, reorg_rewinds, rebuilds, visible_through, visible_hash, target)
SELECT c.network, c.last_block, k.block_hash, c.last_block, k.block_hash,
    c.reorg_rewinds, c.rebuilds, v.folded_through, v.through_hash, c.last_block
FROM shop.chain_cursor c
JOIN shop.chain_checkpoint k ON (k.indexer,k.network,k.block_number,k.block_hash) =
    (c.indexer,c.network,c.last_block,c.last_block_hash)
LEFT JOIN shop.legacy_order_cursor v ON v.network = c.network
WHERE c.indexer = 'nft' AND c.network = $1 AND c.diverged_at IS NULL
ON CONFLICT (network) DO UPDATE SET phase = 'reset', upto = EXCLUDED.upto,
    upto_hash = EXCLUDED.upto_hash, anchor_block = EXCLUDED.anchor_block,
    anchor_hash = EXCLUDED.anchor_hash, reorg_rewinds = EXCLUDED.reorg_rewinds,
    rebuilds = EXCLUDED.rebuilds, visible_through = EXCLUDED.visible_through,
    visible_hash = EXCLUDED.visible_hash, target = EXCLUDED.target,
    scan_block = -1, scan_log = -1, step_block = -1, step_log = -1,
    repair_contract = NULL, repair_token = NULL, repair_revision = NULL,
    publish_failures = 0, publish_after = NULL,
    progressed_at = timezone('UTC', now())";

pub(super) async fn open(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<bool> {
    Ok(sqlx::query(OPEN_SQL)
        .bind(net.db_name())
        .execute(conn)
        .await?
        .rows_affected()
        == 1)
}

const READ_BUILD: &str = "
SELECT phase, upto, upto_hash, anchor_block, anchor_hash, reorg_rewinds, rebuilds,
    visible_through, visible_hash, target, scan_block, scan_log, step_block, step_log,
    repair_contract::text, repair_token::text, repair_revision
FROM shop.legacy_order_rebuild WHERE network = $1 FOR UPDATE";

const READ_CHECKPOINT: &str = "
SELECT c.last_block AS last, k.block_hash AS hash, c.reorg_rewinds, c.rebuilds,
    orig.block_hash AS opening_hash, anchor.block_hash AS anchor_hash,
    (SELECT count(*) = $4 AND min(block_number) > $3
     FROM (SELECT block_number FROM shop.chain_checkpoint
           WHERE indexer = c.indexer AND network = c.network
           ORDER BY block_number DESC LIMIT $4) kept) AS anchor_pruned
FROM shop.chain_cursor c
JOIN shop.chain_checkpoint k ON (k.indexer,k.network,k.block_number,k.block_hash) =
    (c.indexer,c.network,c.last_block,c.last_block_hash)
LEFT JOIN shop.chain_checkpoint orig ON (orig.indexer,orig.network,orig.block_number) =
    (c.indexer,c.network,$2)
LEFT JOIN shop.chain_checkpoint anchor ON (anchor.indexer,anchor.network,anchor.block_number) =
    (c.indexer,c.network,$3)
WHERE c.indexer = 'nft' AND c.network = $1 AND c.diverged_at IS NULL";

impl Build {
    fn scan(&self) -> Position {
        (self.scan_block, self.scan_log)
    }

    fn step(&self) -> Position {
        (self.step_block, self.step_log)
    }

    fn token(&self) -> sqlx::Result<Token<'_>> {
        self.repair_contract
            .as_deref()
            .zip(self.repair_token.as_deref())
            .map(|(contract, id)| Token { contract, id })
            .ok_or_else(|| sqlx::Error::Protocol("missing legacy order repair token".into()))
    }

    fn matches(&self, checkpoint: &Checkpoint) -> bool {
        checkpoint.last >= self.anchor_block
            && self.reorg_rewinds == checkpoint.reorg_rewinds
            && self.rebuilds == checkpoint.rebuilds
            && checkpoint
                .opening_hash
                .as_ref()
                .is_none_or(|h| h == &self.upto_hash)
            && checkpoint.anchor_hash.as_ref().map_or(
                checkpoint.anchor_pruned && !self.anchor_hash.is_empty(),
                |h| h == &self.anchor_hash,
            )
    }
}

async fn update_phase(conn: &mut PgConnection, net: ChainNet, phase: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE shop.legacy_order_rebuild SET phase = $2, step_block = -1, step_log = -1
         WHERE network = $1",
    )
    .bind(net.db_name())
    .bind(phase)
    .execute(conn)
    .await?;
    Ok(())
}

async fn save_position(
    conn: &mut PgConnection,
    net: ChainNet,
    position: Position,
    scan: bool,
) -> sqlx::Result<()> {
    let sql = if scan {
        "UPDATE shop.legacy_order_rebuild SET scan_block = $2, scan_log = $3 WHERE network = $1"
    } else {
        "UPDATE shop.legacy_order_rebuild SET step_block = $2, step_log = $3 WHERE network = $1"
    };
    sqlx::query(sql)
        .bind(net.db_name())
        .bind(position.0)
        .bind(position.1)
        .execute(conn)
        .await?;
    Ok(())
}

// LIMIT applies before filtering for order events, so long stretches of
// other logs and high-volume tokens still cost only one page of the index.
const COPY_BATCH: &str = "
WITH batch AS MATERIALIZED (
    SELECT network, block_number, log_index, block_time, kind, emitter, contract_address,
           token_id, from_address, to_address, amount, expires_at, detail
    FROM shop.nft_event
    WHERE network = $1 AND (block_number, log_index) > ($2, $3)
      AND (block_number, log_index) <= ($4, $5)";
const COPY_EVENTS: &str = "
    ORDER BY block_number, log_index LIMIT $6
), staged AS (
    INSERT INTO shop.legacy_order_build_event
    SELECT * FROM batch
    WHERE kind IN ('order_created', 'order_successful', 'order_cancelled') AND detail IS NOT NULL
    RETURNING contract_address, token_id
)";
const QUEUE_TAIL: &str = ",
queued AS (
    INSERT INTO shop.legacy_order_build_dirty (network, contract_address, token_id)
    SELECT DISTINCT $1, contract_address, token_id FROM staged ON CONFLICT DO NOTHING
)";
const LAST_POSITION: &str = "
SELECT block_number, log_index FROM batch ORDER BY block_number DESC, log_index DESC LIMIT 1";
const TOKEN_SCOPE: &str = " AND contract_address = $7 AND token_id = $8::text::numeric";

async fn copy_events(
    conn: &mut PgConnection,
    net: ChainNet,
    after: Position,
    end: Position,
    rows: i64,
    token: Option<&Token<'_>>,
    tail: bool,
) -> sqlx::Result<Option<Position>> {
    let scope = if token.is_some() { TOKEN_SCOPE } else { "" };
    let queue = if tail { QUEUE_TAIL } else { "" };
    let mut query = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{COPY_BATCH}{scope}{COPY_EVENTS}{queue}{LAST_POSITION}"
    )))
    .bind(net.db_name())
    .bind(after.0)
    .bind(after.1)
    .bind(end.0)
    .bind(end.1)
    .bind(rows);
    if let Some(token) = token {
        query = query.bind(token.contract).bind(token.id);
    }
    query.fetch_optional(conn).await
}

const BUILD_BATCH: &str = "
WITH batch AS MATERIALIZED (
    SELECT * FROM shop.legacy_order_build_event
    WHERE network = $1 AND (block_number, log_index) > ($2, $3) AND kind = 'order_created'";
const BUILD_ORDERS: &str = "
    ORDER BY block_number, log_index LIMIT $4
), latest AS (
    SELECT DISTINCT ON (emitter, detail) *
    FROM batch ORDER BY emitter, detail, block_number DESC, log_index DESC
), written AS (
    INSERT INTO shop.legacy_order_build AS book
    SELECT c.network, c.emitter, c.detail, c.contract_address, c.token_id,
           COALESCE(c.from_address, $5),
           CASE WHEN t.kind = 'order_successful' THEN t.to_address END,
           CASE WHEN t.kind = 'order_successful' THEN COALESCE(t.amount, c.amount, 0)
                ELSE COALESCE(c.amount, 0) END,
           COALESCE(c.expires_at, 0),
           CASE WHEN t.kind = 'order_successful' THEN 'sold'
                WHEN t.kind = 'order_cancelled' OR n.block_number IS NOT NULL THEN 'cancelled'
                ELSE 'open' END,
           c.block_number, c.log_index, c.block_time,
           COALESCE(t.block_number, n.block_number, c.block_number),
           COALESCE(t.log_index, n.log_index, c.log_index),
           COALESCE(t.block_time, n.block_time, c.block_time)
    FROM latest c
    LEFT JOIN LATERAL (
        SELECT n.block_number, n.log_index, n.block_time FROM shop.legacy_order_build_event n
        WHERE (n.network, n.contract_address, n.token_id) = (c.network, c.contract_address, c.token_id)
          AND n.kind = 'order_created' AND (n.block_number, n.log_index) > (c.block_number, c.log_index)
        ORDER BY n.block_number, n.log_index LIMIT 1
    ) n ON true
    LEFT JOIN LATERAL (
        SELECT t.kind, t.to_address, t.amount, t.block_number, t.log_index, t.block_time
        FROM shop.legacy_order_build_event t
        WHERE (t.network, t.contract_address, t.token_id, t.emitter, t.detail) =
              (c.network, c.contract_address, c.token_id, c.emitter, c.detail)
          AND t.kind IN ('order_successful', 'order_cancelled')
          AND (t.block_number, t.log_index) > (c.block_number, c.log_index)
        ORDER BY t.block_number DESC, t.log_index DESC LIMIT 1
    ) t ON true
    ON CONFLICT (network, marketplace_address, order_id) DO UPDATE SET
        contract_address = EXCLUDED.contract_address, token_id = EXCLUDED.token_id,
        seller = EXCLUDED.seller, buyer = EXCLUDED.buyer, price = EXCLUDED.price,
        expires_at = EXCLUDED.expires_at, status = EXCLUDED.status, created_block = EXCLUDED.created_block,
        created_log = EXCLUDED.created_log, created_at = EXCLUDED.created_at, updated_block = EXCLUDED.updated_block,
        updated_log = EXCLUDED.updated_log, updated_at = EXCLUDED.updated_at
    WHERE (book.created_block, book.created_log) < (EXCLUDED.created_block, EXCLUDED.created_log)
)";

async fn materialize(
    conn: &mut PgConnection,
    net: ChainNet,
    after: Position,
    rows: i64,
    token: Option<&Token<'_>>,
) -> sqlx::Result<Option<Position>> {
    let scope = if token.is_some() {
        " AND contract_address = $6 AND token_id = $7::text::numeric"
    } else {
        ""
    };
    let mut query = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{BUILD_BATCH}{scope}{BUILD_ORDERS}{LAST_POSITION}"
    )))
    .bind(net.db_name())
    .bind(after.0)
    .bind(after.1)
    .bind(rows)
    .bind(crate::market::nft_index::ZERO_ADDRESS);
    if let Some(token) = token {
        query = query.bind(token.contract).bind(token.id);
    }
    query.fetch_optional(conn).await
}

const SHADOW_TABLES: [(&str, &str); 3] = [
    ("legacy_order_build", "marketplace_address, order_id"),
    ("legacy_order_build_event", "block_number, log_index"),
    ("legacy_order_build_dirty", "contract_address, token_id"),
];

async fn clear_shadow(
    conn: &mut PgConnection,
    net: ChainNet,
    rows: i64,
    token: Option<&Token<'_>>,
) -> sqlx::Result<bool> {
    let tables = if token.is_some() {
        &SHADOW_TABLES[..2]
    } else {
        &SHADOW_TABLES[..]
    };
    // Only one nonempty page is deleted in a step. The scoped queries use
    // the shadow tables' token indexes when replaying a particular token.
    for (table, order) in tables {
        let scope = if token.is_some() {
            " AND contract_address = $3 AND token_id = $4::text::numeric"
        } else {
            ""
        };
        let mut query = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM shop.{table} WHERE ctid = ANY(ARRAY(
                 SELECT ctid FROM shop.{table} WHERE network = $1{scope}
                 ORDER BY {order} LIMIT $2))"
        )))
        .bind(net.db_name())
        .bind(rows);
        if let Some(token) = token {
            query = query.bind(token.contract).bind(token.id);
        }
        if query.execute(&mut *conn).await?.rows_affected() != 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn select_repair(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<bool> {
    Ok(sqlx::query(
        "WITH next AS (
            SELECT contract_address, token_id, revision FROM shop.legacy_order_build_dirty
            WHERE network = $1 ORDER BY contract_address, token_id LIMIT 1
         )
         UPDATE shop.legacy_order_rebuild r SET phase = 'repair_clear',
            repair_contract = n.contract_address, repair_token = n.token_id,
            repair_revision = n.revision, step_block = -1, step_log = -1
         FROM next n WHERE r.network = $1",
    )
    .bind(net.db_name())
    .execute(conn)
    .await?
    .rows_affected()
        != 0)
}

async fn repair(
    conn: &mut PgConnection,
    net: ChainNet,
    build: &Build,
    rows: i64,
) -> sqlx::Result<()> {
    let token = build.token()?;
    match build.phase.as_str() {
        "repair_clear" => {
            if clear_shadow(conn, net, rows, Some(&token)).await? {
                update_phase(conn, net, "repair_scan").await?;
            }
        }
        "repair_scan" => {
            if let Some(last) = copy_events(
                conn,
                net,
                build.step(),
                build.scan(),
                rows,
                Some(&token),
                false,
            )
            .await?
            {
                save_position(conn, net, last, false).await?;
            } else {
                update_phase(conn, net, "repair_build").await?;
            }
        }
        _ => {
            if let Some(last) = materialize(conn, net, build.step(), rows, Some(&token)).await? {
                save_position(conn, net, last, false).await?;
            } else {
                // A change made during the multi-transaction replay has a
                // later revision and leaves the token queued for another pass.
                sqlx::query(
                    "DELETE FROM shop.legacy_order_build_dirty WHERE network = $1
                     AND contract_address = $2 AND token_id = $3::text::numeric AND revision = $4",
                )
                .bind(net.db_name())
                .bind(token.contract)
                .bind(token.id)
                .bind(build.repair_revision)
                .execute(&mut *conn)
                .await?;
                update_phase(conn, net, "tail").await?;
            }
        }
    }
    Ok(())
}

async fn drain_live_queue(conn: &mut PgConnection, net: ChainNet, rows: i64) -> sqlx::Result<bool> {
    let drained = sqlx::query(
        "DELETE FROM shop.legacy_order_refold WHERE ctid = ANY(ARRAY(
             SELECT ctid FROM shop.legacy_order_refold WHERE network = $1
             ORDER BY contract_address, token_id LIMIT $2))",
    )
    .bind(net.db_name())
    .bind(rows)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if drained != 0 {
        // The private replay has its own queue. Removing live queue entries
        // cannot make an old reader believe the unrepaired live book stands.
        sqlx::query("UPDATE shop.legacy_order_cursor SET through_hash = NULL WHERE network = $1 AND through_hash IS NOT NULL")
            .bind(net.db_name())
            .execute(conn)
            .await?;
    }
    Ok(drained == 0)
}

async fn old_book_published(
    conn: &mut PgConnection,
    net: ChainNet,
    build: &Build,
    checkpoint: &Checkpoint,
) -> sqlx::Result<bool> {
    if build.visible_through == Some(checkpoint.last)
        && build.visible_hash.as_deref() == Some(checkpoint.hash.as_str())
    {
        // The opener already saw this marker. It is no proof of a handover.
        return Ok(false);
    }
    let through: Option<i64> = sqlx::query_scalar(STANDS_SQL)
        .bind(net.db_name())
        .bind(INDEXER_NFT)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(through == Some(checkpoint.last)
        && super::visible_readiness_on(conn, net).await? == super::Readiness::Published
        && chain_walk::rows_published_on(conn, INDEXER_NFT, net, &["nft", "legacy_order"], None)
            .await?)
}

async fn tail(
    conn: &mut PgConnection,
    net: ChainNet,
    build: &Build,
    checkpoint: &Checkpoint,
    rows: i64,
) -> sqlx::Result<()> {
    let through = (build.target, END_LOG);
    if build.scan() < through {
        let page = copy_events(conn, net, build.scan(), through, rows, None, true).await?;
        save_position(conn, net, page.unwrap_or(through), true).await?;
    } else if select_repair(conn, net).await? {
        // Replay the frozen scan prefix before extending the tail again.
    } else if build.target < checkpoint.last {
        sqlx::query("UPDATE shop.legacy_order_rebuild SET target = $2 WHERE network = $1")
            .bind(net.db_name())
            .bind(checkpoint.last)
            .execute(conn)
            .await?;
    } else if drain_live_queue(conn, net, rows).await? {
        update_phase(conn, net, "publish").await?;
    }
    Ok(())
}

async fn publish(
    conn: &mut PgConnection,
    net: ChainNet,
    build: &Build,
    checkpoint: &Checkpoint,
) -> sqlx::Result<()> {
    let queued: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.legacy_order_build_dirty WHERE network = $1)
             OR EXISTS (SELECT 1 FROM shop.legacy_order_refold WHERE network = $1)",
    )
    .bind(net.db_name())
    .fetch_one(&mut *conn)
    .await?;
    if queued || build.scan() != (checkpoint.last, END_LOG) {
        return update_phase(conn, net, "tail").await;
    }

    // One snapshot boundary for old and new readers. Preserve the live
    // relation and its write triggers, which the ordinary NFT publication
    // acknowledges, including at an unchanged height.
    sqlx::query(
        "DELETE FROM shop.legacy_order o WHERE o.network = $1 AND NOT EXISTS (
             SELECT 1 FROM shop.legacy_order_build b
             WHERE (b.network, b.marketplace_address, b.order_id) =
                   (o.network, o.marketplace_address, o.order_id))",
    )
    .bind(net.db_name())
    .execute(&mut *conn)
    .await?;
    write_changes(conn, net).await?;
    write_marker(conn, net).await?;
    update_phase(conn, net, "cleanup").await?;
    tracing::info!(
        network = net.db_name(),
        block = checkpoint.last,
        "legacy order book published"
    );
    Ok(())
}

// Use the same column names for projection, comparison and update. Filter
// unchanged rows before the upsert so they do not acquire tuple locks or
// fire the publication trigger.
async fn write_changes(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<()> {
    const VALUES: [&str; 13] = [
        "contract_address",
        "token_id",
        "seller",
        "buyer",
        "price",
        "expires_at",
        "status",
        "created_block",
        "created_log",
        "created_at",
        "updated_block",
        "updated_log",
        "updated_at",
    ];
    let from = |table: &str| VALUES.map(|column| format!("{table}.{column}")).join(", ");
    let updates = VALUES
        .map(|column| format!("{column} = EXCLUDED.{column}"))
        .join(", ");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.legacy_order (network, marketplace_address, order_id, {columns})
         SELECT b.network, b.marketplace_address, b.order_id, {build}
         FROM shop.legacy_order_build b
         LEFT JOIN shop.legacy_order o USING (network, marketplace_address, order_id)
         WHERE b.network = $1 AND (o.network IS NULL OR ({live}) IS DISTINCT FROM ({build}))
         ON CONFLICT (network, marketplace_address, order_id) DO UPDATE SET {updates}",
        columns = VALUES.join(", "),
        build = from("b"),
        live = from("o"),
    )))
    .bind(net.db_name())
    .execute(conn)
    .await?;
    Ok(())
}

/// One bounded private step, or one atomic publication. The caller holds the
/// NFT cursor lock on this transaction.
pub async fn rebuild_batch(
    conn: &mut PgConnection,
    net: ChainNet,
    rows: i64,
) -> sqlx::Result<RebuildStep> {
    let rows = rows.max(1);
    let Some(build) = sqlx::query_as::<_, Build>(READ_BUILD)
        .bind(net.db_name())
        .fetch_optional(&mut *conn)
        .await?
    else {
        return Ok(RebuildStep::Idle);
    };
    if build.phase == "cleanup" {
        if !clear_shadow(conn, net, rows, None).await? {
            sqlx::query(
                "UPDATE shop.legacy_order_rebuild SET progressed_at = timezone('UTC', now())
                 WHERE network = $1",
            )
            .bind(net.db_name())
            .execute(conn)
            .await?;
            return Ok(RebuildStep::Batched);
        }
        sqlx::query("DELETE FROM shop.legacy_order_rebuild WHERE network = $1")
            .bind(net.db_name())
            .execute(conn)
            .await?;
        return Ok(RebuildStep::Finished);
    }
    let Some(checkpoint) = sqlx::query_as::<_, Checkpoint>(READ_CHECKPOINT)
        .bind(net.db_name())
        .bind(build.upto)
        .bind(build.anchor_block)
        .bind(chain_walk::CHECKPOINTS_KEPT)
        .fetch_optional(&mut *conn)
        .await?
    else {
        return Ok(RebuildStep::Idle);
    };
    if !build.matches(&checkpoint) {
        dcl_metrics::counter!("catalyst_legacy_order_rebuild_restarts_total",
            "network" => net.db_name())
        .increment(1);
        open(conn, net).await?;
        return Ok(RebuildStep::Batched);
    }
    if old_book_published(conn, net, &build, &checkpoint).await? {
        update_phase(conn, net, "cleanup").await?;
        return Ok(RebuildStep::Batched);
    }
    match build.phase.as_str() {
        "reset" => {
            if clear_shadow(conn, net, rows, None).await? {
                update_phase(conn, net, "scan").await?;
            }
        }
        "scan" => {
            let end = (build.upto, END_LOG);
            if let Some(last) = copy_events(conn, net, build.scan(), end, rows, None, false).await?
            {
                save_position(conn, net, last, true).await?;
            } else {
                save_position(conn, net, end, true).await?;
                update_phase(conn, net, "build").await?;
            }
        }
        "build" => {
            if let Some(last) = materialize(conn, net, build.step(), rows, None).await? {
                save_position(conn, net, last, false).await?;
            } else {
                update_phase(conn, net, "tail").await?;
            }
        }
        "tail" => tail(conn, net, &build, &checkpoint, rows).await?,
        "repair_clear" | "repair_scan" | "repair_build" => repair(conn, net, &build, rows).await?,
        "publish" => publish(conn, net, &build, &checkpoint).await?,
        _ => {
            return Err(sqlx::Error::Protocol(
                "unknown legacy order build phase".into(),
            ));
        }
    }
    sqlx::query(
        "UPDATE shop.legacy_order_rebuild SET anchor_block = $2, anchor_hash = $3,
             progressed_at = timezone('UTC', now()) WHERE network = $1",
    )
    .bind(net.db_name())
    .bind(checkpoint.last)
    .bind(checkpoint.hash)
    .execute(conn)
    .await?;
    Ok(RebuildStep::Batched)
}

fn milliseconds(timeout: Duration) -> String {
    format!("{}ms", timeout.as_millis().max(1))
}

fn timed_out(error: &anyhow::Error) -> bool {
    error.is::<tokio::time::error::Elapsed>()
        || error.downcast_ref::<sqlx::Error>().is_some_and(|error| {
            matches!(
                error.as_database_error().and_then(|e| e.code()).as_deref(),
                Some("57014" | "25P04")
            )
        })
}

async fn defer_publish(
    pool: &PgPool,
    net: ChainNet,
    progress: chrono::NaiveDateTime,
    deadline: Instant,
) -> anyhow::Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Ok(());
    }
    let failures = tokio::time::timeout(remaining.min(PUBLISH_FAILURE_RESERVE), async {
        let mut tx = pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout = '250ms';")
            .execute(&mut *tx)
            .await?;
        let failures: Option<i32> = sqlx::query_scalar(
            "UPDATE shop.legacy_order_rebuild
             SET publish_failures = least(publish_failures, 30) + 1,
                 publish_after = timezone('UTC', now()) + make_interval(
                     secs => least(600, 30 * (1 << least(publish_failures, 5))))
             WHERE network = $1 AND phase = 'publish' AND progressed_at = $2
             RETURNING publish_failures",
        )
        .bind(net.db_name())
        .bind(progress)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok::<_, sqlx::Error>(failures)
    })
    .await??;
    if let Some(failures) = failures {
        dcl_metrics::counter!("catalyst_legacy_order_publish_timeouts_total",
            "network" => net.db_name())
        .increment(1);
        tracing::error!(
            network = net.db_name(),
            failures,
            "legacy order publication timed out; retry deferred"
        );
    }
    Ok(())
}

/// Bound pool acquisition and the whole batch as well as the cursor lock.
/// A swap gets its full budget and leaves time to persist a failed attempt.
pub(crate) async fn run_rebuild(
    pool: &PgPool,
    net: ChainNet,
    owner: &Owner,
    deadline: Instant,
    lane: &RebuildLane,
) -> anyhow::Result<bool> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        let build = tokio::time::timeout(
            remaining,
            sqlx::query_as::<_, (String, bool, bool, chrono::NaiveDateTime)>(
                "SELECT phase, progressed_at <= timezone('UTC', now()) - make_interval(secs => $2),
                        coalesce(publish_after > timezone('UTC', now()), false), progressed_at
                 FROM shop.legacy_order_rebuild WHERE network = $1",
            )
            .bind(net.db_name())
            .bind(f64::from(STALLED_SECONDS))
            .fetch_optional(pool),
        )
        .await??;
        let Some((phase, stalled, cooling_down, progress)) = build else {
            lane.rows.store(REBUILD_ROWS_PER_STEP, Ordering::Relaxed);
            return Ok(true);
        };
        if stalled {
            tracing::error!(network = net.db_name(), %phase, "legacy order book rebuild stalled");
        }
        let publish = phase == "publish";
        let remaining = deadline.saturating_duration_since(Instant::now());
        if publish && (cooling_down || remaining < lane.publish_timeout + PUBLISH_FAILURE_RESERVE) {
            return Ok(false);
        }
        let budget = if publish {
            lane.publish_timeout
        } else {
            lane.step_timeout
        };
        let timeout = budget.min(remaining);
        if timeout.is_zero() {
            return Ok(false);
        }
        let rows = lane.rows.load(Ordering::Relaxed);
        let mut started = false;
        let result: anyhow::Result<RebuildStep> = tokio::time::timeout(timeout, async {
            let mut tx = pool.begin().await?;
            sqlx::query(
                "SELECT set_config('statement_timeout', $1, true), set_config('lock_timeout', $2, true)",
            )
            .bind(milliseconds(timeout))
            .bind(milliseconds(timeout.min(LOCK_TIMEOUT)))
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "SELECT CASE WHEN current_setting('server_version_num')::int >= 170000
                     THEN set_config('transaction_timeout', $1, true) END",
            )
            .bind(milliseconds(timeout))
            .execute(&mut *tx)
            .await?;
            owner.check(&mut tx).await?;
            started = true;
            let step = rebuild_batch(&mut tx, net, rows).await?;
            tx.commit().await?;
            Ok(step)
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(std::convert::identity);
        match result {
            Ok(RebuildStep::Finished) => {
                lane.rows.store(REBUILD_ROWS_PER_STEP, Ordering::Relaxed);
                return Ok(true);
            }
            Ok(RebuildStep::Idle) => return Ok(true),
            Ok(RebuildStep::Batched) => {}
            Err(error) if started && timed_out(&error) => {
                if publish {
                    defer_publish(pool, net, progress, deadline).await?;
                    return Err(error);
                }
                if timeout < budget {
                    return Ok(false);
                }
                if rows == 1 {
                    return Err(error);
                }
                lane.rows.store(rows / 2, Ordering::Relaxed);
            }
            Err(error) => return Err(error),
        }
    }
}
