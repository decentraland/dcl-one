DO $release$ BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
              WHERE table_schema = 'builder' AND table_name = 'items'
                AND column_name = 'data' AND data_type = 'jsonb') THEN
    RAISE EXCEPTION 'builder-server/item: builder.items.data is jsonb, which loses the saved key order; run schemas/releases/builder-item-documents-and-uploaders.sql first';
  END IF; END $release$;

CREATE OR REPLACE FUNCTION pg_temp.builder_item_urn_fits(urn text, collection uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
  SELECT urn IS NULL OR collection IS NULL OR EXISTS (
    SELECT 1 FROM builder.collections c
     WHERE c.id = collection
       AND (c.third_party_id IS NULL OR left(urn, char_length(c.urn_suffix) + 1) = c.urn_suffix || ':'))
$$;

CREATE TEMP TABLE incoming_item ON COMMIT DROP AS
SELECT s.id, c.id AS collection_id, lower(s.eth_address) AS eth_address, btrim(s.name) AS name, s.description,
       s.thumbnail, s.video, s.type, lower(NULLIF(s.rarity, '')) AS rarity, NULLIF(s.price, '') AS price,
       lower(NULLIF(s.beneficiary, '')) AS beneficiary, s.data, s.metrics, s.utility, s.mappings, s.urn_suffix,
       s.blockchain_item_id, s.is_published, s.total_supply, s.local_content_hash, s.content_hash,
       s.created_at AT TIME ZONE 'UTC' AS created_at, s.updated_at AT TIME ZONE 'UTC' AS updated_at,
       CASE WHEN s.video IS NOT NULL THEN lower(s.eth_address) END AS video_named_by,
       m.id IS NOT NULL AS imported
FROM stage s
LEFT JOIN builder.collections c ON c.id = s.collection_id
LEFT JOIN dcl_migration.builder_imported m ON m.step = 'item' AND m.id = s.id::text;

CREATE TEMP TABLE changed_item ON COMMIT DROP AS
SELECT s.id, i.collection_id AS unit_collection, CASE WHEN i.collection_id IS NULL THEN i.id END AS unit_item
FROM incoming_item s JOIN builder.items i ON i.id = s.id
WHERE (i.collection_id, i.eth_address, i.name, i.description, i.thumbnail, i.video, i.type, i.rarity, i.price,
       i.beneficiary, i.data::text, i.metrics::text, i.utility, i.mappings, i.urn_suffix, i.blockchain_item_id,
       i.is_published, i.total_supply, i.local_content_hash, i.content_hash, i.updated_at, i.video_named_by)
      IS DISTINCT FROM
      (s.collection_id, s.eth_address, s.name, s.description, s.thumbnail, s.video, s.type, s.rarity, s.price,
       s.beneficiary, s.data::text, s.metrics::text, s.utility, s.mappings, s.urn_suffix, s.blockchain_item_id,
       s.is_published, s.total_supply, s.local_content_hash, s.content_hash, s.updated_at, s.video_named_by);

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(
    ARRAY(SELECT unit_collection FROM changed_item WHERE unit_collection IS NOT NULL
          UNION SELECT s.collection_id FROM incoming_item s
                 WHERE s.collection_id IS NOT NULL AND (NOT s.imported OR s.id IN (SELECT id FROM changed_item))),
    ARRAY(SELECT id FROM changed_item));
  PERFORM pg_advisory_xact_lock(hashtextextended('builder item urn ' || u.part, 0))
     FROM (SELECT DISTINCT lower(substring(s.urn_suffix FROM '[^:]*$')) AS part
             FROM incoming_item s JOIN builder.collections c ON c.id = s.collection_id
            WHERE c.third_party_id IS NOT NULL AND s.urn_suffix IS NOT NULL
              AND (NOT s.imported OR s.id IN (SELECT id FROM changed_item))) u
    ORDER BY u.part;
END $lock$;

CREATE TEMP TABLE item_urn_taken ON COMMIT DROP AS
WITH parts AS MATERIALIZED (
  SELECT s.id, c.third_party_id AS provider, lower(substr(s.urn_suffix, char_length(c.urn_suffix) + 2)) AS part,
         NOT s.imported OR s.id IN (SELECT id FROM changed_item) AS asked
    FROM incoming_item s JOIN builder.collections c ON c.id = s.collection_id
   WHERE c.third_party_id IS NOT NULL AND s.urn_suffix IS NOT NULL
     AND left(s.urn_suffix, char_length(c.urn_suffix) + 1) = c.urn_suffix || ':'),
held AS MATERIALIZED (
  SELECT x.id, cx.third_party_id AS provider,
         substr(lower(x.urn_suffix), char_length(lower(cx.urn_suffix)) + 2) AS part
    FROM builder.items x JOIN builder.collections cx ON cx.id = x.collection_id
   WHERE cx.third_party_id IS NOT NULL AND x.urn_suffix IS NOT NULL
     AND left(lower(x.urn_suffix), char_length(lower(cx.urn_suffix)) + 1) = lower(cx.urn_suffix) || ':')
SELECT a.id FROM parts a JOIN held h ON h.provider = a.provider AND h.part = a.part AND h.id <> a.id WHERE a.asked
UNION
SELECT a.id FROM parts a JOIN parts y ON y.provider = a.provider AND y.part = a.part AND y.id <> a.id WHERE a.asked;

CREATE TEMP TABLE item_plan ON COMMIT DROP AS
SELECT p.*, p.prefix_fits AND p.urn_free AS fits
FROM (SELECT s.id, s.imported,
             pg_temp.builder_item_urn_fits(s.urn_suffix, s.collection_id) AS prefix_fits,
             s.id NOT IN (SELECT id FROM item_urn_taken) AS urn_free
        FROM incoming_item s
       WHERE NOT s.imported OR s.id IN (SELECT id FROM changed_item)) p;

DO $left_out$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_plan WHERE NOT imported AND NOT prefix_fits) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item: % new items whose URN differs from their collection''s on the node are left out, first: %', n, ids;
  END IF;
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_plan WHERE imported AND NOT prefix_fits) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item: % items changed upstream whose URN differs from their collection''s on the node are left as the node holds them, first: %', n, ids;
  END IF;
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_plan WHERE NOT urn_free) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item: % items whose URN another item holds are left out, first: %; if upstream holds no such duplicate (a swap, or a row deleted and created again), start over (MAPPING.md, "Start over")', n, ids;
  END IF; END $left_out$;

