-- Entries are append-only on both sides and keep upstream's id, so after every run each studio's
-- balance (the sum of its entries) is upstream's. A grant names the credit it gave: the ledger row
-- the ledger step imported as `credits-server:credit:<credit_id>`. A grant whose credit is not in
-- the ledger yet (a USD credit not yet available, which a later run imports) is carried as an
-- `adjustment` of the same amount whose note names the credit, so the balance stays exact; the
-- run that finds the credit turns it into the grant.
WITH linked AS (
    SELECT s.*,
           CASE WHEN s.kind = 'grant' THEN
               (SELECT l.id FROM credits.ledger l
                 WHERE l.tx_ref = 'credits-server:credit:' || s.credit_id
                 ORDER BY l.id LIMIT 1)
           END AS ledger_id
    FROM stage s
)
INSERT INTO credits.studio_budget_entries AS e
    (id, studio_id, usd_cents, kind, ledger_id, note, idempotency_key, request_hash, created_by, created_at)
SELECT id, studio_id, usd_cents,
       CASE WHEN kind = 'grant' AND ledger_id IS NULL THEN 'adjustment' ELSE kind END,
       ledger_id,
       CASE WHEN kind = 'grant' AND ledger_id IS NULL
            THEN concat_ws(' ', note, '(credits-server grant of credit ' || credit_id || ', not imported yet)')
            ELSE note
       END,
       idempotency_key, request_hash, created_by, created_at AT TIME ZONE 'UTC'
FROM linked
ON CONFLICT (id) DO UPDATE SET
    kind = 'grant',
    ledger_id = EXCLUDED.ledger_id,
    note = EXCLUDED.note
WHERE e.kind = 'adjustment' AND EXCLUDED.kind = 'grant';
