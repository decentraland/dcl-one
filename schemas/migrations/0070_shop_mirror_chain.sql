-- Preserve the production mirrors' completed copy and page cursor at upgrade.
-- A testnet NFT reader requires a pass proven on its own chain.
ALTER TABLE shop.mirror_state ADD COLUMN IF NOT EXISTS chain_id bigint;
UPDATE shop.mirror_state SET chain_id = CASE network WHEN 'ethereum' THEN 1 ELSE 137 END
WHERE chain_id IS NULL AND (network, source_url) IN (
    ('ethereum', 'https://subgraph.decentraland.org/collections-ethereum-mainnet'),
    ('matic', 'https://subgraph.decentraland.org/collections-matic-mainnet')
);
