DO $release$ BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
              WHERE table_schema = 'builder' AND table_name = 'items'
                AND column_name = 'data' AND data_type = 'jsonb') THEN
    RAISE EXCEPTION 'builder-server/item: builder.items.data is jsonb, which loses the saved key order; run schemas/releases/builder-item-documents-and-uploaders.sql first';
  END IF; END $release$;

CREATE OR REPLACE FUNCTION pg_temp.builder_collection_fingerprint(c builder.collections) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(c.name, c.eth_address, c.salt, c.contract_address, c.urn_suffix, c.third_party_id,
                              c.publication_pending, c.created_at, c.updated_at, c.linked_contract_address,
                              c.linked_contract_network, c.lock, c.forum_link, c.forum_id, c.forum_claimed_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_item_fingerprint(i builder.items) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(i.collection_id, i.eth_address, i.name, i.description, i.thumbnail, i.video, i.type,
                              i.rarity, i.price, i.beneficiary, i.data, i.metrics, i.utility, i.mappings, i.urn_suffix,
                              i.blockchain_item_id, i.is_published, i.in_catalyst, i.total_supply,
                              i.local_content_hash, i.content_hash, i.catalyst_content_hash, i.created_at,
                              i.updated_at, i.video_named_by)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_collection_curation_fingerprint(k builder.collection_curations) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(k.collection_id, k.status, k.assignee, k.reviewed_by, k.rejection_reasons,
                              k.rejection_message, k.created_at, k.updated_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_item_curation_fingerprint(k builder.item_curations) RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT md5(json_build_array(k.item_id, k.status, k.content_hash, k.is_mapping_complete, k.created_at,
                              k.updated_at)::text)
$$;

CREATE OR REPLACE FUNCTION pg_temp.builder_lock_units(unit_collections uuid[], unit_items uuid[]) RETURNS void
LANGUAGE plpgsql AS $$ BEGIN
  PERFORM 1 FROM builder.collections c WHERE c.id = ANY(unit_collections) ORDER BY c.id FOR UPDATE;
  PERFORM 1 FROM builder.items i WHERE i.collection_id = ANY(unit_collections) OR i.id = ANY(unit_items)
    ORDER BY i.id FOR UPDATE;
END $$;

CREATE OR REPLACE FUNCTION pg_temp.builder_unit_untouched(unit_collection uuid, unit_item uuid) RETURNS boolean
LANGUAGE sql STABLE AS $$
  SELECT (unit_collection IS NULL OR EXISTS (
            SELECT 1 FROM builder.collections c
              JOIN dcl_migration.builder_imported m ON m.step = 'collection' AND m.id = c.id::text
             WHERE c.id = unit_collection AND m.fingerprint = pg_temp.builder_collection_fingerprint(c)))
     AND NOT EXISTS (
            SELECT 1 FROM builder.items i
              LEFT JOIN dcl_migration.builder_imported m ON m.step = 'item' AND m.id = i.id::text
             WHERE i.id IN (SELECT x.id FROM builder.items x WHERE x.collection_id = unit_collection
                            UNION ALL SELECT unit_item)
               AND m.fingerprint IS DISTINCT FROM pg_temp.builder_item_fingerprint(i))
     AND NOT EXISTS (
            SELECT 1 FROM builder.collection_curations k
              LEFT JOIN dcl_migration.builder_imported m ON m.step = 'collection_curation' AND m.id = k.id::text
             WHERE k.collection_id = unit_collection
               AND m.fingerprint IS DISTINCT FROM pg_temp.builder_collection_curation_fingerprint(k))
     AND NOT EXISTS (
            SELECT 1 FROM builder.item_curations k
              LEFT JOIN dcl_migration.builder_imported m ON m.step = 'item_curation' AND m.id = k.id::text
             WHERE k.item_id IN (SELECT x.id FROM builder.items x WHERE x.collection_id = unit_collection
                                 UNION ALL SELECT unit_item)
               AND m.fingerprint IS DISTINCT FROM pg_temp.builder_item_curation_fingerprint(k))
$$;

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

DO $taken$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(t.id::text, ', ' ORDER BY t.id) FILTER (WHERE t.rn <= 10) INTO n, ids
    FROM (SELECT x.id, row_number() OVER (ORDER BY x.id) AS rn
            FROM incoming_item s JOIN builder.items x ON x.id = s.id
           WHERE NOT s.imported AND x.created_at IS DISTINCT FROM s.created_at) t;
  IF n > 0 THEN RAISE EXCEPTION
    'builder-server/item: the node holds % ids the import did not write (another creation time), first: %; resolve them before importing', n, ids;
  ELSE NULL; END IF; END $taken$;

CREATE TEMP TABLE changed_item ON COMMIT DROP AS
SELECT s.id, i.collection_id AS unit_collection, CASE WHEN i.collection_id IS NULL THEN i.id END AS unit_item
FROM incoming_item s JOIN builder.items i ON i.id = s.id
WHERE s.imported
  AND (i.collection_id, i.eth_address, i.name, i.description, i.thumbnail, i.video, i.type, i.rarity, i.price,
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
END $lock$;

CREATE TEMP TABLE kept_unit ON COMMIT DROP AS
WITH u AS MATERIALIZED (SELECT DISTINCT unit_collection, unit_item FROM changed_item)
SELECT u.unit_collection, u.unit_item FROM u
WHERE NOT pg_temp.builder_unit_untouched(u.unit_collection, u.unit_item);

CREATE TEMP TABLE item_plan ON COMMIT DROP AS
SELECT s.id, s.imported,
       EXISTS (SELECT 1 FROM kept_unit k
                WHERE k.unit_collection IS NOT DISTINCT FROM ch.unit_collection
                  AND k.unit_item IS NOT DISTINCT FROM ch.unit_item) AS kept,
       pg_temp.builder_item_urn_fits(s.urn_suffix, s.collection_id) AS fits
FROM incoming_item s LEFT JOIN changed_item ch ON ch.id = s.id
WHERE NOT s.imported OR ch.id IS NOT NULL;

DO $kept$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_plan WHERE imported AND kept) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item: % items changed upstream are left as the node holds them (the node changed their collection, an item or a curation of it), first: %', n, ids;
  END IF;
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_plan WHERE NOT imported AND NOT fits) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item: % new items whose URN differs from their collection''s on the node are left out, first: %', n, ids;
  END IF;
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_plan WHERE imported AND NOT kept AND NOT fits) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item: % items changed upstream whose URN differs from their collection''s on the node are left as the node holds them, first: %', n, ids;
  END IF; END $kept$;

