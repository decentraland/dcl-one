-- Settings the node changed or reset since this source last wrote them (they
-- no longer match dcl_migration.social_user_settings_loaded) stay the node's.
WITH owned AS (
    SELECT s.*
    FROM stage s
    LEFT JOIN dcl_migration.social_user_settings_loaded l ON l.address = s.address
    LEFT JOIN social.user_settings t ON t.address = s.address
    WHERE l.address IS NULL
       OR (t.private_messages_privacy, t.blocked_users_messages_visibility, t.show_situation_reactions)
          = (l.private_messages_privacy, l.blocked_users_messages_visibility, l.show_situation_reactions)
), written AS (
    INSERT INTO social.user_settings AS t
        (address, private_messages_privacy, blocked_users_messages_visibility, show_situation_reactions, updated_at)
    SELECT o.address, o.private_messages_privacy, o.blocked_users_messages_visibility, o.show_situation_reactions,
           timezone('UTC', now())
    FROM owned o
    ON CONFLICT (address) DO UPDATE SET
        private_messages_privacy = EXCLUDED.private_messages_privacy,
        blocked_users_messages_visibility = EXCLUDED.blocked_users_messages_visibility,
        show_situation_reactions = EXCLUDED.show_situation_reactions,
        updated_at = CASE
            WHEN (t.private_messages_privacy, t.blocked_users_messages_visibility, t.show_situation_reactions)
                 IS DISTINCT FROM (EXCLUDED.private_messages_privacy, EXCLUDED.blocked_users_messages_visibility,
                                   EXCLUDED.show_situation_reactions)
            THEN EXCLUDED.updated_at ELSE t.updated_at END
    RETURNING address, private_messages_privacy, blocked_users_messages_visibility, show_situation_reactions
)
INSERT INTO dcl_migration.social_user_settings_loaded AS l
    (address, private_messages_privacy, blocked_users_messages_visibility, show_situation_reactions)
SELECT address, private_messages_privacy, blocked_users_messages_visibility, show_situation_reactions FROM written
ON CONFLICT (address) DO UPDATE SET
    private_messages_privacy = EXCLUDED.private_messages_privacy,
    blocked_users_messages_visibility = EXCLUDED.blocked_users_messages_visibility,
    show_situation_reactions = EXCLUDED.show_situation_reactions;
