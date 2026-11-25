, own AS MATERIALIZED (SELECT * FROM proceeds s WHERE seller=$7 OR (seller IS NULL AND EXISTS
    (SELECT FROM seller_evidence ev WHERE ev.address=$7 AND
      (ev.network,ev.block_number,ev.log_index,ev.ordinal)=(s.network,s.block_number,s.log_index,s.ordinal)))),
page AS (SELECT * FROM own ORDER BY block_time DESC,public_id LIMIT $10 OFFSET $11)
SELECT (SELECT count(*) FROM (SELECT 1 FROM own LIMIT $12) bounded),
       EXISTS(SELECT FROM own WHERE seller IS NULL) OR EXISTS(SELECT FROM page WHERE NOT settled),
       coalesce((SELECT jsonb_agg(jsonb_build_object('id',public_id,'timestamp',block_time*1000,
             'type',kind,'contractAddress',contract_address,'itemId',item_id::text,'tokenId',token_id::text,
             'buyer',buyer,'seller',seller,'price',price::text,'fees',fees::text,'royalties',royalties::text,
             'net',net::text,'txHash',tx_hash,'network',upper(network),'chainId',CASE WHEN network='ethereum' THEN 1 ELSE 137 END)
             ORDER BY block_time DESC,public_id) FROM page),'[]'::jsonb)
