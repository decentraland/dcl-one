-- builder.item_contents names an accepted upload (FK to uploaded_contents),
-- and an accepted upload is a blob already in the ONE store with its size.
-- Moving the bytes is out of scope for this import: the blob copy records
-- each stored hash and its size in dcl_migration.blob_inventory (created by the engine) (source
-- 'builder-server', object_key = the hash). A file whose hash is missing
-- there stops the batch rather than declaring an upload the node cannot
-- serve.

DO $missing$ DECLARE n bigint; BEGIN
  SELECT count(DISTINCT s.hash) INTO n FROM stage s
   WHERE NOT EXISTS (SELECT 1 FROM dcl_migration.blob_inventory b
                      WHERE b.source = 'builder-server' AND b.object_key = s.hash);
  IF n > 0 THEN RAISE EXCEPTION
    'builder-server/item_content: % content hashes are not in dcl_migration.blob_inventory; copy the blobs first', n;
  ELSE NULL; END IF; END $missing$;

INSERT INTO builder.uploaded_contents (hash, size, uploaded_by)
SELECT DISTINCT ON (s.hash) s.hash, b.size, lower(s.owner)
FROM stage s
JOIN dcl_migration.blob_inventory b ON b.source = 'builder-server' AND b.object_key = s.hash
ORDER BY s.hash, s.item_id
ON CONFLICT (hash) DO NOTHING;

INSERT INTO builder.item_contents AS c (item_id, file, hash)
SELECT s.item_id, s.file, s.hash
FROM stage s
ON CONFLICT (item_id, file) DO UPDATE SET hash = EXCLUDED.hash
WHERE c.hash IS DISTINCT FROM EXCLUDED.hash;
