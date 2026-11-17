-- The history of the friendships the previous step extracts (same filters),
-- plus one `delete` by the forger right after each forged accept (see
-- friendships.extract.sql), with an id derived from the friendship so a
-- re-run upserts the same row. Its metadata is NULL, as a node delete's is.
WITH kept AS (
    SELECT f.id
    FROM friendships f
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
), forged AS (
    SELECT k.id AS friendship_id, la.acting_user, la.at
    FROM kept k
    JOIN LATERAL (
        SELECT a.id, a.action::text AS action, a.acting_user, COALESCE(a."timestamp", 'epoch') AS at
        FROM friendship_actions a
        WHERE a.friendship_id = k.id
          AND a.action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block')
        ORDER BY a."timestamp" DESC NULLS LAST, a.id DESC
        LIMIT 1
    ) la ON la.action = 'accept'
    LEFT JOIN LATERAL (
        SELECT p.action::text AS action, p.acting_user
        FROM friendship_actions p
        WHERE p.friendship_id = k.id
          AND p.action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block')
          AND (COALESCE(p."timestamp", 'epoch'), p.id) < (la.at, la.id)
        ORDER BY p."timestamp" DESC NULLS LAST, p.id DESC
        LIMIT 1
    ) pa ON true
    WHERE pa.action IS DISTINCT FROM 'request' OR lower(pa.acting_user) = lower(la.acting_user)
)
SELECT a.id,
       a.friendship_id,
       a.action::text AS action,
       lower(a.acting_user) AS acting_user,
       a.metadata,
       COALESCE(a."timestamp", 'epoch') AT TIME ZONE 'UTC' AS acted_at
FROM friendship_actions a
JOIN kept k ON k.id = a.friendship_id
WHERE a.action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block')
UNION ALL
SELECT md5('dcl-migration:forged-accept:' || g.friendship_id::text)::uuid,
       g.friendship_id,
       'delete',
       lower(g.acting_user),
       NULL::json,
       (g.at + interval '1 millisecond') AT TIME ZONE 'UTC'
FROM forged g
