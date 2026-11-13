-- The program flag only: available/earned_available are the ledger step's, and
-- is_blocked_for_claiming is the node operator's block, never imported.
-- dcl_migration.credit_wallet_loaded holds the flag this step last wrote: a
-- wallet whose flag no longer matches started the program on the node and
-- keeps it. Without a record only false -> true is applied (the node never
-- sets false, so a true it holds may be its own).
INSERT INTO credits.wallets AS w (address, has_started_program, created_at, updated_at)
SELECT address, has_started_program, created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (address) DO UPDATE SET
    has_started_program = CASE
        WHEN w.has_started_program = EXCLUDED.has_started_program THEN w.has_started_program
        WHEN EXISTS (SELECT 1 FROM dcl_migration.credit_wallet_loaded l
                     WHERE l.address = w.address AND l.has_started_program = w.has_started_program)
          OR (NOT w.has_started_program
              AND NOT EXISTS (SELECT 1 FROM dcl_migration.credit_wallet_loaded l WHERE l.address = w.address))
            THEN EXCLUDED.has_started_program
        ELSE w.has_started_program
    END,
    created_at = LEAST(w.created_at, EXCLUDED.created_at),
    updated_at = GREATEST(w.updated_at, EXCLUDED.updated_at)
WHERE w.has_started_program IS DISTINCT FROM EXCLUDED.has_started_program
   OR EXCLUDED.created_at < w.created_at;

-- Record the flag where the wallet now holds upstream's value.
INSERT INTO dcl_migration.credit_wallet_loaded AS l (address, has_started_program)
SELECT s.address, s.has_started_program
FROM stage s
JOIN credits.wallets w ON w.address = s.address AND w.has_started_program = s.has_started_program
ON CONFLICT (address) DO UPDATE SET has_started_program = EXCLUDED.has_started_program
WHERE l.has_started_program IS DISTINCT FROM EXCLUDED.has_started_program;
