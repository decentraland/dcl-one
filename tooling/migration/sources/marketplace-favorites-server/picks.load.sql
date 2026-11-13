-- Only into lists the target holds (a skipped list's picks are skipped).
DO $$
DECLARE skipped bigint;
BEGIN
    SELECT count(*) INTO skipped FROM stage s WHERE NOT EXISTS (SELECT 1 FROM shop.lists l WHERE l.id = s.list_id);
    IF skipped > 0 THEN
        RAISE NOTICE 'marketplace-favorites-server/picks: % pick(s) skipped: their list was not imported', skipped;
    END IF;
END $$;

INSERT INTO shop.picks (item_id, user_address, list_id, created_at)
SELECT s.item_id, s.user_address, s.list_id, s.created_at AT TIME ZONE 'UTC'
FROM stage s
JOIN shop.lists l ON l.id = s.list_id
ON CONFLICT (item_id, user_address, list_id) DO UPDATE
   SET created_at = LEAST(shop.picks.created_at, EXCLUDED.created_at);
