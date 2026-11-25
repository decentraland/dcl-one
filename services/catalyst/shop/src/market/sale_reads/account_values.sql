-- The beneficiary buys the presented asset. Earned credits the seller's
-- net and credits the royalty or protocol cut to its actual recipient.
, account_entries AS (
    SELECT network,seller AS address,count(*) AS sales,0 AS purchases,
           sum(net) AS earned,0::numeric AS royalties,0::numeric AS spent
    FROM sold WHERE seller IS NOT NULL GROUP BY network,seller
    UNION ALL
    SELECT network,buyer,0,count(*),0,0,sum(price) FROM sales GROUP BY network,buyer
    UNION ALL
    SELECT network,address,0,0,royalties+fees_received,royalties+fees_received,0
    FROM shop.sale_account_total
    WHERE network=ANY($1) AND currency=CASE WHEN network='ethereum' THEN $2 ELSE $3 END
      AND (royalties>0 OR fees_received>0)
), account_values AS (
    SELECT network,address,sum(sales)::bigint AS sales,sum(purchases)::bigint AS purchases,
           sum(earned) AS earned,sum(royalties) AS royalties,sum(spent) AS spent
    FROM account_entries GROUP BY network,address
)
