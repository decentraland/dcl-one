, candidates AS NOT MATERIALIZED (
    SELECT * FROM sales
    WHERE (NOT $23::boolean OR seller IS NOT NULL) AND ($7::text IS NULL OR kind=$7) AND ($8::text IS NULL OR buyer=$8)
      AND ($9::text IS NULL OR seller=$9 OR (seller IS NULL AND EXISTS
          (SELECT FROM seller_evidence ev WHERE ev.address=$9 AND
           (ev.network,ev.block_number,ev.log_index,ev.ordinal)=(sales.network,sales.block_number,sales.log_index,sales.ordinal))))
      AND ($10::text IS NULL OR contract_address=$10)
      AND ($11::text IS NULL OR item_id=$11::text::numeric) AND ($12::text IS NULL OR token_id=$12::text::numeric)
      AND ($13::bigint IS NULL OR block_time>=$13) AND ($14::bigint IS NULL OR block_time<=$14)
      AND ($15::text IS NULL OR price>=$15::text::numeric) AND ($16::text IS NULL OR price<=$16::text::numeric)
      AND (cardinality($17::text[])=0 OR category=ANY($17) OR (category IS NULL AND $17 && ARRAY['wearable','emote']))
), page AS (
    SELECT * FROM candidates ORDER BY CASE WHEN $18 THEN price END DESC,CASE WHEN NOT $18 THEN block_time END DESC,public_id
    LIMIT $19 OFFSET $20
)
SELECT CASE WHEN $21 THEN coalesce((SELECT sum(sales)::bigint FROM shop.sale_analytics_day d
                WHERE d.network=ANY($1) AND d.currency=CASE WHEN d.network='ethereum' THEN $2 ELSE $3 END),0)
            ELSE (SELECT count(*) FROM (SELECT 1 FROM candidates LIMIT $22) counted) END AS total,
   EXISTS(SELECT FROM candidates WHERE ($9 IS NOT NULL AND seller IS NULL)
         OR ($17 && ARRAY['wearable','emote'] AND category IS NULL))
      OR EXISTS(SELECT FROM page WHERE seller IS NULL) AS incomplete,
   coalesce((SELECT jsonb_agg(jsonb_build_object('id',public_id,'itemId',item_id::text,
       'contractAddress',contract_address,'buyer',buyer,'seller',seller,'price',price::text,
       'network',upper(network),'chainId',CASE WHEN network='ethereum' THEN 1 ELSE 137 END,
       'timestamp',block_time*1000,'tokenId',token_id::text,'txHash',tx_hash,'type',kind)
       ORDER BY CASE WHEN $18 THEN price END DESC,CASE WHEN NOT $18 THEN block_time END DESC,public_id) FROM page),'[]'::jsonb)
