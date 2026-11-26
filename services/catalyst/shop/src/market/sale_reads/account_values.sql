-- The fold attributes net proceeds to the public seller and records payouts
-- to royalty and protocol recipients separately.
, account_values AS (
    SELECT network,address,sales,purchases,spent,net_sold+royalties+fees_received AS earned,
           royalties+fees_received AS royalties,unknown_proceeds
    FROM shop.sale_account_total
    WHERE network=ANY($1) AND currency=CASE WHEN network='ethereum' THEN $2 ELSE $3 END
)
