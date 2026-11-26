-- Arguments are network, (exclusive start, inclusive end), the deployed
-- legacy order addresses, bid address, off-chain marketplaces, NAME controllers,
-- store address and fixed registry assets. Filter the emitters as well as the event kind.
WITH catalog AS NOT MATERIALIZED (
    SELECT contract_address, coalesce(backfill_from_block,walked_from_block) AS first_block
    FROM shop.nft_contract WHERE network=$1
    UNION ALL SELECT address, 0 FROM unnest($9::text[]) address
)
INSERT INTO shop.sale_execution (network, block_number, log_index, tx_hash, venue, kind, block_time)
SELECT network, block_number, log_index, tx_hash, emitter, 'order', block_time
FROM shop.nft_event e
WHERE network = $1 AND block_number > $2 AND block_number <= $3
  AND kind = 'order_successful' AND emitter = ANY($4)
  AND EXISTS (SELECT FROM catalog c WHERE c.contract_address=e.contract_address AND c.first_block<=e.block_number)
UNION ALL
SELECT network, block_number, log_index, tx_hash, emitter, 'bid', block_time
FROM shop.sale_event e
WHERE network = $1 AND block_number > $2 AND block_number <= $3
  AND kind = 'bid_accepted' AND emitter = $5
  AND EXISTS (SELECT FROM catalog c WHERE c.contract_address=e.contract_address AND c.first_block<=e.block_number)
UNION ALL
SELECT $1, s.block_number, s.bought_log, s.tx_hash, $8, 'store', min(s.block_time)
FROM shop.verified_store_sale_line s
WHERE $1 = 'matic' AND s.block_number > $2 AND s.block_number <= $3
GROUP BY s.block_number, s.bought_log, s.tx_hash
UNION ALL
SELECT $1, block_number, log_index, tx_hash, venue, 'name_fee', block_time
FROM shop.name_purchase WHERE $1 = 'ethereum' AND venue=ANY($7) AND block_number > $2 AND block_number <= $3
UNION ALL
SELECT network, block_number, log_index, tx_hash, marketplace_address, 'trade', block_time
FROM shop.trade_event t
WHERE network = $1 AND block_number > $2 AND block_number <= $3
  AND kind = 'traded' AND marketplace_address = ANY($6)
  AND (SELECT bool_and(EXISTS (SELECT FROM catalog c
                     WHERE c.contract_address=a->>'contractAddress' AND c.first_block<=t.block_number))
       FROM jsonb_array_elements((assets->'sent') || (assets->'received')) a
       WHERE a->>'assetType' IN ('3','4'));
