-- Upstream is unique on (place_id, banned_address); lower() could merge two.
SELECT place_id, banned_address, banned_by, banned_at
FROM (
    SELECT place_id,
           lower(banned_address) AS banned_address,
           CASE WHEN banned_by ~* '^0x[0-9a-f]{40}$' THEN lower(banned_by) ELSE banned_by END AS banned_by,
           to_timestamp(banned_at / 1000.0) AS banned_at,
           row_number() OVER (PARTITION BY place_id, lower(banned_address)
                              ORDER BY banned_at, id) AS pick
    FROM scene_bans
    WHERE lower(banned_address) ~ '^0x[0-9a-f]{40}$'
) b
WHERE pick = 1
