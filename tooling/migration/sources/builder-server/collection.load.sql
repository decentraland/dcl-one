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

CREATE TEMP TABLE changed_collection ON COMMIT DROP AS
SELECT s.id FROM incoming_collection s JOIN builder.collections c ON c.id = s.id
WHERE (c.name, c.eth_address, c.salt, c.contract_address, c.urn_suffix, c.third_party_id, c.updated_at,
       c.linked_contract_address, c.linked_contract_network, c.lock, c.forum_link, c.forum_id)
      IS DISTINCT FROM
      (s.name, s.eth_address, s.salt, s.contract_address, s.urn_suffix, s.third_party_id, s.updated_at,
       s.linked_contract_address, s.linked_contract_network, s.lock, s.forum_link, s.forum_id);

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(ARRAY(SELECT id FROM changed_collection), '{}');
  PERFORM pg_advisory_xact_lock(hashtextextended('builder.collections urn:' || u.urn_suffix, 0))
     FROM (SELECT DISTINCT s.urn_suffix FROM incoming_collection s
            WHERE s.third_party_id IS NOT NULL AND s.urn_suffix IS NOT NULL
              AND (NOT s.imported OR s.id IN (SELECT id FROM changed_collection))) u
    ORDER BY u.urn_suffix;
END $lock$;

CREATE TEMP TABLE collection_urn_taken ON COMMIT DROP AS
WITH asked AS MATERIALIZED (
  SELECT s.id, s.urn_suffix AS urn FROM incoming_collection s
   WHERE s.third_party_id IS NOT NULL AND s.urn_suffix IS NOT NULL
     AND (NOT s.imported OR s.id IN (SELECT id FROM changed_collection)))
SELECT a.id FROM asked a JOIN builder.collections x ON x.third_party_id IS NOT NULL AND x.urn_suffix = a.urn AND x.id <> a.id
UNION
SELECT a.id FROM asked a JOIN incoming_collection y ON y.third_party_id IS NOT NULL AND y.urn_suffix = a.urn AND y.id <> a.id;

DO $left_out$ DECLARE n bigint; ids text; BEGIN
  SELECT count(*), string_agg(id::text, ', ' ORDER BY id) FILTER (WHERE rn <= 10) INTO n, ids
    FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM collection_urn_taken) k;
  IF n > 0 THEN RAISE NOTICE
    'builder-server/collection: % third-party collections whose URN another collection holds are left out, first: %; if upstream holds no such duplicate (a swap, or a row deleted and created again), start over (MAPPING.md, "Start over")', n, ids;
  END IF; END $left_out$;

CREATE TEMP TABLE refreshed_collection ON COMMIT DROP AS
SELECT id FROM changed_collection EXCEPT SELECT id FROM collection_urn_taken;

WITH moved AS (
  UPDATE builder.items AS i
     SET urn_suffix = s.urn_suffix || substr(i.urn_suffix, char_length(c.urn_suffix) + 1)
    FROM incoming_collection s JOIN builder.collections c ON c.id = s.id
   WHERE i.collection_id = s.id
     AND s.id IN (SELECT id FROM refreshed_collection)
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
   WHERE c.id = s.id AND s.id IN (SELECT id FROM refreshed_collection)
  RETURNING c.id, pg_temp.builder_collection_fingerprint(c) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

WITH inserted AS (
  INSERT INTO builder.collections AS c
      (id, name, eth_address, salt, contract_address, urn_suffix, third_party_id,
       publication_pending, created_at, updated_at, linked_contract_address, linked_contract_network, lock,
       forum_link, forum_id)
  SELECT id, name, eth_address, salt, contract_address, urn_suffix, third_party_id,
         false, created_at, updated_at, linked_contract_address, linked_contract_network, lock,
         forum_link, forum_id
  FROM incoming_collection
  WHERE NOT imported AND id NOT IN (SELECT id FROM collection_urn_taken)
  RETURNING c.id, pg_temp.builder_collection_fingerprint(c) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection', id::text, fingerprint FROM inserted;
