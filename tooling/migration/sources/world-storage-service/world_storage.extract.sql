SELECT world_name::text AS world_name, place_id, key::text AS key, value, value_size,
       created_at AT TIME ZONE 'UTC' AS created_at,
       updated_at AT TIME ZONE 'UTC' AS updated_at
FROM world_storage
