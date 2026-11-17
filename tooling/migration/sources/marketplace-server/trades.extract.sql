-- One row per trade with every asset inlined, so a trade's head and assets
-- land in one batch. Hex and addresses are lowercased as the node's insert
-- paths store them (keccak of the decoded bytes is case-blind); `checks` is
-- copied as stored. `extra` follows the mirror's convention: '' / '0x' -> ''.
-- A trade without assets is refused by the mirror and skipped here too.
SELECT t.id,
       t.network,
       t.chain_id,
       lower(t.signature) AS signature,
       lower(t.hashed_signature) AS hashed_signature,
       t.checks,
       lower(t.signer) AS signer,
       t.type::text AS type,
       t.expires_at,
       t.effective_since,
       t.contract,
       t.created_at,
       a.assets
FROM marketplace.trades t
JOIN LATERAL (
    SELECT jsonb_agg(jsonb_build_object(
               'id', ta.id,
               'direction', ta.direction::text,
               'asset_type', ta.asset_type,
               'contract_address', lower(ta.contract_address),
               'beneficiary', NULLIF(lower(ta.beneficiary), ''),
               'extra', CASE WHEN ta.extra IS NULL OR ta.extra IN ('', '0x') THEN '' ELSE ta.extra END,
               'amount', e20.amount::text,
               'token_id', e721.token_id,
               'item_id', it.item_id,
               'created_at', ta.created_at)
           ORDER BY ta.created_at, ta.id) AS assets
    FROM marketplace.trade_assets ta
    LEFT JOIN marketplace.trade_assets_erc20 e20 ON e20.asset_id = ta.id
    LEFT JOIN marketplace.trade_assets_erc721 e721 ON e721.asset_id = ta.id
    LEFT JOIN marketplace.trade_assets_item it ON it.asset_id = ta.id
    WHERE ta.trade_id = t.id
) a ON a.assets IS NOT NULL
