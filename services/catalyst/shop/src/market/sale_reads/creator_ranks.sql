, seller_days AS (
    SELECT network,seller AS id,block_time/86400 AS day,count(*) AS sales,sum(net) FILTER (WHERE $7=0 OR kind='mint') AS earned,
           count(*) FILTER (WHERE kind='mint') AS mints
    FROM sold WHERE block_time >= $7 GROUP BY network,seller,block_time/86400
), sellers AS (
    SELECT network,id,sum(CASE WHEN $7=0 THEN sales ELSE mints END) AS sales,sum(earned) AS earned
    FROM seller_days WHERE $7=0 OR mints>0 GROUP BY network,id
), relationships AS (
    SELECT network,seller AS id,count(DISTINCT buyer) AS collectors,
           count(DISTINCT contract_address) FILTER (WHERE kind='mint') AS collections
    FROM sales WHERE $7=0 OR kind='mint' GROUP BY network,seller
), mint_creators AS (
    -- Store mints record the creator at Issue. For an off-chain mint the
    -- signer may be an authorized minter. Find the creator at that execution.
    SELECT s.network,s.net,CASE WHEN s.venue_kind='store' THEN s.seller
        WHEN s.network='matic' THEN coalesce(ch.account,f.creator) ELSE s.creator END AS creator
    FROM sold s LEFT JOIN founders f USING(network,contract_address)
    LEFT JOIN LATERAL (
        SELECT e.account FROM shop.collection_event e
        WHERE (e.network,e.contract_address)=(s.network,s.contract_address)
          AND e.kind='creatorship_transferred' AND (e.block_number,e.log_index)<(s.block_number,s.log_index)
        ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
    ) ch ON s.venue_kind='trade' AND s.network='matic'
    WHERE $7=0 AND s.kind='mint'
), primary_sales AS (
    SELECT network,creator,count(*) AS mints,sum(net) AS earned FROM mint_creators GROUP BY network,creator
), totals AS (
    SELECT s.id,sum(s.sales)::bigint AS sales,sum(CASE WHEN $7=0 THEN a.earned ELSE s.earned END) AS earned,
           max(r.collectors) AS collectors,
           CASE WHEN $7=0 THEN sum(c.collections) ELSE max(r.collections) END::bigint AS collections,
           coalesce(sum(m.mints),0) AS mints,coalesce(sum(m.earned),0) AS primary_net
    FROM sellers s JOIN relationships r USING(network,id)
    LEFT JOIN collections c ON c.network=s.network AND c.address=s.id
    LEFT JOIN primary_sales m ON m.network=s.network AND m.creator=s.id
    LEFT JOIN account_values a ON $7=0 AND a.network=s.network AND a.address=s.id
    WHERE $7>0 OR c.collections>0 GROUP BY s.id
), page AS (
    SELECT *, CASE WHEN $8='sales' THEN CASE WHEN $7=0 THEN mints ELSE sales END
                   WHEN $8='volume' AND $7=0 THEN primary_net ELSE earned END AS sort_key
    FROM totals ORDER BY sort_key DESC,id LIMIT $9
)
SELECT coalesce((SELECT jsonb_agg(jsonb_build_object('id',id,'sales',sales,'earned',earned::text,
       'uniqueCollectors',collectors,'collections',collections) ORDER BY sort_key DESC,id) FROM page),'[]'::jsonb),
       EXISTS(SELECT FROM sold WHERE block_time >= $7 AND (seller IS NULL OR NOT settled)),
       ($7=0 AND EXISTS(SELECT FROM founders WHERE creator IS NULL))
         OR ($8 IN ('sales','volume') AND EXISTS(SELECT FROM mint_creators WHERE creator IS NULL))
