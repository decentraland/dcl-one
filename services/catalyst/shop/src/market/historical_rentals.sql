WITH candidates AS (
    SELECT DISTINCT e.contract_address,e.token_id
    FROM shop.rental_event e
    WHERE e.network='ethereum' AND e.rentals_address=$6 AND e.kind='rented'
      AND e.signer=ANY($2::text[]) AND e.block_number<=$1
      AND (cardinality($3::text[])=0 OR e.contract_address=ANY($3))
)
SELECT jsonb_build_object('contractAddress',c.contract_address,'tokenId',c.token_id::text,
                          'lessor',latest.signer,'isClaimed',false)
FROM candidates c CROSS JOIN LATERAL (
    SELECT e.kind,e.signer FROM shop.rental_event e
    WHERE e.network='ethereum' AND e.rentals_address=$6 AND e.contract_address IS NOT NULL
      AND (e.contract_address,e.token_id)=(c.contract_address,c.token_id)
      AND e.kind IN ('rented','claimed') AND e.block_number<=$1
    ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
) latest
WHERE latest.kind='rented' AND latest.signer=ANY($2)
ORDER BY (c.contract_address || '-' || c.token_id::text) COLLATE "C" LIMIT $4 OFFSET $5
