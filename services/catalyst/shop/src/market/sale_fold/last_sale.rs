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
