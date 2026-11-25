, own AS (SELECT * FROM proceeds WHERE seller=$7),
royalty_resales AS (SELECT * FROM proceeds WHERE creator=$7 AND seller<>$7 AND kind IN ('bid','order'))
SELECT count(*) AS sales, count(*) FILTER (WHERE kind='mint') AS mints, count(*) FILTER (WHERE kind IN ('order','bid')) AS resales,
       coalesce(sum(price),0)::text AS gross_wei,coalesce(sum(fees),0)::text AS fees_wei,
       coalesce(sum(royalties),0)::text AS royalties_wei,coalesce(sum(net),0)::text AS net_wei,
       (SELECT count(*) FROM royalty_resales) AS royalty_resales,
       (SELECT coalesce(sum(royalties),0)::text FROM royalty_resales) AS royalties_earned_wei,
       min(block_time)*1000 AS first_sale_at,max(block_time)*1000 AS last_sale_at,
       EXISTS(SELECT FROM proceeds WHERE seller IS NULL) OR EXISTS(SELECT FROM own WHERE NOT settled)
           OR EXISTS(SELECT FROM royalty_resales WHERE NOT settled) AS incomplete
FROM own
