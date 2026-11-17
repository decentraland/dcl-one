-- The newer muted_at wins. A pair this source wrote (recorded in
-- dcl_migration.social_user_mute_loaded) that the node has since removed is not
-- re-added unless upstream's mute is newer than the recorded one.
WITH wanted AS (
    SELECT s.muter_address, s.muted_address, s.muted_at AT TIME ZONE 'UTC' AS muted_at
    FROM stage s
    WHERE NOT EXISTS (
        SELECT 1 FROM dcl_migration.social_user_mute_loaded l
        WHERE l.muter_address = s.muter_address AND l.muted_address = s.muted_address
          AND l.muted_at >= s.muted_at AT TIME ZONE 'UTC'
          AND NOT EXISTS (SELECT 1 FROM social.user_mutes t
                          WHERE t.muter_address = s.muter_address AND t.muted_address = s.muted_address))
), written AS (
    INSERT INTO social.user_mutes AS t (muter_address, muted_address, muted_at)
    SELECT muter_address, muted_address, muted_at FROM wanted
    ON CONFLICT (muter_address, muted_address) DO UPDATE SET muted_at = EXCLUDED.muted_at
    WHERE t.muted_at <= EXCLUDED.muted_at
    RETURNING muter_address, muted_address, muted_at
)
INSERT INTO dcl_migration.social_user_mute_loaded AS l (muter_address, muted_address, muted_at)
SELECT muter_address, muted_address, muted_at FROM written
ON CONFLICT (muter_address, muted_address) DO UPDATE SET muted_at = EXCLUDED.muted_at;
