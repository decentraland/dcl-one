-- A grant with no parcels is world-wide; the row carries the whole parcel
-- set, so the grant's parcel rows are replaced.
INSERT INTO worlds.world_permissions (world_name, permission, address, created_at, updated_at)
SELECT s.world_name, s.permission, s.address,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
JOIN worlds.worlds w ON w.name = s.world_name
ON CONFLICT (world_name, permission, address) DO UPDATE SET
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;

DELETE FROM worlds.world_permission_parcels p
USING stage s
WHERE p.world_name = s.world_name AND p.permission = s.permission AND p.address = s.address
  AND NOT (p.parcel = ANY (s.parcels));

INSERT INTO worlds.world_permission_parcels (world_name, permission, address, parcel)
SELECT s.world_name, s.permission, s.address, x
FROM stage s
CROSS JOIN LATERAL unnest(s.parcels) x
JOIN worlds.world_permissions g
  ON g.world_name = s.world_name AND g.permission = s.permission AND g.address = s.address
ON CONFLICT DO NOTHING;
