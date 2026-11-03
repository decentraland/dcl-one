-- Upstream money still in flight at cutover. Each row is a reason the
-- import would not reconcile: a PENDING USD intent (a live signed credit the
-- balance holds), an EXPIRED intent still inside upstream's one-hour
-- recovery horizon (it can still turn SETTLED), or a Stripe order between
-- checkout and crediting. The load refuses any of them.
SELECT 'intent:' || i.id::text AS ref, i.status AS state
FROM usd_purchase_intents i
WHERE i.status = 'PENDING'
   OR (i.status = 'EXPIRED' AND i.expires_at + 3600 > extract(epoch FROM now()))
UNION ALL
SELECT 'order:' || o.id::text, o.status
FROM stripe_orders o
WHERE o.status IN ('processing', 'crediting')
   OR (o.status = 'initiated' AND o.created_at > (extract(epoch FROM now()) - 86400) * 1000)
