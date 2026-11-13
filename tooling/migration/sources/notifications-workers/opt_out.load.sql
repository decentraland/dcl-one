-- The row is the fact; the earliest created_at is kept. Rows this load
-- inserted are recorded with the created_at they hold, for opt_out_reconcile;
-- an opt-out the node already held is the node's and is not recorded.
WITH upserted AS (
    INSERT INTO social.subscription_opt_outs (address, scope, scope_id, created_at)
    SELECT address, scope, scope_id, min(created_at)
    FROM stage
    GROUP BY address, scope, scope_id
    ON CONFLICT (address, scope, scope_id) DO UPDATE
       SET created_at = LEAST(social.subscription_opt_outs.created_at, EXCLUDED.created_at)
    RETURNING address, scope, scope_id, created_at, xmax = 0 AS inserted
)
INSERT INTO dcl_migration.notification_opt_out_loaded (address, scope, scope_id, created_at)
SELECT u.address, u.scope, u.scope_id, u.created_at
FROM upserted u
WHERE u.inserted
   OR EXISTS (SELECT 1 FROM dcl_migration.notification_opt_out_loaded l
              WHERE l.address = u.address AND l.scope = u.scope AND l.scope_id = u.scope_id)
ON CONFLICT (address, scope, scope_id) DO UPDATE SET created_at = EXCLUDED.created_at
