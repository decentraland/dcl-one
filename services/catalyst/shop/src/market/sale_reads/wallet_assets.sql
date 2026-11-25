, candidate_collections AS (
    SELECT network,contract_address FROM shop.collections c,requested_wallet w WHERE c.network=ANY($1) AND c.creator=w.address
    UNION SELECT network,contract_address FROM shop.collection_seed c,requested_wallet w WHERE c.network=ANY($1) AND c.creator=w.address
    UNION SELECT c.network,c.contract_address FROM shop.collection_event c JOIN shop.sale_publication pub USING(network),requested_wallet w
        WHERE c.network=ANY($1) AND c.kind='creatorship_transferred' AND c.block_number<=pub.block_number
          AND w.address IN (c.account,c.previous_account)
), wallet_keys AS (
    SELECT a.network,a.block_number,a.log_index,a.ordinal
    FROM shop.sale_asset a, requested_wallet w WHERE a.network=ANY($1) AND a.price>0 AND a.seller=w.address
    UNION
    SELECT a.network,a.block_number,a.log_index,a.ordinal
    FROM shop.sale_payment p JOIN shop.sale_asset a USING(network,block_number,log_index), requested_wallet w
    WHERE p.network=ANY($1) AND p.consideration AND p.beneficiary=w.address AND a.price>0
    UNION
    SELECT a.network,a.block_number,a.log_index,a.ordinal
    FROM shop.items i JOIN shop.sale_asset a USING(network,contract_address,item_id), requested_wallet w
    WHERE i.network=ANY($1) AND i.creator=w.address AND a.price>0
    UNION
    SELECT a.network,a.block_number,a.log_index,a.ordinal FROM candidate_collections c JOIN shop.sale_asset a USING(network,contract_address)
    WHERE a.price>0 AND (a.seller IS NULL OR a.kind='bid')
), scoped_assets AS NOT MATERIALIZED (
    SELECT a.* FROM wallet_keys k JOIN shop.sale_asset a USING(network,block_number,log_index,ordinal)
)
