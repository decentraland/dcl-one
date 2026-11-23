-- ranking_score and the counters it is computed from are loaded by the
-- community_ranking_metrics step.
SELECT c.id,
       c.name,
       c.description,
       lower(c.owner_address) AS owner_address,
       c.private,
       c.active,
       c.unlisted,
       c.editors_choice,
       c.created_at AT TIME ZONE 'UTC' AS created_at,
       c.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM communities c
