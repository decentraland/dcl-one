ALTER TABLE builder.item_contents ADD COLUMN IF NOT EXISTS named_by text NOT NULL DEFAULT '';
UPDATE builder.item_contents f
SET named_by = COALESCE((
        SELECT w.wallet
        FROM (
            SELECT 1 AS rank, i.eth_address AS wallet
            UNION ALL SELECT 2, c.eth_address WHERE c.third_party_id IS NOT NULL
            UNION ALL SELECT 3, COALESCE(lower(chain.creator), c.eth_address) WHERE c.id IS NOT NULL AND c.third_party_id IS NULL
            UNION ALL SELECT 4, lower(m) FROM unnest(chain.managers) m WHERE c.third_party_id IS NULL
        ) w
        WHERE EXISTS (SELECT 1 FROM builder.uploaded_contents u WHERE u.hash = f.hash AND u.uploaded_by = w.wallet)
        ORDER BY w.rank, w.wallet
        LIMIT 1
    ), i.eth_address)
FROM builder.items i
LEFT JOIN builder.collections c ON c.id = i.collection_id
LEFT JOIN shop.collections chain ON chain.network = 'matic' AND chain.contract_address = lower(c.contract_address)
WHERE i.id = f.item_id AND f.named_by = '';
ALTER TABLE builder.item_contents ADD CONSTRAINT item_contents_named_by_check CHECK (named_by <> '' AND named_by = lower(named_by));
