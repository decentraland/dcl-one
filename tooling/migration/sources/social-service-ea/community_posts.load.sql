-- A post this source wrote (recorded in
-- dcl_migration.social_community_post_loaded) that the node has since edited
-- or deleted stays the node's.
WITH owned AS (
    SELECT s.*
    FROM stage s
    JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
    LEFT JOIN dcl_migration.social_community_post_loaded l ON l.id = s.id
    LEFT JOIN social.community_posts t ON t.id = s.id
    WHERE l.id IS NULL OR t.content = l.content
), written AS (
    INSERT INTO social.community_posts AS t (id, community_id, author_address, content, created_at)
    SELECT o.id, o.community_id, o.author_address, o.content, o.created_at AT TIME ZONE 'UTC'
    FROM owned o
    ON CONFLICT (id) DO UPDATE SET
        community_id = EXCLUDED.community_id,
        author_address = EXCLUDED.author_address,
        content = EXCLUDED.content,
        created_at = EXCLUDED.created_at
    RETURNING id, content
)
INSERT INTO dcl_migration.social_community_post_loaded AS l (id, content)
SELECT id, content FROM written
ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content;
