SELECT p.id,
       p.community_id,
       lower(p.author_address) AS author_address,
       p.content,
       p.created_at AT TIME ZONE 'UTC' AS created_at
FROM community_posts p
