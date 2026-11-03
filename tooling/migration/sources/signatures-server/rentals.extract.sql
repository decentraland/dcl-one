-- A rental and its listing row (1:1). Addresses lowercased as the node
-- stores them; the signature is copied as stored.
SELECT r.id,
       lower(r.contract_address) AS contract_address,
       r.token_id::numeric AS token_id,
       r.network,
       r.chain_id,
       r.expiration AT TIME ZONE 'UTC' AS expiration,
       r.nonces,
       r.signature,
       lower(r.rental_contract_address) AS rental_contract_address,
       r.status::text AS status,
       lower(COALESCE(r.target, '0x0000000000000000000000000000000000000000')) AS target,
       r.rented_days,
       r.period_chosen,
       r.created_at AT TIME ZONE 'UTC' AS created_at,
       r.updated_at AT TIME ZONE 'UTC' AS updated_at,
       r.started_at AT TIME ZONE 'UTC' AS started_at,
       lower(l.lessor) AS lessor,
       NULLIF(lower(l.tenant), '') AS tenant
FROM rentals r
JOIN rentals_listings l ON l.id = r.id
