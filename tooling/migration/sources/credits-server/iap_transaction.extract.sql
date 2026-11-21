-- Every App Store transaction upstream credited (each credit row survives a
-- refund, `iap_credits_survive_refund`). A USD-rail one carries its wallet,
-- cents and time from its `user_credits` row, so the node's per-wallet caps
-- count it; its Credits are already in the ledger step's grants. Run after
-- upstream's App Store routes stop writing: a purchase credited upstream
-- after this read would not be recorded.
SELECT i.apple_transaction_id AS transaction_id,
       COALESCE(NULLIF(i.apple_product_id, ''), 'unknown') AS product_id,
       lower(u.user_address) AS address,
       u.usd_cents AS amount_cents,
       to_timestamp(u.timestamp / 1000.0) AS created_at
FROM iap_credits i
LEFT JOIN user_credits u
  ON u.id = i.credit_id AND u.denomination = 'USD' AND u.usd_cents > 0
