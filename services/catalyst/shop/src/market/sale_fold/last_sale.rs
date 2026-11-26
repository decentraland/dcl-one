//! `shop.nft_last_sale`: each token's latest public sale, kept by the fold
//! in the transaction that publishes each window, for the `soldAt` of
//! /v1/nfts. `sale_publication.last_sale_through` is the block through which
//! the table holds every sale of the ledger. A history change first deletes
//! the rows past the prefix the fold keeps and restores each of those
//! tokens' latest sale within it. Then each pass applies the ledger past the
//! table's block, at most [`WINDOW_BLOCKS`] at a time: the new window
//! itself once the table is current, the history before it after an upgrade
//! or a rebuild.

use sqlx::PgConnection;

use super::super::chain_walk::ChainNet;

/// The ledger blocks one pass applies to the table.
pub(super) const WINDOW_BLOCKS: i64 = 5_000_000;

/// The public sale rule of `sale_reads/public_sales.sql` over `a`
/// (`shop.sale_asset`) and `e` (its `shop.sale_execution`), with the
/// network's MANA as `$4`.
const PUBLIC: &str = "a.token_id IS NOT NULL AND a.price > 0 AND a.currency = $4 \
    AND (e.kind <> 'trade' OR NOT EXISTS (SELECT FROM shop.sale_asset other \
        WHERE (other.network, other.block_number, other.log_index) = (a.network, a.block_number, a.log_index) \
          AND other.ordinal <> a.ordinal))";

/// After the fold deleted the ledger past `kept`: each token whose latest
/// sale lies past it takes its latest sale at or before it, or leaves the
/// table.
pub(super) async fn rewind(
    c: &mut PgConnection,
    net: ChainNet,
    kept: i64,
    mana: &str,
) -> sqlx::Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.nft_last_sale AS l (network, contract_address, token_id, sold_at, block_number) \
         SELECT DISTINCT ON (a.contract_address, a.token_id) a.network, a.contract_address, a.token_id, e.block_time, a.block_number \
         FROM shop.nft_last_sale r \
         JOIN shop.sale_asset a ON a.network = r.network AND a.token_id = r.token_id AND a.contract_address = r.contract_address \
         JOIN shop.sale_execution e ON (e.network, e.block_number, e.log_index) = (a.network, a.block_number, a.log_index) \
         WHERE r.network = $1 AND r.block_number > $2 AND a.block_number <= $3 AND {PUBLIC} \
         ORDER BY a.contract_address, a.token_id, a.block_number DESC, a.log_index DESC \
         ON CONFLICT (network, contract_address, token_id) DO UPDATE \
         SET sold_at = EXCLUDED.sold_at, block_number = EXCLUDED.block_number"
    )))
    .bind(net.db_name())
    .bind(kept)
    .bind(kept)
    .bind(mana)
    .execute(&mut *c)
    .await?;
    sqlx::query("DELETE FROM shop.nft_last_sale WHERE network = $1 AND block_number > $2")
        .bind(net.db_name())
        .bind(kept)
        .execute(&mut *c)
        .await?;
    Ok(())
}

/// Applies the ledger's public sales in `(from, to]`: a token's latest one
/// there replaces an earlier row. Returns the rows written.
pub(super) async fn advance(
    c: &mut PgConnection,
    net: ChainNet,
    from: i64,
    to: i64,
    mana: &str,
) -> sqlx::Result<u64> {
    if to <= from {
        return Ok(0);
    }
    let written = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.nft_last_sale AS l (network, contract_address, token_id, sold_at, block_number) \
         SELECT DISTINCT ON (a.contract_address, a.token_id) a.network, a.contract_address, a.token_id, e.block_time, a.block_number \
         FROM shop.sale_asset a JOIN shop.sale_execution e USING (network, block_number, log_index) \
         WHERE a.network = $1 AND a.block_number > $2 AND a.block_number <= $3 AND {PUBLIC} \
         ORDER BY a.contract_address, a.token_id, a.block_number DESC, a.log_index DESC \
         ON CONFLICT (network, contract_address, token_id) DO UPDATE \
         SET sold_at = EXCLUDED.sold_at, block_number = EXCLUDED.block_number \
         WHERE EXCLUDED.block_number >= l.block_number"
    )))
    .bind(net.db_name())
    .bind(from)
    .bind(to)
    .bind(mana)
    .execute(&mut *c)
    .await?;
    Ok(written.rows_affected())
}

