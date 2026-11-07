-- Every upstream world-scene place (places.world IS TRUE) as a world-scene
-- identity: the place UUID is the identity world storage is keyed on, kept
-- for retired scenes too. A row is the active binding of its deployment when
-- it carries a deployment_id, is enabled or opted out, and no newer such row
-- of the same world overlaps it (backfilled opt_out rows may be stale scenes
-- under the live one; upstream resolvers prefer enabled, then newest).
WITH scene AS (
    SELECT p.id::text AS id,
           lower(btrim(COALESCE(p.world_id, p.world_name))) AS world_name,
           p.deployment_id,
           CASE WHEN cardinality(p.positions) > 0 THEN p.positions::text[]
                ELSE ARRAY[p.base_position::text] END AS parcels,
           p.disabled,
           p.deployed_at,
           p.deployment_id IS NOT NULL
               AND (p.disabled IS FALSE OR p.disabled_reason = 'opt_out') AS candidate,
           p.created_at,
           p.updated_at
    FROM places p
    WHERE p.world IS TRUE
      AND btrim(COALESCE(p.world_id, p.world_name, '')) <> ''
      AND p.id::text ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
)
SELECT s.world_name,
       s.id::uuid AS identity,
       CASE WHEN live.active THEN s.deployment_id END AS entity_id,
       s.parcels,
       live.active,
       s.created_at AT TIME ZONE 'UTC' AS created_at,
       s.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM scene s
CROSS JOIN LATERAL (
    SELECT s.candidate AND NOT EXISTS (
        SELECT 1 FROM scene o
        WHERE o.candidate AND o.world_name = s.world_name AND o.id <> s.id
          AND (o.parcels && s.parcels OR o.deployment_id = s.deployment_id)
          AND (o.disabled, s.deployed_at, s.id) < (s.disabled, o.deployed_at, o.id)
    ) AS active
) live
