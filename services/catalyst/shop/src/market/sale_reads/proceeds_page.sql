, own AS MATERIALIZED (SELECT * FROM proceeds WHERE seller=$7 OR seller IS NULL),
page AS (SELECT * FROM own ORDER BY block_time DESC,public_id LIMIT $10 OFFSET $11)
SELECT (SELECT count(*) FROM own),
       EXISTS(SELECT FROM own WHERE seller IS NULL) OR EXISTS(SELECT FROM page WHERE NOT settled),
       coalesce((SELECT jsonb_agg(jsonb_build_object('id',public_id,'timestamp',block_time*1000,
             'type',kind,'contractAddress',contract_address,'itemId',item_id::text,'tokenId',token_id::text,
             'buyer',buyer,'seller',seller,'price',price::text,'fees',fees::text,'royalties',royalties::text,
             'net',net::text,'txHash',tx_hash,'network',upper(network),'chainId',CASE WHEN network='ethereum' THEN 1 ELSE 137 END)
             ORDER BY block_time DESC,public_id) FROM page),'[]'::jsonb)
