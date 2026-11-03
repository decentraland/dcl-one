-- An RSVP or cancellation this node recorded after the upstream one wins.
INSERT INTO explore.event_attendance_local AS t (event_id, signer, signed_payload, action, signed_at)
SELECT s.event_id, s.signer, jsonb_build_object('user_name', s.user_name), 'going',
       s.signed_at AT TIME ZONE 'UTC'
FROM stage s
WHERE EXISTS (SELECT 1 FROM explore.event e WHERE e.id = s.event_id)
ON CONFLICT (event_id, signer) DO UPDATE SET
    signed_payload = EXCLUDED.signed_payload,
    action = 'going',
    signed_at = EXCLUDED.signed_at
WHERE t.signed_at <= EXCLUDED.signed_at;
