SELECT e.id,
       e.collection_id,
       e.type,
       e.actor,
       e.actor_address,
       e.payload,
       e.created_at AT TIME ZONE 'UTC' AS created_at
FROM collection_events e
