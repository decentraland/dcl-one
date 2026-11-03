-- credits.purchases keyed by the upstream order id (order_id; the bigint id
-- is the node's identity). A credited order is 'paid' until its reversals
-- cover the charge, then 'refunded', the way the node's webhook moves it, so
-- a later Stripe reversal of an imported charge claws back only the rest.
INSERT INTO credits.purchases AS p
    (address, sku, credits, amount_cents, quantity, source, currency, method, status,
     order_id, stripe_payment_intent, stripe_checkout_session, refunded_cents,
     revoked_credits, failure_reason, created_at, updated_at)
SELECT s.address, s.sku, s.credits, s.amount_cents, s.quantity, s.source, 'usd', 'card',
       CASE
           WHEN s.credited AND LEAST(s.refunded_cents, s.amount_cents) >= s.amount_cents THEN 'refunded'
           WHEN s.credited THEN 'paid'
           WHEN s.upstream_status = 'abandoned' THEN 'abandoned'
           WHEN s.upstream_status IN ('initiated', 'processing', 'crediting') THEN 'abandoned'
           ELSE 'failed' END,
       s.order_id, s.payment_intent_id, s.session_id,
       LEAST(s.refunded_cents, s.amount_cents),
       LEAST(s.revoked_credits, s.credits),
       s.failure_reason,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (order_id) WHERE order_id IS NOT NULL DO UPDATE SET
    address = EXCLUDED.address,
    sku = EXCLUDED.sku,
    credits = EXCLUDED.credits,
    amount_cents = EXCLUDED.amount_cents,
    quantity = EXCLUDED.quantity,
    source = EXCLUDED.source,
    status = EXCLUDED.status,
    stripe_payment_intent = EXCLUDED.stripe_payment_intent,
    stripe_checkout_session = EXCLUDED.stripe_checkout_session,
    refunded_cents = EXCLUDED.refunded_cents,
    revoked_credits = EXCLUDED.revoked_credits,
    failure_reason = EXCLUDED.failure_reason,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE (p.address, p.sku, p.credits, p.amount_cents, p.quantity, p.source, p.status,
       p.stripe_payment_intent, p.stripe_checkout_session, p.refunded_cents, p.revoked_credits,
       p.failure_reason, p.created_at, p.updated_at)
      IS DISTINCT FROM
      (EXCLUDED.address, EXCLUDED.sku, EXCLUDED.credits, EXCLUDED.amount_cents, EXCLUDED.quantity,
       EXCLUDED.source, EXCLUDED.status, EXCLUDED.stripe_payment_intent,
       EXCLUDED.stripe_checkout_session, EXCLUDED.refunded_cents, EXCLUDED.revoked_credits,
       EXCLUDED.failure_reason, EXCLUDED.created_at, EXCLUDED.updated_at);
