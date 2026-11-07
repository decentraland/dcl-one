-- Drafts are the creator's local authority on the node; an upstream re-run
-- overwrites the imported columns. publication_pending starts false: the
-- upstream `lock` is a short-lived publish lock, not a durable state.
INSERT INTO builder.collections AS c
    (id, name, eth_address, salt, contract_address, urn_suffix, third_party_id,
     is_published, is_approved, publication_pending, created_at, updated_at)
SELECT id, btrim(name), lower(eth_address), salt, contract_address, urn_suffix, third_party_id,
       is_published, is_approved, false,
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    eth_address = EXCLUDED.eth_address,
    salt = EXCLUDED.salt,
    contract_address = EXCLUDED.contract_address,
    urn_suffix = EXCLUDED.urn_suffix,
    third_party_id = EXCLUDED.third_party_id,
    is_published = EXCLUDED.is_published,
    is_approved = EXCLUDED.is_approved,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE (c.name, c.eth_address, c.salt, c.contract_address, c.urn_suffix, c.third_party_id,
       c.is_published, c.is_approved, c.created_at, c.updated_at)
      IS DISTINCT FROM
      (EXCLUDED.name, EXCLUDED.eth_address, EXCLUDED.salt, EXCLUDED.contract_address, EXCLUDED.urn_suffix,
       EXCLUDED.third_party_id, EXCLUDED.is_published, EXCLUDED.is_approved, EXCLUDED.created_at,
       EXCLUDED.updated_at);
