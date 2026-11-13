-- This mapping requires the filename-origin schema capability. It never
-- overwrites immutable target facts or moderation. Visibility follows tracked imports only.
DO $capability$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                 WHERE table_schema = 'camera_reel' AND table_name = 'images'
                   AND column_name = 'original_filename') THEN
    RAISE EXCEPTION 'camera import requires images.original_filename schema capability';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                 WHERE table_schema = 'camera_reel' AND table_name = 'images'
                   AND column_name = 'image_content_type') THEN
    RAISE EXCEPTION 'camera import requires images.image_content_type schema capability';
  END IF;
END $capability$;

CREATE TEMP TABLE camera_import ON COMMIT DROP AS
SELECT s.*, substring(s.url FROM length(:'camera_api_url' || '/api/images/') + 1) AS image_key,
       substring(s.thumbnail_url FROM length(:'camera_api_url' || '/api/images/') + 1) AS thumbnail_key,
       substring(s.url FROM length(:'camera_api_url' || '/api/images/' || s.id::text || '-') + 1) AS original_filename
FROM stage s;

DO $preflight$ BEGIN
  IF EXISTS (SELECT 1 FROM camera_import
             WHERE created_at IS NULL OR metadata IS NULL OR is_public IS NULL
                OR lower(metadata->>'userAddress') IS DISTINCT FROM user_address
                OR user_address !~ '^0x[0-9a-f]{40}$'
                OR original_filename IS NULL OR original_filename = ''
                OR octet_length(image_key) > 255 OR octet_length(thumbnail_key) > 255
                OR original_filename ~ '[/\\[:cntrl:]]'
                OR image_key IS DISTINCT FROM id::text || '-' || original_filename
                OR thumbnail_key IS DISTINCT FROM id::text || '-thumbnail-' || original_filename) THEN
    RAISE EXCEPTION 'unsupported camera row; preflight source facts and canonical paired filenames';
  END IF;
END $preflight$;
-- Raw URL equality also rejects differing historical origins.
SELECT set_config('dcl_migration.camera_api_url', :'camera_api_url', true);
DO $urls$ BEGIN
  IF EXISTS (SELECT 1 FROM camera_import
             WHERE url IS DISTINCT FROM current_setting('dcl_migration.camera_api_url') || '/api/images/' || image_key
                OR thumbnail_url IS DISTINCT FROM current_setting('dcl_migration.camera_api_url') || '/api/images/' || thumbnail_key) THEN
    RAISE EXCEPTION 'camera URL origin differs from configured import API URL';
  END IF;
  IF EXISTS (SELECT 1 FROM camera_import s
             WHERE NOT EXISTS (SELECT 1 FROM dcl_migration.blob_inventory b
                               WHERE b.source = 'camera-reel-service' AND b.object_key = s.image_key)
                OR NOT EXISTS (SELECT 1 FROM dcl_migration.blob_inventory b
                               WHERE b.source = 'camera-reel-service' AND b.object_key = s.thumbnail_key)) THEN
    RAISE EXCEPTION 'camera objects missing from blob inventory; copy both original objects first';
  END IF;
  IF EXISTS (SELECT 1 FROM camera_import s
             JOIN dcl_migration.blob_inventory bi ON bi.source='camera-reel-service' AND bi.object_key=s.image_key
             JOIN dcl_migration.blob_inventory bt ON bt.source='camera-reel-service' AND bt.object_key=s.thumbnail_key
             WHERE bi.content_type IS NULL OR bi.content_type NOT IN ('image/png', 'image/jpeg')
                OR bt.content_type IS DISTINCT FROM bi.content_type) THEN
    RAISE EXCEPTION 'camera objects require equal explicit original-object MIME metadata; no byte sniffing';
  END IF;
END $urls$;

WITH inserted AS (
INSERT INTO camera_reel.images
    (id, user_address, image_hash, thumbnail_hash, original_filename, image_content_type,
     is_public, review_status, ranking_published, metadata, created_at)
SELECT s.id, s.user_address, bi.hash, bt.hash, s.original_filename, bi.content_type,
       s.is_public, 'ok', true, s.metadata, s.created_at AT TIME ZONE 'UTC'
FROM camera_import s
JOIN dcl_migration.blob_inventory bi ON bi.source = 'camera-reel-service' AND bi.object_key = s.image_key
JOIN dcl_migration.blob_inventory bt ON bt.source = 'camera-reel-service' AND bt.object_key = s.thumbnail_key
ON CONFLICT (id) DO NOTHING
RETURNING id, is_public
)
INSERT INTO dcl_migration.camera_reel_image_loaded (id, is_public)
SELECT id, is_public FROM inserted
ON CONFLICT (id) DO NOTHING;

-- Check after INSERT so a concurrent insert is also compared. Moderation is
-- deliberately omitted: a node's existing rejection/flag must remain intact.
DO $conflict$ BEGIN
  IF EXISTS (
    SELECT 1 FROM camera_import s
    JOIN camera_reel.images i USING (id)
    JOIN dcl_migration.blob_inventory bi ON bi.source = 'camera-reel-service' AND bi.object_key = s.image_key
    JOIN dcl_migration.blob_inventory bt ON bt.source = 'camera-reel-service' AND bt.object_key = s.thumbnail_key
    WHERE (i.user_address, i.image_hash, i.thumbnail_hash, i.original_filename,
           i.metadata, i.created_at, i.ranking_published, i.image_content_type)
      IS DISTINCT FROM
          (s.user_address, bi.hash, bt.hash, s.original_filename,
           s.metadata, s.created_at AT TIME ZONE 'UTC', true, bi.content_type)
    OR (i.is_public IS DISTINCT FROM s.is_public AND NOT EXISTS
        (SELECT 1 FROM dcl_migration.camera_reel_image_loaded l WHERE l.id = i.id))
  ) THEN RAISE EXCEPTION 'camera UUID conflicts with existing target facts; no rows overwritten';
  END IF;
END $conflict$;
-- Follow upstream visibility only while the node still has our last value.
-- Rows that predate the importer are never adopted as importer-owned rows.
UPDATE camera_reel.images i SET is_public = s.is_public
FROM camera_import s, dcl_migration.camera_reel_image_loaded l
WHERE i.id = s.id AND l.id = i.id AND i.is_public = l.is_public;

UPDATE dcl_migration.camera_reel_image_loaded l SET is_public = s.is_public
FROM camera_import s JOIN camera_reel.images i USING (id)
WHERE l.id = s.id AND i.is_public = s.is_public;
DROP TABLE camera_import;
