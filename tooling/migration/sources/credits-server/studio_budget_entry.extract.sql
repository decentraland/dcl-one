-- Every budget entry: deposits add, grants subtract (naming the credit they gave), adjustments go
-- either way. A studio's balance is the sum of its entries, on both sides.
SELECT e.id, e.studio_id, e.usd_cents, e.kind, e.credit_id, e.note, e.idempotency_key, e.request_hash,
       e.created_by, to_timestamp(e.created_at / 1000.0) AS created_at
FROM studio_budget_entries e
