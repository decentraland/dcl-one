INSERT INTO social.community_posts AS t (id, community_id, author_address, content, created_at)
SELECT s.id, s.community_id, s.author_address, s.content, s.created_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
ON CONFLICT (id) DO UPDATE SET
    community_id = EXCLUDED.community_id,
    author_address = EXCLUDED.author_address,
    content = EXCLUDED.content,
    created_at = EXCLUDED.created_at;