WITH refreshed AS (
  UPDATE builder.items AS i
     SET (collection_id, eth_address, name, description, thumbnail, video, type, rarity, price, beneficiary,
          data, metrics, utility, mappings, urn_suffix, blockchain_item_id, is_published, total_supply,
          local_content_hash, content_hash, updated_at, video_named_by)
       = (s.collection_id, s.eth_address, s.name, s.description, s.thumbnail, s.video, s.type, s.rarity, s.price,
          s.beneficiary, s.data, s.metrics, s.utility, s.mappings, s.urn_suffix, s.blockchain_item_id,
          s.is_published, s.total_supply, s.local_content_hash, s.content_hash, s.updated_at, s.video_named_by)
    FROM incoming_item s JOIN item_plan p ON p.id = s.id
   WHERE i.id = s.id AND p.imported AND p.fits
  RETURNING i.id, pg_temp.builder_item_fingerprint(i) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

WITH inserted AS (
  INSERT INTO builder.items AS i
      (id, collection_id, eth_address, name, description, thumbnail, video, type, rarity,
       price, beneficiary, data, metrics, utility, mappings, urn_suffix, blockchain_item_id,
       is_published, total_supply, local_content_hash, content_hash,
       created_at, updated_at, video_named_by)
  SELECT s.id, s.collection_id, s.eth_address, s.name, s.description, s.thumbnail, s.video, s.type, s.rarity,
         s.price, s.beneficiary, s.data, s.metrics, s.utility, s.mappings, s.urn_suffix, s.blockchain_item_id,
         s.is_published, s.total_supply, s.local_content_hash, s.content_hash,
         s.created_at, s.updated_at, s.video_named_by
  FROM incoming_item s JOIN item_plan p ON p.id = s.id
  WHERE NOT s.imported AND p.fits
  RETURNING i.id, pg_temp.builder_item_fingerprint(i) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item', id::text, fingerprint FROM inserted;
