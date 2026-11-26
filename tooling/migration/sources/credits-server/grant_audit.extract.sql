-- The metadata of the positive USD grants eligible for the ledger load.
SELECT odc.credit_id,
       lower(uc.user_address) AS address,
       uc.credit_source,
       lower(odc.granter_address) AS actor,
       odc.reason,
       odc.annotation,
       to_timestamp(uc.timestamp / 1000.0) AS granted_at
FROM on_demand_credits odc
JOIN user_credits uc ON uc.id = odc.credit_id
WHERE uc.denomination = 'USD'
  AND uc.credit_source IN ('grant', 'studio_grant')
  AND uc.usd_cents > 0
  AND (uc.available_at IS NULL OR uc.available_at <= extract(epoch FROM now()) * 1000)
