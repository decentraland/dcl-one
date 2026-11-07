INSERT INTO social.user_settings AS t
    (address, private_messages_privacy, blocked_users_messages_visibility, show_situation_reactions, updated_at)
SELECT s.address, s.private_messages_privacy, s.blocked_users_messages_visibility, s.show_situation_reactions,
       timezone('UTC', now())
FROM stage s
ON CONFLICT (address) DO UPDATE SET
    private_messages_privacy = EXCLUDED.private_messages_privacy,
    blocked_users_messages_visibility = EXCLUDED.blocked_users_messages_visibility,
    show_situation_reactions = EXCLUDED.show_situation_reactions,
    updated_at = CASE
        WHEN (t.private_messages_privacy, t.blocked_users_messages_visibility, t.show_situation_reactions)
             IS DISTINCT FROM (EXCLUDED.private_messages_privacy, EXCLUDED.blocked_users_messages_visibility,
                               EXCLUDED.show_situation_reactions)
        THEN EXCLUDED.updated_at ELSE t.updated_at END;
