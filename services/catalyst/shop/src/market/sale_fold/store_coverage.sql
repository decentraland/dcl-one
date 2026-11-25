-- Raw lines await recovery if a factory creation or the trusted Polygon
-- catalogue names their issuer. Remaining addresses can be excluded only
-- after the factory history covers their purchase height.
WITH pending AS (
    SELECT l.block_number,
        EXISTS(SELECT FROM shop.collections c WHERE c.network='matic' AND c.chain_id=137
               AND c.contract_address=l.contract_address AND c.created_at<=l.block_time)
        OR EXISTS(SELECT FROM shop.collection_event e WHERE e.network='matic' AND e.kind='proxy_created'
                  AND e.contract_address=l.contract_address AND e.block_number<=l.block_number) AS listed
    FROM shop.store_sale_line l
    WHERE l.block_number <= $1 AND l.issue_log IS NULL AND NOT l.issue_checked
)
SELECT coalesce(bool_or(listed),false),min(block_number),max(block_number) FROM pending
