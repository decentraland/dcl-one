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
//! token's current owner on its own snapshot (`orders::legacy_leg`), which
//! is the same answer without a second index in the fold.
//!
//! The fold runs in `nft_index::fold`'s transaction, so the order book is
//! published with the ownership it is read beside. It rewrites every order
//! of each token an order event past the fold's marker touched, over the
//! token's whole history (a new order supersedes the token's earlier ones);
//! `since = None` rebuilds. A rewind deletes the orders of the tokens whose
//! events it deletes, and the rebuild that follows restores those that
//! remain. The schema's seed statement fills a database that predates the
//! table with the same rules.

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

/// Folds `net`'s orders touched past `since` (`-1`: every order) on the nft
/// fold's transaction; the rows written.
pub async fn fold(conn: &mut PgConnection, net: ChainNet, since: i64) -> sqlx::Result<u64> {
    Ok(sqlx::query(FOLD_SQL)
        .bind(net.db_name())
        .bind(since)
        .bind(super::nft_index::ZERO_ADDRESS)
        .execute(conn)
        .await?
        .rows_affected())
}

/// Deletes `net`'s orders of the tokens (`contracts[i]`, `tokens[i]`) a
/// rewind deleted events of, in the rewind's transaction; the rebuild that
/// follows restores the ones whose creation remains.
pub async fn rewind(
    conn: &mut PgConnection,
    net: ChainNet,
    contracts: &[String],
    tokens: &[String],
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM shop.legacy_order o USING UNNEST($2::text[], $3::text[]) AS g(c, t) \
         WHERE o.network = $1 AND o.contract_address = g.c AND o.token_id = g.t::numeric",
    )
    .bind(net.db_name())
    .bind(contracts)
    .bind(tokens)
    .execute(conn)
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
        let m = nft_index::legacy_marketplaces(ChainNet::Matic);
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
        fold_matic(&db.pool, -1).await;
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
        let seed = crate::db::schema_migrate::statements(crate::db::SCHEMA_SQL)
            .into_iter()
            .find(|s| s.starts_with("INSERT INTO shop.legacy_order "))
            .expect("the schema seeds shop.legacy_order");
        assert_eq!(
            crate::db::schema_migrate::classify(&seed),
            Some(crate::db::schema_migrate::Step::FollowUp(
                crate::db::schema_migrate::Object::Relation("shop.legacy_order".into())
            )),
            "the seed runs only in the run that creates the table"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(seed))
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(book(&db.pool).await, expected());
        assert_eq!(
            fold_matic(&db.pool, -1).await,
            0,
            "the fold finds every seeded order as it would write it"
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
        fold_matic(&db.pool, -1).await;
        let mut tx = db.pool.begin().await.unwrap();
        NftIndex::new(ChainNet::Matic)
            .rewind(&mut tx, 29)
            .await
            .unwrap();
        tx.commit().await.unwrap();
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
