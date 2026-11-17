-- shop.outfit_thumbnails names its bytes by blob cid in the ONE store.
-- Moving them is out of scope here: the blob copy records each thumbnail's
-- sha256 hex (object_key), the cid it stored the bytes under (hash,
-- `hash_bytes_v1`) and their size in dcl_migration.blob_inventory (source
-- 'shop-server'). A thumbnail missing there, or stored with another size,
-- stops the batch rather than declaring a row the node cannot serve.
DO $missing$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE NOT EXISTS (SELECT 1 FROM dcl_migration.blob_inventory b
                      WHERE b.source = 'shop-server' AND b.object_key = s.hash AND b.size = s.size);
  IF n > 0 THEN RAISE EXCEPTION
    'shop-server/outfit_thumbnail: % thumbnails are not in dcl_migration.blob_inventory with their size; copy the blobs first', n;
  ELSE NULL; END IF; END $missing$;

-- Content-addressed: the same hash is the same bytes, so a conflict is a no-op.
INSERT INTO shop.outfit_thumbnails (hash, blob_cid, content_type, size, author_address, created_at)
SELECT s.hash, b.hash, s.content_type, s.size, lower(s.author_address), s.created_at_ms
FROM stage s
JOIN dcl_migration.blob_inventory b ON b.source = 'shop-server' AND b.object_key = s.hash
ON CONFLICT (hash) DO NOTHING;
