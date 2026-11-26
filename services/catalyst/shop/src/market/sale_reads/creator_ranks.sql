, mint_creators AS MATERIALIZED (
    -- Store attribution is proved at Issue, before the Bought execution.
    -- A trade uses the creator at Traded, even within a creation block.
    SELECT s.network,s.block_number,s.log_index,s.ordinal,s.contract_address,s.buyer,s.block_time,
           CASE WHEN s.venue_kind='store' THEN s.seller
                WHEN s.network='matic' THEN coalesce(earlier.account,later.previous_account,
                    CASE WHEN earlier.log_index IS NULL AND later.log_index IS NULL
                         THEN coalesce(seed.creator,c.creator) END)
                ELSE s.creator END AS creator
    FROM sales s JOIN shop.sale_publication pub USING(network)
    LEFT JOIN founders f ON (f.network,f.contract_address)=(s.network,s.contract_address)
      AND (f.created_block,f.created_log)<(s.block_number,s.log_index)
    LEFT JOIN LATERAL (
        SELECT t.account,t.log_index FROM shop.collection_event t
        WHERE (t.network,t.contract_address)=(f.network,f.contract_address)
          AND t.kind='creatorship_transferred' AND (t.block_number,t.log_index)<(s.block_number,s.log_index)
        ORDER BY t.block_number DESC,t.log_index DESC LIMIT 1
    ) earlier ON s.network='matic' AND s.venue_kind='trade'
    LEFT JOIN LATERAL (
        SELECT t.previous_account,t.log_index FROM shop.collection_event t
        WHERE (t.network,t.contract_address)=(f.network,f.contract_address)
          AND t.kind='creatorship_transferred' AND (t.block_number,t.log_index)>(s.block_number,s.log_index)
          AND t.block_number<=pub.block_number
        ORDER BY t.block_number,t.log_index LIMIT 1
    ) later ON s.network='matic' AND s.venue_kind='trade' AND earlier.account IS NULL
    LEFT JOIN shop.chain_cursor history ON history.network=f.network AND history.indexer='collections'
    LEFT JOIN shop.collection_seed seed ON (seed.network,seed.contract_address)=(f.network,f.contract_address)
      AND seed.verified AND seed.read_hash IS NOT NULL AND history.reorg_rewinds=0
      AND seed.read_block BETWEEN f.created_block AND pub.block_number
    LEFT JOIN shop.collections c ON (c.network,c.contract_address)=(f.network,f.contract_address)
      AND c.chain_id=137 AND c.created_at=f.created_at AND c.updated_at<=s.block_time
    WHERE s.kind='mint'
), primary_sales AS MATERIALIZED (
    SELECT m.creator AS id,
           count(*) FILTER (WHERE m.block_time >= $7 AND ($7>0 OR eligible.address IS NOT NULL)) AS mints,
           coalesce(sum(p.net) FILTER (WHERE $7>0 OR eligible.address IS NOT NULL),0) AS net,
           count(DISTINCT m.buyer) AS collectors,
           count(DISTINCT (m.network,m.contract_address)) AS collections,
           bool_or(m.block_time >= $7 AND (p.seller IS NULL OR NOT p.settled)) AS missing_payment
    FROM mint_creators m
    LEFT JOIN collections eligible ON (eligible.network,eligible.address)=(m.network,m.creator)
    LEFT JOIN (
        SELECT network,block_number,log_index,ordinal,seller,settled,net
        FROM sold WHERE kind='mint' AND block_time >= $7
    ) p ON (p.network,p.block_number,p.log_index,p.ordinal)=(m.network,m.block_number,m.log_index,m.ordinal)
    GROUP BY m.creator
), lifetime_sellers AS (
    SELECT s.seller AS id,count(*) AS sales,count(DISTINCT s.buyer) AS collectors
    FROM sales s JOIN collections c ON (c.network,c.address)=(s.network,s.seller)
    WHERE $7=0 GROUP BY s.seller
), lifetime_earnings AS (
    SELECT address,sum(earned) AS earned,bool_or(unknown_proceeds>0) AS unresolved
    FROM account_values JOIN collections USING(network,address)
    WHERE $7=0 GROUP BY address
), totals AS (
    SELECT id,mints AS sales,net AS earned,collectors,collections,mints,net AS primary_net
    FROM primary_sales WHERE $7>0 AND id IS NOT NULL AND mints>0
    UNION ALL
    SELECT c.address,coalesce(s.sales,0),coalesce(a.earned,0),coalesce(s.collectors,0),
           c.collections,coalesce(m.mints,0),coalesce(m.net,0)
    FROM collections c
    LEFT JOIN lifetime_sellers s ON s.id=c.address
    LEFT JOIN lifetime_earnings a USING(address)
    LEFT JOIN primary_sales m ON m.id=c.address
    WHERE $7=0 AND (s.sales>0 OR ($8 IN ('sales','volume') AND m.mints>0))
), page AS (
    SELECT *,CASE WHEN $8='sales' THEN mints
                  WHEN $8='volume' AND $7=0 THEN primary_net ELSE earned END AS sort_key
    FROM totals ORDER BY sort_key DESC,id LIMIT $9
)
SELECT coalesce((SELECT jsonb_agg(jsonb_build_object('id',id,'sales',sales,'earned',earned::text,
       'uniqueCollectors',collectors,'collections',collections) ORDER BY sort_key DESC,id) FROM page),'[]'::jsonb),
       EXISTS(SELECT FROM primary_sales WHERE missing_payment)
         OR ($7=0 AND EXISTS(SELECT FROM lifetime_earnings WHERE unresolved)),
       ($7=0 AND EXISTS(SELECT FROM founders WHERE creator IS NULL))
         OR (($7>0 OR $8 IN ('sales','volume')) AND EXISTS(SELECT FROM primary_sales WHERE id IS NULL))
