-- Each public sale has a positive MANA price allocated to a catalogue NFT.
-- A mediated bid exposes the payment beneficiary under the marketplace contract.
, sales AS (
    SELECT a.network,a.block_number,a.log_index,a.ordinal,a.public_id,a.kind,a.contract_address,
           a.token_id,a.item_id,a.buyer,CASE WHEN e.kind='trade' AND a.kind='bid'
               THEN bid.beneficiary ELSE a.seller END AS seller,a.price,a.currency,
           e.tx_hash,e.block_time,e.kind AS venue_kind,
           i.creator,i.rarity,
           CASE WHEN a.network='ethereum' AND a.contract_address=$4 THEN 'parcel'
                WHEN a.network='ethereum' AND a.contract_address=$5 THEN 'estate'
                WHEN a.network='ethereum' AND a.contract_address=$6 THEN 'ens'
                WHEN i.item_type='emote_v1' THEN 'emote'
                WHEN i.item_type IS NOT NULL THEN 'wearable' END AS category
    FROM scoped_assets a JOIN shop.sale_execution e USING(network,block_number,log_index)
    JOIN shop.sale_publication pub ON pub.network=a.network AND a.block_number<=pub.block_number
    LEFT JOIN shop.items i ON (i.network,i.contract_address,i.item_id)=(a.network,a.contract_address,a.item_id)
      AND i.chain_id=CASE a.network WHEN 'ethereum' THEN 1 ELSE 137 END
    LEFT JOIN LATERAL (
        SELECT p.beneficiary FROM shop.sale_payment p
        WHERE e.kind='trade' AND a.kind='bid' AND (p.network,p.block_number,p.log_index)=(a.network,a.block_number,a.log_index)
          AND p.consideration AND p.currency=a.currency
        ORDER BY p.ordinal LIMIT 1
    ) bid ON true
    WHERE (e.kind<>'trade' OR NOT EXISTS (
        SELECT FROM shop.sale_asset other WHERE (other.network,other.block_number,other.log_index)=
            (a.network,a.block_number,a.log_index) AND other.ordinal<>a.ordinal))
      AND a.network=ANY($1) AND a.price>0 AND a.currency=CASE WHEN a.network='ethereum' THEN $2 ELSE $3 END
)
