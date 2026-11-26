, mine AS MATERIALIZED (
    SELECT *, ($8::bigint IS NULL OR block_time*1000 >= $8)
          AND ($9::bigint IS NULL OR block_time*1000 <= $9) AS in_window
    FROM sales WHERE seller=$7
), window_sales AS MATERIALIZED (
    SELECT mine.*, rate.usd FROM mine
    LEFT JOIN shop.mana_usd_daily rate ON rate.day=(to_timestamp(mine.block_time) AT TIME ZONE 'UTC')::date
    WHERE in_window
), totals AS (
    SELECT count(*) AS total,count(*) FILTER (WHERE kind='mint') AS mints,
        count(*) FILTER (WHERE kind IN ('order','bid')) AS resales,coalesce(sum(price),0)::text AS earned,
        round(coalesce(sum(price*usd),0)*1e-18::numeric,6)::text AS earned_usd,
        count(*) FILTER (WHERE usd IS NULL) AS unpriced_sales
    FROM window_sales
), collections AS (
    SELECT contract_address,count(*) AS sold,sum(price)::text AS earned,
        round(coalesce(sum(price*usd),0)*1e-18::numeric,6)::text AS earned_usd,
        count(*) FILTER (WHERE usd IS NULL) AS unpriced_sales
    FROM window_sales GROUP BY contract_address
), items AS (
    SELECT contract_address,item_id::text AS item_id,count(*) AS lifetime,
        count(*) FILTER (WHERE in_window) AS sold,coalesce(sum(price) FILTER (WHERE in_window),0)::text AS earned
    FROM mine WHERE kind='mint' AND item_id IS NOT NULL GROUP BY contract_address,item_id
), royalty_candidates AS (
    SELECT s.price,s.royalties,s.settled,s.seller,coalesce(s.creator,v1.creator) AS creator
    FROM sold s
    LEFT JOIN v1_resales v1 USING(network,block_number,log_index,ordinal)
    WHERE s.kind IN ('order','bid') AND ($8::bigint IS NULL OR s.block_time*1000 >= $8)
      AND ($9::bigint IS NULL OR s.block_time*1000 <= $9) AND (
        s.creator=$7 OR v1.network IS NOT NULL OR (s.creator IS NULL AND s.item_id IS NOT NULL AND EXISTS (
            SELECT 1 FROM candidate_collections c WHERE (c.network,c.contract_address)=(s.network,s.contract_address)
        ))
    )
), royalties AS (
    SELECT count(*) FILTER (WHERE creator=$7) AS resales,
        coalesce(sum(price) FILTER (WHERE creator=$7),0)::text AS volume,
        CASE WHEN bool_or(seller IS NULL OR creator IS NULL OR NOT settled)
                  FILTER (WHERE seller IS DISTINCT FROM $7) THEN NULL
             ELSE coalesce(sum(royalties) FILTER (WHERE creator=$7 AND seller<>$7),0)::text END AS paid
    FROM royalty_candidates
)
SELECT total,mints,totals.resales,earned,earned_usd,unpriced_sales,
       royalties.resales AS royalty_resales,volume,paid AS royalty_paid,
       coalesce((SELECT jsonb_agg(jsonb_build_object('contractAddress',contract_address,'sold',sold,'earnedWei',earned,
          'earnedUsd',earned_usd,'unpricedSales',unpriced_sales)
          ORDER BY contract_address) FROM collections),'[]'::jsonb) AS by_collection,
       coalesce((SELECT jsonb_agg(jsonb_build_object('contract_address',contract_address,'item_id',item_id,
          'lifetime',lifetime,'sold',sold,'earned',earned) ORDER BY contract_address,item_id) FROM items),'[]'::jsonb) AS items,
       EXISTS (SELECT FROM sales s WHERE s.seller IS NULL
         AND ((($8 IS NULL OR s.block_time*1000 >= $8) AND ($9 IS NULL OR s.block_time*1000 <= $9))
              OR (s.kind='mint' AND s.item_id IS NOT NULL))
         AND EXISTS (SELECT FROM seller_evidence ev WHERE ev.address=$7 AND
             (ev.network,ev.block_number,ev.log_index,ev.ordinal)=(s.network,s.block_number,s.log_index,s.ordinal))) AS incomplete
FROM totals CROSS JOIN royalties
