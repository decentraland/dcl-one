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

CREATE TEMP TABLE incoming_item_curation ON COMMIT DROP AS
SELECT s.id, s.item_id, s.status, lower(s.content_hash) AS content_hash, s.is_mapping_complete,
       s.created_at AT TIME ZONE 'UTC' AS created_at,
       GREATEST(s.updated_at, s.created_at) AT TIME ZONE 'UTC' AS updated_at,
       s.updated_at AS upstream_updated_at,
       m.id IS NOT NULL AS imported
FROM stage s
LEFT JOIN dcl_migration.builder_imported m ON m.step = 'item_curation' AND m.id = s.id::text
WHERE s.status IN ('pending', 'approved', 'rejected')
  AND lower(s.content_hash) ~ '^[0-9a-f]{64}$';

CREATE TEMP TABLE item_curation_units ON COMMIT DROP AS
SELECT s.id, s.imported, i.collection_id AS unit_collection,
       CASE WHEN i.collection_id IS NULL THEN i.id END AS unit_item
FROM incoming_item_curation s
LEFT JOIN builder.item_curations k ON k.id = s.id
JOIN builder.items i ON i.id = COALESCE(k.item_id, s.item_id)
WHERE (s.imported AND k.id IS NOT NULL
       AND (k.status, k.content_hash, k.is_mapping_complete, k.updated_at)
           IS DISTINCT FROM (s.status, s.content_hash, s.is_mapping_complete, s.updated_at))
   OR NOT s.imported;

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(
    ARRAY(SELECT DISTINCT unit_collection FROM item_curation_units WHERE unit_collection IS NOT NULL),
    ARRAY(SELECT DISTINCT unit_item FROM item_curation_units WHERE unit_item IS NOT NULL));
END $lock$;

CREATE TEMP TABLE kept_unit ON COMMIT DROP AS
WITH u AS MATERIALIZED (SELECT DISTINCT unit_collection, unit_item FROM item_curation_units)
SELECT u.unit_collection, u.unit_item FROM u
WHERE NOT pg_temp.builder_unit_untouched(u.unit_collection, u.unit_item);

CREATE TEMP TABLE item_curation_plan ON COMMIT DROP AS
SELECT u.id, u.imported, EXISTS (
  SELECT 1 FROM kept_unit k WHERE k.unit_collection IS NOT DISTINCT FROM u.unit_collection
                              AND k.unit_item IS NOT DISTINCT FROM u.unit_item) AS kept
FROM item_curation_units u;

DO $kept$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM item_curation_plan WHERE kept) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/item_curation: % curations, changed or new upstream, of items whose collection or item the node changed are left out, first: %', n, ids;
  END IF; END $kept$;

WITH refreshed AS (
  UPDATE builder.item_curations AS k
     SET (status, content_hash, is_mapping_complete, updated_at)
       = (s.status, s.content_hash, s.is_mapping_complete, s.updated_at)
    FROM incoming_item_curation s JOIN item_curation_plan p ON p.id = s.id
   WHERE k.id = s.id AND s.imported AND NOT p.kept
  RETURNING k.id, pg_temp.builder_item_curation_fingerprint(k) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item_curation', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at)
SELECT DISTINCT ON (s.item_id, s.created_at)
       s.id, s.item_id, s.status, s.content_hash, s.is_mapping_complete, s.created_at, s.updated_at
FROM incoming_item_curation s
JOIN item_curation_plan p ON p.id = s.id
WHERE NOT s.imported AND NOT p.kept
  AND NOT EXISTS (SELECT 1 FROM builder.item_curations o
                  WHERE o.item_id = s.item_id AND o.created_at = s.created_at AND o.id <> s.id)
ORDER BY s.item_id, s.created_at, s.upstream_updated_at DESC, s.id DESC
ON CONFLICT (id) DO NOTHING;

INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item_curation', k.id::text, pg_temp.builder_item_curation_fingerprint(k)
FROM incoming_item_curation s JOIN builder.item_curations k ON k.id = s.id
WHERE NOT s.imported
ON CONFLICT DO NOTHING;
