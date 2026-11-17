-- Upstream ids kept. Upstream validated with the same rules the target
-- declares (uuid id, 1..60 name, <= 20 items, body shape, '#rrggbb' stops,
-- the publish bar); a row breaking one is skipped and counted, never fixed.
DO $invalid$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE NOT (s.id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
          AND char_length(s.name) BETWEEN 1 AND 60
          AND (s.thumbnail_hash = '' OR s.thumbnail_hash ~ '^[0-9a-f]{64}$')
          AND jsonb_typeof(s.items) = 'array' AND jsonb_array_length(s.items) <= 20
          AND s.body_shape IN ('male', 'female', 'unisex')
          AND (s.gradient_from = '' OR lower(s.gradient_from) ~ '^#[0-9a-f]{6}$')
          AND (s.gradient_to = '' OR lower(s.gradient_to) ~ '^#[0-9a-f]{6}$')
          AND lower(s.author_address) ~ '^0x[0-9a-f]{40}$'
          AND (NOT s.published OR (s.thumbnail_hash <> '' AND s.gradient_from <> ''
                                   AND s.gradient_to <> '' AND jsonb_array_length(s.items) >= 1)));
  IF n > 0 THEN RAISE NOTICE 'shop-server/outfit: skipped % outfits that break the target constraints', n;
  ELSE NULL; END IF; END $invalid$;

DO $nothumb$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE s.thumbnail_hash <> ''
     AND NOT EXISTS (SELECT 1 FROM shop.outfit_thumbnails t WHERE t.hash = s.thumbnail_hash);
  IF n > 0 THEN RAISE NOTICE 'shop-server/outfit: % outfits name a thumbnail the target does not hold', n;
  ELSE NULL; END IF; END $nothumb$;

-- A newer edit already in the target (updated_at) wins.
INSERT INTO shop.outfits AS o
    (id, name, thumbnail_hash, items, body_shape, gradient_from, gradient_to,
     author_address, published, created_at, updated_at)
SELECT s.id, s.name, s.thumbnail_hash, s.items, s.body_shape, lower(s.gradient_from),
       lower(s.gradient_to), lower(s.author_address), s.published, s.created_at_ms, s.updated_at_ms
FROM stage s
WHERE s.id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
  AND char_length(s.name) BETWEEN 1 AND 60
  AND (s.thumbnail_hash = '' OR s.thumbnail_hash ~ '^[0-9a-f]{64}$')
  AND jsonb_typeof(s.items) = 'array' AND jsonb_array_length(s.items) <= 20
  AND s.body_shape IN ('male', 'female', 'unisex')
  AND (s.gradient_from = '' OR lower(s.gradient_from) ~ '^#[0-9a-f]{6}$')
  AND (s.gradient_to = '' OR lower(s.gradient_to) ~ '^#[0-9a-f]{6}$')
  AND lower(s.author_address) ~ '^0x[0-9a-f]{40}$'
  AND (NOT s.published OR (s.thumbnail_hash <> '' AND s.gradient_from <> ''
                           AND s.gradient_to <> '' AND jsonb_array_length(s.items) >= 1))
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name, thumbnail_hash = EXCLUDED.thumbnail_hash, items = EXCLUDED.items,
    body_shape = EXCLUDED.body_shape, gradient_from = EXCLUDED.gradient_from,
    gradient_to = EXCLUDED.gradient_to, author_address = EXCLUDED.author_address,
    published = EXCLUDED.published, created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at
WHERE o.updated_at <= EXCLUDED.updated_at
  AND (o.name, o.thumbnail_hash, o.items, o.body_shape, o.gradient_from, o.gradient_to,
       o.author_address, o.published, o.created_at, o.updated_at)
      IS DISTINCT FROM
      (EXCLUDED.name, EXCLUDED.thumbnail_hash, EXCLUDED.items, EXCLUDED.body_shape,
       EXCLUDED.gradient_from, EXCLUDED.gradient_to, EXCLUDED.author_address,
       EXCLUDED.published, EXCLUDED.created_at, EXCLUDED.updated_at);
