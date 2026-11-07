-- credits.ledger is append-only with a generated id, so a replay is
-- recognised by its tx_ref (one per upstream row, 'credits-server:...').
INSERT INTO credits.ledger (address, kind, bucket, amount, tx_ref, captcha_ok, created_at)
SELECT s.address, s.kind, 'paid', s.amount, s.tx_ref, false, s.created_at AT TIME ZONE 'UTC'
FROM stage s
WHERE NOT EXISTS (SELECT 1 FROM credits.ledger l
                   WHERE l.tx_ref = s.tx_ref AND l.address = s.address)
ORDER BY s.address, s.ord, s.tx_ref;

-- The wallet projections, recomputed from the whole ledger of each wallet in
-- the batch (the reconciler's signed sums), so the wallet always equals its
-- ledger whatever ran before. A wallet that would go below zero fails the
-- table's CHECK and stops the run: that wallet needs a human decision.
INSERT INTO credits.wallets AS w (address, available, earned_available)
SELECT l.address,
       SUM(CASE WHEN l.kind IN ('grant', 'refund', 'purchase', 'claim') THEN l.amount ELSE -l.amount END),
       COALESCE(SUM(CASE WHEN l.kind IN ('grant', 'refund', 'purchase', 'claim') THEN l.amount ELSE -l.amount END)
                FILTER (WHERE l.bucket = 'earned'), 0)
FROM credits.ledger l
WHERE l.address IN (SELECT DISTINCT address FROM stage)
GROUP BY l.address
ON CONFLICT (address) DO UPDATE SET
    available = EXCLUDED.available,
    earned_available = EXCLUDED.earned_available,
    updated_at = timezone('UTC', now())
WHERE (w.available, w.earned_available) IS DISTINCT FROM (EXCLUDED.available, EXCLUDED.earned_available);
