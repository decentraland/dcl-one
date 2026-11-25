, mine AS MATERIALIZED (
    SELECT *, ($8::bigint IS NULL OR block_time*1000 >= $8)
          AND ($9::bigint IS NULL OR block_time*1000 <= $9) AS in_window
    FROM sales WHERE seller=$7
), totals AS (
    SELECT count(*) AS total,count(*) FILTER (WHERE kind='mint') AS mints,
        count(*) FILTER (WHERE kind IN ('order','bid')) AS resales,coalesce(sum(price),0)::text AS earned
    FROM mine WHERE in_window
), collections AS (
    SELECT contract_address,count(*) AS sold,sum(price)::text AS earned FROM mine WHERE in_window GROUP BY contract_address
), items AS (
    SELECT contract_address,item_id::text AS item_id,count(*) AS lifetime,
        count(*) FILTER (WHERE in_window) AS sold,coalesce(sum(price) FILTER (WHERE in_window),0)::text AS earned
    FROM mine WHERE kind='mint' AND item_id IS NOT NULL GROUP BY contract_address,item_id
), royalties AS (
    SELECT count(*) AS resales,coalesce(sum(price),0)::text AS volume FROM sales
    WHERE kind IN ('order','bid') AND creator=$7 AND ($8 IS NULL OR block_time*1000 >= $8) AND ($9 IS NULL OR block_time*1000 <= $9)
)
SELECT total,mints,totals.resales,earned,royalties.resales AS royalty_resales,volume,
       coalesce((SELECT jsonb_agg(jsonb_build_object('contractAddress',contract_address,'sold',sold,'earnedWei',earned)
          ORDER BY contract_address) FROM collections),'[]'::jsonb) AS by_collection,
       coalesce((SELECT jsonb_agg(jsonb_build_object('contract_address',contract_address,'item_id',item_id,
          'lifetime',lifetime,'sold',sold,'earned',earned) ORDER BY contract_address,item_id) FROM items),'[]'::jsonb) AS items,
       EXISTS (SELECT FROM sales WHERE seller IS NULL) AS incomplete
FROM totals CROSS JOIN royalties
