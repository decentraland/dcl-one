-- One row per Stripe Checkout order. `credits` is what the order's purchase
-- credit row actually granted (usd_cents / 10, the shared 10 cents per
-- Credit), so credits.purchases agrees with the 'purchase' ledger rows by
-- construction; an order never credited keeps the pack's credits.
-- `revoked_credits` is what its refund rows clawed back net of dispute
-- restorations; `refunded_cents` the fiat reversed net of restorations.
SELECT o.id::text AS order_id,
       lower(o.user_address) AS address,
       o.pack_id AS sku,
       COALESCE(round(p.usd_cents::numeric / 10, 1), o.credits::numeric) AS credits,
       o.usd_cents AS amount_cents,
       o.quantity,
       o.marketplace_source AS source,
       o.status AS upstream_status,
       (p.id IS NOT NULL) AS credited,
       o.session_id,
       o.payment_intent_id,
       GREATEST(0, COALESCE(o.reversal_usd_cents, 0) - COALESCE(o.restored_usd_cents, 0)) AS refunded_cents,
       GREATEST(0, round(-COALESCE(adj.cents, 0)::numeric / 10, 1)) AS revoked_credits,
       o.error AS failure_reason,
       to_timestamp(o.created_at / 1000.0) AS created_at,
       to_timestamp(o.updated_at / 1000.0) AS updated_at
FROM stripe_orders o
LEFT JOIN user_credits p
  ON p.stripe_order_id = o.id::text AND p.credit_source = 'purchase' AND p.denomination = 'USD'
LEFT JOIN LATERAL (
    SELECT SUM(a.usd_cents) AS cents
    FROM user_credits a
    WHERE a.stripe_order_id = o.id::text AND a.denomination = 'USD'
      AND a.credit_source IN ('refund', 'dispute_won')
) adj ON true
