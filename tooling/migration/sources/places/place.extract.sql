-- The catalogue as dcl-one holds it: Genesis City places (world IS FALSE) that
-- are listed, opted out or disabled by moderation, and the worlds aggregate
-- (upstream `worlds`, id = lowercase name) for every world with an enabled
-- scene, as /api/worlds lists it. `raw` is the upstream wire row (p.* / w.*
-- without textsearch, the counters and the viewer columns), tagged
-- source = 'places' and local = true. World scenes go to world_scene_place.
-- Upstream timestamps are `timestamp without time zone` holding UTC.
WITH rating AS (
    SELECT DISTINCT ON (entity_id::text) entity_id::text AS entity_id, update_rating::text AS rating
    FROM content_ratings
    ORDER BY entity_id::text, created_at DESC, id DESC
), latest_scene AS (
    SELECT DISTINCT ON (p.world_id) p.world_id, p.image, p.contact_name, p.deployed_at
    FROM places p
    WHERE p.world_id IS NOT NULL AND p.disabled IS FALSE
    ORDER BY p.world_id, p.deployed_at DESC, p.id
)
SELECT p.id::text AS id,
       p.base_position::text AS base_position,
       p.title::text AS title,
       p.description,
       lower(NULLIF(btrim(COALESCE(p.creator_address::text, p.owner::text)), '')) AS creator_address,
       p.content_rating::text AS content_rating,
       p.categories::text[] AS categories,
       p.deployed_at AT TIME ZONE 'UTC' AS deployed_at,
       p.disabled,
       p.highlighted,
       NULL::text AS world_name,
       jsonb_build_object(
           'id', p.id::text, 'title', p.title, 'description', p.description, 'image', p.image,
           'owner', p.owner::text, 'positions', to_jsonb(p.positions::text[]),
           'base_position', p.base_position, 'contact_name', p.contact_name,
           'contact_email', p.contact_email, 'content_rating', p.content_rating,
           'disabled', p.disabled,
           'disabled_at', to_char(p.disabled_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'disabled_reason', p.disabled_reason,
           'created_at', to_char(p.created_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'updated_at', to_char(p.updated_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'categories', to_jsonb(p.categories::text[]), 'like_rate', p.like_rate,
           'like_score', p.like_score, 'highlighted', p.highlighted,
           'highlighted_image', p.highlighted_image, 'world', false, 'world_name', p.world_name,
           'world_id', p.world_id,
           'deployed_at', to_char(p.deployed_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'creator_address', p.creator_address::text, 'sdk', p.sdk, 'ranking', p.ranking,
           'deployment_id', p.deployment_id, 'exclude_from_ranking', p.exclude_from_ranking,
           'source', 'places', 'local', true
       ) AS raw,
       (p.disabled AND p.disabled_reason = 'moderation') AS mod_disabled,
       CASE WHEN r.rating = p.content_rating AND r.rating IN ('RP', 'E', 'T', 'A', 'R')
            THEN r.rating END AS mod_rating,
       NULLIF(jsonb_strip_nulls(jsonb_build_object(
           'ranking', CASE WHEN p.ranking <> 0 THEN p.ranking END,
           'exclude_from_ranking', CASE WHEN p.exclude_from_ranking THEN true END
       )), '{}'::jsonb) AS ranking_overlay
FROM places p
LEFT JOIN rating r ON r.entity_id = p.id::text
WHERE p.world IS FALSE
  AND (p.disabled IS FALSE OR p.disabled_reason IN ('opt_out', 'moderation'))
UNION ALL
SELECT w.id,
       '0,0',
       w.title::text,
       w.description,
       lower(NULLIF(btrim(w.owner::text), '')),
       w.content_rating,
       w.categories::text[],
       ls.deployed_at AT TIME ZONE 'UTC',
       false,
       w.highlighted,
       w.world_name,
       jsonb_build_object(
           'id', w.id, 'world_name', w.world_name, 'title', w.title, 'description', w.description,
           'image', COALESCE(w.image, ls.image), 'content_rating', w.content_rating,
           'categories', to_jsonb(w.categories::text[]), 'owner', w.owner::text,
           'show_in_places', w.show_in_places, 'single_player', w.single_player,
           'skybox_time', w.skybox_time, 'like_rate', w.like_rate, 'like_score', w.like_score,
           'created_at', to_char(w.created_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'updated_at', to_char(w.updated_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'is_private', w.is_private, 'highlighted', w.highlighted,
           'highlighted_image', w.highlighted_image, 'ranking', w.ranking,
           'settings_version', w.settings_version, 'exclude_from_ranking', w.exclude_from_ranking,
           'disabled', false, 'disabled_at', NULL, 'contact_name', ls.contact_name,
           'base_position', '0,0', 'world', true,
           'deployed_at', to_char(ls.deployed_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
           'source', 'places', 'local', true
       ),
       false,
       CASE WHEN r.rating = w.content_rating AND r.rating IN ('RP', 'E', 'T', 'A', 'R')
            THEN r.rating END,
       NULLIF(jsonb_strip_nulls(jsonb_build_object(
           'ranking', CASE WHEN w.ranking <> 0 THEN w.ranking END,
           'exclude_from_ranking', CASE WHEN w.exclude_from_ranking THEN true END
       )), '{}'::jsonb)
FROM worlds w
JOIN latest_scene ls ON ls.world_id = w.id
LEFT JOIN rating r ON r.entity_id = w.id
WHERE btrim(w.world_name) <> ''
