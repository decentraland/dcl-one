-- `stage` is upstream's whole key set (a `whole` step). A row this source
-- imported whose upstream key is gone is deleted, unless the node wrote it
-- since (its updated_at differs from the recorded one) or another upstream
-- key still lands on it. Rows the import never recorded (the node's own, or
-- retired scopes upstream still holds) are never touched.
DO $empty$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM stage)
     AND EXISTS (SELECT 1 FROM dcl_migration.wss_player_storage_loaded) THEN
    RAISE EXCEPTION 'world-storage-service/player_storage_reconcile: upstream lists no player values but imported ones are recorded; refusing to delete them all (wrong source database?)';
  END IF;
END $empty$;

CREATE TEMP TABLE wss_gone ON COMMIT DROP AS
SELECT l.* FROM dcl_migration.wss_player_storage_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s
                  WHERE (s.world_name, s.place_id, s.player_address, s.key) = (l.world_name, l.place_id, l.player_address, l.key));

DELETE FROM dcl_migration.wss_player_storage_loaded l USING wss_gone g
WHERE (l.world_name, l.place_id, l.player_address, l.key) = (g.world_name, g.place_id, g.player_address, g.key);

DELETE FROM worlds.player_storage t USING wss_gone g
WHERE (t.realm, t.scene_namespace, t.player_address, t.key) = (g.realm, g.scene_namespace, g.player, g.key)
  AND t.updated_at = g.updated_at
  AND NOT EXISTS (SELECT 1 FROM dcl_migration.wss_player_storage_loaded l
                  WHERE (l.realm, l.scene_namespace, l.player, l.key)
                      = (t.realm, t.scene_namespace, t.player_address, t.key));

DROP TABLE wss_gone;
