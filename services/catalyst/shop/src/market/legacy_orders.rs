//! The legacy Marketplace order book (`shop.legacy_order`): one row per
//! order ever created on the Marketplace contracts the nft walk takes in
//! (Ethereum, and Polygon's two), folded from their order logs in
//! `shop.nft_event` with the rules both subgraphs applied (the collections
//! graph's `marketplace.ts`/`marketplaceV2.ts`, the marketplace graph's
//! `marketplace.ts`):
//!
//! - `OrderCreated` opens an order, keyed by its marketplace and id; the
//!   latest creation of an id is the order.
//! - The `OrderSuccessful` naming the id marks it `sold`, with the buyer and
//!   the price paid; the `OrderCancelled` naming it marks it `cancelled`.
//! - A later `OrderCreated` on the same token, on either Marketplace of the
//!   network, cancels it (the subgraphs' `cancelActiveOrder`) unless one of
//!   those follows: the contract keeps one order per asset and a marketplace,
//!   and the graphs one active order per token.
//!
//! What the graphs folded from transfers (`handleTransferOrder`: an open
//! order whose token leaves its seller reads `transferred`, and `open` again
//! when it comes back) is not folded here: the reader derives it from the
//! token's current owner on its own snapshot (`orders::book_status`), and
//! reads an open order whose ownership that source does not prove (another
//! owner, none, no row) as `transferred`.
//!
//! The incremental fold runs in the NFT fold's transaction. It catches up
//! from the older of the NFT and book markers and consumes repairs queued
//! for the live book. An initial load, a lost book checkpoint, or a rebuild
//! of the NFT fold opens a private build instead. The NFT walk runs it in
//! resumable steps alongside ordinary publication. Older binaries see the
//! same live table and marker throughout the build.
//!
//! A reader requires the book's checkpoint hash, an empty live repair
//! queue, and the NFT publication on the same snapshot ([`caught_up`]).

#[path = "legacy_order_rebuild.rs"]
mod rebuild;

pub use rebuild::run_rebuild;
#[cfg(test)]
pub use rebuild::{RebuildStep, rebuild_batch};

use sqlx::PgConnection;

use super::chain_walk::ChainNet;

/// The fold of one network (`$1`): the tokens with an order event past
/// `$2`, and those queued because a writer changed an order event at or
/// below the book's marker (`shop.legacy_order_refold`, filled by a trigger
/// on `shop.nft_event` for every writer and consumed here), every order of
/// each refolded over the token's history. An unchanged order is left alone. The event that ends an order is one
/// backward probe of `nft_event_token_idx` per order (`LATERAL ... LIMIT
/// 1`), not a join of two derived sets: the planner estimates those at one
/// row each, and as a join it rescanned every order per order (a rebuild of
/// 300,000 orders did not finish in ten minutes).
const FOLD_SQL: &str = "
WITH queued AS (
    DELETE FROM shop.legacy_order_refold WHERE network = $1 RETURNING contract_address, token_id
), touched AS (
    SELECT contract_address, token_id FROM shop.nft_event
    WHERE network = $1 AND block_number > $2 AND kind IN ('order_created', 'order_successful', 'order_cancelled')
    UNION
    SELECT contract_address, token_id FROM queued
), created AS (
    SELECT DISTINCT ON (c.network, c.emitter, c.detail) c.*
    FROM (
        SELECT e.network, e.contract_address, e.token_id, e.block_number, e.log_index, e.block_time, e.emitter, e.detail,
               e.from_address, e.amount, e.expires_at,
               lead(e.block_number) OVER w AS next_block, lead(e.log_index) OVER w AS next_log, lead(e.block_time) OVER w AS next_time
        FROM shop.nft_event e JOIN touched k ON (k.contract_address, k.token_id) = (e.contract_address, e.token_id)
        WHERE e.network = $1 AND e.kind = 'order_created' AND e.detail IS NOT NULL
        WINDOW w AS (PARTITION BY e.contract_address, e.token_id ORDER BY e.block_number, e.log_index)
    ) c
    ORDER BY c.network, c.emitter, c.detail, c.block_number DESC, c.log_index DESC
)
INSERT INTO shop.legacy_order (network, marketplace_address, order_id, contract_address, token_id, seller, buyer, price, expires_at, status,
    created_block, created_log, created_at, updated_block, updated_log, updated_at)
SELECT c.network, c.emitter, c.detail, c.contract_address, c.token_id,
       COALESCE(c.from_address, $3),
       CASE WHEN t.kind = 'order_successful' THEN t.to_address END,
       CASE WHEN t.kind = 'order_successful' THEN COALESCE(t.amount, c.amount, 0) ELSE COALESCE(c.amount, 0) END,
       COALESCE(c.expires_at, 0),
       CASE WHEN t.kind = 'order_successful' THEN 'sold' WHEN t.kind = 'order_cancelled' OR c.next_block IS NOT NULL THEN 'cancelled' ELSE 'open' END,
       c.block_number, c.log_index, c.block_time,
       COALESCE(t.block_number, c.next_block, c.block_number), COALESCE(t.log_index, c.next_log, c.log_index), COALESCE(t.block_time, c.next_time, c.block_time)
FROM created c
LEFT JOIN LATERAL (
    SELECT t.kind, t.to_address, t.amount, t.block_number, t.log_index, t.block_time
    FROM shop.nft_event t
    WHERE t.network = c.network AND t.contract_address = c.contract_address AND t.token_id = c.token_id
      AND t.kind IN ('order_successful', 'order_cancelled') AND t.emitter = c.emitter AND t.detail = c.detail
      AND (t.block_number, t.log_index) > (c.block_number, c.log_index)
    ORDER BY t.block_number DESC, t.log_index DESC
    LIMIT 1
) t ON true
ON CONFLICT (network, marketplace_address, order_id) DO UPDATE SET
    contract_address = EXCLUDED.contract_address, token_id = EXCLUDED.token_id, seller = EXCLUDED.seller,
    buyer = EXCLUDED.buyer, price = EXCLUDED.price, expires_at = EXCLUDED.expires_at, status = EXCLUDED.status,
    created_block = EXCLUDED.created_block, created_log = EXCLUDED.created_log, created_at = EXCLUDED.created_at,
    updated_block = EXCLUDED.updated_block, updated_log = EXCLUDED.updated_log, updated_at = EXCLUDED.updated_at
WHERE (legacy_order.contract_address, legacy_order.token_id, legacy_order.seller, legacy_order.buyer, legacy_order.price,
       legacy_order.expires_at, legacy_order.status, legacy_order.created_block, legacy_order.created_log, legacy_order.created_at,
       legacy_order.updated_block, legacy_order.updated_log, legacy_order.updated_at)
    IS DISTINCT FROM (EXCLUDED.contract_address, EXCLUDED.token_id, EXCLUDED.seller, EXCLUDED.buyer, EXCLUDED.price,
       EXCLUDED.expires_at, EXCLUDED.status, EXCLUDED.created_block, EXCLUDED.created_log, EXCLUDED.created_at,
       EXCLUDED.updated_block, EXCLUDED.updated_log, EXCLUDED.updated_at)";

