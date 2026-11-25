-- Ethereum v1 sales have token IDs in the ledger. Resolve the latest Issue
-- at each sale position against the mainnet collection and item URNs.
, v1_collections AS (
    SELECT c.network,c.contract_address,CASE WHEN $10::boolean THEN c.urn END AS urn
    FROM candidate_collections candidate JOIN shop.collections c USING(network,contract_address)
    WHERE c.network='ethereum' AND c.chain_id=1
    UNION
    SELECT i.network,i.contract_address,c.urn
    FROM requested_wallet w JOIN shop.items i ON i.creator=w.address
    LEFT JOIN shop.collections c ON (c.network,c.contract_address)=(i.network,i.contract_address) AND c.chain_id=1 AND $10
    WHERE i.network='ethereum' AND i.network=ANY($1) AND i.item_type='wearable_v1' AND i.chain_id=1
), v1_resales AS MATERIALIZED (
    SELECT a.network,a.block_number,a.log_index,a.ordinal,i.creator
    FROM v1_collections c JOIN shop.sale_asset a USING(network,contract_address)
    JOIN shop.sale_publication pub ON pub.network=a.network AND a.block_number<=pub.block_number
    JOIN shop.sale_execution execution
      ON (execution.network,execution.block_number,execution.log_index)=(a.network,a.block_number,a.log_index)
    CROSS JOIN requested_wallet w
    LEFT JOIN LATERAL (
        SELECT detail FROM shop.nft_event e
        WHERE c.urn IS NOT NULL AND (e.network,e.contract_address,e.token_id)=(a.network,a.contract_address,a.token_id)
          AND e.kind='issue_v1' AND (e.block_number,e.log_index)<=(a.block_number,a.log_index)
        ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
    ) issue ON true
    LEFT JOIN shop.items i ON i.urn=c.urn || ':' || issue.detail AND i.chain_id=1
      AND (i.network,i.contract_address)=(c.network,c.contract_address) AND i.item_type='wearable_v1'
    WHERE a.price>0 AND a.kind IN ('order','bid') AND a.item_id IS NULL AND a.token_id IS NOT NULL
      AND ($8::bigint IS NULL OR execution.block_time*1000 >= $8)
      AND ($9::bigint IS NULL OR execution.block_time*1000 <= $9)
      AND (i.creator=w.address OR i.creator IS NULL)
), summary_keys AS (
    SELECT * FROM wallet_keys
    UNION
    SELECT network,block_number,log_index,ordinal FROM v1_resales
    UNION
    SELECT a.network,a.block_number,a.log_index,a.ordinal
    FROM candidate_collections c JOIN shop.sale_asset a USING(network,contract_address)
    WHERE a.price>0 AND a.kind='order' AND a.item_id IS NOT NULL AND NOT EXISTS (
        SELECT FROM shop.items i WHERE (i.network,i.contract_address,i.item_id)=(a.network,a.contract_address,a.item_id)
          AND i.chain_id=CASE i.network WHEN 'ethereum' THEN 1 ELSE 137 END)
), scoped_assets AS NOT MATERIALIZED (
    SELECT a.* FROM summary_keys k JOIN shop.sale_asset a USING(network,block_number,log_index,ordinal)
)
