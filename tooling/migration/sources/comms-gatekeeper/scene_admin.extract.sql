-- One row per (place, admin): the active grant, else the newest revoked one.
SELECT place_id, admin_address, added_by, added_at, active
FROM (
    SELECT place_id,
           lower(admin) AS admin_address,
           CASE WHEN added_by ~* '^0x[0-9a-f]{40}$' THEN lower(added_by) ELSE added_by END AS added_by,
           to_timestamp(created_at / 1000.0) AS added_at,
           active,
           row_number() OVER (PARTITION BY place_id, lower(admin)
                              ORDER BY active DESC, created_at DESC, id DESC) AS pick
    FROM scene_admin
    WHERE lower(admin) ~ '^0x[0-9a-f]{40}$'
) a
WHERE pick = 1
