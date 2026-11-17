-- The program flag only: available/earned_available are the ledger step's, and
-- is_blocked_for_claiming is the node operator's block, never imported.
-- dcl_migration.credit_wallet_loaded holds the flag this step last wrote: a
-- wallet whose flag no longer matches started the program on the node and
-- keeps it. Without a record only false -> true is applied (the node never
-- sets false, so a true it holds may be its own).
-- The wallets whose flag this load owns before it writes: new ones, recorded ones the node left
-- alone, and unrecorded ones still false. Only these move the record, so a node start that
-- upstream later matches is never recorded as upstream's.
CREATE TEMP TABLE credit_wallet_owned ON COMMIT DROP AS
SELECT s.address
FROM stage s
LEFT JOIN credits.wallets w ON w.address = s.address
LEFT JOIN dcl_migration.credit_wallet_loaded l ON l.address = s.address
WHERE w.address IS NULL
   OR l.has_started_program = w.has_started_program
   OR (l.address IS NULL AND NOT w.has_started_program);

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

-- Record upstream's flag for the wallets this load owns, and first-record an unrecorded wallet
-- that already holds it; an existing record the node diverged from is left as it is.
INSERT INTO dcl_migration.credit_wallet_loaded AS l (address, has_started_program)
SELECT s.address, s.has_started_program
FROM stage s
JOIN credit_wallet_owned o ON o.address = s.address
ON CONFLICT (address) DO UPDATE SET has_started_program = EXCLUDED.has_started_program
WHERE l.has_started_program IS DISTINCT FROM EXCLUDED.has_started_program;

INSERT INTO dcl_migration.credit_wallet_loaded (address, has_started_program)
SELECT s.address, s.has_started_program
FROM stage s
JOIN credits.wallets w ON w.address = s.address AND w.has_started_program = s.has_started_program
ON CONFLICT (address) DO NOTHING;

DROP TABLE credit_wallet_owned;
