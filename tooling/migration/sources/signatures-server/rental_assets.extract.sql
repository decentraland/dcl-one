-- One `metadata` row per asset (its id is the nft id, `contract-token`),
-- keyed by the asset of its rentals: network and chain from the newest
-- rental, fetched_at the newest create (when upstream last read the NFT).
-- Timestamps are `timestamp` holding UTC.
SELECT lower(r.contract_address) AS contract_address,
       r.token_id::numeric AS token_id,
       lower(m.id) AS nft_id,
       r.network,
       r.chain_id,
       m.category,
       m.search_text,
       m.distance_to_plaza,
       m.adjacent_to_road,
       m.estate_size,
       m.created_at AT TIME ZONE 'UTC' AS nft_created_at,
       m.updated_at AT TIME ZONE 'UTC' AS nft_updated_at,
       r.fetched_at AT TIME ZONE 'UTC' AS fetched_at
FROM metadata m
JOIN LATERAL (
    SELECT x.contract_address, x.token_id, x.network, x.chain_id,
           max(x.created_at) OVER () AS fetched_at
    FROM rentals x
    WHERE x.metadata_id = m.id
    ORDER BY x.created_at DESC, x.id
    LIMIT 1
) r ON true
