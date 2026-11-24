SELECT ic.id,
       ic.item_id,
       ic.status::text AS status,
       ic.content_hash,
       ic.is_mapping_complete,
       ic.created_at AT TIME ZONE 'UTC' AS created_at,
       ic.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM item_curations ic
