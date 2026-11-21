-- A USD-rail upstream purchase is kept as `imported` with its wallet, value
-- and time: it counts toward the caps, and a later delivery finds it
-- credited. Any other upstream transaction is kept as a `refunded` row
-- without a wallet. Either way the node never credits it again (a device's
-- or a notification's later delivery answers it credited or
-- `refunded_transaction`). A later Apple refund charges an `imported`
-- row's Credits back from its wallet; a `refunded` one (MANA credits, never
-- granted on the node) is not reversed. A row the node already holds is
-- left alone.
INSERT INTO credits.iap_transactions
    (transaction_id, product_id, address, credits, amount_cents, status,
     refund_ref, refund_reason, refunded_at, created_at)
SELECT transaction_id, product_id, address,
       CASE WHEN address IS NOT NULL THEN round(amount_cents::numeric / 10, 1) END,
       CASE WHEN address IS NOT NULL THEN amount_cents END,
       CASE WHEN address IS NOT NULL THEN 'imported' ELSE 'refunded' END,
       CASE WHEN address IS NULL THEN 'credits-server' END,
       CASE WHEN address IS NULL THEN 'credited upstream' END,
       CASE WHEN address IS NULL THEN timezone('UTC', now()) END,
       COALESCE(created_at AT TIME ZONE 'UTC', timezone('UTC', now()))
FROM stage
ON CONFLICT (transaction_id) DO NOTHING;
