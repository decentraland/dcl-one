-- Append-only. Deltas are numeric(78,0) base units, copied as text by COPY
-- (never through a float); `created_at` stays the stored milliseconds.
SELECT l.id,
       l.type,
       l.usdc_delta,
       l.mana_delta,
       l.reference,
       l.metadata,
       l.created_at AS created_at_ms,
       timestamptz 'epoch' + l.created_at * interval '1 millisecond' AS created_at
FROM treasury_ledger l
