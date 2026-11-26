-- Hold the shared side through the fact and count writes so a claim either
-- folds this batch's original UUID or lets the batch resolve the durable key.
SELECT pg_advisory_xact_lock_shared(hashtextextended('explore.scene_interaction_import', 0));
UPDATE stage SET entity_id = explore.scene_interaction_entity_id(entity_id)
WHERE entity_id IS DISTINCT FROM explore.scene_interaction_entity_id(entity_id);

-- Match the endpoint's lock order: counters, then facts.
INSERT INTO explore.place_interaction_counts (entity_id)
SELECT DISTINCT entity_id FROM stage ORDER BY entity_id
ON CONFLICT (entity_id) DO NOTHING;
DO $favorite_keys$ BEGIN
    PERFORM 1 FROM explore.place_interaction_counts
    WHERE entity_id IN (SELECT entity_id FROM stage)
    ORDER BY entity_id FOR UPDATE;
END $favorite_keys$;

INSERT INTO explore.user_favorites AS f ("user", entity_id, user_activity, created_at)
SELECT DISTINCT ON ("user", entity_id) "user", entity_id, user_activity, created_at AT TIME ZONE 'UTC'
FROM stage
ORDER BY "user", entity_id, created_at DESC, user_activity DESC
ON CONFLICT ("user", entity_id) DO UPDATE SET
    user_activity = EXCLUDED.user_activity,
    created_at = EXCLUDED.created_at
WHERE f.created_at <= EXCLUDED.created_at
  AND (f.user_activity, f.created_at) IS DISTINCT FROM (EXCLUDED.user_activity, EXCLUDED.created_at);

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
