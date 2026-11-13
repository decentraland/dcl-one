-- Seed parents as well so this step can be resumed or run independently.
INSERT INTO social.broadcast_notifications (id, type, metadata, occurred_at, created_at)
SELECT DISTINCT ON (notification_id) notification_id, type, metadata,
       occurred_at AT TIME ZONE 'UTC', created_at AT TIME ZONE 'UTC'
FROM stage ORDER BY notification_id, source_address
ON CONFLICT (id) DO NOTHING;

INSERT INTO social.broadcast_reads AS t (broadcast_id, address, read_at)
SELECT notification_id, address, min(read_at) AT TIME ZONE 'UTC'
FROM stage GROUP BY notification_id, address
ON CONFLICT (broadcast_id, address) DO UPDATE SET read_at = LEAST(t.read_at, EXCLUDED.read_at);
