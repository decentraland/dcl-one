-- One row per upstream collection. The urn and contract are rendered the way
-- the node writes them: a standard collection carries its collections-v2 urn
-- and contract only once published (upstream stores the CREATE2 address from
-- the draft on); a linked (third-party) collection carries its full provider
-- urn, third_party_id || ':' || urn_suffix.
SELECT c.id,
       c.name,
       c.eth_address,
       c.salt,
       CASE WHEN c.is_published AND c.third_party_id IS NULL THEN lower(c.contract_address) END AS contract_address,
       CASE
           WHEN c.third_party_id IS NOT NULL AND c.urn_suffix IS NOT NULL
               THEN c.third_party_id || ':' || c.urn_suffix
           WHEN c.third_party_id IS NULL AND c.is_published AND c.contract_address IS NOT NULL
               THEN 'urn:decentraland:matic:collections-v2:' || lower(c.contract_address)
       END AS urn_suffix,
       c.third_party_id,
       c.is_published,
       c.is_approved,
       c.created_at AT TIME ZONE 'UTC' AS created_at,
       c.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM collections c
