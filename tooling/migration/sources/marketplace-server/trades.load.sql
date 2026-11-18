-- Heads keep upstream ids and become local authority. A trade whose
-- signature is already held under another id is skipped (the node's copy
-- stays); an id held with another signature is left alone.
INSERT INTO shop.trades
    (id, network, chain_id, signature, hashed_signature, checks, signer, type,
     expires_at, effective_since, contract, created_at, mirrored_from, mirrored_at)
SELECT s.id, s.network, s.chain_id, s.signature, s.hashed_signature, s.checks, s.signer,
       s.type::shop.trade_type,
       s.expires_at AT TIME ZONE 'UTC', s.effective_since AT TIME ZONE 'UTC',
       s.contract, s.created_at AT TIME ZONE 'UTC', NULL, NULL
FROM stage s
WHERE NOT EXISTS (
    SELECT 1 FROM shop.trades t
    WHERE (t.hashed_signature = s.hashed_signature OR t.signature = s.signature) AND t.id <> s.id)
ON CONFLICT (id) DO UPDATE
   SET network = EXCLUDED.network, chain_id = EXCLUDED.chain_id, checks = EXCLUDED.checks,
       signer = EXCLUDED.signer, type = EXCLUDED.type, expires_at = EXCLUDED.expires_at,
       effective_since = EXCLUDED.effective_since, contract = EXCLUDED.contract,
       created_at = EXCLUDED.created_at, mirrored_from = NULL, mirrored_at = NULL
 WHERE shop.trades.hashed_signature = EXCLUDED.hashed_signature;

-- The trades that landed: same id and signature as upstream.
CREATE TEMP TABLE stage_asset ON COMMIT DROP AS
SELECT s.id AS trade_id,
       (x ->> 'id')::uuid AS id,
       x ->> 'direction' AS direction,
       (x ->> 'asset_type')::smallint AS asset_type,
       x ->> 'contract_address' AS contract_address,
       x ->> 'beneficiary' AS beneficiary,
       x ->> 'extra' AS extra,
       x ->> 'amount' AS amount,
       x ->> 'token_id' AS token_id,
       x ->> 'item_id' AS item_id,
       (x ->> 'created_at')::timestamptz AT TIME ZONE 'UTC' AS created_at
FROM stage s
JOIN shop.trades t ON t.id = s.id AND t.hashed_signature = s.hashed_signature
CROSS JOIN LATERAL jsonb_array_elements(s.assets) x;

-- A mirrored copy carries the same assets under random ids: replace them
-- with upstream's (the sub-rows cascade).
DELETE FROM shop.trade_assets a
USING (SELECT DISTINCT trade_id FROM stage_asset) st
WHERE a.trade_id = st.trade_id
  AND NOT EXISTS (SELECT 1 FROM stage_asset x WHERE x.id = a.id);

INSERT INTO shop.trade_assets (id, trade_id, direction, asset_type, contract_address, beneficiary, extra, created_at)
SELECT id, trade_id, direction::shop.asset_direction_type, asset_type, contract_address, beneficiary, extra, created_at
FROM stage_asset
ON CONFLICT (id) DO UPDATE
   SET direction = EXCLUDED.direction, asset_type = EXCLUDED.asset_type,
       contract_address = EXCLUDED.contract_address, beneficiary = EXCLUDED.beneficiary,
       extra = EXCLUDED.extra, created_at = EXCLUDED.created_at
 WHERE shop.trade_assets.trade_id = EXCLUDED.trade_id;

INSERT INTO shop.trade_assets_erc20 (asset_id, amount)
SELECT x.id, x.amount::numeric FROM stage_asset x
JOIN shop.trade_assets a ON a.id = x.id AND a.trade_id = x.trade_id
WHERE x.asset_type IN (1, 2) AND x.amount IS NOT NULL
ON CONFLICT (asset_id) DO UPDATE SET amount = EXCLUDED.amount;

INSERT INTO shop.trade_assets_erc721 (asset_id, token_id)
SELECT x.id, x.token_id FROM stage_asset x
JOIN shop.trade_assets a ON a.id = x.id AND a.trade_id = x.trade_id
WHERE x.asset_type = 3 AND x.token_id IS NOT NULL
ON CONFLICT (asset_id) DO UPDATE SET token_id = EXCLUDED.token_id;

INSERT INTO shop.trade_assets_item (asset_id, item_id)
SELECT x.id, x.item_id FROM stage_asset x
JOIN shop.trade_assets a ON a.id = x.id AND a.trade_id = x.trade_id
WHERE x.asset_type = 4 AND x.item_id IS NOT NULL
ON CONFLICT (asset_id) DO UPDATE SET item_id = EXCLUDED.item_id;

DROP TABLE stage_asset;
