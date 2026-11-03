-- Only into lists the target holds (a skipped list's picks are skipped).
INSERT INTO shop.picks (item_id, user_address, list_id, created_at)
SELECT s.item_id, s.user_address, s.list_id, s.created_at AT TIME ZONE 'UTC'
FROM stage s
JOIN shop.lists l ON l.id = s.list_id
ON CONFLICT (item_id, user_address, list_id) DO UPDATE
   SET created_at = LEAST(shop.picks.created_at, EXCLUDED.created_at);
