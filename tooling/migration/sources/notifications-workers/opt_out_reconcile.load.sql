-- `stage` is the whole upstream set (a `whole` step). An opt-out this source
-- imported and upstream no longer holds goes, unless the node changed it
-- since (its created_at differs from the recorded one): an opt-out the node
-- already held, or recreated, is the node's.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.notification_opt_out_loaded) THEN
    RAISE EXCEPTION 'notifications-workers/opt_out_reconcile: upstream lists no opt-outs but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE opt_out_gone ON COMMIT DROP AS
SELECT l.address, l.scope, l.scope_id, l.created_at
FROM dcl_migration.notification_opt_out_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s
                  WHERE s.address = l.address AND s.scope = l.scope AND s.scope_id = l.scope_id);

DELETE FROM social.subscription_opt_outs o
USING opt_out_gone g
WHERE o.address = g.address AND o.scope = g.scope AND o.scope_id = g.scope_id
  AND o.created_at = g.created_at;

DELETE FROM dcl_migration.notification_opt_out_loaded l
USING opt_out_gone g
WHERE l.address = g.address AND l.scope = g.scope AND l.scope_id = g.scope_id;

DROP TABLE opt_out_gone;
