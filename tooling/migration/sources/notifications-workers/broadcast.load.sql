-- The node stores a broadcast as one row per recipient, idempotent per
-- (address, broadcast_key); row ids derive from (upstream id, address).
-- Upstream showed a broadcast to every caller; the audience here is every
-- wallet the target knows from the notification tables: what the earlier
-- steps imported (subscriptions, addressed notifications, broadcast_read's
-- copies, opt-outs, pending emails) plus the node's own rows there and every
-- wallet that polled the node's bell. Each stage row covers one address shard.
CREATE TEMP TABLE broadcast_audience ON COMMIT DROP AS
SELECT sh.shard, a.address
FROM (SELECT DISTINCT shard FROM stage) sh
CROSS JOIN LATERAL (
    SELECT address FROM social.subscriptions
     WHERE address >= '0x' || sh.shard AND address < '0x' || sh.shard || 'g'
    UNION
    SELECT address FROM social.notifications
     WHERE address >= '0x' || sh.shard AND address < '0x' || sh.shard || 'g'
    UNION
    SELECT address FROM social.subscription_opt_outs
     WHERE address >= '0x' || sh.shard AND address < '0x' || sh.shard || 'g'
    UNION
    SELECT address FROM social.unconfirmed_emails
     WHERE address >= '0x' || sh.shard AND address < '0x' || sh.shard || 'g'
    UNION
    SELECT address FROM social.notification_reader_seen
     WHERE address >= '0x' || sh.shard AND address < '0x' || sh.shard || 'g'
) a
WHERE a.address ~ '^0x[0-9a-f]{40}$' AND substr(a.address, 3, 3) = sh.shard;

INSERT INTO social.notifications
    (id, address, type, metadata, broadcast_key, occurred_at, created_at)
SELECT md5(st.id::text || ':' || a.address)::uuid,
       a.address, st.type, st.metadata,
       'notifications-workers:' || st.id::text,
       st.occurred_at, st.created_at
FROM stage st
JOIN broadcast_audience a ON a.shard = st.shard
ON CONFLICT (address, broadcast_key) WHERE broadcast_key IS NOT NULL DO UPDATE
   SET type = EXCLUDED.type,
       metadata = EXCLUDED.metadata,
       occurred_at = EXCLUDED.occurred_at
 WHERE (social.notifications.type, social.notifications.metadata, social.notifications.occurred_at)
       IS DISTINCT FROM (EXCLUDED.type, EXCLUDED.metadata, EXCLUDED.occurred_at);

DROP TABLE broadcast_audience;
