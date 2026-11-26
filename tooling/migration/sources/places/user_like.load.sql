-- Hold the shared side through the fact and count writes so a claim either
-- folds this batch's original UUID or lets the batch resolve the durable key.
SELECT pg_advisory_xact_lock_shared(hashtextextended('explore.scene_interaction_import', 0));
UPDATE stage SET entity_id = explore.scene_interaction_entity_id(entity_id)
WHERE entity_id IS DISTINCT FROM explore.scene_interaction_entity_id(entity_id);

-- Match the endpoint's lock order: counters, then facts.
INSERT INTO explore.place_interaction_counts (entity_id)
SELECT DISTINCT entity_id FROM stage ORDER BY entity_id
ON CONFLICT (entity_id) DO NOTHING;
DO $like_keys$ BEGIN
    PERFORM 1 FROM explore.place_interaction_counts
    WHERE entity_id IN (SELECT entity_id FROM stage)
    ORDER BY entity_id FOR UPDATE;
END $like_keys$;

-- Match the claim's conflict order so imports converge before or after a claim.
INSERT INTO explore.user_likes AS l ("user", entity_id, "like", user_activity, source, created_at, updated_at)
SELECT DISTINCT ON ("user", entity_id) "user", entity_id, "like", user_activity, 'upstream',
       created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC'
FROM stage
ORDER BY "user", entity_id, stage.updated_at DESC, stage.created_at DESC, "like", user_activity DESC
ON CONFLICT ("user", entity_id) DO UPDATE SET
    "like" = EXCLUDED."like",
    user_activity = EXCLUDED.user_activity,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at
WHERE l.source = 'upstream'
  AND (l.updated_at, l.created_at, NOT l."like", l.user_activity)
      < (EXCLUDED.updated_at, EXCLUDED.created_at, NOT EXCLUDED."like", EXCLUDED.user_activity);

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

-- World-scene rows have no upstream catalogue aggregate to provide rates.
-- Refresh their qualified-vote rates from the same facts as Explore's recount.
WITH scenes AS (
    SELECT DISTINCT e.entity_id FROM stage e
    JOIN explore.world_scene_place p ON p.identity = CASE
        WHEN pg_input_is_valid(e.entity_id, 'uuid') THEN e.entity_id::uuid END
), qualified AS (
    SELECT s.entity_id, count(*) FILTER (WHERE l."like") AS likes,
           count(*) FILTER (WHERE NOT l."like") AS dislikes
    FROM scenes s LEFT JOIN explore.user_likes l
      ON l.entity_id = s.entity_id AND l.user_activity >= 100
    GROUP BY s.entity_id
), rates AS (
    SELECT entity_id, likes, dislikes, NULLIF(likes + dislikes, 0) AS total
    FROM qualified
)
UPDATE explore.place_interaction_counts c
SET like_rate = r.likes::float8 / r.total,
    like_score = ((r.likes + 1.9208) / r.total - 1.96 * sqrt((r.likes * r.dislikes) / r.total + 0.9604) / r.total)
                 / (1.0 + 3.8416 / r.total)
FROM rates r WHERE c.entity_id = r.entity_id;
