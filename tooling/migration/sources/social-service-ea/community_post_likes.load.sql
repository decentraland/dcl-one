INSERT INTO social.community_post_likes AS t (post_id, user_address, liked_at)
SELECT s.post_id, s.user_address, s.liked_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.community_posts p ON p.id = s.post_id
ON CONFLICT (post_id, user_address) DO UPDATE SET liked_at = EXCLUDED.liked_at;
