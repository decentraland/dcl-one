, ranked AS (
    SELECT d.contract_address||'-'||d.item_id AS id,sum(d.sales)::bigint AS sales,sum(d.volume) AS volume
    FROM shop.sale_item_day d JOIN shop.items i USING(network,contract_address,item_id)
    WHERE d.network=ANY($1) AND d.day >= $7 AND d.currency=CASE WHEN d.network='ethereum' THEN $2 ELSE $3 END
      AND (i.item_type='emote_v1')=$8
      AND ($9::text IS NULL OR i.rarity=$9)
    GROUP BY d.contract_address,d.item_id HAVING sum(d.sales)>0
)
SELECT id,sales,volume::text FROM ranked
ORDER BY CASE WHEN $10 THEN sales ELSE volume END DESC,
         CASE WHEN $10 THEN volume ELSE sales END DESC,id
LIMIT $11
