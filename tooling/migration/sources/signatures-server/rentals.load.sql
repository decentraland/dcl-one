-- Upstream ids kept. Skipped: a signature the node holds under another id,
-- an asset the previous step did not land, and a second open listing of an
-- asset (the node's one-open-per-asset rule).
WITH accepted AS (
INSERT INTO signatures.rentals
    (id, contract_address, token_id, network, chain_id, expiration, nonces, signature,
     rental_contract_address, status, target, rented_days, period_chosen, created_at, updated_at, started_at)
SELECT s.id, s.contract_address, s.token_id, s.network, s.chain_id, s.expiration AT TIME ZONE 'UTC',
       s.nonces, s.signature, s.rental_contract_address, s.status::signatures.rental_status, s.target,
       s.rented_days, s.period_chosen, s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC',
       s.started_at AT TIME ZONE 'UTC'
FROM stage s
JOIN signatures.rental_assets a ON a.contract_address = s.contract_address AND a.token_id = s.token_id
WHERE NOT EXISTS (SELECT 1 FROM signatures.rentals r WHERE r.signature = s.signature AND r.id <> s.id)
  AND NOT (s.status = 'open' AND EXISTS (
        SELECT 1 FROM signatures.rentals r
        WHERE r.contract_address = s.contract_address AND r.token_id = s.token_id
          AND r.status = 'open' AND r.id <> s.id))
ON CONFLICT (id) DO UPDATE
   SET expiration = EXCLUDED.expiration, nonces = EXCLUDED.nonces,
       rental_contract_address = EXCLUDED.rental_contract_address, status = EXCLUDED.status,
       target = EXCLUDED.target, rented_days = EXCLUDED.rented_days,
       period_chosen = EXCLUDED.period_chosen, created_at = EXCLUDED.created_at,
       updated_at = EXCLUDED.updated_at, started_at = EXCLUDED.started_at
 WHERE signatures.rentals.signature = EXCLUDED.signature
   AND signatures.rentals.updated_at <= EXCLUDED.updated_at
RETURNING id, signature
)

INSERT INTO signatures.rental_listings (id, lessor, tenant)
SELECT s.id, s.lessor, s.tenant
FROM stage s
JOIN accepted r ON r.id = s.id AND r.signature = s.signature
ON CONFLICT (id) DO UPDATE
   SET lessor = EXCLUDED.lessor, tenant = COALESCE(EXCLUDED.tenant, signatures.rental_listings.tenant);
