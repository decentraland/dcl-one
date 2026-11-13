-- The program flag only: available/earned_available are the ledger step's, and
-- is_blocked_for_claiming is the node operator's block, never imported.
INSERT INTO credits.wallets AS w (address, has_started_program, created_at, updated_at)
SELECT address, has_started_program, created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (address) DO UPDATE SET
    has_started_program = EXCLUDED.has_started_program,
    created_at = LEAST(w.created_at, EXCLUDED.created_at),
    updated_at = GREATEST(w.updated_at, EXCLUDED.updated_at)
WHERE w.has_started_program IS DISTINCT FROM EXCLUDED.has_started_program
   OR EXCLUDED.created_at < w.created_at;
