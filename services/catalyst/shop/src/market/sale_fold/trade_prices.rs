//! Derive NFT prices from the registered MANA consideration of a trade.
use sqlx::PgConnection;

use super::ChainNet;

/// Assigns a price after the receipt settled every leg of the execution.
/// Test all payments before allowing one currency to price the NFT.
pub(super) async fn publish_proved(
    c: &mut PgConnection,
    net: ChainNet,
    executions: &[(i64, i32)],
    mana: &str,
) -> sqlx::Result<Vec<(i64, i32)>> {
    if executions.is_empty() {
        return Ok(Vec::new());
    }
    let (blocks, logs): (Vec<_>, Vec<_>) = executions.iter().copied().unzip();
    sqlx::query_as(
        "WITH proved AS (
            SELECT p.block_number,p.log_index,sum(p.gross) AS price
            FROM shop.sale_payment p
            WHERE p.network=$1 AND (p.block_number,p.log_index) IN
                (SELECT * FROM unnest($2::bigint[],$3::int[]))
            GROUP BY p.block_number,p.log_index
            HAVING bool_and(p.consideration AND p.currency=$4 AND p.settled) AND sum(p.gross)>0
         )
         UPDATE shop.sale_asset a SET price=p.price,currency=$4
         FROM proved p WHERE a.network=$1 AND a.price IS NULL
           AND (a.block_number,a.log_index)=(p.block_number,p.log_index)
           AND NOT EXISTS (SELECT FROM shop.sale_asset other
               WHERE (other.network,other.block_number,other.log_index)=(a.network,a.block_number,a.log_index)
                 AND other.ordinal<>a.ordinal)
         RETURNING a.block_number,a.log_index",
    )
    .bind(net.db_name())
    .bind(blocks)
    .bind(logs)
    .bind(mana)
    .fetch_all(c)
    .await
}

/// Clears v1–v3 prices in the retained prefix. Prior successful settlement
/// proves each matching MANA leg. Keeping a price without that proof
/// requires a complete direct-MANA trade event.
/// Returns the affected days and positions and whether each was MANA.
pub(super) async fn clear_unproved(
    c: &mut PgConnection,
    net: ChainNet,
    through: i64,
    mana: &str,
) -> sqlx::Result<Vec<(i64, i64, i32, bool)>> {
    sqlx::query_as(
        "WITH unproved AS MATERIALIZED (
            SELECT a.block_number,a.log_index,a.ordinal,e.block_time/86400*86400 AS day,
                   a.currency=$3 AS was_mana
            FROM shop.sale_execution e JOIN shop.sale_asset a USING(network,block_number,log_index)
            WHERE e.network=$1 AND e.kind='trade' AND e.block_number<=$2 AND a.price IS NOT NULL
              AND (a.price<=0 OR a.currency<>$3 OR (
                NOT EXISTS (SELECT FROM shop.trade_event t
                   CROSS JOIN LATERAL (
                       SELECT bool_and((leg->>'assetType'='1' AND leg->>'contractAddress'=$3) IS TRUE)
                           FILTER (WHERE leg->>'assetType' IN ('1','2')) AS direct_mana
                       FROM jsonb_array_elements(
                           CASE WHEN jsonb_typeof(t.assets->'sent')='array' AND jsonb_typeof(t.assets->'received')='array'
                               THEN (t.assets->'sent') || (t.assets->'received') ELSE '[]'::jsonb END
                       ) leg
                   ) evidence
                   WHERE (t.network,t.block_number,t.log_index)=(e.network,e.block_number,e.log_index)
                     AND evidence.direct_mana)
                AND NOT EXISTS (SELECT FROM shop.sale_payment p
                    WHERE (p.network,p.block_number,p.log_index)=(e.network,e.block_number,e.log_index)
                    GROUP BY p.block_number,p.log_index
                    HAVING bool_and(p.settled AND p.consideration AND p.currency=$3) AND sum(p.gross)=a.price)
              ))
         )
         UPDATE shop.sale_asset a SET price=NULL,currency=NULL
         FROM unproved u WHERE a.network=$1
           AND (a.block_number,a.log_index,a.ordinal)=(u.block_number,u.log_index,u.ordinal)
         RETURNING u.day,a.block_number,a.log_index,u.was_mana",
    )
    .bind(net.db_name())
    .bind(through)
    .bind(mana)
    .fetch_all(c)
    .await
}