/// Whether `net`'s book stands on `conn`'s snapshot: its marker names a
/// recovery point of the nft walk that still has the hash it recorded.
const STANDS_SQL: &str = "SELECT b.folded_through FROM shop.legacy_order_cursor b \
     JOIN shop.chain_checkpoint k ON k.indexer = $2 AND k.network = b.network \
         AND k.block_number = b.folded_through AND k.block_hash = b.through_hash \
     WHERE b.network = $1";

// Read only a small prefix of the primary key. An over-budget history can
// be empty of orders, but the lane is where we prove it in that case.
const CHEAP_EMPTY_SQL: &str = "
SELECT count(*) < 513
       AND NOT coalesce(bool_or(kind IN ('order_created', 'order_successful', 'order_cancelled')), false)
       AND (SELECT network FROM shop.legacy_order WHERE network = $1
            ORDER BY marketplace_address, order_id LIMIT 1) IS NULL
       AND (SELECT network FROM shop.legacy_order_refold WHERE network = $1
            ORDER BY contract_address, token_id LIMIT 1) IS NULL
FROM (
    SELECT kind FROM shop.nft_event WHERE network = $1
    ORDER BY block_number, log_index LIMIT 513
) prefix";

async fn building(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.legacy_order_rebuild WHERE network = $1 AND phase <> 'cleanup')",
    )
    .bind(net.db_name())
    .fetch_one(conn)
    .await
}

/// Folds `net`'s orders on the nft fold's transaction while the book stands:
/// the orders touched past the lower of the nft fold's `since` and the
/// book's marker, and the queued refolds, then the nft walk's last block and
/// hash as the marker; the rows written. It never refolds the book whole in
/// the nft fold's transaction: a book that does not stand (never folded,
/// rewound, on a branch the nft walk no longer has) or a rebuild of the nft
/// index (`since = -1`) opens a rebuild instead, which
/// the nft walk's pass runs a batch at a time ([`rebuild::rebuild_batch`]); while one
/// is open this folds nothing.
pub async fn fold(conn: &mut PgConnection, net: ChainNet, since: i64) -> sqlx::Result<u64> {
    if building(conn, net).await? {
        return Ok(0);
    }
    let stands: Option<i64> = sqlx::query_scalar(STANDS_SQL)
        .bind(net.db_name())
        .bind(super::chain_walk::INDEXER_NFT)
        .fetch_optional(&mut *conn)
        .await?;
    let since = match stands {
        Some(through) if since >= 0 => since.min(through),
        _ => {
            let empty: bool = sqlx::query_scalar(CHEAP_EMPTY_SQL)
                .bind(net.db_name())
                .fetch_one(&mut *conn)
                .await?;
            if empty {
                write_marker(conn, net).await?;
            } else if rebuild::open(conn, net).await? {
                tracing::info!(network = net.db_name(), "legacy order book rebuild opened");
            }
            return Ok(0);
        }
    };
    let written = sqlx::query(FOLD_SQL)
        .bind(net.db_name())
        .bind(since)
        .bind(super::nft_index::ZERO_ADDRESS)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    write_marker(conn, net).await?;
    Ok(written)
}

/// The book's marker: the nft walk's last block and its hash.
async fn write_marker(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO shop.legacy_order_cursor (network, folded_through, through_hash) \
         SELECT network, last_block, last_block_hash FROM shop.chain_cursor WHERE indexer = $2 AND network = $1 \
         ON CONFLICT (network) DO UPDATE SET folded_through = EXCLUDED.folded_through, through_hash = EXCLUDED.through_hash",
    )
    .bind(net.db_name())
    .bind(super::chain_walk::INDEXER_NFT)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Whether `net`'s order book may be read beside the published nft fold on
/// `conn`'s snapshot: it stands (its marker's recovery point keeps the hash
/// it recorded) and its marker, written in the fold's transaction, is the
/// nft cursor's published `folded_block`. False while the book has not
/// caught up with nft folds that ran without it, when an earlier binary
/// rewound the walk under it, and between a fold's commit and its
/// publication: the rows then already hold the next fold, which is not
/// published yet (the walk publishes only after its head reads, which can
/// stall). A walk that advanced without folding leaves the book at the
/// published block, which stays readable. Also false while `net` has a
/// refold queued (`shop.legacy_order_refold`): a writer changed an order
/// event the book has already folded, and an earlier binary can republish
/// the same height without folding it, so the book is read again only once
/// a fold of this binary has taken it. Another network's queue does not
/// withhold this one.
pub async fn caught_up(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<bool> {
    if building(conn, net).await? {
        return Ok(false);
    }
    let stands: Option<i64> = sqlx::query_scalar(STANDS_SQL)
        .bind(net.db_name())
        .bind(super::chain_walk::INDEXER_NFT)
        .fetch_optional(&mut *conn)
        .await?;
    let Some(through) = stands else {
        return Ok(false);
    };
    published(conn, net, through).await
}

async fn published(conn: &mut PgConnection, net: ChainNet, through: i64) -> sqlx::Result<bool> {
    let queued: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.legacy_order_refold WHERE network = $1)",
    )
    .bind(net.db_name())
    .fetch_one(&mut *conn)
    .await?;
    if queued {
        return Ok(false);
    }
    super::chain_walk::holds_published_fold_on(
        conn,
        super::chain_walk::INDEXER_NFT,
        net,
        Some(through),
    )
    .await
}

