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
WHERE (s.imported
       AND (k.status, k.assignee, k.reviewed_by, k.rejection_reasons, k.rejection_message, k.updated_at)
           IS DISTINCT FROM (s.status, s.assignee, s.reviewed_by, s.rejection_reasons, s.rejection_message, s.updated_at))
   OR (NOT s.imported AND EXISTS (SELECT 1 FROM builder.collections c WHERE c.id = s.collection_id));

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(ARRAY(SELECT DISTINCT unit_collection FROM collection_curation_plan), '{}');
END $lock$;

WITH refreshed AS (
  UPDATE builder.collection_curations AS k
     SET (status, assignee, reviewed_by, rejection_reasons, rejection_message, updated_at)
       = (s.status, s.assignee, s.reviewed_by, s.rejection_reasons, s.rejection_message, s.updated_at)
    FROM incoming_collection_curation s JOIN collection_curation_plan p ON p.id = s.id
   WHERE k.id = s.id AND s.imported
  RETURNING k.id, pg_temp.builder_collection_curation_fingerprint(k) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection_curation', id::text, fingerprint FROM refreshed
ON CONFLICT (step, id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint;

WITH inserted AS (
  INSERT INTO builder.collection_curations AS k
      (id, collection_id, status, assignee, reviewed_by, rejection_reasons, rejection_message, created_at, updated_at)
  SELECT DISTINCT ON (s.collection_id, s.created_at)
         s.id, s.collection_id, s.status, s.assignee, s.reviewed_by, s.rejection_reasons, s.rejection_message,
         s.created_at, s.updated_at
  FROM incoming_collection_curation s
  JOIN collection_curation_plan p ON p.id = s.id
  WHERE NOT s.imported
    AND NOT EXISTS (SELECT 1 FROM builder.collection_curations o
                    WHERE o.collection_id = s.collection_id AND o.created_at = s.created_at AND o.id <> s.id)
  ORDER BY s.collection_id, s.created_at, s.upstream_updated_at DESC, s.id DESC
  RETURNING k.id, pg_temp.builder_collection_curation_fingerprint(k) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection_curation', id::text, fingerprint FROM inserted;
