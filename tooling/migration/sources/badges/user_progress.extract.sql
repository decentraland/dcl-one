-- progress is the per-badge document (the counter `steps` plus each
-- evaluator's dedupe and resume state); achieved_tiers is
-- [{tier_id, completed_at(ms)}] or NULL. Rows stay one per upstream address,
-- so two spellings of one wallet are merged by the load, not here.
SELECT p.user_address AS source_address,
       lower(p.user_address) AS address,
       p.badge_id,
       p.progress,
       CASE WHEN jsonb_typeof(p.progress -> 'steps') = 'number'
            THEN floor(LEAST((p.progress ->> 'steps')::numeric, 9223372036854775807))::bigint
       END AS steps,
       p.achieved_tiers,
       timestamptz 'epoch' + p.completed_at * interval '1 millisecond' AS completed_at,
       timestamptz 'epoch' + p.updated_at * interval '1 millisecond' AS updated_at
FROM user_progress p
WHERE lower(p.user_address) ~ '^0x[0-9a-f]{40}$'