/// Deletes `net`'s orders of the tokens (`contracts[i]`, `tokens[i]`) a
/// rewind to `to` deleted events of, and lowers the book's marker to `to`
/// with no hash, in the rewind's transaction: the book does not stand until
/// a new private build covers the surviving branch.
pub async fn rewind(
    conn: &mut PgConnection,
    net: ChainNet,
    to: i64,
    contracts: &[String],
    tokens: &[String],
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE shop.legacy_order_cursor SET folded_through = LEAST(folded_through, $2), through_hash = NULL WHERE network = $1",
    )
    .bind(net.db_name())
    .bind(to)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE shop.legacy_order_rebuild SET anchor_hash = '' WHERE network = $1 AND phase <> 'cleanup'",
    )
    .bind(net.db_name())
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "DELETE FROM shop.legacy_order o USING UNNEST($2::text[], $3::text[]) AS g(c, t) \
         WHERE o.network = $1 AND o.contract_address = g.c AND o.token_id = g.t::numeric",
    )
    .bind(net.db_name())
    .bind(contracts)
    .bind(tokens)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::chain_walk::{self, ChainIndex};
    use crate::market::nft_index::{self, NftIndex};
    use sqlx::PgPool;

    const C: &str = "0x00000000000000000000000000000000000000c1";
    const SELLER: &str = "0xaaaa000000000000000000000000000000000001";
    const OTHER: &str = "0xaaaa000000000000000000000000000000000002";
    const BUYER: &str = "0xbbbb000000000000000000000000000000000003";

    fn markets() -> (&'static str, &'static str) {
        let m = nft_index::deployments::Deployments::mainnet(ChainNet::Matic).markets;
        (m[0].0, m[1].0)
    }

    fn id(n: u8) -> String {
        format!("0x{}", hex::encode([n; 32]))
    }

    /// One order log of `kind` on `market` at block `block`: an
    /// `order_created` by `seller` at `price`, an `order_successful` to
    /// `buyer` at `price`, an `order_cancelled`.
    #[allow(clippy::too_many_arguments, reason = "one argument per column")]
    async fn order_event(
        pool: &PgPool,
        block: i64,
        kind: &str,
        market: &str,
        token: i64,
        order: u8,
        seller: &str,
        buyer: Option<&str>,
        price: Option<i64>,
    ) {
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, expires_at, detail) \
             VALUES ('matic', $1, 0, $1 * 10, $2, $3, $4, $5::numeric, $6, $7, $8::numeric, CASE WHEN $2 = 'order_created' THEN 4000000000000 END, $9)",
        )
        .bind(block)
        .bind(kind)
        .bind(market)
        .bind(C)
        .bind(token.to_string())
        .bind(seller)
        .bind(buyer)
        .bind(price.map(|p| p.to_string()))
        .bind(id(order))
        .execute(pool)
        .await
        .unwrap();
    }

    type Row = (String, String, Option<String>, String, i64, i64);

    /// (block, kind, market, token, order id byte, seller, buyer, price).
    type LogRow = (
        i64,
        &'static str,
        &'static str,
        i64,
        u8,
        &'static str,
        Option<&'static str>,
        Option<i64>,
    );

    /// Every order: (id, status, buyer, price, updated block, updated time).
    async fn book(pool: &PgPool) -> Vec<Row> {
        sqlx::query_as(
            "SELECT order_id, status, buyer, price::text, updated_block, updated_at FROM shop.legacy_order ORDER BY order_id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// Block `n`'s hash on branch `branch`.
    fn hash(branch: u8, n: i64) -> String {
        format!("0x{branch:02x}{n:062x}")
    }

    /// The Polygon nft cursor on branch `branch`: walked through `last`, its
    /// fold through `fold_through`, published at `folded`, with the
    /// recovery points of `last` and `fold_through`.
    async fn nft_on(
        pool: &PgPool,
        branch: u8,
        last: i64,
        fold_through: Option<i64>,
        folded: Option<i64>,
    ) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, last_block_hash, fold_through, folded_block, folded_at, full_history, chain_id) \
             VALUES ('nft', 'matic', 1, $1, $4, $2, $3, CASE WHEN $3 IS NOT NULL THEN timezone('UTC', now()) END, true, 137) \
             ON CONFLICT (indexer, network) DO UPDATE SET last_block = $1, last_block_hash = $4, fold_through = $2, folded_block = $3, folded_at = EXCLUDED.folded_at",
        )
        .bind(last)
        .bind(fold_through)
        .bind(folded)
        .bind(hash(branch, last))
        .execute(pool)
        .await
        .unwrap();
        for block in [Some(last), fold_through].into_iter().flatten() {
            sqlx::query(
                "INSERT INTO shop.chain_checkpoint (indexer, network, block_number, block_hash, block_time) VALUES ('nft', 'matic', $1, $2, $1) \
                 ON CONFLICT (indexer, network, block_number) DO UPDATE SET block_hash = EXCLUDED.block_hash",
            )
            .bind(block)
            .bind(hash(branch, block))
            .execute(pool)
            .await
            .unwrap();
        }
    }

    async fn nft_cursor(pool: &PgPool, last: i64, fold_through: Option<i64>, folded: Option<i64>) {
        nft_on(pool, 0, last, fold_through, folded).await;
    }

    async fn stands(pool: &PgPool) -> bool {
        let mut tx = pool.begin().await.unwrap();
        let caught = caught_up(&mut tx, ChainNet::Matic).await.unwrap();
        tx.rollback().await.unwrap();
        caught
    }

    async fn book_through(pool: &PgPool) -> Option<i64> {
        sqlx::query_scalar(
            "SELECT folded_through FROM shop.legacy_order_cursor WHERE network = 'matic'",
        )
        .fetch_optional(pool)
        .await
        .unwrap()
    }

    /// The nft fold's book fold, then any rebuild it opened run to the end,
    /// a batch a transaction (one contract a batch, so every test crosses
    /// batches), as the nft walk's pass runs it.
    async fn fold_matic(pool: &PgPool, since: i64) -> u64 {
        let mut tx = pool.begin().await.unwrap();
        let n = fold(&mut tx, ChainNet::Matic, since).await.unwrap();
        tx.commit().await.unwrap();
        finish_rebuild(pool).await;
        n
    }

    async fn finish_rebuild(pool: &PgPool) -> usize {
        let mut batches = 0;
        loop {
            let mut tx = pool.begin().await.unwrap();
            let step = rebuild_batch(&mut tx, ChainNet::Matic, 2).await.unwrap();
            tx.commit().await.unwrap();
            match step {
                RebuildStep::Batched => batches += 1,
                RebuildStep::Idle | RebuildStep::Finished => return batches,
            }
        }
    }

    /// The log of the rules: an order a later one on its token supersedes
    /// (`a`), the superseding one sold (`b`), an untouched open one (`c`),
    /// one cancelled by id (`d`), and one superseded on the other
    /// Marketplace and then sold on its own (`e`, with `f` left open).
    async fn seed_log(pool: &PgPool, upto: i64) {
        let (m1, m2) = markets();
        let rows: Vec<LogRow> = vec![
            (10, "order_created", m1, 1, 0xa, SELLER, None, Some(100)),
            (11, "order_created", m1, 2, 0xc, OTHER, None, Some(300)),
            (12, "order_created", m1, 3, 0xd, SELLER, None, Some(400)),
            (13, "order_cancelled", m1, 3, 0xd, SELLER, None, None),
            (20, "order_created", m2, 1, 0xb, SELLER, None, Some(200)),
            (
                30,
                "order_successful",
                m2,
                1,
                0xb,
                SELLER,
                Some(BUYER),
                Some(210),
            ),
            (40, "order_created", m1, 4, 0xe, SELLER, None, Some(500)),
            (41, "order_created", m2, 4, 0xf, SELLER, None, Some(600)),
            (
                42,
                "order_successful",
                m1,
                4,
                0xe,
                SELLER,
                Some(BUYER),
                Some(500),
            ),
        ];
        for (block, kind, market, token, order, seller, buyer, price) in rows {
            if block <= upto {
                order_event(
                    pool, block, kind, market, token, order, seller, buyer, price,
                )
                .await;
            }
        }
    }

    fn expected() -> Vec<Row> {
        let row = |n: u8, status: &str, buyer: Option<&str>, price: &str, block: i64| {
            (
                id(n),
                status.to_string(),
                buyer.map(str::to_string),
                price.to_string(),
                block,
                block * 10,
            )
        };
        vec![
            row(0xa, "cancelled", None, "100", 20),
            row(0xb, "sold", Some(BUYER), "210", 30),
            row(0xc, "open", None, "300", 11),
            row(0xd, "cancelled", None, "400", 13),
            row(0xe, "sold", Some(BUYER), "500", 42),
            row(0xf, "open", None, "600", 41),
        ]
    }

    #[tokio::test]
    async fn the_fold_keeps_every_order_with_the_subgraph_rules_incrementally_and_whole() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders").await else {
            return;
        };
        // A first fold over the log up to block 20, then the rest past it.
        seed_log(&db.pool, 20).await;
        nft_cursor(&db.pool, 20, None, None).await;
        fold_matic(&db.pool, -1).await;
        assert_eq!(
            book_through(&db.pool).await,
            Some(20),
            "the walk's last block"
        );
        let first = book(&db.pool).await;
        assert_eq!(
            first
                .iter()
                .map(|r| (r.0.clone(), r.1.clone()))
                .collect::<Vec<_>>(),
            vec![
                (id(0xa), "cancelled".to_string()),
                (id(0xb), "open".to_string()),
                (id(0xc), "open".to_string()),
                (id(0xd), "cancelled".to_string()),
            ]
        );
        sqlx::query("DELETE FROM shop.nft_event")
            .execute(&db.pool)
            .await
            .unwrap();
        seed_log(&db.pool, 100).await;
        nft_cursor(&db.pool, 100, Some(20), Some(20)).await;
        let written = fold_matic(&db.pool, 20).await;
        assert_eq!(book(&db.pool).await, expected());
        assert_eq!(
            written, 3,
            "the touched tokens' changed orders only: b sold, e and f new"
        );
        assert_eq!(
            fold_matic(&db.pool, 20).await,
            0,
            "an unchanged order is left alone"
        );
        // A rebuild reaches the same book.
        sqlx::query("DELETE FROM shop.legacy_order")
            .execute(&db.pool)
            .await
            .unwrap();
        fold_matic(&db.pool, -1).await;
        assert_eq!(book(&db.pool).await, expected());
        db.drop().await;
    }

    /// The upgrade: the table arrives empty and with no marker over a log
    /// the nft fold has folded and published through 100. The book does not
    /// stand, so no reader takes it; the next fold, an incremental one by
    /// the nft fold's own marker, refolds it whole and it stands.
    #[tokio::test]
    async fn an_upgraded_database_reads_no_book_until_its_first_fold_refolds_it() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_upgrade").await
        else {
            return;
        };
        seed_log(&db.pool, 100).await;
        nft_cursor(&db.pool, 100, Some(100), Some(100)).await;
        assert_eq!(book(&db.pool).await, Vec::new());
        assert_eq!(book_through(&db.pool).await, None);
        assert!(!stands(&db.pool).await, "no marker: not read");
        fold_matic(&db.pool, 100).await;
        assert_eq!(book(&db.pool).await, expected(), "refolded whole");
        assert_eq!(book_through(&db.pool).await, Some(100));
        assert!(stands(&db.pool).await);
        assert_eq!(
            fold_matic(&db.pool, 100).await,
            0,
            "an incremental fold over the standing book changes nothing"
        );
        db.drop().await;
    }

    /// The nft fold never refolds the book: over a book that does not stand
    /// it opens a rebuild and writes nothing, so the nft index publishes
    /// while the book is refused. The rebuild then runs a batch of contracts
    /// a transaction (here one, over two contracts), and a sale a repair
    /// writes below the open rebuild's block meanwhile is queued by the
    /// trigger and taken by the last step; the book then stands.
    #[tokio::test]
    async fn a_rebuild_runs_outside_the_nft_fold_a_batch_at_a_time() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_batches").await
        else {
            return;
        };
        seed_log(&db.pool, 41).await;
        // Include a second contract in the same block as an order for the first.
        sqlx::query(
            "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount, expires_at, detail) \
             SELECT network, block_number, log_index + 1, block_time, kind, emitter, '0x00000000000000000000000000000000000000c2', token_id, from_address, to_address, amount, expires_at, \
                    '0x' || repeat('f', 64) \
             FROM shop.nft_event WHERE contract_address = $1 AND kind = 'order_created' AND detail = $2",
        )
        .bind(C)
        .bind(id(0xc))
        .execute(&db.pool)
        .await
        .unwrap();
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        let mut tx = db.pool.begin().await.unwrap();
        assert_eq!(fold(&mut tx, ChainNet::Matic, 50).await.unwrap(), 0);
        tx.commit().await.unwrap();
        assert_eq!(
            book(&db.pool).await,
            Vec::new(),
            "the nft fold wrote no order"
        );
        assert!(!stands(&db.pool).await);
        let open: (i64, String, i64) = sqlx::query_as(
            "SELECT upto, upto_hash, scan_block FROM shop.legacy_order_rebuild WHERE network = 'matic'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(open, (50, hash(0, 50), -1));
        assert!(
            NftIndex::new(ChainNet::Matic)
                .walks_alone(&db.pool)
                .await
                .unwrap(),
            "an open rebuild walks alone, so a shared pass never skips its lane"
        );
        // Another nft fold while it is open folds nothing.
        let mut tx = db.pool.begin().await.unwrap();
        assert_eq!(fold(&mut tx, ChainNet::Matic, 50).await.unwrap(), 0);
        tx.commit().await.unwrap();
        // Materialize the initial input, then repair e's missed sale at 42.
        while build_phase(&db.pool).await != "tail" {
            assert_eq!(step_matic(&db.pool, 1).await, RebuildStep::Batched);
        }
        assert!(!stands(&db.pool).await, "still rebuilding");
        let before: String =
            sqlx::query_scalar("SELECT status FROM shop.legacy_order_build WHERE order_id = $1")
                .bind(id(0xe))
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_ne!(before, "sold");
        let (m1, _) = markets();
        let mut tx = db.pool.begin().await.unwrap();
        nft_index::insert_events(
            &mut tx,
            ChainNet::Matic,
            &[nft_index::NftEvent {
                block_number: 42,
                log_index: 0,
                block_time: 420,
                kind: "order_successful",
                emitter: m1.to_string(),
                contract_address: C.to_string(),
                token_id: "4".to_string(),
                from_address: Some(SELLER.to_string()),
                to_address: Some(BUYER.to_string()),
                amount: Some("500".to_string()),
                expires_at: None,
                detail: Some(id(0xe)),
                tx_hash: Some(format!("0x{:064x}", 42)),
            }],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let queued: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM shop.legacy_order_build_dirty WHERE network = 'matic'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(queued, 1, "the repair lies behind the shadow scan");
        while build_phase(&db.pool).await != "repair_build" {
            assert_eq!(step_matic(&db.pool, 2).await, RebuildStep::Batched);
        }
        // Even if another writer drains the live queue, a revision made
        // after this replay read the token forces a second replay.
        sqlx::query(
            "UPDATE shop.nft_event SET amount = 501 WHERE network = 'matic' AND block_number = 42",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        finish_rebuild(&db.pool).await;
        let price: String =
            sqlx::query_scalar("SELECT price::text FROM shop.legacy_order WHERE order_id = $1")
                .bind(id(0xe))
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(price, "501");
        let rows = book(&db.pool).await;
        let mut repaired = expected()[4].clone();
        repaired.3 = "501".into();
        assert!(rows.contains(&repaired), "the repaired sale: {rows:?}");
        assert_eq!(rows.len(), expected().len() + 1, "both contracts' orders");
        assert_eq!(book_through(&db.pool).await, Some(50));
        assert!(stands(&db.pool).await);
        assert!(
            !NftIndex::new(ChainNet::Matic)
                .walks_alone(&db.pool)
                .await
                .unwrap(),
            "back in the shared group once finished"
        );
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.legacy_order_rebuild")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(left, 0);
        db.drop().await;
    }

    /// The auditor's case: a rebuild has already read past a token (an order
    /// created at 10, sold at 40) when a rewind to 30 deletes the sale and the
    /// token's orders. The rewind queues the token, so the rebuild's last
    /// step refolds its surviving history: the order is back, open.
    #[tokio::test]
    async fn a_rewind_during_a_rebuild_keeps_the_surviving_orders_of_its_tokens() {
        let Some(db) =
            crate::market::tests::support::OwnDb::create("legacy_orders_rewind_open").await
        else {
            return;
        };
        let (m1, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            m1,
            1,
            0xa,
            SELLER,
            None,
            Some(100),
        )
        .await;
        order_event(
            &db.pool,
            40,
            "order_successful",
            m1,
            1,
            0xa,
            SELLER,
            Some(BUYER),
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        let mut tx = db.pool.begin().await.unwrap();
        assert_eq!(fold(&mut tx, ChainNet::Matic, 50).await.unwrap(), 0);
        tx.commit().await.unwrap();
        // Commit the input and a materialized sale to the private build.
        let mut tx = db.pool.begin().await.unwrap();
        for _ in 0..3 {
            assert_eq!(
                rebuild_batch(&mut tx, ChainNet::Matic, 100).await.unwrap(),
                RebuildStep::Batched
            );
        }
        tx.commit().await.unwrap();
        let status = |rows: Vec<Row>| rows.into_iter().map(|r| r.1).collect::<Vec<_>>();
        let shadow: String = sqlx::query_scalar("SELECT status FROM shop.legacy_order_build")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(shadow, "sold");
        assert!(book(&db.pool).await.is_empty());
        // A rewind to 30: the sale goes, and with it the token's orders.
        sqlx::query("DELETE FROM shop.nft_event WHERE network = 'matic' AND block_number > 30")
            .execute(&db.pool)
            .await
            .unwrap();
        let mut tx = db.pool.begin().await.unwrap();
        rewind(
            &mut tx,
            ChainNet::Matic,
            30,
            &[C.to_string()],
            &["1".to_string()],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(book(&db.pool).await, Vec::new());
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        finish_rebuild(&db.pool).await;
        assert_eq!(
            status(book(&db.pool).await),
            vec!["open"],
            "the order created at 10 survives"
        );
        assert!(stands(&db.pool).await);
        db.drop().await;
    }

    async fn step_matic(pool: &PgPool, rows: i64) -> RebuildStep {
        let mut tx = pool.begin().await.unwrap();
        let step = rebuild_batch(&mut tx, ChainNet::Matic, rows).await.unwrap();
        tx.commit().await.unwrap();
        step
    }

    async fn open_matic(pool: &PgPool) {
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(fold(&mut tx, ChainNet::Matic, -1).await.unwrap(), 0);
        tx.commit().await.unwrap();
    }

    async fn build_phase(pool: &PgPool) -> String {
        sqlx::query_scalar("SELECT phase FROM shop.legacy_order_rebuild WHERE network = 'matic'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn an_earlier_writer_rewind_discards_an_open_build_on_the_lost_branch() {
        let Some(db) =
            crate::market::tests::support::OwnDb::create("legacy_orders_old_rewind").await
        else {
            return;
        };
        let (market, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            market,
            1,
            1,
            SELLER,
            None,
            Some(100),
        )
        .await;
        order_event(
            &db.pool,
            40,
            "order_successful",
            market,
            1,
            1,
            SELLER,
            Some(BUYER),
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        open_matic(&db.pool).await;
        for _ in 0..3 {
            assert_eq!(step_matic(&db.pool, 4).await, RebuildStep::Batched);
        }
        let sold: Option<String> = sqlx::query_scalar("SELECT buyer FROM shop.legacy_order_build")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(sold.as_deref(), Some(BUYER));

        // An older writer never calls legacy_orders::rewind or writes rebuild progress.
        sqlx::query("DELETE FROM shop.nft_event WHERE network = 'matic' AND block_number > 30")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.chain_checkpoint WHERE indexer = 'nft' AND network = 'matic' AND block_number > 30")
            .execute(&db.pool).await.unwrap();
        nft_on(&db.pool, 1, 50, Some(50), Some(50)).await;
        assert_eq!(step_matic(&db.pool, 4).await, RebuildStep::Batched);
        assert_eq!(build_phase(&db.pool).await, "reset");
        let opening: String = sqlx::query_scalar("SELECT upto_hash FROM shop.legacy_order_rebuild")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(opening, hash(1, 50));
        finish_rebuild(&db.pool).await;
        assert_eq!(book(&db.pool).await[0].1, "open");
        assert!(stands(&db.pool).await);
        db.drop().await;
    }

    #[tokio::test]
    async fn checkpoint_pruning_keeps_a_build_on_the_same_branch() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_prune").await
        else {
            return;
        };
        let (market, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            market,
            1,
            1,
            SELLER,
            None,
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        open_matic(&db.pool).await;
        assert_eq!(step_matic(&db.pool, 1).await, RebuildStep::Batched);

        for block in 51..=114 {
            nft_cursor(&db.pool, block, Some(block), Some(block)).await;
        }
        sqlx::query("DELETE FROM shop.chain_checkpoint WHERE indexer = 'nft' AND network = 'matic' AND block_number <= 50")
            .execute(&db.pool).await.unwrap();
        assert_eq!(step_matic(&db.pool, 1).await, RebuildStep::Batched);
        assert_eq!(build_phase(&db.pool).await, "build");
        let anchor: (i64, i64) =
            sqlx::query_as("SELECT upto, anchor_block FROM shop.legacy_order_rebuild")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(anchor, (50, 114));
        finish_rebuild(&db.pool).await;
        assert_eq!(book_through(&db.pool).await, Some(114));
        assert_eq!(book(&db.pool).await.len(), 1);
        db.drop().await;
    }

    #[tokio::test]
    async fn a_deployed_writer_and_a_shadow_swap_publish_complete_reader_snapshots() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_handover").await
        else {
            return;
        };
        let (market, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            market,
            1,
            1,
            SELLER,
            None,
            Some(100),
        )
        .await;
        order_event(
            &db.pool,
            15,
            "order_created",
            market,
            2,
            2,
            SELLER,
            None,
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        open_matic(&db.pool).await;
        assert_eq!(step_matic(&db.pool, 1).await, RebuildStep::Batched);

        // The deployed full fold knows only the live table and marker.
        let mut old = db.pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM shop.chain_cursor WHERE indexer = 'nft' AND network = 'matic' FOR UPDATE")
            .execute(&mut *old).await.unwrap();
        sqlx::query("DELETE FROM shop.legacy_order WHERE network = 'matic'")
            .execute(&mut *old)
            .await
            .unwrap();
        sqlx::query(FOLD_SQL)
            .bind("matic")
            .bind(-1_i64)
            .bind(nft_index::ZERO_ADDRESS)
            .execute(&mut *old)
            .await
            .unwrap();
        write_marker(&mut old, ChainNet::Matic).await.unwrap();
        old.commit().await.unwrap();
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;

        let mut reader = db.pool.begin().await.unwrap();
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *reader)
            .await
            .unwrap();
        let marker: i64 = sqlx::query_scalar(STANDS_SQL)
            .bind("matic")
            .bind("nft")
            .fetch_one(&mut *reader)
            .await
            .unwrap();
        assert_eq!(marker, 50);
        assert!(
            published(&mut reader, ChainNet::Matic, marker)
                .await
                .unwrap()
        );
        assert!(
            chain_walk::rows_published_on(
                &mut reader,
                "nft",
                ChainNet::Matic,
                &["nft", "legacy_order"]
            )
            .await
            .unwrap()
        );
        assert_eq!(book(&db.pool).await.len(), 2);

        // The newer walk gets ahead of that publication before the lane
        // resumes, so a swap rather than retirement is needed.
        order_event(
            &db.pool,
            51,
            "order_created",
            market,
            3,
            3,
            SELLER,
            None,
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 51, Some(51), Some(51)).await;
        assert_eq!(step_matic(&db.pool, 1).await, RebuildStep::Batched);
        assert_eq!(book(&db.pool).await.len(), 2);
        finish_rebuild(&db.pool).await;
        assert_eq!(book(&db.pool).await.len(), 3);
        assert_eq!(book_through(&db.pool).await, Some(51));

        let still_old: i64 =
            sqlx::query_scalar("SELECT count(*) FROM shop.legacy_order WHERE network = 'matic'")
                .fetch_one(&mut *reader)
                .await
                .unwrap();
        assert_eq!(still_old, 2);
        assert!(
            published(&mut reader, ChainNet::Matic, marker)
                .await
                .unwrap()
        );
        reader.commit().await.unwrap();

        let mut fresh = db.pool.acquire().await.unwrap();
        assert!(
            !chain_walk::rows_published_on(
                &mut fresh,
                "nft",
                ChainNet::Matic,
                &["nft", "legacy_order"]
            )
            .await
            .unwrap()
        );
        drop(fresh);
        // The ordinary same-height NFT publication acknowledges the swap.
        nft_cursor(&db.pool, 51, Some(51), Some(51)).await;
        let mut fresh = db.pool.acquire().await.unwrap();
        assert!(
            chain_walk::rows_published_on(
                &mut fresh,
                "nft",
                ChainNet::Matic,
                &["nft", "legacy_order"]
            )
            .await
            .unwrap()
        );
        drop(fresh);
        db.drop().await;
    }

    #[tokio::test]
    async fn a_large_token_and_the_tail_both_advance_in_small_steps() {
        let Some(db) =
            crate::market::tests::support::OwnDb::create("legacy_orders_large_token").await
        else {
            return;
        };
        let (market, _) = markets();
        for n in 1..=20_u8 {
            order_event(
                &db.pool,
                i64::from(n),
                "order_created",
                market,
                1,
                n,
                SELLER,
                None,
                Some(100),
            )
            .await;
        }
        nft_cursor(&db.pool, 20, Some(20), Some(20)).await;
        open_matic(&db.pool).await;
        for n in 21..=40_u8 {
            order_event(
                &db.pool,
                i64::from(n),
                "order_created",
                market,
                1,
                n,
                SELLER,
                None,
                Some(100),
            )
            .await;
        }
        nft_cursor(&db.pool, 40, Some(40), Some(40)).await;

        let counts = || {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT
            (SELECT count(*) FROM shop.legacy_order_build_event),
            (SELECT count(*) FROM shop.legacy_order_build)",
            )
            .fetch_one(&db.pool)
        };
        let mut before = (0, 0);
        let mut steps = 0;
        loop {
            let step = step_matic(&db.pool, 2).await;
            let after = counts().await.unwrap();
            assert!(
                (after.0 - before.0).abs() <= 2,
                "raw work: {before:?} to {after:?}"
            );
            assert!(
                (after.1 - before.1).abs() <= 2,
                "order work: {before:?} to {after:?}"
            );
            before = after;
            steps += 1;
            assert!(steps < 200, "bounded replay must finish");
            if step != RebuildStep::Batched {
                assert_eq!(step, RebuildStep::Finished);
                break;
            }
        }
        let shadow_result = book(&db.pool).await;
        assert_eq!(shadow_result.len(), 40);
        let mut tx = db.pool.begin().await.unwrap();
        sqlx::query("DELETE FROM shop.legacy_order WHERE network = 'matic'")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query(FOLD_SQL)
            .bind("matic")
            .bind(-1_i64)
            .bind(nft_index::ZERO_ADDRESS)
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(book(&db.pool).await, shadow_result);
        db.drop().await;
    }

    #[tokio::test]
    async fn an_empty_book_is_only_published_from_a_bounded_probe() {
        let Some(db) =
            crate::market::tests::support::OwnDb::create("legacy_orders_empty_probe").await
        else {
            return;
        };
        nft_cursor(&db.pool, 1000, Some(1000), Some(1000)).await;
        open_matic(&db.pool).await;
        assert!(stands(&db.pool).await);
        let insert = "INSERT INTO shop.nft_event
            (network,block_number,log_index,kind,block_time,emitter,contract_address,token_id)
            SELECT 'matic', n, 0, 'transfer', n, $1, $1, 1 FROM generate_series($2::bigint, $3::bigint) n";
        sqlx::query(insert)
            .bind(C)
            .bind(1_i64)
            .bind(512_i64)
            .execute(&db.pool)
            .await
            .unwrap();
        open_matic(&db.pool).await;
        assert!(stands(&db.pool).await);
        sqlx::query(insert)
            .bind(C)
            .bind(513_i64)
            .bind(513_i64)
            .execute(&db.pool)
            .await
            .unwrap();
        open_matic(&db.pool).await;
        assert_eq!(build_phase(&db.pool).await, "scan");
        assert!(!stands(&db.pool).await);
        loop {
            if step_matic(&db.pool, 128).await != RebuildStep::Batched {
                break;
            }
        }
        assert!(stands(&db.pool).await);
        assert!(book(&db.pool).await.is_empty());
        db.drop().await;
    }

    #[tokio::test]
    async fn the_rebuild_deadline_includes_waiting_for_the_cursor_lock() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_lock").await
        else {
            return;
        };
        let (market, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            market,
            1,
            1,
            SELLER,
            None,
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        open_matic(&db.pool).await;
        let index = NftIndex::new(ChainNet::Matic);
        let owner =
            chain_walk::test_owner(&db.pool, &index, &chain_walk::IndexSettings::default()).await;
        let mut blocker = db.pool.begin().await.unwrap();
        owner.check(&mut blocker).await.unwrap();

        let start = std::time::Instant::now();
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            run_rebuild(
                &db.pool,
                ChainNet::Matic,
                &owner,
                start + std::time::Duration::from_millis(150),
            ),
        )
        .await
        .expect("lane releases the caller while the cursor is still held");
        assert!(outcome.is_err());
        assert_eq!(build_phase(&db.pool).await, "scan");
        blocker.rollback().await.unwrap();
        assert!(
            run_rebuild(
                &db.pool,
                ChainNet::Matic,
                &owner,
                start + std::time::Duration::from_secs(100)
            )
            .await
            .unwrap()
        );
        assert!(stands(&db.pool).await);
        db.drop().await;
    }

    #[tokio::test]
    async fn the_object_pass_installs_the_build_over_an_existing_book() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_upgrade").await
        else {
            return;
        };
        let (market, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            market,
            1,
            1,
            SELLER,
            None,
            Some(100),
        )
        .await;
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        fold_matic(&db.pool, -1).await;
        let live = book(&db.pool).await;
        assert_eq!(live.len(), 1);
        // The deployed layout has the live book and its cursor but none of
        // these objects. Simulate that layout in this isolated database.
        sqlx::raw_sql(
            "DROP TRIGGER nft_event_order_build_insert ON shop.nft_event;
             DROP TRIGGER nft_event_order_build_update ON shop.nft_event;
             DROP TRIGGER nft_event_order_build_delete ON shop.nft_event;
             DROP FUNCTION shop.capture_legacy_order_build_change();
             DROP FUNCTION shop.note_legacy_order_build_change(text,varchar,numeric,bigint,integer);
             DROP TABLE shop.legacy_order_rebuild, shop.legacy_order_build,
                        shop.legacy_order_build_event, shop.legacy_order_build_dirty;",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        crate::db::schema_migrate::migrate(
            &db.pool,
            crate::db::SCHEMA_SQL,
            crate::db::MIGRATIONS_SQL,
        )
        .await
        .unwrap();
        assert_eq!(book(&db.pool).await, live);
        assert_eq!(book_through(&db.pool).await, Some(50));
        open_matic(&db.pool).await;
        finish_rebuild(&db.pool).await;
        assert_eq!(book(&db.pool).await, live);
        db.drop().await;
    }

    /// A rollout where an earlier binary, which never writes the book,
    /// rewinds the nft walk to 90 and republishes another branch at the same
    /// height, without the order a sale closed at 97. The book's recovery
    /// point at 100 now has another hash: it does not stand, so no reader
    /// takes it beside the new ownership, and the next fold, an incremental
    /// one by its marker, deletes the network's orders and refolds them
    /// whole: the lost branch's order is gone and the book stands again. A
    /// republish of the same branch leaves it standing.
    #[tokio::test]
    async fn a_branch_an_earlier_binary_republished_at_the_same_height_drops_the_book() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_branch").await
        else {
            return;
        };
        let (m1, _) = markets();
        order_event(
            &db.pool,
            10,
            "order_created",
            m1,
            2,
            0xc,
            OTHER,
            None,
            Some(300),
        )
        .await;
        order_event(
            &db.pool,
            95,
            "order_created",
            m1,
            1,
            0xa,
            SELLER,
            None,
            Some(100),
        )
        .await;
        order_event(
            &db.pool,
            97,
            "order_successful",
            m1,
            1,
            0xa,
            SELLER,
            Some(BUYER),
            Some(100),
        )
        .await;
        nft_on(&db.pool, 0, 100, Some(100), Some(100)).await;
        fold_matic(&db.pool, -1).await;
        let ids = |rows: Vec<Row>| rows.into_iter().map(|r| (r.0, r.1)).collect::<Vec<_>>();
        assert_eq!(
            ids(book(&db.pool).await),
            vec![(id(0xa), "sold".to_string()), (id(0xc), "open".to_string())]
        );
        assert!(stands(&db.pool).await);
        // The same branch republished: the book stands.
        nft_on(&db.pool, 0, 100, Some(100), Some(100)).await;
        assert!(stands(&db.pool).await);
        // An earlier binary rewinds to 90 (its log and recovery points past
        // it go) and republishes another branch at 100.
        sqlx::query("DELETE FROM shop.nft_event WHERE network = 'matic' AND block_number > 90")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.chain_checkpoint WHERE indexer = 'nft' AND network = 'matic' AND block_number > 90")
            .execute(&db.pool)
            .await
            .unwrap();
        nft_on(&db.pool, 1, 100, Some(100), Some(100)).await;
        assert!(!stands(&db.pool).await, "same height, another branch");
        fold_matic(&db.pool, 100).await;
        assert_eq!(
            ids(book(&db.pool).await),
            vec![(id(0xc), "open".to_string())],
            "the lost branch's sold order is gone"
        );
        assert!(stands(&db.pool).await, "refolded whole on the new branch");
        db.drop().await;
    }

    /// A rollout: an earlier binary's nft folds ran past the book (its marker
    /// at 20, the nft fold's at 40) and its readers wait; the next fold
    /// refolds from the book's marker, not the nft fold's, and the book
    /// catches up.
    #[tokio::test]
    async fn a_book_behind_the_nft_fold_catches_up_from_its_own_marker() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_behind").await
        else {
            return;
        };
        seed_log(&db.pool, 20).await;
        nft_cursor(&db.pool, 20, None, None).await;
        fold_matic(&db.pool, -1).await;
        sqlx::query("DELETE FROM shop.nft_event")
            .execute(&db.pool)
            .await
            .unwrap();
        seed_log(&db.pool, 100).await;
        nft_cursor(&db.pool, 100, Some(40), Some(40)).await;
        let mut tx = db.pool.begin().await.unwrap();
        assert!(
            !caught_up(&mut tx, ChainNet::Matic).await.unwrap(),
            "the book (20) trails the published fold (40)"
        );
        tx.rollback().await.unwrap();
        fold_matic(&db.pool, 40).await;
        assert_eq!(
            book(&db.pool).await,
            expected(),
            "b's sale at 30 is folded too"
        );
        assert_eq!(book_through(&db.pool).await, Some(100));
        db.drop().await;
    }

    /// A sale-history repair, by this binary or an earlier one (both write
    /// through `nft_index::insert_events`), rewrites sales at or below the
    /// book's marker: it fills the block time of `b`'s sale at 30 (only
    /// `updated_at` changes), and inserts `e`'s sale at 42, which the walk
    /// missed (so `f`'s creation at 41 had cancelled `e`); then it fills
    /// `b`'s missing buyer. The trigger on `shop.nft_event` queues those
    /// tokens; the next fold has no order event past its marker each time,
    /// refolds them, and empties the queue.
    #[tokio::test]
    async fn a_sale_repaired_below_the_marker_reaches_the_book_on_the_next_fold() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_repair").await
        else {
            return;
        };
        let (m1, m2) = markets();
        let sale =
            |block: i64, market: &str, token: i64, order: u8, price: i64| nft_index::NftEvent {
                block_number: block,
                log_index: 0,
                block_time: block * 10,
                kind: "order_successful",
                emitter: market.to_string(),
                contract_address: C.to_string(),
                token_id: token.to_string(),
                from_address: Some(SELLER.to_string()),
                to_address: Some(BUYER.to_string()),
                amount: Some(price.to_string()),
                expires_at: None,
                detail: Some(id(order)),
                tx_hash: Some(format!("0x{block:064x}")),
            };
        let repair = |sales: Vec<nft_index::NftEvent>| {
            let pool = db.pool.clone();
            async move {
                // The repair's write as every binary makes it, an earlier
                // one's included: nothing here queues the tokens but the
                // trigger on `shop.nft_event`.
                let mut tx = pool.begin().await.unwrap();
                nft_index::insert_events(&mut tx, ChainNet::Matic, &sales)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }
        };
        let queued = || async {
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.legacy_order_refold")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        };
        seed_log(&db.pool, 41).await;
        sqlx::query("UPDATE shop.nft_event SET block_time = 0 WHERE block_number = 30")
            .execute(&db.pool)
            .await
            .unwrap();
        nft_cursor(&db.pool, 50, Some(50), Some(50)).await;
        fold_matic(&db.pool, -1).await;
        let mut before = expected();
        before[1].5 = 0;
        before[4] = (id(0xe), "cancelled".into(), None, "500".into(), 41, 410);
        assert_eq!(
            book(&db.pool).await,
            before,
            "folded from the incomplete sales"
        );
        repair(vec![sale(30, m2, 1, 0xb, 210), sale(42, m1, 4, 0xe, 500)]).await;
        assert_eq!(
            book(&db.pool).await,
            before,
            "the repair alone leaves the book"
        );
        assert_eq!(queued().await, 2);
        assert!(
            !stands(&db.pool).await,
            "a queued refold withholds the book, though the fold is published at its marker"
        );
        fold_matic(&db.pool, 50).await;
        assert_eq!(
            book(&db.pool).await,
            expected(),
            "the next fold takes the repaired sales"
        );
        assert_eq!(queued().await, 0, "the fold consumed the queue");
        assert!(stands(&db.pool).await, "read again once folded");

        sqlx::query("UPDATE shop.nft_event SET to_address = NULL WHERE block_number = 30")
            .execute(&db.pool)
            .await
            .unwrap();
        fold_matic(&db.pool, -1).await;
        let mut buyerless = expected();
        buyerless[1].2 = None;
        assert_eq!(
            book(&db.pool).await,
            buyerless,
            "folded from a sale with no buyer"
        );
        repair(vec![sale(30, m2, 1, 0xb, 210)]).await;
        fold_matic(&db.pool, 50).await;
        assert_eq!(
            book(&db.pool).await,
            expected(),
            "the repaired buyer reaches the book"
        );
        assert_eq!(book_through(&db.pool).await, Some(50));
        assert_eq!(queued().await, 0);
        db.drop().await;
    }

    /// The book is read only at the published nft fold. A walk that advanced
    /// without folding leaves it at the published block, still readable;
    /// a fold committed past that block (its head reads before publishing
    /// stalled) is not read until it is published.
    #[tokio::test]
    async fn a_fold_committed_before_its_publication_is_not_read_until_published() {
        let Some(db) =
            crate::market::tests::support::OwnDb::create("legacy_orders_unpublished").await
        else {
            return;
        };
        seed_log(&db.pool, 100).await;
        nft_cursor(&db.pool, 100, None, None).await;
        fold_matic(&db.pool, -1).await;
        nft_cursor(&db.pool, 100, Some(100), Some(100)).await;
        assert!(stands(&db.pool).await, "the published fold at 100");
        nft_cursor(&db.pool, 120, Some(100), Some(100)).await;
        assert!(
            stands(&db.pool).await,
            "the walk advanced to 120 without folding: the book stays at the published 100"
        );
        fold_matic(&db.pool, 100).await;
        assert_eq!(book_through(&db.pool).await, Some(120));
        assert!(
            !stands(&db.pool).await,
            "the fold through 120 committed while 100 is still the published fold"
        );
        nft_cursor(&db.pool, 120, Some(120), Some(120)).await;
        assert!(stands(&db.pool).await, "published at 120");
        db.drop().await;
    }

    /// The schema's write record binds every writer of a projection to the
    /// cursor's publications: a write (by this binary or an earlier one, past
    /// the published block or at it) is not served until a publication
    /// commits after it, at whatever height (a rebuild below the old tip
    /// included); a raw walk advance writes nothing and stays served; no
    /// publication serves nothing.
    #[tokio::test]
    async fn a_projection_written_since_its_publication_is_not_served() {
        let Some(db) = crate::market::tests::support::OwnDb::create("projection_write").await
        else {
            return;
        };
        let published = || async {
            let mut tx = db.pool.begin().await.unwrap();
            let held = crate::market::chain_walk::rows_published_on(
                &mut tx,
                crate::market::chain_walk::INDEXER_NFT,
                ChainNet::Matic,
                &["nft"],
            )
            .await
            .unwrap();
            tx.rollback().await.unwrap();
            held
        };
        let owner = |to: Option<&'static str>| {
            let pool = db.pool.clone();
            async move {
                match to {
                    Some(to) => sqlx::query(
                        "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
                         VALUES ('matic', '0x00000000000000000000000000000000000000c1', 1, $1, 1, 1, 1, 1, 1, 0, 1) \
                         ON CONFLICT (network, contract_address, token_id) DO UPDATE SET owner = EXCLUDED.owner \
                         WHERE shop.nft.owner IS DISTINCT FROM EXCLUDED.owner",
                    )
                    .bind(to),
                    None => sqlx::query("DELETE FROM shop.nft WHERE network = 'matic'"),
                }
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        let raw_advance = |last: i64| {
            let pool = db.pool.clone();
            async move {
                sqlx::query("UPDATE shop.chain_cursor SET last_block = $1 WHERE indexer = 'nft' AND network = 'matic'")
                    .bind(last)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        };
        nft_cursor(&db.pool, 100, None, None).await;
        assert!(!published().await, "no publication");
        nft_cursor(&db.pool, 100, Some(100), Some(100)).await;
        assert!(published().await, "published, nothing written");
        owner(Some(SELLER)).await;
        assert!(!published().await, "a write at the published height");
        nft_cursor(&db.pool, 100, Some(100), Some(100)).await;
        assert!(published().await, "republished at the same height");
        owner(Some(SELLER)).await;
        assert!(published().await, "a no-op upsert writes nothing");
        raw_advance(120).await;
        assert!(published().await, "a raw walk advance writes nothing");
        owner(Some(OTHER)).await;
        assert!(
            !published().await,
            "a fold through 120 written, 100 published"
        );
        nft_cursor(&db.pool, 120, Some(120), Some(120)).await;
        assert!(published().await, "published at 120");
        owner(None).await;
        assert!(!published().await, "a rewind's deletes");
        nft_cursor(&db.pool, 110, Some(110), Some(110)).await;
        assert!(
            published().await,
            "the rebuilt branch published below the old tip"
        );
        db.drop().await;
    }

    #[tokio::test]
    async fn a_rewind_deletes_the_orders_of_its_tokens_and_the_rebuild_restores_what_remains() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_rewind").await
        else {
            return;
        };
        seed_log(&db.pool, 100).await;
        nft_cursor(&db.pool, 100, None, None).await;
        fold_matic(&db.pool, -1).await;
        let mut tx = db.pool.begin().await.unwrap();
        NftIndex::new(ChainNet::Matic)
            .rewind(&mut tx, 29)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            book_through(&db.pool).await,
            Some(29),
            "the book's marker drops to the point"
        );
        let left: Vec<String> = book(&db.pool).await.into_iter().map(|r| r.0).collect();
        assert_eq!(
            left,
            vec![id(0xc), id(0xd)],
            "tokens 1 and 4 had events past block 29"
        );
        fold_matic(&db.pool, -1).await;
        let rebuilt: Vec<(String, String)> = book(&db.pool)
            .await
            .into_iter()
            .map(|r| (r.0, r.1))
            .collect();
        assert_eq!(
            rebuilt,
            vec![
                (id(0xa), "cancelled".to_string()),
                (id(0xb), "open".to_string()),
                (id(0xc), "open".to_string()),
                (id(0xd), "cancelled".to_string()),
            ],
            "b's sale and e/f were reorged out"
        );
        db.drop().await;
    }
}
