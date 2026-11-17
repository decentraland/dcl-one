-- The newer blocked_at wins. A pair this source wrote (recorded in
-- dcl_migration.social_block_loaded) that the node has since removed is not
-- re-added unless upstream's block is newer than the recorded one.
WITH wanted AS (
    SELECT s.blocker_address, s.blocked_address, s.blocked_at AT TIME ZONE 'UTC' AS blocked_at
    FROM stage s
    WHERE NOT EXISTS (
        SELECT 1 FROM dcl_migration.social_block_loaded l
        WHERE l.blocker_address = s.blocker_address AND l.blocked_address = s.blocked_address
          AND l.blocked_at >= s.blocked_at AT TIME ZONE 'UTC'
          AND NOT EXISTS (SELECT 1 FROM social.blocks t
                          WHERE t.blocker_address = s.blocker_address AND t.blocked_address = s.blocked_address))
), written AS (
    INSERT INTO social.blocks AS t (blocker_address, blocked_address, blocked_at)
    SELECT blocker_address, blocked_address, blocked_at FROM wanted
    ON CONFLICT (blocker_address, blocked_address) DO UPDATE SET blocked_at = EXCLUDED.blocked_at
    WHERE t.blocked_at <= EXCLUDED.blocked_at
    RETURNING blocker_address, blocked_address, blocked_at
)
INSERT INTO dcl_migration.social_block_loaded AS l (blocker_address, blocked_address, blocked_at)
SELECT blocker_address, blocked_address, blocked_at FROM written
ON CONFLICT (blocker_address, blocked_address) DO UPDATE SET blocked_at = EXCLUDED.blocked_at;
