-- The handler checks these publications in the same read-only snapshot.
WITH published AS (
    SELECT network, chain_id, folded_block
    FROM shop.chain_cursor
    WHERE indexer = 'trade_events' AND chain_id = CASE network
        WHEN 'ethereum' THEN $4::bigint WHEN 'matic' THEN $5::bigint END
), signer_trades AS MATERIALIZED (
    SELECT t.id, t.type, p.network, t.chain_id, lower(t.contract) AS venue,
           t.created_at, t.expires_at, p.folded_block, t.hashed_signature, d.digest,
           coalesce((t.checks->>'contractSignatureIndex')::numeric, 0) AS contract_index,
           coalesce((t.checks->>'signerSignatureIndex')::numeric, 0) AS signer_index,
           coalesce((t.checks->>'uses')::numeric, 1) AS uses
    FROM shop.trades t JOIN published p ON p.network = lower(t.network) AND p.chain_id = t.chain_id
    LEFT JOIN shop.trade_chain_state d ON d.trade_id = t.id AND d.network = p.network
    WHERE t.signer = $1 AND (cardinality($2::text[]) = 0 OR t.type::text = ANY($2))
), assets AS MATERIALIZED (
    SELECT t.*, a.contract_address AS asset_contract, nft.token_id, item.item_id,
           CASE WHEN nft.token_id ~ '^[0-9]{1,78}$' THEN nft.token_id::numeric END AS token_key,
           CASE WHEN item.item_id ~ '^[0-9]{1,78}$' THEN item.item_id::numeric END AS item_key,
           t.network = 'ethereum' AND a.contract_address IN ($6, $7) AS is_land
    FROM signer_trades t JOIN shop.trade_assets a ON a.trade_id = t.id
    LEFT JOIN shop.trade_assets_erc721 nft ON nft.asset_id = a.id
    LEFT JOIN shop.trade_assets_item item ON item.asset_id = a.id
    WHERE a.asset_type IN (3, 4) AND a.direction =
        CASE WHEN t.type = 'bid' THEN 'received'::shop.asset_direction_type ELSE 'sent'::shop.asset_direction_type END
), contract_bumps AS MATERIALIZED (
    SELECT t.*, b.block_number, b.log_index, b.block_time
    FROM signer_trades t
    JOIN LATERAL (
        SELECT e.block_number, e.log_index, e.block_time
        FROM shop.trade_event e
        WHERE e.network = t.network AND e.marketplace_address = t.venue
            AND e.kind = 'contract_index_increased' AND e.new_index > t.contract_index
            AND e.block_number <= t.folded_block
        ORDER BY e.block_number, e.log_index
        LIMIT 1
    ) b ON true
    WHERE t.expires_at > $3 AND t.expires_at > to_timestamp(b.block_time) AT TIME ZONE 'UTC'
      -- Both counters had to match just before the bump.
      AND t.contract_index = coalesce((
          SELECT e.new_index FROM shop.trade_event e
          WHERE e.network = t.network AND e.marketplace_address = t.venue
              AND e.kind = 'contract_index_increased' AND e.new_index IS NOT NULL
              AND (e.block_number, e.log_index) < (b.block_number, b.log_index)
          ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1
      ), 0)
      AND t.signer_index = coalesce((
          SELECT e.new_index FROM shop.trade_event e
          WHERE e.network = t.network AND e.marketplace_address = t.venue
              AND e.kind = 'signer_index_increased' AND e.caller = $1 AND e.new_index IS NOT NULL
              AND (e.block_number, e.log_index) < (b.block_number, b.log_index)
          ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1
      ), 0)
      AND NOT EXISTS (
          SELECT FROM shop.trade_event e
          WHERE e.network = t.network AND e.marketplace_address = t.venue
              AND e.kind = 'signature_cancelled' AND e.caller = $1
              AND e.hashed_signature IN (t.hashed_signature, t.digest)
              AND (e.block_number, e.log_index) < (b.block_number, b.log_index)
      )
      AND (
          SELECT count(*) FROM shop.trade_event e
          WHERE e.network = t.network AND e.marketplace_address = t.venue
              AND e.kind = 'traded' AND e.signer = $1
              AND e.hashed_signature IN (t.hashed_signature, t.digest)
              AND (e.block_number, e.log_index) < (b.block_number, b.log_index)
      ) < t.uses
), unrecreated AS MATERIALIZED (
    SELECT a.*, bump.block_time AS cancelled_at
    FROM contract_bumps bump JOIN assets a ON a.id = bump.id
    WHERE NOT EXISTS (
        -- A new signature on a venue the bump did not reach completes the
        -- re-creation even if that newer trade subsequently expired or sold.
        SELECT FROM assets newer
        WHERE newer.network = a.network AND newer.type = a.type
          AND newer.asset_contract = a.asset_contract
          AND newer.token_key IS NOT DISTINCT FROM a.token_key
          AND newer.item_key IS NOT DISTINCT FROM a.item_key
          AND newer.created_at > a.created_at
          AND NOT EXISTS (
                SELECT FROM shop.trade_event e
                WHERE e.network = newer.network AND e.marketplace_address = newer.venue
                  AND e.kind = 'contract_index_increased' AND e.new_index > newer.contract_index
                  AND e.block_number <= newer.folded_block
          )
    )
), eligible AS (
    SELECT a.*, coalesce(land_data.name, n.ens_subdomain, item.name, wearable.name) AS name,
           coalesce(item.image, wearable.image) AS image,
           CASE WHEN a.is_land THEN land.owner ELSE n.owner END AS owner,
           item.available
    FROM unrecreated a
    LEFT JOIN land_authz.token_right land ON $8::bool AND a.is_land
        AND (land.token_address, land.token_id) = (a.asset_contract, a.token_key)
    LEFT JOIN shop.land_data land_data ON a.is_land AND
        (land_data.token_address, land_data.token_id) = (a.asset_contract, a.token_key)
    LEFT JOIN shop.nft n ON NOT a.is_land AND
        (n.network, n.contract_address, n.token_id) = (a.network, a.asset_contract, a.token_key)
    LEFT JOIN shop.items item ON
        (item.network, item.contract_address, item.item_id) = (a.network, a.asset_contract, a.item_key)
        AND item.chain_id = a.chain_id
    LEFT JOIN shop.collections c ON n.network = 'ethereum' AND
        (c.network, c.contract_address) = (n.network, n.contract_address) AND c.chain_id = a.chain_id
    LEFT JOIN shop.items wearable ON wearable.network = n.network AND wearable.chain_id = a.chain_id
        AND CASE WHEN n.network = 'ethereum' THEN wearable.urn = c.urn || ':' || n.wearable_id
                 ELSE (wearable.contract_address, wearable.item_id) = (n.contract_address, n.item_id) END
), pending AS MATERIALIZED (
    SELECT DISTINCT ON (network, type, asset_contract, token_key, item_key) *
    FROM eligible
    WHERE CASE type
        WHEN 'public_nft_order' THEN owner = $1
        WHEN 'public_item_order' THEN available > 0
        ELSE (token_id IS NULL OR owner <> $1) AND (item_id IS NULL OR available > 0)
    END
    ORDER BY network, type, asset_contract, token_key, item_key, created_at DESC, id
), page AS (
    SELECT * FROM pending ORDER BY created_at DESC, id, asset_contract, token_key, item_key
    LIMIT $9 OFFSET $10
)
SELECT coalesce(jsonb_agg(jsonb_build_object(
           'id', p.id, 'type', p.type, 'network', upper(p.network), 'chainId', p.chain_id,
           'contract', p.venue, 'reason', $12::text,
           'createdAt', (extract(epoch FROM p.created_at)*1000)::bigint,
           'expiresAt', (extract(epoch FROM p.expires_at)*1000)::bigint,
           'cancelledAt', p.cancelled_at*1000,
           'asset', jsonb_build_object('contractAddress', p.asset_contract, 'tokenId', p.token_id,
               'itemId', p.item_id, 'name', p.name, 'image', p.image),
           'price', CASE WHEN price.amount IS NOT NULL THEN
               jsonb_build_object('assetType', price.asset_type, 'amount', price.amount::text) END
       ) ORDER BY p.created_at DESC, p.id, p.asset_contract, p.token_key, p.item_key), '[]'::jsonb),
       (SELECT count(*) FROM pending),
       EXISTS (SELECT FROM unrecreated WHERE is_land),
       EXISTS (SELECT FROM pending WHERE venue = ANY($11::text[]) AND digest IS NULL)
FROM page p
LEFT JOIN LATERAL (
    SELECT a.asset_type, money.amount
    FROM shop.trade_assets a JOIN shop.trade_assets_erc20 money ON money.asset_id = a.id
    WHERE a.trade_id = p.id AND a.direction =
        CASE WHEN p.type = 'bid' THEN 'sent'::shop.asset_direction_type ELSE 'received'::shop.asset_direction_type END
    ORDER BY a.created_at, a.id LIMIT 1
) price ON true
