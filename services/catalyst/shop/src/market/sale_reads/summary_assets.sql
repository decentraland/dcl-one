-- Ethereum v1 sales have token IDs in the ledger. The latest Issue at
-- the sale position must resolve against rows from completed mirror copies,
-- or, once the collections index owns Ethereum, from its standing projection
-- (which publishes the v1 royalty catalogue with the rows it writes).
, v1_markers AS (
    SELECT mirror
    FROM shop.mirror_state
    WHERE network='ethereum' AND last_complete_pass_at IS NOT NULL
      AND (chain_id=1 OR (chain_id IS NULL AND pass_block_hash IS NULL))
    UNION
    SELECT copy FROM shop.collection_projection,
        unnest(ARRAY['collections','items']) AS copy
    WHERE network='ethereum' AND owned_at IS NOT NULL AND projected_through IS NOT NULL
), v1_contracts AS (
    SELECT contract_address FROM candidate_collections WHERE network='ethereum'
    UNION
    SELECT i.contract_address FROM shop.items i,requested_wallet w
    WHERE i.creator=w.address AND i.network='ethereum' AND i.chain_id=1 AND i.item_type='wearable_v1'
    UNION
    SELECT i.contract_address FROM shop.v1_royalty_item i,requested_wallet w WHERE i.creator=w.address
    UNION
    SELECT c.contract_address FROM shop.v1_royalty_collection c,requested_wallet w WHERE c.creator=w.address
), v1_collections AS (
    SELECT 'ethereum'::text AS network,candidate.contract_address,c.urn
    FROM v1_contracts candidate
    LEFT JOIN shop.v1_royalty_collection c ON c.contract_address=candidate.contract_address
      AND EXISTS (SELECT FROM v1_markers WHERE mirror='collections')
    WHERE 'ethereum'=ANY($1)
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
    LEFT JOIN shop.v1_royalty_item i ON i.urn=c.urn || ':' || issue.detail
      AND i.contract_address=c.contract_address
      AND EXISTS (SELECT FROM v1_markers WHERE mirror='items')
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
