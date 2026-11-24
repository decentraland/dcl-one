SELECT v.id,
       v.managers,
       v.raw_metadata,
       v."isProgrammatic" AS is_programmatic,
       v.created_at AT TIME ZONE 'UTC' AS created_at,
       v.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM virtual_third_parties v
