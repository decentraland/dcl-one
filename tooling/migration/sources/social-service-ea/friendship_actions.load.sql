-- Only actions of friendships the previous step imported under this id.
INSERT INTO social.friendship_actions AS t (id, friendship_id, action, acting_user, metadata, "timestamp")
SELECT s.id, s.friendship_id, s.action, s.acting_user, s.metadata, s.acted_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.friendships f ON f.id = s.friendship_id
ON CONFLICT (id) DO UPDATE SET
    friendship_id = EXCLUDED.friendship_id,
    action = EXCLUDED.action,
    acting_user = EXCLUDED.acting_user,
    metadata = EXCLUDED.metadata,
    "timestamp" = EXCLUDED."timestamp";
