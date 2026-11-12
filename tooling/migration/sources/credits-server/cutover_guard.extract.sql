-- Fail closed: EXPIRED is not proof of released liability. Upstream also
-- checks the active squid deployment's head and indexed consumption, which
-- this snapshot cannot establish. Age alone never releases a hold here.
SELECT 'intent:' || i.id::text AS ref, i.status AS state
FROM usd_purchase_intents i
WHERE i.status IN ('PENDING', 'EXPIRED')
UNION ALL
SELECT 'order:' || o.id::text, o.status
FROM stripe_orders o
WHERE o.status IN ('processing', 'crediting')
   OR (o.status = 'initiated' AND o.created_at > (extract(epoch FROM now()) - 86400) * 1000)

UNION ALL
SELECT 'dispute:' || d.dispute_id, COALESCE(d.outcome, 'unresolved')
FROM stripe_payment_disputes d
WHERE d.outcome IS NULL
   OR d.outcome NOT IN ('won', 'lost', 'warning_closed', 'charge_refunded')
   OR d.reconciled_at IS NULL OR d.reconciled_at < d.updated_at
UNION ALL
SELECT 'refund:' || r.payment_intent_id, 'unreconciled'
FROM stripe_payment_refunds r
WHERE r.reconciled_at IS NULL OR r.reconciled_at < r.updated_at
