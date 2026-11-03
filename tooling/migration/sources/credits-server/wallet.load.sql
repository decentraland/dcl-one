-- The flags only: available/earned_available are projections of the ledger
-- and are written by the ledger step.
INSERT INTO credits.wallets AS w
    (address, has_started_program, is_blocked_for_claiming, created_at, updated_at)
SELECT address, has_started_program, is_blocked_for_claiming,
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (address) DO UPDATE SET
    has_started_program = EXCLUDED.has_started_program,
    is_blocked_for_claiming = EXCLUDED.is_blocked_for_claiming,
    created_at = LEAST(w.created_at, EXCLUDED.created_at),
    updated_at = GREATEST(w.updated_at, EXCLUDED.updated_at)
WHERE (w.has_started_program, w.is_blocked_for_claiming)
      IS DISTINCT FROM (EXCLUDED.has_started_program, EXCLUDED.is_blocked_for_claiming)
   OR EXCLUDED.created_at < w.created_at;
