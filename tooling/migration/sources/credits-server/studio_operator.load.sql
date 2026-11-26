-- dcl_migration.credit_studio_operator_loaded holds the operators this step added. Upstream removes
-- an operator by deleting its row, so a recorded operator missing upstream is removed here too: it
-- must stop granting from the studio's budget. An operator added on the node is never recorded and
-- never touched, and one the node removed after the import is not added back.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND current_setting('dcl_migration.allow_empty', true) IS DISTINCT FROM 'on'
     AND EXISTS (SELECT 1 FROM dcl_migration.credit_studio_operator_loaded) THEN
    RAISE EXCEPTION 'credits-server/studio_operator: upstream lists no operators but imported ones exist; refusing to remove them all (wrong source database? if upstream is really empty, rerun with --allow-empty credits-server/studio_operator)';
  END IF;
END $empty$;

DELETE FROM credits.studio_operators o
USING dcl_migration.credit_studio_operator_loaded l
WHERE l.studio_id = o.studio_id AND l.address = o.address
  AND NOT EXISTS (SELECT 1 FROM stage s WHERE s.studio_id = o.studio_id AND s.address = o.address);

DELETE FROM dcl_migration.credit_studio_operator_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s WHERE s.studio_id = l.studio_id AND s.address = l.address);

-- An operator the node already holds stays the node's (not recorded).
WITH added AS (
    INSERT INTO credits.studio_operators (studio_id, address, added_by, created_at)
    SELECT s.studio_id, s.address, s.added_by, s.created_at AT TIME ZONE 'UTC'
    FROM stage s
    WHERE NOT EXISTS (SELECT 1 FROM dcl_migration.credit_studio_operator_loaded l
                       WHERE l.studio_id = s.studio_id AND l.address = s.address)
    ON CONFLICT (studio_id, address) DO NOTHING
    RETURNING studio_id, address
)
INSERT INTO dcl_migration.credit_studio_operator_loaded (studio_id, address)
SELECT studio_id, address FROM added
ON CONFLICT DO NOTHING;
