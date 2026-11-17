INSERT INTO signatures.rental_assets
    (contract_address, token_id, nft_id, network, chain_id, category, search_text,
     distance_to_plaza, adjacent_to_road, estate_size, nft_created_at, nft_updated_at, fetched_at)
SELECT s.contract_address, s.token_id, s.nft_id, s.network, s.chain_id, s.category, s.search_text,
       s.distance_to_plaza, s.adjacent_to_road, s.estate_size,
       s.nft_created_at AT TIME ZONE 'UTC', s.nft_updated_at AT TIME ZONE 'UTC', s.fetched_at AT TIME ZONE 'UTC'
FROM stage s
WHERE NOT EXISTS (SELECT 1 FROM signatures.rental_assets a
                  WHERE a.nft_id = s.nft_id
                    AND (a.contract_address, a.token_id) <> (s.contract_address, s.token_id))
ON CONFLICT (contract_address, token_id) DO UPDATE
   SET nft_id = EXCLUDED.nft_id, network = EXCLUDED.network, chain_id = EXCLUDED.chain_id,
       category = EXCLUDED.category, search_text = EXCLUDED.search_text,
       distance_to_plaza = EXCLUDED.distance_to_plaza, adjacent_to_road = EXCLUDED.adjacent_to_road,
       estate_size = EXCLUDED.estate_size, nft_created_at = EXCLUDED.nft_created_at,
       nft_updated_at = EXCLUDED.nft_updated_at, fetched_at = EXCLUDED.fetched_at
 WHERE signatures.rental_assets.fetched_at <= EXCLUDED.fetched_at;
