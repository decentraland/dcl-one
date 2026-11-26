, days AS MATERIALIZED (
    SELECT * FROM shop.sale_analytics_day
    WHERE network = ANY($1) AND currency = CASE WHEN network='ethereum' THEN $2 ELSE $3 END
      AND day >= $7::bigint
), name_income AS (
    SELECT sum(r.amount) AS amount FROM days d
    JOIN shop.sale_publication pub ON pub.network=d.network
    JOIN shop.sale_execution e ON e.network=d.network AND e.block_number<=pub.block_number AND e.block_time/86400*86400=d.day AND e.kind='name_fee'
    JOIN shop.sale_payout r ON (r.network,r.block_number,r.log_index)=(e.network,e.block_number,e.log_index)
    JOIN shop.sale_payment p ON (p.network,p.block_number,p.log_index,p.ordinal)=
        (r.network,r.block_number,r.log_index,r.payment_ordinal) AND p.currency=d.currency
    WHERE d.name_fees>0 AND r.role='name_fee'
)
SELECT coalesce(sum(d.sales),0)::bigint,coalesce(sum(d.volume),0)::text,
       coalesce(sum(d.creator_earnings),0)::text,
       (coalesce(sum(d.venue_fees+d.store_fees),0) + coalesce((SELECT amount FROM name_income),0))::text,
       coalesce(bool_or(d.unknown_splits>0),false)
FROM days d
