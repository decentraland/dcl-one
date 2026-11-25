, copies AS (
    SELECT n.network, n.contract_address, n.item_id, n.owner, n.transferred_at, n.minted_at
    FROM creator_items i JOIN shop.nft n USING (network, contract_address, item_id)
    WHERE n.network = 'matic'
    UNION ALL
    SELECT 'ethereum', contract_address, item_id, owner, transferred_at, minted_at
    FROM ethereum_tokens
), holders AS MATERIALIZED (
    SELECT owner AS address, count(*) AS nfts,
           count(DISTINCT (network, contract_address, item_id)) AS items,
           count(DISTINCT contract_address) AS collections, -- The wire count groups by contract address.
           max(coalesce(nullif(transferred_at, 0), minted_at)) * 1000 AS acquired_at
    FROM copies
    WHERE owner <> $7 AND owner <> $12
    GROUP BY owner
), spending AS (
    SELECT s.buyer, sum(s.price) AS spent
    FROM sales s
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
