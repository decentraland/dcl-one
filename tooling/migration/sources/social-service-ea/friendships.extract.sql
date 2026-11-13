-- The node keeps the latest friendship_actions row as a projection on the
-- friendship (last_action, last_actor, last_action_at; is_active follows it).
-- It is derived here from upstream's log, so the imported state is what the
-- node's transition table sees. A friendship without actions projects to
-- 'accept' by the requested wallet when active, 'delete' otherwise.
-- Upstream's pair key is ordered and case-sensitive; the node's is unordered
-- and lowercase: of A->B, B->A and their case variants the most recently
-- updated row wins (then the larger id).
-- A latest `accept` not preceded by the other wallet's `request` was forged
-- through upstream's missing state-machine check (fixed upstream in #427).
-- It is neutralized: the friendship projects to `delete` by the forger one
-- millisecond after the accept, matching the `delete` action the
-- friendship_actions step appends, so the pair can only become friends again
-- through a real request and accept. The accept stays in the history.
WITH base AS (
    SELECT f.*, la.action AS la_action, la.acting_user AS la_actor, la.at AS la_at,
           la.action = 'accept'
             AND (pa.action IS DISTINCT FROM 'request' OR lower(pa.acting_user) = lower(la.acting_user)) AS forged
    FROM friendships f
    LEFT JOIN LATERAL (
        SELECT a.id, a.action::text AS action, a.acting_user, COALESCE(a."timestamp", 'epoch') AS at
        FROM friendship_actions a
        WHERE a.friendship_id = f.id
          AND a.action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block')
        ORDER BY a."timestamp" DESC NULLS LAST, a.id DESC
        LIMIT 1
    ) la ON true
    LEFT JOIN LATERAL (
        SELECT p.action::text AS action, p.acting_user
        FROM friendship_actions p
        WHERE p.friendship_id = f.id
          AND p.action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block')
          AND (COALESCE(p."timestamp", 'epoch'), p.id) < (la.at, la.id)
        ORDER BY p."timestamp" DESC NULLS LAST, p.id DESC
        LIMIT 1
    ) pa ON la.action = 'accept'
)
SELECT f.id,
       lower(f.address_requester) AS address_requester,
       lower(f.address_requested) AS address_requested,
       CASE WHEN f.forged THEN 'delete'
            ELSE COALESCE(f.la_action, CASE WHEN COALESCE(f.is_active, false) THEN 'accept' ELSE 'delete' END)
       END AS last_action,
       CASE
           WHEN f.forged THEN lower(f.la_actor)
           WHEN lower(f.la_actor) IN (lower(f.address_requester), lower(f.address_requested))
               THEN lower(f.la_actor)
           WHEN f.la_action IS NULL AND COALESCE(f.is_active, false) THEN lower(f.address_requested)
           ELSE lower(f.address_requester)
       END AS last_actor,
       CASE WHEN f.forged THEN (f.la_at + interval '1 millisecond') AT TIME ZONE 'UTC'
            ELSE COALESCE(f.la_at, f.updated_at, f.created_at, 'epoch') AT TIME ZONE 'UTC'
       END AS last_action_at,
       COALESCE(f.created_at, f.updated_at, 'epoch') AT TIME ZONE 'UTC' AS created_at,
       COALESCE(f.updated_at, f.created_at, 'epoch') AT TIME ZONE 'UTC' AS updated_at
FROM base f
WHERE lower(f.address_requester) ~ '^0x[0-9a-f]{40}$'
  AND lower(f.address_requested) ~ '^0x[0-9a-f]{40}$'
  AND lower(f.address_requester) <> lower(f.address_requested)
  AND NOT EXISTS (
      SELECT 1 FROM friendships r
      WHERE LEAST(lower(r.address_requester), lower(r.address_requested))
            = LEAST(lower(f.address_requester), lower(f.address_requested))
        AND GREATEST(lower(r.address_requester), lower(r.address_requested))
            = GREATEST(lower(f.address_requester), lower(f.address_requested))
        AND (COALESCE(r.updated_at, r.created_at, 'epoch'), r.id)
          > (COALESCE(f.updated_at, f.created_at, 'epoch'), f.id))
