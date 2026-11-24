SELECT c.id,
       c.name,
       c.eth_address,
       c.salt,
       CASE WHEN c.third_party_id IS NULL THEN lower(c.contract_address) END AS contract_address,
       CASE
           WHEN c.third_party_id IS NOT NULL AND c.urn_suffix IS NOT NULL
               THEN c.third_party_id || ':' || c.urn_suffix
           WHEN c.third_party_id IS NULL AND c.is_published AND c.contract_address IS NOT NULL
               THEN 'urn:decentraland:matic:collections-v2:' || lower(c.contract_address)
       END AS urn_suffix,
       c.third_party_id,
       c.created_at AT TIME ZONE 'UTC' AS created_at,
       c.updated_at AT TIME ZONE 'UTC' AS updated_at,
       lower(c.linked_contract_address) AS linked_contract_address,
       c.linked_contract_network,
       c.lock,
       c.forum_link,
       c.forum_id
FROM collections c
