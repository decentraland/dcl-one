-- A schedule written on this node after the upstream's last save is kept.
INSERT INTO explore.schedules_local AS t
    (id, name, description, image, theme, background, active_since, active_until, active, created_at, updated_at)
SELECT id, name, description, image, theme, background,
       active_since AT TIME ZONE 'UTC', active_until AT TIME ZONE 'UTC', active,
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name, description = EXCLUDED.description, image = EXCLUDED.image,
    theme = EXCLUDED.theme, background = EXCLUDED.background,
    active_since = EXCLUDED.active_since, active_until = EXCLUDED.active_until,
    active = EXCLUDED.active, created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at
WHERE t.updated_at <= EXCLUDED.updated_at;
