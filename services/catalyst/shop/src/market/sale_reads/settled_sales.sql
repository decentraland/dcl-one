-- One NFT can have several payments in the same currency. A store execution
-- has explicitly priced line ordinals. Check all matching payments for the asset.
, sold AS (
    SELECT s.*,coalesce(legs.settled AND legs.gross=s.price,false) AS settled,
           legs.fees,legs.royalties,legs.net
    FROM sales s
    CROSS JOIN LATERAL (
        SELECT bool_and(p.settled) AS settled,sum(p.gross) AS gross,
               coalesce(sum(r.fees),0) AS fees,coalesce(sum(r.royalties),0) AS royalties,
               coalesce(sum(r.net),0) AS net
        FROM shop.sale_payment p
        LEFT JOIN LATERAL (
            SELECT sum(amount) FILTER (WHERE role IN ('burn','store_fee','venue_fee')) AS fees,
                   sum(amount) FILTER (WHERE role='royalty') AS royalties,
                   sum(amount) FILTER (WHERE role='seller') AS net
            FROM shop.sale_payout r
            WHERE (r.network,r.block_number,r.log_index,r.payment_ordinal)=(p.network,p.block_number,p.log_index,p.ordinal)
        ) r ON p.settled
        WHERE (p.network,p.block_number,p.log_index,p.currency)=(s.network,s.block_number,s.log_index,s.currency)
          AND p.consideration AND (s.venue_kind<>'store' OR p.ordinal=s.ordinal)
    ) legs
)
