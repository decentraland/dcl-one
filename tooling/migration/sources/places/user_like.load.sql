-- A like changed on the node after the upstream one (a later updated_at) wins.
INSERT INTO explore.user_likes AS l ("user", entity_id, "like", user_activity, created_at, updated_at)
SELECT "user", entity_id, "like", user_activity,
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT ("user", entity_id) DO UPDATE SET
    "like" = EXCLUDED."like",
    user_activity = EXCLUDED.user_activity,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE l.updated_at <= EXCLUDED.updated_at
  AND (l."like", l.user_activity, l.created_at, l.updated_at)
      IS DISTINCT FROM (EXCLUDED."like", EXCLUDED.user_activity, EXCLUDED.created_at, EXCLUDED.updated_at);

-- place_interaction_counts is derived from the two fact tables (the rule of
-- places::interactions REBUILD_COUNTS); recount every entity of the batch.
INSERT INTO explore.place_interaction_counts AS c (entity_id, favorites, likes, dislikes, updated_at)
SELECT e.entity_id,
       (SELECT count(*) FROM explore.user_favorites f WHERE f.entity_id = e.entity_id),
       (SELECT count(*) FROM explore.user_likes l WHERE l.entity_id = e.entity_id AND l."like"),
       (SELECT count(*) FROM explore.user_likes l WHERE l.entity_id = e.entity_id AND NOT l."like"),
       timezone('UTC', now())
FROM (SELECT DISTINCT entity_id FROM stage) e
ON CONFLICT (entity_id) DO UPDATE SET
    favorites = EXCLUDED.favorites,
    likes = EXCLUDED.likes,
    dislikes = EXCLUDED.dislikes,
    updated_at = EXCLUDED.updated_at
WHERE (c.favorites, c.likes, c.dislikes) IS DISTINCT FROM (EXCLUDED.favorites, EXCLUDED.likes, EXCLUDED.dislikes);
