-- Inventory and economic totals are separate. A wallet can keep its account
-- after transferring every NFT or leaving the committee.
, prefix AS (
    SELECT network,block_number FROM shop.sale_publication WHERE network=ANY($1)
), inventory_assets AS (
    SELECT a.network,a.block_number,a.log_index,a.ordinal,a.currency,a.buyer,a.seller,e.kind AS venue_kind
    FROM shop.sale_asset a JOIN prefix p USING(network)
    JOIN shop.sale_execution e ON (e.network,e.block_number,e.log_index)=(a.network,a.block_number,a.log_index)
    WHERE a.block_number<=p.block_number AND a.price>0
      -- Fixed-price MANA orders and bids already record both participants
      -- in the account totals. Store and trade payments can name others.
      AND (e.kind IN ('store','trade') OR a.currency<>CASE WHEN a.network='ethereum' THEN $2 ELSE $3 END)
      AND (e.kind<>'trade' OR NOT EXISTS (
        SELECT FROM shop.sale_asset other WHERE (other.network,other.block_number,other.log_index)=
            (a.network,a.block_number,a.log_index) AND other.ordinal<>a.ordinal))
), native_accounts AS (
    SELECT a.network,w.address
    FROM inventory_assets a
    CROSS JOIN LATERAL (VALUES (a.buyer),(a.seller)) w(address)
    WHERE w.address IS NOT NULL
    UNION
    SELECT a.network,w.address
    FROM inventory_assets a JOIN shop.sale_payment m ON
        (m.network,m.block_number,m.log_index,m.currency)=(a.network,a.block_number,a.log_index,a.currency)
        AND (a.venue_kind='trade' OR m.ordinal=a.ordinal)
    CROSS JOIN LATERAL (VALUES (nullif(m.payer,a.buyer)),(nullif(m.beneficiary,a.seller))) w(address)
    WHERE m.consideration AND m.gross>0 AND w.address IS NOT NULL
    UNION
    SELECT m.network,m.payer
    FROM shop.sale_execution e JOIN shop.sale_payment m USING(network,block_number,log_index)
    JOIN prefix p USING(network)
    WHERE e.kind='name_fee' AND e.block_number<=p.block_number AND m.consideration AND m.gross>0
    UNION
    SELECT network,address FROM collections
    UNION
    SELECT n.network,w.address
    FROM shop.nft_event n JOIN prefix p USING(network)
    CROSS JOIN LATERAL (VALUES
        (n.to_address),
        (CASE WHEN n.kind='transfer' THEN nullif(n.from_address,'0x0000000000000000000000000000000000000000') END)
    ) w(address)
    WHERE n.kind IN ('transfer','issue_v1','name_registered') AND n.block_number<=p.block_number
      AND w.address IS NOT NULL
    UNION
    SELECT m.network,m.buyer FROM shop.sale_event m JOIN prefix p USING(network)
    WHERE m.kind='mint' AND m.block_number<=p.block_number
      AND EXISTS (SELECT FROM founders f WHERE (f.network,f.contract_address)=(m.network,m.contract_address))
    UNION
    SELECT t.network,t.address FROM shop.sale_account_total t
    WHERE t.network=ANY($1)
    UNION
    SELECT c.network,c.member FROM shop.committee_event c JOIN prefix p USING(network)
    WHERE c.block_number<=p.block_number
)
, rows AS MATERIALIZED (
    SELECT n.network,n.address,coalesce(a.sales,0) AS sales,coalesce(a.purchases,0) AS purchases,
           coalesce(a.spent,0) AS spent,coalesce(a.earned,0) AS earned,
           coalesce(a.royalties,0) AS royalties,coalesce(c.collections,0) AS collections
    FROM native_accounts n LEFT JOIN account_values a USING(network,address)
    LEFT JOIN collections c USING(network,address)
    WHERE n.address IS NOT NULL AND (cardinality($7::text[])=0 OR n.address=ANY($7))
), page AS (
    SELECT *,CASE $8 WHEN 'sales' THEN sales WHEN 'purchases' THEN purchases WHEN 'spent' THEN spent
           WHEN 'royalties' THEN royalties WHEN 'collections' THEN collections ELSE earned END AS sort_key
    FROM rows ORDER BY sort_key DESC,address,network LIMIT $9 OFFSET $10
)
SELECT (SELECT count(*) FROM rows) AS total,
       EXISTS(SELECT FROM page r JOIN account_values a USING(network,address) WHERE a.unknown_proceeds>0)
         OR EXISTS(SELECT FROM seller_evidence ev JOIN page r USING(network,address)
                   WHERE ev.currency=CASE WHEN ev.network='ethereum' THEN $2 ELSE $3 END) AS missing_payment,
       EXISTS(SELECT FROM founders WHERE creator IS NULL) AS missing_creator,
       coalesce((SELECT jsonb_agg(jsonb_build_object('id',address,'address',address,
          'sales',sales,'purchases',purchases,'spent',spent::text,'earned',earned::text,
          'royalties',royalties::text,'collections',collections) ORDER BY sort_key DESC,address,network)
          FROM page),'[]'::jsonb) AS accounts
