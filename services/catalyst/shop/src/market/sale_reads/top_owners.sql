-- Ethereum v1 Issue logs identify an item by wearable ID inside its collection.
-- Polygon v2 token IDs carry the item ID directly. Retain burned tokens in the
-- Ethereum mapping because purchases of them still count toward a holder's spend.
, creator_items AS MATERIALIZED (
    SELECT network, contract_address, item_id, urn
    FROM shop.items
    WHERE lower(creator) = $7 AND network = ANY($1)
), ethereum_tokens AS MATERIALIZED (
    SELECT i.contract_address, i.item_id, n.token_id, n.owner, n.transferred_at, n.minted_at
    FROM creator_items i
    JOIN shop.collections c USING (network, contract_address)
    JOIN shop.nft n USING (network, contract_address)
    WHERE i.network = 'ethereum' AND i.urn = c.urn || ':' || n.wearable_id
), copies AS (
    SELECT n.network, n.contract_address, n.item_id, n.owner, n.transferred_at, n.minted_at
    FROM creator_items i JOIN shop.nft n USING (network, contract_address, item_id)
    WHERE n.network = 'matic'
    UNION ALL
    SELECT 'ethereum', contract_address, item_id, owner, transferred_at, minted_at
    FROM ethereum_tokens
), holders AS MATERIALIZED (
    SELECT owner AS address, count(*) AS nfts,
           count(DISTINCT (network, contract_address, item_id)) AS items,
           count(DISTINCT contract_address) AS collections,
           max(coalesce(nullif(transferred_at, 0), minted_at)) * 1000 AS acquired_at
    FROM copies
    WHERE owner <> $7 AND owner <> $12
    GROUP BY owner
), spending AS (
    SELECT s.buyer, sum(s.price) AS spent
    FROM sales s JOIN holders h ON h.address = s.buyer
    LEFT JOIN creator_items i ON (i.network, i.contract_address, i.item_id) =
        (s.network, s.contract_address, s.item_id) AND s.network = 'matic'
    LEFT JOIN ethereum_tokens e ON s.network = 'ethereum'
        AND (e.contract_address, e.token_id) = (s.contract_address, s.token_id)
    WHERE i.item_id IS NOT NULL OR e.token_id IS NOT NULL
    GROUP BY s.buyer
), ranked AS (
    SELECT h.*, coalesce(s.spent, 0) AS spent,
           CASE $8::text
               WHEN 'items' THEN h.items
               WHEN 'collections' THEN h.collections
               WHEN 'recent' THEN h.acquired_at
               WHEN 'spent' THEN coalesce(s.spent, 0)
               ELSE h.nfts
           END * CASE WHEN $9::bool THEN 1 ELSE -1 END AS sort_value
    FROM holders h LEFT JOIN spending s ON s.buyer = h.address
), page AS (
    SELECT * FROM ranked
    ORDER BY sort_value, nfts DESC, acquired_at DESC, address COLLATE "C"
    LIMIT $10 OFFSET $11
)
SELECT coalesce(jsonb_agg(jsonb_build_object(
    'address', address, 'nfts', nfts, 'items', items, 'collections', collections,
    'lastAcquiredAt', acquired_at, 'spentWei', spent::text
) ORDER BY sort_value, nfts DESC, acquired_at DESC, address COLLATE "C"), '[]'::jsonb),
    (SELECT count(*) FROM holders)
FROM page
