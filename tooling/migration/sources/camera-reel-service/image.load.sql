-- camera_reel.images names its bytes by hash in the ONE store. Moving them is
-- out of scope for this import: the blob copy records each upstream object
-- key and the hash it stored it under in dcl_migration.blob_inventory (created by the engine)
-- (source 'camera-reel-service'). A photo whose image key is unmapped stops
-- the batch. A photo with no thumbnail serves its image as the thumbnail.

DO $missing$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE s.image_key IS NULL
      OR NOT EXISTS (SELECT 1 FROM dcl_migration.blob_inventory b
                      WHERE b.source = 'camera-reel-service' AND b.object_key = s.image_key)
      OR (s.thumbnail_key IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM dcl_migration.blob_inventory b
                           WHERE b.source = 'camera-reel-service' AND b.object_key = s.thumbnail_key));
  IF n > 0 THEN RAISE EXCEPTION
    'camera-reel-service/image: % photos name objects missing from dcl_migration.blob_inventory; copy the blobs first', n;
  ELSE NULL; END IF; END $missing$;

-- The node holds user_address and metadata.userAddress together (CHECK);
-- upstream compared them case-sensitively at upload. A row where they differ
-- even ignoring case is skipped and counted.
DO $owner$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage
   WHERE lower(metadata->>'userAddress') IS DISTINCT FROM lower(user_address);
  IF n > 0 THEN RAISE NOTICE 'camera-reel-service/image: skipped % photos whose metadata.userAddress is not the owner', n;
  ELSE NULL; END IF; END $owner$;

-- review_status is the node moderator's: set on insert, never overwritten.
INSERT INTO camera_reel.images AS i
    (id, user_address, image_hash, thumbnail_hash, is_public, review_status, metadata, created_at)
SELECT s.id, lower(s.user_address), bi.hash, COALESCE(bt.hash, bi.hash), s.is_public, 'ok',
       s.metadata, s.created_at AT TIME ZONE 'UTC'
FROM stage s
JOIN dcl_migration.blob_inventory bi
  ON bi.source = 'camera-reel-service' AND bi.object_key = s.image_key
LEFT JOIN dcl_migration.blob_inventory bt
  ON bt.source = 'camera-reel-service' AND bt.object_key = s.thumbnail_key
WHERE lower(s.metadata->>'userAddress') = lower(s.user_address)
ON CONFLICT (id) DO UPDATE SET
    user_address = EXCLUDED.user_address,
    image_hash = EXCLUDED.image_hash,
    thumbnail_hash = EXCLUDED.thumbnail_hash,
    is_public = EXCLUDED.is_public,
    metadata = EXCLUDED.metadata,
    created_at = EXCLUDED.created_at
WHERE (i.user_address, i.image_hash, i.thumbnail_hash, i.is_public, i.metadata, i.created_at)
      IS DISTINCT FROM
      (EXCLUDED.user_address, EXCLUDED.image_hash, EXCLUDED.thumbnail_hash, EXCLUDED.is_public,
       EXCLUDED.metadata, EXCLUDED.created_at);
