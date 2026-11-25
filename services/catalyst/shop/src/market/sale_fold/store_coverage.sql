-- A catalogue or factory discovery reopens Issue repair even behind a saved
-- exclusion. A rewind of the factory cursor clips its attested prefix.
WITH pending AS (
    SELECT l.block_number,
        EXISTS(SELECT FROM shop.collections c WHERE c.network='matic' AND c.chain_id=137
               AND c.contract_address=l.contract_address AND c.created_at<=l.block_time)
        OR EXISTS(SELECT FROM shop.collection_event e WHERE e.network='matic' AND e.kind='proxy_created'
                  AND e.contract_address=l.contract_address AND e.block_number<=l.block_number) AS listed
    FROM shop.store_sale_line l
    WHERE l.block_number <= $1 AND l.issue_log IS NULL AND NOT l.issue_checked
), proof AS (
    SELECT coalesce((SELECT sale_verified_through FROM shop.chain_cursor
        WHERE indexer='collections' AND network='matic' AND chain_id=137 AND full_history
          AND sale_input_version=$2),-1) AS through
)
SELECT coalesce(bool_or(listed),false),
       min(block_number) FILTER (WHERE block_number>through),
       max(block_number) FILTER (WHERE block_number>through)
FROM pending CROSS JOIN proof
