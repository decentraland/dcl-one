SELECT i.id,
       i.collection_id,
       i.eth_address,
       i.name,
       i.description,
       i.thumbnail,
       i.video,
       i.type,
       i.rarity,
       i.price,
       i.beneficiary,
       i.data,
       i.metrics,
       i.utility,
       i.mappings::jsonb AS mappings,
       CASE
           WHEN c.third_party_id IS NOT NULL AND c.urn_suffix IS NOT NULL AND i.urn_suffix IS NOT NULL
               THEN c.third_party_id || ':' || c.urn_suffix || ':' || i.urn_suffix
           WHEN c.third_party_id IS NULL AND c.is_published AND c.contract_address IS NOT NULL
                AND i.blockchain_item_id IS NOT NULL
               THEN 'urn:decentraland:matic:collections-v2:' || lower(c.contract_address) || ':' || i.blockchain_item_id
       END AS urn_suffix,
       i.blockchain_item_id,
       (c.third_party_id IS NULL AND c.is_published IS TRUE AND c.contract_address IS NOT NULL
        AND i.blockchain_item_id IS NOT NULL) AS is_published,
       i.total_supply,
       i.local_content_hash,
       ic.content_hash,
       i.created_at AT TIME ZONE 'UTC' AS created_at,
       i.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM items i
LEFT JOIN collections c ON c.id = i.collection_id
LEFT JOIN LATERAL (
    SELECT content_hash
    FROM item_curations
    WHERE item_id = i.id
    ORDER BY created_at DESC, id DESC
    LIMIT 1
) ic ON true
