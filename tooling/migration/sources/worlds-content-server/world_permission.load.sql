-- Never discard an invalid restriction: an empty parcel set grants the whole world.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM stage WHERE invalid_parcels > 0) THEN
        RAISE EXCEPTION 'world permission has unrepresentable parcel restrictions; repair the source before migration';
    END IF;
END $$;

-- The stage is every upstream deployment/streaming grant (a `whole` step).
-- dcl_migration.world_permission_loaded holds the timestamps this step last
-- wrote per grant: a target row still holding them is the import's; any
-- other row was granted or edited on the node and is never touched.
CREATE TEMP TABLE wcs_grant AS
SELECT s.world_name, s.permission, s.address, s.parcels,
       s.created_at AT TIME ZONE 'UTC' AS created_at, s.updated_at AT TIME ZONE 'UTC' AS updated_at,
       CASE
           -- the node revoked this same upstream grant: stays revoked
           WHEN t.world_name IS NULL AND l.created_at = s.created_at AT TIME ZONE 'UTC' THEN 'skip'
           WHEN t.world_name IS NULL THEN 'write'
           WHEN (t.created_at, t.updated_at) = (l.created_at, l.updated_at) THEN 'write'
           -- an identical row imported before provenance was recorded
           WHEN l.world_name IS NULL
                AND (t.created_at, t.updated_at) = (s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC')
               THEN 'write'
           ELSE 'skip'
       END AS action
FROM stage s
JOIN worlds.worlds w ON w.name = s.world_name
LEFT JOIN worlds.world_permissions t
  ON t.world_name = s.world_name AND t.permission = s.permission AND t.address = s.address
LEFT JOIN dcl_migration.world_permission_loaded l
  ON l.world_name = s.world_name AND l.permission = s.permission AND l.address = s.address;

-- Revoked upstream: delete the imported grants (parcels by cascade) the node
-- left as imported, and forget every grant upstream no longer holds.
DELETE FROM worlds.world_permissions t
USING dcl_migration.world_permission_loaded l
WHERE t.world_name = l.world_name AND t.permission = l.permission AND t.address = l.address
  AND (t.created_at, t.updated_at) = (l.created_at, l.updated_at)
  AND NOT EXISTS (SELECT 1 FROM stage s
                  WHERE s.world_name = l.world_name AND s.permission = l.permission AND s.address = l.address);

DELETE FROM dcl_migration.world_permission_loaded l
WHERE NOT EXISTS (SELECT 1 FROM stage s
                  WHERE s.world_name = l.world_name AND s.permission = l.permission AND s.address = l.address);

-- A grant with no parcels is world-wide; the row carries the whole parcel
-- set, so the grant's parcel rows are replaced.
INSERT INTO worlds.world_permissions (world_name, permission, address, created_at, updated_at)
SELECT g.world_name, g.permission, g.address, g.created_at, g.updated_at
FROM wcs_grant g
WHERE g.action = 'write'
ON CONFLICT (world_name, permission, address) DO UPDATE SET
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;

DELETE FROM worlds.world_permission_parcels p
USING wcs_grant g
WHERE g.action = 'write'
  AND p.world_name = g.world_name AND p.permission = g.permission AND p.address = g.address
  AND NOT (p.parcel = ANY (g.parcels));

INSERT INTO worlds.world_permission_parcels (world_name, permission, address, parcel)
SELECT g.world_name, g.permission, g.address, x
FROM wcs_grant g
CROSS JOIN LATERAL unnest(g.parcels) x
WHERE g.action = 'write'
ON CONFLICT DO NOTHING;

INSERT INTO dcl_migration.world_permission_loaded AS l (world_name, permission, address, created_at, updated_at)
SELECT g.world_name, g.permission, g.address, g.created_at, g.updated_at
FROM wcs_grant g
WHERE g.action = 'write'
ON CONFLICT (world_name, permission, address) DO UPDATE SET
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;

DO $kept$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM wcs_grant WHERE action = 'skip';
  IF n > 0 THEN RAISE NOTICE
    'worlds-content-server/world_permission: kept % grants revoked or edited on the node', n;
  END IF; END $kept$;

DROP TABLE wcs_grant;