WITH refreshed AS (
  UPDATE builder.items AS i
     SET (collection_id, eth_address, name, description, thumbnail, video, type, rarity, price, beneficiary,
          data, metrics, utility, mappings, urn_suffix, blockchain_item_id, is_published, total_supply,
          local_content_hash, content_hash, updated_at, video_named_by)
       = (s.collection_id, s.eth_address, s.name, s.description, s.thumbnail, s.video, s.type, s.rarity, s.price,
          s.beneficiary, s.data, s.metrics, s.utility, s.mappings, s.urn_suffix, s.blockchain_item_id,
          s.is_published, s.total_supply, s.local_content_hash, s.content_hash, s.updated_at, s.video_named_by)
    FROM incoming_item s JOIN item_plan p ON p.id = s.id
   WHERE i.id = s.id AND p.imported AND NOT p.kept AND p.fits
  RETURNING i.id, pg_temp.builder_item_fingerprint(i) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

INSERT INTO builder.items
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
ON CONFLICT (id) DO NOTHING;

INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item', i.id::text, pg_temp.builder_item_fingerprint(i)
FROM incoming_item s JOIN builder.items i ON i.id = s.id JOIN item_plan p ON p.id = s.id
WHERE NOT s.imported AND p.fits
ON CONFLICT DO NOTHING;
