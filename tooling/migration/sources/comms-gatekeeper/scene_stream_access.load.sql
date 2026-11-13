-- Per staged row, whether it is the place's active credential once loaded:
-- never a credential the node retired, nor one older than the node's active one.
CREATE TEMP TABLE migration_stream_access_active ON COMMIT DROP AS
SELECT s.id, s.place_id,
       EXISTS (SELECT 1 FROM social.scene_stream_access t WHERE t.id = s.id) AS present,
       s.active
       AND NOT EXISTS (SELECT 1 FROM social.scene_stream_access t WHERE t.id = s.id AND NOT t.active)
       AND NOT EXISTS (SELECT 1 FROM social.scene_stream_access t
                       WHERE t.place_id = s.place_id AND t.active AND t.id <> s.id
                         AND t.created_at > s.created_at AT TIME ZONE 'UTC') AS active
FROM stage s;

-- Retire the place's other active credential (one-active index), with the
-- cleanup intent the node writes when it retires one, so its ingress is deleted.
WITH retired AS (
    UPDATE social.scene_stream_access t
    SET active = false, streaming = false
    FROM migration_stream_access_active a
    WHERE a.active AND t.place_id = a.place_id AND t.active AND t.id <> a.id
    RETURNING t.id, t.place_id, t.ingress_id
)
INSERT INTO social.scene_stream_cleanup (access_id, place_id, ingress_id, occurred_at, due_at)
SELECT id, place_id, ingress_id, timezone('UTC', now()), timezone('UTC', now())
FROM retired
ON CONFLICT (access_id) DO NOTHING;

-- A row retired on the node stays retired; node streaming state with a newer
-- webhook version wins.
INSERT INTO social.scene_stream_access AS t
    (id, place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by,
     created_at, expiration_time, streaming, streaming_start_time,
     streaming_state_version, streaming_checked_at, cleanup_retry_at, active)
SELECT s.id, s.place_id, s.streaming_url, s.streaming_key, s.ingress_id, s.room_id, s.generated_by,
       s.created_at AT TIME ZONE 'UTC', s.expiration_time AT TIME ZONE 'UTC',
       s.streaming AND a.active, s.streaming_start_time AT TIME ZONE 'UTC',
       s.streaming_state_version, s.streaming_checked_at AT TIME ZONE 'UTC', NULL, a.active
FROM stage s
JOIN migration_stream_access_active a ON a.id = s.id
ON CONFLICT (id) DO UPDATE
SET place_id = EXCLUDED.place_id,
    streaming_url = EXCLUDED.streaming_url,
    streaming_key = EXCLUDED.streaming_key,
    ingress_id = EXCLUDED.ingress_id,
    room_id = EXCLUDED.room_id,
    generated_by = EXCLUDED.generated_by,
    created_at = EXCLUDED.created_at,
    expiration_time = EXCLUDED.expiration_time,
    streaming = CASE WHEN NOT (t.active AND EXCLUDED.active) THEN false
                     WHEN t.streaming_state_version > EXCLUDED.streaming_state_version THEN t.streaming
                     ELSE EXCLUDED.streaming END,
    streaming_start_time = CASE WHEN t.streaming_state_version > EXCLUDED.streaming_state_version
                                THEN t.streaming_start_time ELSE EXCLUDED.streaming_start_time END,
    streaming_checked_at = CASE WHEN t.streaming_state_version > EXCLUDED.streaming_state_version
                                THEN t.streaming_checked_at ELSE EXCLUDED.streaming_checked_at END,
    streaming_state_version = GREATEST(t.streaming_state_version, EXCLUDED.streaming_state_version),
    active = t.active AND EXCLUDED.active;

-- An unfinished upstream ingress deletion becomes a node cleanup intent, due
-- now; a historic row without catalogue context, so the node resolves the
-- owner for the expiry notice (sent only when no newer credential exists).
-- A new upstream-active credential kept inactive here (the node holds a
-- newer one) still has a live ingress, so it gets an intent with no notice.
INSERT INTO social.scene_stream_cleanup
    (access_id, place_id, ingress_id, notification_kind, occurred_at, due_at)
SELECT s.id, s.place_id, s.ingress_id,
       CASE WHEN s.cleanup_pending AND s.cleanup_expired THEN 'streaming_key_expired' END,
       CASE WHEN NOT s.cleanup_pending THEN timezone('UTC', now())
            WHEN s.cleanup_expired
            THEN COALESCE(s.expiration_time, s.created_at + interval '4 days') AT TIME ZONE 'UTC'
            ELSE s.created_at AT TIME ZONE 'UTC' END,
       CASE WHEN s.cleanup_pending THEN s.created_at AT TIME ZONE 'UTC' ELSE timezone('UTC', now()) END
FROM stage s
JOIN migration_stream_access_active a ON a.id = s.id
WHERE (s.cleanup_pending AND NOT s.active) OR (s.active AND NOT a.active AND NOT a.present)
ON CONFLICT (access_id) DO NOTHING;

DROP TABLE migration_stream_access_active;
