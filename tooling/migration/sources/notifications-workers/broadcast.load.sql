-- The node stores a broadcast as one row per recipient, idempotent per
-- (address, broadcast_key); recipients are the subscriptions, as in the
-- node's own broadcast. Row ids derive from (upstream id, address).
INSERT INTO social.notifications
    (id, address, type, metadata, broadcast_key, occurred_at, created_at)
SELECT md5(st.id::text || ':' || s.address)::uuid,
       s.address, st.type, st.metadata,
       'notifications-workers:' || st.id::text,
       st.occurred_at, st.created_at
FROM stage st
CROSS JOIN social.subscriptions s
ON CONFLICT (address, broadcast_key) WHERE broadcast_key IS NOT NULL DO UPDATE
   SET type = EXCLUDED.type,
       metadata = EXCLUDED.metadata,
       occurred_at = EXCLUDED.occurred_at
