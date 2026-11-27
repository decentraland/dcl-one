-- Each active buyer's window counts and spend cover the requested days.
-- The distinct relationships include purchases through the end of the window.
, per_network AS (
    SELECT network,buyer,count(*) FILTER (WHERE block_time >= $7) AS purchases,
           sum(price) FILTER (WHERE block_time >= $7) AS spent,
           count(DISTINCT seller) AS supported,
           count(DISTINCT (contract_address,item_id)) FILTER (WHERE rarity IN ('unique','mythic')) AS rare_items,
           bool_or(seller IS NULL) AS incomplete
    FROM sales GROUP BY network,buyer HAVING max(block_time) >= $7
), merged AS (
    SELECT buyer AS id,sum(purchases)::bigint AS purchases,sum(spent) AS spent,
           CASE WHEN $7=0 THEN sum(rare_items) ELSE max(rare_items) END::bigint AS rare_items,
           max(supported) AS supported
    FROM per_network GROUP BY buyer
), page AS (
    SELECT * FROM merged ORDER BY CASE WHEN $8 THEN purchases ELSE spent END DESC,id LIMIT $9
)
SELECT coalesce((SELECT jsonb_agg(jsonb_build_object('id',id,'purchases',purchases,'spent',spent::text,
        'uniqueAndMythicItems',rare_items,'creatorsSupported',supported)
        ORDER BY CASE WHEN $8 THEN purchases ELSE spent END DESC,id) FROM page),'[]'::jsonb),
       EXISTS(SELECT FROM per_network WHERE incomplete)
