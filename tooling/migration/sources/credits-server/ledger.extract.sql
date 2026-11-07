-- The USD money history as ledger movements, in Credits (usd_cents / 10, the
-- shared 10 cents per Credit; one decimal, exact):
-- - every USD user_credits row already available (available_at unset or
--   past): a purchase credit -> 'purchase', any other positive row (grant,
--   studio_grant, iap, sale_proceeds, sale_royalty, dispute_won) -> 'grant',
--   a negative row (a refund/charge-back claw-back) -> 'consume';
-- - every SETTLED USD purchase intent (a credit consumed on-chain) -> 'spend'.
-- `ord` puts a wallet's credits before its debits so a batch boundary never
-- leaves a partial wallet below zero. Upstream spendable USD is exactly
-- credits - SETTLED - held intents; the cutover guard proves held = 0.
SELECT lower(uc.user_address) AS address,
       CASE WHEN uc.usd_cents > 0 THEN 0 ELSE 1 END AS ord,
       'credits-server:credit:' || uc.id AS tx_ref,
       CASE
           WHEN uc.usd_cents < 0 THEN 'consume'
           WHEN uc.credit_source = 'purchase' THEN 'purchase'
           ELSE 'grant'
       END AS kind,
       round(abs(uc.usd_cents)::numeric / 10, 1) AS amount,
       to_timestamp(COALESCE(uc.available_at, uc.timestamp) / 1000.0) AS created_at
FROM user_credits uc
WHERE uc.denomination = 'USD'
  AND uc.usd_cents IS NOT NULL AND uc.usd_cents <> 0
  AND (uc.available_at IS NULL OR uc.available_at <= extract(epoch FROM now()) * 1000)
UNION ALL
SELECT lower(i.user_address),
       1,
       'credits-server:intent:' || i.id::text,
       'spend',
       round(i.usd_cents::numeric / 10, 1),
       to_timestamp(i.created_at / 1000.0)
FROM usd_purchase_intents i
WHERE i.status = 'SETTLED'
