CREATE TEMP TABLE incoming_collection_event ON COMMIT DROP AS
SELECT s.id, s.collection_id, s.type, s.actor,
       CASE WHEN lower(s.actor_address) ~ '^0x[0-9a-f]{40}$' THEN lower(s.actor_address) END AS actor_address,
       s.payload, s.created_at AT TIME ZONE 'UTC' AS created_at
FROM stage s
JOIN builder.collections c ON c.id = s.collection_id
WHERE s.type IN ('collection.published', 'review.human_required', 'review.automated_started',
                 'review.automated_passed', 'review.automated_rejected', 'review.automated_error',
                 'review.appeal_requested', 'review.assigned', 'review.approved', 'review.rejected',
                 'changes.submitted', 'review.comment')
  AND s.actor IN ('creator', 'curator', 'validator', 'system')
  AND jsonb_typeof(s.payload) = 'object'
  AND NOT EXISTS (SELECT 1 FROM builder.collection_events e WHERE e.id = s.id);

DO $lock$ BEGIN
  PERFORM pg_temp.builder_lock_units(ARRAY(SELECT DISTINCT collection_id FROM incoming_collection_event), '{}');
END $lock$;

WITH inserted AS (
  INSERT INTO builder.collection_events AS e (id, collection_id, type, actor, actor_address, payload, created_at)
  SELECT id, collection_id, type, actor, actor_address, payload, created_at
  FROM incoming_collection_event
  ON CONFLICT DO NOTHING
  RETURNING e.id, pg_temp.builder_collection_event_fingerprint(e) AS fingerprint)
INSERT INTO dcl_migration.builder_imported (step, id, fingerprint)
SELECT 'collection_event', id::text, fingerprint FROM inserted;
