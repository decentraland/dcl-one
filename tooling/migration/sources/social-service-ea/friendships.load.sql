-- A pair the node already holds under another id stays the node's (its
-- actions are skipped by the next step). A row the node moved after the
-- import (a newer local action) is not rolled back.
INSERT INTO social.friendships AS t
    (id, address_requester, address_requested, is_active, last_action, last_actor, last_action_at,
     created_at, updated_at)
SELECT s.id, s.address_requester, s.address_requested, s.last_action = 'accept', s.last_action, s.last_actor,
       s.last_action_at AT TIME ZONE 'UTC', s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
WHERE NOT EXISTS (
    SELECT 1 FROM social.friendships o
    WHERE LEAST(o.address_requester, o.address_requested) = LEAST(s.address_requester, s.address_requested)
      AND GREATEST(o.address_requester, o.address_requested) = GREATEST(s.address_requester, s.address_requested)
      AND o.id <> s.id)
ON CONFLICT (id) DO UPDATE SET
    address_requester = EXCLUDED.address_requester,
    address_requested = EXCLUDED.address_requested,
    is_active = EXCLUDED.is_active,
    last_action = EXCLUDED.last_action,
    last_actor = EXCLUDED.last_actor,
    last_action_at = EXCLUDED.last_action_at,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE t.last_action_at <= EXCLUDED.last_action_at;
