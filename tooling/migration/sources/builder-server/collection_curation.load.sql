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

CREATE TEMP TABLE incoming_collection_curation ON COMMIT DROP AS
SELECT s.id, s.collection_id, s.status,
       CASE WHEN lower(s.assignee) ~ '^0x[0-9a-f]{40}$' THEN lower(s.assignee) END AS assignee,
       CASE WHEN lower(s.reviewed_by) ~ '^0x[0-9a-f]{40}$' OR s.reviewed_by = 'validator' THEN lower(s.reviewed_by) END
           AS reviewed_by,
       CASE WHEN s.rejection_reasons <@ ARRAY['clipping', 'thumbnail', 'category_hides', 'rigging',
                 'triangle_count', 'emote', 'file_size', 'textures_materials', 'reversed_faces',
                 'smart_wearable_files', 'content_policy_ip', 'other']
            THEN NULLIF(s.rejection_reasons, '{}') END AS rejection_reasons,
       CASE WHEN char_length(btrim(s.rejection_message)) <= 3000 THEN NULLIF(btrim(s.rejection_message), '') END
           AS rejection_message,
       s.created_at AT TIME ZONE 'UTC' AS created_at,
       GREATEST(s.updated_at, s.created_at) AT TIME ZONE 'UTC' AS updated_at,
       s.updated_at AS upstream_updated_at,
       m.id IS NOT NULL AS imported
FROM stage s
LEFT JOIN dcl_migration.builder_imported m ON m.step = 'collection_curation' AND m.id = s.id::text
WHERE s.status IN ('pending', 'approved', 'rejected');

CREATE TEMP TABLE collection_curation_plan ON COMMIT DROP AS
SELECT s.id, s.imported, COALESCE(k.collection_id, s.collection_id) AS unit_collection
FROM incoming_collection_curation s
LEFT JOIN builder.collection_curations k ON k.id = s.id
WHERE (s.imported AND k.id IS NOT NULL
       AND (k.status, k.assignee, k.reviewed_by, k.rejection_reasons, k.rejection_message, k.updated_at)
           IS DISTINCT FROM (s.status, s.assignee, s.reviewed_by, s.rejection_reasons, s.rejection_message, s.updated_at))
   OR (NOT s.imported AND EXISTS (SELECT 1 FROM builder.collections c WHERE c.id = s.collection_id));

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(ARRAY(SELECT DISTINCT unit_collection FROM collection_curation_plan), '{}');
END $lock$;

CREATE TEMP TABLE kept_unit ON COMMIT DROP AS
WITH u AS MATERIALIZED (SELECT DISTINCT unit_collection FROM collection_curation_plan)
SELECT u.unit_collection FROM u
WHERE NOT pg_temp.builder_unit_untouched(u.unit_collection, NULL);

DO $kept$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT p.id, row_number() OVER (ORDER BY p.id) AS rn FROM collection_curation_plan p
           WHERE p.unit_collection IN (SELECT unit_collection FROM kept_unit)) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/collection_curation: % curations, changed or new upstream, of collections the node changed are left out, first: %', n, ids;
  END IF; END $kept$;

WITH refreshed AS (
  UPDATE builder.collection_curations AS k
     SET (status, assignee, reviewed_by, rejection_reasons, rejection_message, updated_at)
       = (s.status, s.assignee, s.reviewed_by, s.rejection_reasons, s.rejection_message, s.updated_at)
    FROM incoming_collection_curation s JOIN collection_curation_plan p ON p.id = s.id
   WHERE k.id = s.id AND s.imported AND p.unit_collection NOT IN (SELECT unit_collection FROM kept_unit)
  RETURNING k.id, pg_temp.builder_collection_curation_fingerprint(k) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection_curation', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

INSERT INTO builder.collection_curations
    (id, collection_id, status, assignee, reviewed_by, rejection_reasons, rejection_message, created_at, updated_at)
SELECT DISTINCT ON (s.collection_id, s.created_at)
       s.id, s.collection_id, s.status, s.assignee, s.reviewed_by, s.rejection_reasons, s.rejection_message,
       s.created_at, s.updated_at
FROM incoming_collection_curation s
JOIN collection_curation_plan p ON p.id = s.id
WHERE NOT s.imported
  AND p.unit_collection NOT IN (SELECT unit_collection FROM kept_unit)
  AND NOT EXISTS (SELECT 1 FROM builder.collection_curations o
                  WHERE o.collection_id = s.collection_id AND o.created_at = s.created_at AND o.id <> s.id)
ORDER BY s.collection_id, s.created_at, s.upstream_updated_at DESC, s.id DESC
ON CONFLICT (id) DO NOTHING;

INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection_curation', k.id::text, pg_temp.builder_collection_curation_fingerprint(k)
FROM incoming_collection_curation s JOIN builder.collection_curations k ON k.id = s.id
WHERE NOT s.imported
ON CONFLICT DO NOTHING;
