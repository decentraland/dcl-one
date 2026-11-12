-- Aggregate wallet spellings before pagination: every batch and incremental
-- walk sees the complete state of each selected wallet/badge pair.
WITH rows AS (
    SELECT p.*, lower(p.user_address) AS address
    FROM user_progress p
    WHERE lower(p.user_address) ~ '^0x[0-9a-f]{40}$'
), grouped AS (
    SELECT address, badge_id, min(user_address) AS source_address,
           max(updated_at) AS updated_at, min(completed_at) AS completed_at,
           jsonb_agg(progress) AS documents,
           jsonb_agg(COALESCE(achieved_tiers, '[]'::jsonb)) AS tier_documents,
           (array_agg(progress ORDER BY
               CASE WHEN jsonb_typeof(progress -> 'steps') = 'number'
                    THEN (progress ->> 'steps')::numeric END DESC NULLS LAST,
               updated_at DESC, user_address))[1] AS progress
    FROM rows GROUP BY address, badge_id
), merged AS (
    SELECT g.*, g.progress || COALESCE((
        SELECT jsonb_object_agg(k.key, (
            SELECT COALESCE(jsonb_agg(DISTINCT e ORDER BY e), '[]'::jsonb)
            FROM jsonb_array_elements(g.documents) d
            CROSS JOIN LATERAL jsonb_array_elements(
                CASE WHEN jsonb_typeof(d -> k.key) = 'array' THEN d -> k.key ELSE '[]'::jsonb END) e))
        FROM (SELECT DISTINCT kv.key FROM jsonb_array_elements(g.documents) d
              CROSS JOIN LATERAL jsonb_each(CASE WHEN jsonb_typeof(d) = 'object'
                                               THEN d ELSE '{}'::jsonb END) kv
              WHERE jsonb_typeof(kv.value) = 'array') k), '{}'::jsonb) AS document
    FROM grouped g
)
SELECT source_address, address, badge_id, document AS progress,
       CASE WHEN jsonb_typeof(document -> 'steps') = 'number'
            THEN floor(LEAST((document ->> 'steps')::numeric, 9223372036854775807))::bigint
       END AS steps,
       (SELECT jsonb_agg(e) FROM jsonb_array_elements(tier_documents) d
        CROSS JOIN LATERAL jsonb_array_elements(
            CASE WHEN jsonb_typeof(d) = 'array' THEN d ELSE '[]'::jsonb END) e) AS achieved_tiers,
       timestamptz 'epoch' + completed_at * interval '1 millisecond' AS completed_at,
       timestamptz 'epoch' + updated_at * interval '1 millisecond' AS updated_at
FROM merged