/// Recomputes, from the ledger through `through`, the latest sale of each
/// token minted by the executions `minted` (block, log index): a retried
/// receipt sets a mint's token id after its block is already covered. A
/// token's later sale stays.
pub(super) async fn refresh(
    c: &mut PgConnection,
    net: ChainNet,
    minted: &[(i64, i32)],
    through: i64,
    mana: &str,
) -> sqlx::Result<u64> {
    if minted.is_empty() {
        return Ok(0);
    }
    let (blocks, logs): (Vec<i64>, Vec<i32>) = minted.iter().copied().unzip();
    let written = sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO shop.nft_last_sale AS l (network, contract_address, token_id, sold_at, block_number) \
         SELECT DISTINCT ON (a.contract_address, a.token_id) a.network, a.contract_address, a.token_id, e.block_time, a.block_number \
         FROM shop.sale_asset a \
         JOIN shop.sale_execution e ON (e.network, e.block_number, e.log_index) = (a.network, a.block_number, a.log_index) \
         WHERE a.network = $1 AND a.block_number <= $2 AND {PUBLIC} \
           AND (a.contract_address, a.token_id) IN (SELECT m.contract_address, m.token_id FROM shop.sale_asset m \
               JOIN unnest($3::bigint[], $5::int[]) k(block_number, log_index) \
                 ON (m.block_number, m.log_index) = (k.block_number, k.log_index) \
               WHERE m.network = $1 AND m.token_id IS NOT NULL) \
         ORDER BY a.contract_address, a.token_id, a.block_number DESC, a.log_index DESC \
         ON CONFLICT (network, contract_address, token_id) DO UPDATE \
         SET sold_at = EXCLUDED.sold_at, block_number = EXCLUDED.block_number \
         WHERE EXCLUDED.block_number >= l.block_number"
    )))
    .bind(net.db_name())
    .bind(through)
    .bind(blocks)
    .bind(mana)
    .bind(logs)
    .execute(&mut *c)
    .await?;
    Ok(written.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::listings;
    use crate::market::tests::support::OwnDb;

    const C: &str = "0x00000000000000000000000000000000000000c1";

    async fn sale(pool: &sqlx::PgPool, block: i64, kind: &str, asset: &str, token: Option<i64>) {
        sqlx::query(
            "INSERT INTO shop.sale_execution (network, block_number, log_index, tx_hash, venue, kind, block_time) \
             VALUES ('matic', $1, 1, '0x' || lpad(to_hex($1), 64, '0'), '0x00000000000000000000000000000000000000ee', $2, $1 * 10)",
        )
        .bind(block)
        .bind(kind)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.sale_asset (network, block_number, log_index, ordinal, public_id, beneficiary, kind, \
                 contract_address, token_id, item_id, seller, buyer, price, currency) \
             VALUES ('matic', $1, 1, 0, 'matic-' || $1, '0xb0', $2, $3, $4, 3, '0xa1', '0xb0', 5, $5)",
        )
        .bind(block)
        .bind(asset)
        .bind(C)
        .bind(token)
        .bind(listings::mana_token(137))
        .execute(pool)
        .await
        .unwrap();
    }

    async fn rows(pool: &sqlx::PgPool) -> Vec<(i64, i64, i64)> {
        sqlx::query_as(
            "SELECT token_id::bigint, sold_at, block_number FROM shop.nft_last_sale ORDER BY token_id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// A receipt retried after the table covers its block sets the token
    /// ids of the trade's mints: the refresh adds each such token's sale and
    /// keeps a later sale of the same token, which the window advance alone
    /// never reads again.
    #[tokio::test]
    async fn a_delayed_receipt_mint_reaches_the_table_without_hiding_a_later_sale() {
        let Some(db) = OwnDb::create("last_sale_refresh").await else {
            return;
        };
        let pool = &db.pool;
        let mana = listings::mana_token(137).unwrap();
        // Two trades mint at blocks 90 and 91, their token ids still unknown;
        // token 11 sells again at block 95.
        sale(pool, 90, "trade", "mint", None).await;
        sale(pool, 91, "trade", "mint", None).await;
        sale(pool, 95, "order", "order", Some(11)).await;
        let mut conn = pool.acquire().await.unwrap();
        advance(&mut conn, ChainNet::Matic, 0, 100, mana)
            .await
            .unwrap();
        assert_eq!(rows(pool).await, vec![(11, 950, 95)]);
        // The retried receipts name tokens 11 and 12.
        for (block, token) in [(90, 11), (91, 12)] {
            sqlx::query("UPDATE shop.sale_asset SET token_id = $2 WHERE network = 'matic' AND block_number = $1")
                .bind(block)
                .bind(token)
                .execute(&mut *conn)
                .await
                .unwrap();
        }
        advance(&mut conn, ChainNet::Matic, 100, 100, mana)
            .await
            .unwrap();
        assert_eq!(
            rows(pool).await,
            vec![(11, 950, 95)],
            "the advance reads no covered block"
        );
        refresh(&mut conn, ChainNet::Matic, &[(90, 1), (91, 1)], 100, mana)
            .await
            .unwrap();
        assert_eq!(rows(pool).await, vec![(11, 950, 95), (12, 910, 91)]);
        drop(conn);
        db.drop().await;
    }
}
