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
//! The fold runs in `nft_index::fold`'s transaction, so the order book is
//! published with the ownership it is read beside. It rewrites every order
//! of each token an order event touched past the lower of the nft fold's
//! marker and the book's own (`shop.legacy_order_cursor`), over the token's
//! whole history (a new order supersedes the token's earlier ones), and
//! records the nft walk's last block as the book's; `since = None`
//! rebuilds. The book's own marker lets it catch up with nft folds that ran
//! without it: a replica on an earlier binary folds the nft index during a
//! rollout and never writes the book. A rewind deletes the orders of the
//! tokens whose events it deletes and lowers the book's marker, and the
//! rebuild that follows restores those that remain. The schema's seed
//! statement fills a database that predates the table with the same rules,
//! up to what the nft fold had folded (`fold_through`), and records that
//! block. The marker also names the branch: the hash of the nft walk's
//! recovery point at its block. Every binary's rewind drops the recovery
//! points past its target, so a branch an earlier binary rewound and
//! republished at the same height brings another hash. A book whose point no
//! longer has its hash does not stand: a reader does not serve it, and the
//! next fold deletes the network's orders and refolds them whole, as it does
//! on a rebuild, so no order of a lost branch survives. A reader serves the
//! book only while it stands and its marker reaches the published nft fold
//! ([`caught_up`]).

use sqlx::PgConnection;

use super::chain_walk::ChainNet;

/// The fold of one network (`$1`): the tokens with an order event past
/// `$2`, every order of each refolded over the token's history. An
/// unchanged order is left alone. The event that ends an order is one
/// backward probe of `nft_event_token_idx` per order (`LATERAL ... LIMIT
/// 1`), not a join of two derived sets: the planner estimates those at one
/// row each, and as a join it rescanned every order per order (a rebuild of
/// 300,000 orders did not finish in ten minutes).
const FOLD_SQL: &str = "
WITH touched AS (
    SELECT DISTINCT contract_address, token_id FROM shop.nft_event
    WHERE network = $1 AND block_number > $2 AND kind IN ('order_created', 'order_successful', 'order_cancelled')
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
       legacy_order.expires_at, legacy_order.status, legacy_order.created_block, legacy_order.created_log,
       legacy_order.updated_block, legacy_order.updated_log)
    IS DISTINCT FROM (EXCLUDED.contract_address, EXCLUDED.token_id, EXCLUDED.seller, EXCLUDED.buyer, EXCLUDED.price,
       EXCLUDED.expires_at, EXCLUDED.status, EXCLUDED.created_block, EXCLUDED.created_log,
       EXCLUDED.updated_block, EXCLUDED.updated_log)";

/// Whether `net`'s book stands on `conn`'s snapshot: its marker names a
/// recovery point of the nft walk that still has the hash it recorded.
const STANDS_SQL: &str = "SELECT b.folded_through FROM shop.legacy_order_cursor b \
     JOIN shop.chain_checkpoint k ON k.indexer = $2 AND k.network = b.network \
         AND k.block_number = b.folded_through AND k.block_hash = b.through_hash \
     WHERE b.network = $1";

