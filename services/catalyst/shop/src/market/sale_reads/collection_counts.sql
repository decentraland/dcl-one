-- Only Polygon factory creations increment the account's collection counter.
-- The factory handler reads creator() at the end of the creation block.
, collection_history AS MATERIALIZED (
    SELECT ix.network,ix.folded_block,
           coalesce(CASE WHEN ix.folded_block=ix.last_block THEN ix.last_block_time END,
                    (SELECT event.block_time FROM shop.collection_event event
                     WHERE event.network=ix.network AND event.block_number<=ix.folded_block
                     ORDER BY event.block_number DESC,event.log_index DESC LIMIT 1)) AS catalogue_time
    FROM shop.chain_cursor ix
    WHERE ix.indexer='collections' AND ix.network='matic' AND ix.network=ANY($1)
), factory_creations AS (
    SELECT e.network,e.contract_address,e.block_number AS created_block,e.log_index AS created_log,e.block_time AS created_at
    FROM shop.collection_event e JOIN shop.sale_publication p USING(network)
    WHERE e.network='matic' AND e.network=ANY($1) AND e.kind='proxy_created'
      AND e.block_number<=p.block_number
), founders AS (
    SELECT f.network,f.contract_address,
           coalesce(initial.account,later.previous_account,seed.creator,c.creator) AS creator
    FROM factory_creations f
    JOIN collection_history ix USING(network)
    LEFT JOIN LATERAL (
        SELECT t.account FROM shop.collection_event t
        WHERE (t.network,t.contract_address,t.block_number)=(f.network,f.contract_address,f.created_block)
          AND t.kind='creatorship_transferred' ORDER BY t.log_index DESC LIMIT 1
    ) initial ON true
    LEFT JOIN LATERAL (
        SELECT t.previous_account FROM shop.collection_event t
        WHERE (t.network,t.contract_address)=(f.network,f.contract_address)
          AND t.kind='creatorship_transferred' AND t.block_number>f.created_block
          AND t.block_number<=ix.folded_block
        ORDER BY t.block_number,t.log_index LIMIT 1
    ) later ON true
    LEFT JOIN shop.collection_seed seed ON (seed.network,seed.contract_address)=(f.network,f.contract_address)
        AND seed.verified AND seed.read_hash IS NOT NULL
        AND seed.read_block BETWEEN f.created_block AND ix.folded_block
    LEFT JOIN shop.collections c ON (c.network,c.contract_address)=(f.network,f.contract_address)
        AND c.chain_id=137 AND c.created_at=f.created_at AND c.updated_at<=ix.catalogue_time
), collections AS (
    SELECT network,creator AS address,count(*) AS collections FROM founders GROUP BY network,creator
)
