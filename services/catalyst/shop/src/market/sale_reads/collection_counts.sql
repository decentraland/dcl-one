-- Only Polygon factory creations increment the account's collection counter.
-- The factory handler reads creator() at the end of the creation block.
, founders AS (
    SELECT e.network,e.contract_address,
           coalesce(initial.account,later.previous_account,seed.creator,c.creator) AS creator
    FROM shop.collection_event e JOIN shop.sale_publication p USING(network)
    JOIN shop.chain_cursor ix ON ix.network=e.network AND ix.indexer='collections'
    LEFT JOIN LATERAL (
        SELECT t.account FROM shop.collection_event t
        WHERE (t.network,t.contract_address,t.block_number)=(e.network,e.contract_address,e.block_number)
          AND t.kind='creatorship_transferred' ORDER BY t.log_index DESC LIMIT 1
    ) initial ON true
    LEFT JOIN LATERAL (
        SELECT t.previous_account FROM shop.collection_event t
        WHERE (t.network,t.contract_address)=(e.network,e.contract_address)
          AND t.kind='creatorship_transferred' AND t.block_number>e.block_number
          AND t.block_number<=ix.folded_block
        ORDER BY t.block_number,t.log_index LIMIT 1
    ) later ON true
    LEFT JOIN shop.collection_seed seed ON (seed.network,seed.contract_address)=(e.network,e.contract_address)
        AND seed.read_block BETWEEN e.block_number AND ix.folded_block AND ix.reorg_rewinds=0
    LEFT JOIN shop.collections c ON (c.network,c.contract_address)=(e.network,e.contract_address)
        AND c.created_at=e.block_time AND c.updated_at<=ix.last_block_time
    WHERE e.network='matic' AND e.network=ANY($1) AND e.kind='proxy_created'
      AND e.block_number<=p.block_number
), collections AS (
    SELECT network,creator AS address,count(*) AS collections FROM founders GROUP BY network,creator
)
