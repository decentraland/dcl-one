WITH land_candidates AS (
    SELECT DISTINCT token_address,token_id
    FROM land_authz.authz_event
    WHERE token_address=ANY($10::text[]) AND kind='transfer' AND token_id IS NOT NULL
      AND block_number<=$1 AND account=ANY($2::text[])
), land AS (
    SELECT c.token_address AS contract_address,c.token_id,
           CASE WHEN c.token_address=$11 THEN 'estate' ELSE 'parcel' END AS category,
           latest.account AS owner,NULL::text AS item_type,NULL::text AS rarity
    FROM land_candidates c
    CROSS JOIN LATERAL (
        SELECT e.account FROM land_authz.authz_event e
        WHERE (e.token_address,e.token_id)=(c.token_address,c.token_id)
          AND e.token_id IS NOT NULL AND e.kind='transfer' AND e.block_number<=$1
        ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
    ) latest
    WHERE latest.account=ANY($2)
), nft_candidates AS (
    SELECT DISTINCT e.contract_address,e.token_id
    FROM shop.nft_event e
    WHERE e.network='ethereum' AND e.kind='transfer' AND e.block_number<=$1 AND e.to_address=ANY($2)
      AND (cardinality($3::text[])=0 OR e.contract_address=ANY($3))
      AND (($13 AND e.contract_address=$12) OR ($14 AND e.contract_address<>$12))
), nft AS (
    SELECT c.contract_address,c.token_id,
           CASE WHEN c.contract_address=$12 THEN 'ens' WHEN i.item_type='emote_v1' THEN 'emote' ELSE 'wearable' END AS category,
           latest.to_address AS owner,i.item_type,i.rarity
    FROM nft_candidates c
    CROSS JOIN LATERAL (
        SELECT e.to_address FROM shop.nft_event e
        WHERE e.network='ethereum' AND (e.contract_address,e.token_id)=(c.contract_address,c.token_id)
          AND e.kind='transfer' AND e.block_number<=$1
        ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
    ) latest
    LEFT JOIN LATERAL (
        SELECT e.detail FROM shop.nft_event e
        WHERE $14 AND e.network='ethereum' AND (e.contract_address,e.token_id)=(c.contract_address,c.token_id)
          AND e.kind='issue_v1' AND e.block_number<=$1
        ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
    ) issue ON c.contract_address<>$12
    LEFT JOIN shop.collections co ON co.network='ethereum' AND co.chain_id=1 AND co.contract_address=c.contract_address
    LEFT JOIN shop.items i ON i.network='ethereum' AND i.chain_id=1 AND i.contract_address=c.contract_address
        AND i.urn=co.urn || ':' || issue.detail
    WHERE latest.to_address=ANY($2) AND (c.contract_address=$12 OR i.item_type IS NOT NULL)
), owned AS MATERIALIZED (
    SELECT *,contract_address || '-' || token_id::text AS id FROM (
        SELECT * FROM land UNION ALL SELECT * FROM nft
    ) composed
    WHERE ($5::text IS NULL OR category=$5)
      AND (cardinality($4::text[])=0 OR item_type=ANY($4))
), estate_sizes AS (
    SELECT e.estate_id,sum(CASE WHEN e.kind='add_land' THEN 1 ELSE -1 END) AS size
    FROM land_authz.authz_event e JOIN owned o ON e.estate_id=o.token_id AND o.category='estate'
    WHERE e.token_address=$11 AND e.block_number<=$1 AND e.kind IN ('add_land','remove_land')
    GROUP BY e.estate_id
), result AS (
    SELECT o.*,CASE WHEN o.category='estate' THEN coalesce(s.size,0) END AS estate_size
    FROM owned o LEFT JOIN estate_sizes s ON o.category='estate' AND s.estate_id=o.token_id
)
SELECT jsonb_build_object(
    'contractAddress',contract_address,'tokenId',token_id::text,'category',category,'owner',owner,
    'itemType',item_type,'searchWearableRarity',rarity,'searchEstateSize',estate_size)
FROM result
WHERE ($6::bigint IS NULL OR estate_size>$6) AND ($7::text IS NULL OR id COLLATE "C">$7)
ORDER BY id COLLATE "C" LIMIT $8 OFFSET $9
