-- Inventory and economic totals are separate. A wallet can keep its account
-- after transferring every NFT or leaving the committee.
, prefix AS (
    SELECT network,block_number FROM shop.sale_publication WHERE network=ANY($1)
), native_accounts AS (
    SELECT a.network,w.address
    FROM shop.sale_asset a JOIN prefix p USING(network)
    CROSS JOIN LATERAL (VALUES (a.buyer),(a.seller)) w(address)
    WHERE a.block_number<=p.block_number AND a.price>0 AND w.address IS NOT NULL
    UNION
    SELECT m.network,w.address
    FROM shop.sale_payment m JOIN prefix p USING(network)
    JOIN shop.sale_execution e ON (e.network,e.block_number,e.log_index)=(m.network,m.block_number,m.log_index)
    CROSS JOIN LATERAL (VALUES (m.payer),(m.beneficiary)) w(address)
    WHERE m.block_number<=p.block_number AND m.consideration AND m.gross>0 AND w.address IS NOT NULL
      AND (e.kind<>'trade' OR EXISTS (SELECT FROM shop.sale_asset a WHERE a.price>0
        AND (a.network,a.block_number,a.log_index,a.currency)=(m.network,m.block_number,m.log_index,m.currency)))
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
    WHERE t.network=ANY($1) AND t.received>0
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
       EXISTS(SELECT FROM sold WHERE seller IS NULL OR NOT settled) AS missing_payment,
       EXISTS(SELECT FROM founders WHERE creator IS NULL) AS missing_creator,
       coalesce((SELECT jsonb_agg(jsonb_build_object('id',address,'address',address,
          'sales',sales,'purchases',purchases,'spent',spent::text,'earned',earned::text,
          'royalties',royalties::text,'collections',collections) ORDER BY sort_key DESC,address,network)
          FROM page),'[]'::jsonb) AS accounts
