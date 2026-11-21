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

CREATE TEMP TABLE incoming_collection ON COMMIT DROP AS
SELECT s.id, btrim(s.name) AS name, lower(s.eth_address) AS eth_address, s.salt, s.contract_address, s.urn_suffix,
       s.third_party_id, s.created_at AT TIME ZONE 'UTC' AS created_at, s.updated_at AT TIME ZONE 'UTC' AS updated_at,
       lower(s.linked_contract_address) AS linked_contract_address, s.linked_contract_network, s.lock,
       CASE WHEN s.web_link THEN s.forum_link END AS forum_link,
       CASE WHEN s.web_link AND s.forum_id > 0 THEN s.forum_id END AS forum_id,
       m.id IS NOT NULL AS imported
FROM (SELECT *, COALESCE(forum_link ~ '^https?://[^[:space:]]+$' AND char_length(forum_link) <= 2048
                         AND left(forum_link, char_length(:'forum_url') + 1) = :'forum_url' || '/', false) AS web_link
      FROM stage) s
LEFT JOIN dcl_migration.builder_imported m ON m.step = 'collection' AND m.id = s.id::text;

DO $taken$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(t.id::text, ', ' ORDER BY t.id) FILTER (WHERE t.rn <= 10) INTO n, ids
    FROM (SELECT x.id, row_number() OVER (ORDER BY x.id) AS rn
            FROM incoming_collection s JOIN builder.collections x ON x.id = s.id
           WHERE NOT s.imported AND x.created_at IS DISTINCT FROM s.created_at) t;
  IF n > 0 THEN RAISE EXCEPTION
    'builder-server/collection: the node holds % ids the import did not write (another creation time), first: %; resolve them before importing', n, ids;
  ELSE NULL; END IF; END $taken$;

CREATE TEMP TABLE changed_collection ON COMMIT DROP AS
SELECT s.id FROM incoming_collection s JOIN builder.collections c ON c.id = s.id
WHERE s.imported
  AND (c.name, c.eth_address, c.salt, c.contract_address, c.urn_suffix, c.third_party_id, c.updated_at,
       c.linked_contract_address, c.linked_contract_network, c.lock, c.forum_link, c.forum_id)
      IS DISTINCT FROM
      (s.name, s.eth_address, s.salt, s.contract_address, s.urn_suffix, s.third_party_id, s.updated_at,
       s.linked_contract_address, s.linked_contract_network, s.lock, s.forum_link, s.forum_id);

DO $lock$ BEGIN PERFORM pg_temp.builder_lock_units(ARRAY(SELECT id FROM changed_collection), '{}'); END $lock$;

CREATE TEMP TABLE kept_collection ON COMMIT DROP AS
SELECT id FROM changed_collection WHERE NOT pg_temp.builder_unit_untouched(id, NULL);

DO $kept$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM kept_collection) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/collection: % collections changed upstream are left as the node holds them (the node changed the collection, an item or a curation of it), first: %', n, ids;
  ELSE NULL; END IF; END $kept$;

WITH moved AS (
  UPDATE builder.items AS i
     SET urn_suffix = s.urn_suffix || substr(i.urn_suffix, char_length(c.urn_suffix) + 1)
    FROM incoming_collection s JOIN builder.collections c ON c.id = s.id
   WHERE i.collection_id = s.id
     AND s.id IN (SELECT id FROM changed_collection EXCEPT SELECT id FROM kept_collection)
     AND c.third_party_id IS NOT NULL AND s.urn_suffix IS NOT NULL AND c.urn_suffix <> s.urn_suffix
     AND left(i.urn_suffix, char_length(c.urn_suffix) + 1) = c.urn_suffix || ':'
  RETURNING i.id, pg_temp.builder_item_fingerprint(i) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'item', id::text, fingerprint FROM moved
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

WITH refreshed AS (
  UPDATE builder.collections AS c
     SET (name, eth_address, salt, contract_address, urn_suffix, third_party_id, updated_at,
          linked_contract_address, linked_contract_network, lock, forum_link, forum_id)
       = (s.name, s.eth_address, s.salt, s.contract_address, s.urn_suffix, s.third_party_id, s.updated_at,
          s.linked_contract_address, s.linked_contract_network, s.lock, s.forum_link, s.forum_id)
    FROM incoming_collection s
   WHERE c.id = s.id AND s.id IN (SELECT id FROM changed_collection EXCEPT SELECT id FROM kept_collection)
  RETURNING c.id, pg_temp.builder_collection_fingerprint(c) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

INSERT INTO builder.collections
    (id, name, eth_address, salt, contract_address, urn_suffix, third_party_id,
     publication_pending, created_at, updated_at, linked_contract_address, linked_contract_network, lock,
     forum_link, forum_id)
SELECT id, name, eth_address, salt, contract_address, urn_suffix, third_party_id,
       false, created_at, updated_at, linked_contract_address, linked_contract_network, lock,
       forum_link, forum_id
FROM incoming_collection
WHERE NOT imported
ON CONFLICT (id) DO NOTHING;

INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection', c.id::text, pg_temp.builder_collection_fingerprint(c)
FROM incoming_collection s JOIN builder.collections c ON c.id = s.id
WHERE NOT s.imported
ON CONFLICT DO NOTHING;
