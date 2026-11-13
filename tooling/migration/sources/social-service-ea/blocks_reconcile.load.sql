-- `stage` is every pair upstream holds (a `whole` step). A pair this source
-- wrote and upstream no longer holds goes, unless the node changed it since
-- (its blocked_at differs from the recorded one).
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.social_block_loaded) THEN
    RAISE EXCEPTION 'social-service-ea/blocks_reconcile: upstream lists no blocks but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE block_gone ON COMMIT DROP AS
SELECT l.blocker_address, l.blocked_address, l.blocked_at
FROM dcl_migration.social_block_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s WHERE s.blocker_address = l.blocker_address AND s.blocked_address = l.blocked_address);

DELETE FROM social.blocks t
USING block_gone g
WHERE t.blocker_address = g.blocker_address AND t.blocked_address = g.blocked_address AND t.blocked_at = g.blocked_at;

DELETE FROM dcl_migration.social_block_loaded l
USING block_gone g
WHERE l.blocker_address = g.blocker_address AND l.blocked_address = g.blocked_address;

DROP TABLE block_gone;
