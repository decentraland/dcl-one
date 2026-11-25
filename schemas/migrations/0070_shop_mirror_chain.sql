-- Preserve the production mirrors' completed copy and page cursor at upgrade.
-- A testnet NFT reader requires a pass proven on its own chain.
ALTER TABLE shop.mirror_state ADD COLUMN IF NOT EXISTS chain_id bigint;
ALTER TABLE shop.mirror_state ADD COLUMN IF NOT EXISTS pass_block_hash text;
UPDATE shop.mirror_state SET chain_id = CASE network WHEN 'ethereum' THEN 1 ELSE 137 END
WHERE chain_id IS NULL AND (network, source_url) IN (
    ('ethereum', 'https://subgraph.decentraland.org/collections-ethereum-mainnet'),
    ('matic', 'https://subgraph.decentraland.org/collections-matic-mainnet')
);
-- A full verified pass may omit rows retained from a previous graph-only source.
ALTER TABLE shop.collections ADD COLUMN IF NOT EXISTS testnet_verified boolean NOT NULL DEFAULT false;