/// Folds `net`'s orders on the nft fold's transaction and records the nft
/// walk's last block and hash as the book's marker; the rows written. The
/// nft fold's `since` (`-1`: a rebuild) is lowered to the book's marker
/// while the book stands. A rebuild, or a book that does not stand (never
/// folded, rewound, or on a branch the nft walk no longer has), deletes the
/// network's orders first and refolds them whole.
pub async fn fold(conn: &mut PgConnection, net: ChainNet, since: i64) -> sqlx::Result<u64> {
    let stands: Option<i64> = sqlx::query_scalar(STANDS_SQL)
        .bind(net.db_name())
        .bind(super::chain_walk::INDEXER_NFT)
        .fetch_optional(&mut *conn)
        .await?;
    let since = match stands {
        Some(through) if since >= 0 => since.min(through),
        _ => -1,
    };
    if since < 0 {
        sqlx::query("DELETE FROM shop.legacy_order WHERE network = $1")
            .bind(net.db_name())
            .execute(&mut *conn)
            .await?;
    }
    let written = sqlx::query(FOLD_SQL)
        .bind(net.db_name())
        .bind(since)
        .bind(super::nft_index::ZERO_ADDRESS)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    sqlx::query(
        "INSERT INTO shop.legacy_order_cursor (network, folded_through, through_hash) \
         SELECT network, last_block, last_block_hash FROM shop.chain_cursor WHERE indexer = $2 AND network = $1 \
         ON CONFLICT (network) DO UPDATE SET folded_through = EXCLUDED.folded_through, through_hash = EXCLUDED.through_hash",
    )
    .bind(net.db_name())
    .bind(super::chain_walk::INDEXER_NFT)
    .execute(&mut *conn)
    .await?;
    Ok(written)
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
/// published block, which stays readable.
pub async fn caught_up(conn: &mut PgConnection, net: ChainNet) -> sqlx::Result<bool> {
    let stands: Option<i64> = sqlx::query_scalar(STANDS_SQL)
        .bind(net.db_name())
        .bind(super::chain_walk::INDEXER_NFT)
        .fetch_optional(&mut *conn)
        .await?;
    let Some(through) = stands else {
        return Ok(false);
    };
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
/// the rebuild that follows refolds it whole.
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
    use crate::market::chain_walk::ChainIndex;
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

    /// The schema's two seed statements (the book's marker, then the book).
    async fn seed(pool: &PgPool) {
        sqlx::query("DELETE FROM shop.legacy_order")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.legacy_order_cursor")
            .execute(pool)
            .await
            .unwrap();
        let statements = crate::db::schema_migrate::statements(crate::db::SCHEMA_SQL);
        for table in ["shop.legacy_order_cursor", "shop.legacy_order"] {
            let seed = statements
                .iter()
                .find(|s| s.starts_with(&format!("INSERT INTO {table} ")))
                .unwrap_or_else(|| panic!("the schema seeds {table}"));
            assert_eq!(
                crate::db::schema_migrate::classify(seed),
                Some(crate::db::schema_migrate::Step::FollowUp(
                    crate::db::schema_migrate::Object::Relation(table.into())
                )),
                "the seed runs only in the run that creates {table}"
            );
            sqlx::raw_sql(sqlx::AssertSqlSafe(seed.clone()))
                .execute(pool)
                .await
                .unwrap();
        }
    }

    async fn fold_matic(pool: &PgPool, since: i64) -> u64 {
        let mut tx = pool.begin().await.unwrap();
        let n = fold(&mut tx, ChainNet::Matic, since).await.unwrap();
        tx.commit().await.unwrap();
        n
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

    #[tokio::test]
    async fn the_schema_seed_fills_the_same_book_as_the_fold() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_seed").await
        else {
            return;
        };
        seed_log(&db.pool, 100).await;
        nft_cursor(&db.pool, 100, Some(100), Some(100)).await;
        seed(&db.pool).await;
        assert_eq!(book(&db.pool).await, expected());
        assert_eq!(book_through(&db.pool).await, Some(100));
        assert!(stands(&db.pool).await);
        assert_eq!(
            fold_matic(&db.pool, 100).await,
            0,
            "an incremental fold over the standing seed changes nothing"
        );
        fold_matic(&db.pool, -1).await;
        assert_eq!(
            book(&db.pool).await,
            expected(),
            "a whole refold writes the book the seed wrote"
        );
        db.drop().await;
    }

    /// The upgrade while the walk is ahead of the fold: an order created at
    /// 90, the nft fold through (and published at) 100, its cancellation
    /// walked at 101. The seed stops at what the fold folded, so the book
    /// holds the order open beside ownership at 100; the next fold takes the
    /// cancellation and moves the book's marker to the walk's block.
    #[tokio::test]
    async fn the_seed_stops_at_the_nft_fold_and_the_next_fold_takes_the_rest() {
        let Some(db) = crate::market::tests::support::OwnDb::create("legacy_orders_ahead").await
        else {
            return;
        };
        let (m1, _) = markets();
        order_event(
            &db.pool,
            90,
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
            101,
            "order_cancelled",
            m1,
            1,
            0xa,
            SELLER,
            None,
            None,
        )
        .await;
        nft_cursor(&db.pool, 101, Some(100), Some(100)).await;
        seed(&db.pool).await;
        let status = |rows: Vec<Row>| rows.into_iter().map(|r| r.1).collect::<Vec<_>>();
        assert_eq!(
            status(book(&db.pool).await),
            vec!["open"],
            "not a cancellation the fold has not folded"
        );
        assert_eq!(book_through(&db.pool).await, Some(100));
        let mut tx = db.pool.begin().await.unwrap();
        assert!(caught_up(&mut tx, ChainNet::Matic).await.unwrap());
        tx.rollback().await.unwrap();
        fold_matic(&db.pool, 100).await;
        assert_eq!(status(book(&db.pool).await), vec!["cancelled"]);
        assert_eq!(book_through(&db.pool).await, Some(101));
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
        seed(&db.pool).await;
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
