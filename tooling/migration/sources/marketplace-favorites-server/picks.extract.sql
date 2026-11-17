-- Picks keyed lowercase, as the node writes them; a re-cased duplicate
-- keeps its earliest pick time.
SELECT p.list_id,
       lower(p.item_id) AS item_id,
       lower(p.user_address) AS user_address,
       min(p.created_at) AT TIME ZONE 'UTC' AS created_at
FROM favorites.picks p
GROUP BY p.list_id, lower(p.item_id), lower(p.user_address)
