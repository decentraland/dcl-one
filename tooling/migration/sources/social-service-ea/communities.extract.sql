-- ranking_score and last_score_calculated_at are upstream's ranking job's
-- output; the node has no writer for ranking_score, so it is not copied.
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
