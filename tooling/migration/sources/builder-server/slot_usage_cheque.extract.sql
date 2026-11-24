SELECT u.id,
       u.collection_id,
       u.third_party_id,
       u.signature,
       u.salt,
       u.qty,
       u.created_at AT TIME ZONE 'UTC' AS created_at,
       u.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM slot_usage_cheques u
