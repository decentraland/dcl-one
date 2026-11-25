, money AS MATERIALIZED (
    SELECT * FROM classified WHERE allocated
), payout AS MATERIALIZED (
    SELECT po.*,p.currency,p.day,p.kind,p.venue FROM money p JOIN shop.sale_payout po
      ON (p.network,p.block_number,p.log_index,p.ordinal)=(po.network,po.block_number,po.log_index,po.payment_ordinal)
), analytics AS (
    SELECT day,currency,count(*) AS sales,sum(price) AS volume,0::numeric AS names,0::numeric AS creator,
        0::numeric AS venue,0::numeric AS store,0::numeric AS burn,count(*) FILTER (WHERE NOT EXISTS (
            SELECT FROM money p WHERE (p.network,p.block_number,p.log_index,p.asset_ordinal)=
                (a.network,a.block_number,a.log_index,a.ordinal)
        )) AS unknowns,0::bigint AS unallocated
    FROM priced a GROUP BY day,currency
    UNION ALL
    SELECT day,currency,0,0,
        coalesce(sum(gross) FILTER (WHERE kind='name_fee'),0),0,0,0,
        coalesce(sum(gross) FILTER (WHERE kind='name_fee' AND venue=$3),0),
        count(*) FILTER (WHERE NOT settled),0::bigint
    FROM money p GROUP BY day,currency
    UNION ALL
    SELECT day,currency,0,0,0,
        coalesce(sum(amount) FILTER (WHERE role='royalty' OR (role='seller' AND
          (kind='store' OR (kind='trade' AND EXISTS (SELECT FROM priced a WHERE a.block_number=p.block_number AND a.log_index=p.log_index AND a.kind='mint'))))),0),
        coalesce(sum(amount) FILTER (WHERE role='venue_fee'),0),
        coalesce(sum(amount) FILTER (WHERE role='store_fee'),0),
        coalesce(sum(amount) FILTER (WHERE role='burn' AND NOT (kind='name_fee' AND venue=$3)),0),0,0
    FROM payout p GROUP BY day,currency
), write_analytics AS (
    INSERT INTO shop.sale_analytics_day AS d (network,day,currency,sales,volume,name_fees,creator_earnings,venue_fees,store_fees,burn_fees,unknown_splits,unallocated_executions)
    SELECT $1,day,currency,sum(sales),sum(volume),sum(names),sum(creator),sum(venue),sum(store),sum(burn),sum(unknowns),sum(unallocated)
    FROM analytics GROUP BY day,currency
), write_items AS (
    INSERT INTO shop.sale_item_day AS d (network,day,currency,contract_address,item_id,sales,volume)
    SELECT $1,day,currency,contract_address,item_id,count(*),sum(price) FROM priced WHERE item_id IS NOT NULL
    GROUP BY day,currency,contract_address,item_id
), first_purchases AS (
    INSERT INTO shop.sale_first_purchase AS f (network,currency,buyer,seller,contract_address,item_id,first_day)
    SELECT $1,currency,buyer,seller,contract_address,item_id,min(day) FROM priced WHERE seller IS NOT NULL
    GROUP BY currency,buyer,seller,contract_address,item_id
    ON CONFLICT(network,currency,buyer,seller,contract_address,item_id) DO UPDATE SET first_day=excluded.first_day
    WHERE excluded.first_day < f.first_day
), account_legs AS (
    SELECT day,currency,buyer AS address,0 AS sales,1 AS purchases,price AS spent,0::numeric AS gross_sold,
           0::numeric AS net_sold,0 AS unknown_proceeds,0::numeric AS received,0::numeric AS royalties,0::numeric AS fees_received
    FROM priced
    UNION ALL
    SELECT a.day,a.currency,a.seller,1,0,0,a.price,net.amount,
           CASE WHEN net.settled THEN 0 ELSE 1 END,0,0,0
    FROM priced a
    CROSS JOIN LATERAL (
        SELECT bool_and(p.settled) AS settled,
               coalesce(sum((SELECT sum(po.amount) FROM shop.sale_payout po
                   WHERE (po.network,po.block_number,po.log_index,po.payment_ordinal)=(p.network,p.block_number,p.log_index,p.ordinal)
                     AND po.role='seller')),0) AS amount
        FROM money p
        WHERE (p.network,p.block_number,p.log_index,p.asset_ordinal)=(a.network,a.block_number,a.log_index,a.ordinal)
    ) net WHERE a.seller IS NOT NULL
    UNION ALL SELECT day,currency,recipient,0,0,0,0,0,0,amount,
        CASE WHEN role='royalty' THEN amount ELSE 0 END,
        CASE WHEN role IN ('venue_fee','store_fee') THEN amount ELSE 0 END
    FROM payout WHERE role NOT IN ('retained','burn')
)
INSERT INTO shop.sale_account_day (network,day,currency,address,sales,purchases,spent,gross_sold,net_sold,unknown_proceeds,received,royalties,fees_received)
SELECT $1,day,currency,address,sum(sales),sum(purchases),sum(spent),sum(gross_sold),
       sum(net_sold),sum(unknown_proceeds),sum(received),sum(royalties),sum(fees_received)
FROM account_legs GROUP BY day,currency,address;
