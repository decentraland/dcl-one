-- `stage` is every pair upstream holds (a `whole` step). A pair this source
-- wrote and upstream no longer holds goes, unless the node changed it since
-- (its muted_at differs from the recorded one).
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.social_user_mute_loaded) THEN
    RAISE EXCEPTION 'social-service-ea/user_mutes_reconcile: upstream lists no mutes but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE user_mute_gone ON COMMIT DROP AS
SELECT l.muter_address, l.muted_address, l.muted_at
FROM dcl_migration.social_user_mute_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s WHERE s.muter_address = l.muter_address AND s.muted_address = l.muted_address);

DELETE FROM social.user_mutes t
USING user_mute_gone g
WHERE t.muter_address = g.muter_address AND t.muted_address = g.muted_address AND t.muted_at = g.muted_at;

DELETE FROM dcl_migration.social_user_mute_loaded l
USING user_mute_gone g
WHERE l.muter_address = g.muter_address AND l.muted_address = g.muted_address;

DROP TABLE user_mute_gone;
