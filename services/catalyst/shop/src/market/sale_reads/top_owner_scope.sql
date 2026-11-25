-- Ethereum v1 Issue logs identify an item by wearable ID inside its collection.
-- Polygon v2 token IDs carry the item ID directly. Retain burned Ethereum
-- tokens because past purchases still count toward a current holder's spend.
, creator_items AS MATERIALIZED (
    SELECT network, contract_address, item_id, urn
    FROM shop.items
    WHERE lower(creator) = $7 AND network = ANY($1)
        AND chain_id = CASE network WHEN 'ethereum' THEN 1 WHEN 'matic' THEN 137 END
), ethereum_tokens AS MATERIALIZED (
    SELECT i.contract_address, i.item_id, n.token_id, n.owner, n.transferred_at, n.minted_at
    FROM creator_items i
    JOIN shop.collections c USING (network, contract_address)
    JOIN shop.nft n USING (network, contract_address)
    WHERE i.network = 'ethereum' AND c.chain_id = 1 AND i.urn = c.urn || ':' || n.wearable_id
), scoped_assets AS NOT MATERIALIZED (
    -- Keep the creator's item keys on the outside of the sale index probes.
    -- Flattening these joins can scan a holder's unrelated purchase history.
    SELECT a.*
    FROM creator_items i
    CROSS JOIN LATERAL (
        SELECT a.* FROM shop.sale_asset a
        WHERE (a.network, a.contract_address, a.item_id) = (i.network, i.contract_address, i.item_id)
            AND a.price > 0 AND a.currency = $3
        OFFSET 0
    ) a
    WHERE i.network = 'matic'
    UNION ALL
    SELECT a.*
    FROM ethereum_tokens n
    CROSS JOIN LATERAL (
        SELECT a.* FROM shop.sale_asset a
        WHERE a.network = 'ethereum'
            AND (a.contract_address, a.token_id) = (n.contract_address, n.token_id)
            AND a.price > 0 AND a.currency = $2
        OFFSET 0
    ) a
)
