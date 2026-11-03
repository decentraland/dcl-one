-- The row is the fact; the earliest created_at is kept.
INSERT INTO social.subscription_opt_outs (address, scope, scope_id, created_at)
SELECT address, scope, scope_id, min(created_at)
FROM stage
GROUP BY address, scope, scope_id
ON CONFLICT (address, scope, scope_id) DO UPDATE
   SET created_at = LEAST(social.subscription_opt_outs.created_at, EXCLUDED.created_at)
