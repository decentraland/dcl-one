-- Epoch-millisecond columns become timestamps; 0 means "never" upstream.
SELECT id,
       place_id,
       streaming_url,
       streaming_key,
       NULLIF(ingress_id, '') AS ingress_id,
       room_id,
       CASE WHEN generated_by ~* '^0x[0-9a-f]{40}$' THEN lower(generated_by)
            ELSE COALESCE(generated_by, '') END AS generated_by,
       to_timestamp(created_at / 1000.0) AS created_at,
       to_timestamp(expiration_time / 1000.0) AS expiration_time,
       streaming,
       to_timestamp(streaming_start_time / 1000.0) AS streaming_start_time,
       streaming_state_version,
       to_timestamp(NULLIF(streaming_checked_at, 0) / 1000.0) AS streaming_checked_at,
       active,
       ingress_cleanup_pending AS cleanup_pending,
       ingress_cleanup_expired AS cleanup_expired
FROM scene_stream_access
