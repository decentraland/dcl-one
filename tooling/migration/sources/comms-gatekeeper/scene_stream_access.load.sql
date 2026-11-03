-- Upstream decides which credential of a place is active: retire any other
-- active row of a staged active place first (keeps the one-active index).
UPDATE social.scene_stream_access t
SET active = false, streaming = false
FROM stage s
WHERE s.active AND t.place_id = s.place_id AND t.active AND t.id <> s.id;

INSERT INTO social.scene_stream_access
    (id, place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by,
     created_at, expiration_time, streaming, streaming_start_time,
     streaming_state_version, streaming_checked_at, cleanup_retry_at, active)
SELECT id, place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by,
       created_at AT TIME ZONE 'UTC', expiration_time AT TIME ZONE 'UTC',
       streaming AND active, streaming_start_time AT TIME ZONE 'UTC',
       streaming_state_version, streaming_checked_at AT TIME ZONE 'UTC', NULL, active
FROM stage
ON CONFLICT (id) DO UPDATE
SET place_id = EXCLUDED.place_id,
    streaming_url = EXCLUDED.streaming_url,
    streaming_key = EXCLUDED.streaming_key,
    ingress_id = EXCLUDED.ingress_id,
    room_id = EXCLUDED.room_id,
    generated_by = EXCLUDED.generated_by,
    created_at = EXCLUDED.created_at,
    expiration_time = EXCLUDED.expiration_time,
    streaming = EXCLUDED.streaming,
    streaming_start_time = EXCLUDED.streaming_start_time,
    streaming_state_version = EXCLUDED.streaming_state_version,
    streaming_checked_at = EXCLUDED.streaming_checked_at,
    active = EXCLUDED.active;

-- An unfinished upstream ingress deletion becomes a node cleanup intent, due
-- now; a historic row without catalogue context, so the node resolves the
-- owner for the expiry notice (sent only when no newer credential exists).
INSERT INTO social.scene_stream_cleanup
    (access_id, place_id, ingress_id, notification_kind, occurred_at, due_at)
SELECT id, place_id, ingress_id,
       CASE WHEN cleanup_expired THEN 'streaming_key_expired' END,
       CASE WHEN cleanup_expired
            THEN COALESCE(expiration_time, created_at + interval '4 days')
            ELSE created_at END AT TIME ZONE 'UTC',
       created_at AT TIME ZONE 'UTC'
FROM stage
WHERE cleanup_pending AND NOT active
ON CONFLICT (access_id) DO NOTHING;
