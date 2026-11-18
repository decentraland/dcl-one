SELECT l.post_id,
       lower(l.user_address) AS user_address,
       l.liked_at AT TIME ZONE 'UTC' AS liked_at
FROM community_post_likes l
WHERE lower(l.user_address) ~ '^0x[0-9a-f]{40}$'
  AND NOT EXISTS (
      SELECT 1 FROM community_post_likes d
      WHERE d.post_id = l.post_id
        AND lower(d.user_address) = lower(l.user_address)
        AND (d.user_address <> lower(d.user_address), d.user_address)
          < (l.user_address <> lower(l.user_address), l.user_address))
