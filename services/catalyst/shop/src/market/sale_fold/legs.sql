-- Every read is bounded by the executions inserted for this pass. Separate
-- the parties to the NFT transfer from the parties to the consideration.
, execution AS MATERIALIZED (
    SELECT e.*,v.fill FROM shop.sale_execution e JOIN positions v USING(block_number,tx_hash,log_index)
    WHERE e.network = $1 AND e.block_number > $2 AND e.block_number <= $3
), fixed_price AS MATERIALIZED (
    SELECT e.network, e.block_number, e.log_index, 0 AS ordinal, e.kind, n.contract_address,
           n.token_id, NULL::numeric AS item_id, n.from_address AS seller, n.to_address AS buyer,
           n.amount AS price, n.to_address AS payer, n.from_address AS payee, NULL::numeric AS bid_fee
    FROM execution e JOIN shop.nft_event n USING (network, block_number, log_index)
    WHERE e.kind = 'order'
    UNION ALL
    SELECT e.network, e.block_number, e.log_index, 0, e.kind, s.contract_address,
           s.token_id, s.item_id, s.seller, s.buyer, s.price, s.buyer, s.seller, s.bid_fee
    FROM execution e JOIN shop.sale_event s USING (network, block_number, log_index)
    WHERE e.kind = 'bid'
    UNION ALL
    SELECT e.network, e.block_number, e.log_index, l.ordinal, 'mint', l.contract_address,
           s.token_id, l.item_id, NULL, l.beneficiary, l.price, NULL, NULL, NULL
    FROM execution e JOIN shop.verified_store_sale_line l ON (l.block_number,l.bought_log) = (e.block_number,e.log_index)
    LEFT JOIN shop.sale_event s ON s.network = e.network AND s.block_number = l.block_number
      AND s.log_index = l.issue_log AND s.tx_hash = l.tx_hash AND s.kind = 'mint'
    WHERE e.kind = 'store'
), trade_leg AS MATERIALIZED (
    SELECT e.network, e.block_number, e.log_index, side.sent, a.ordinality,
           a.asset->>'assetType' AS typ, a.asset->>'contractAddress' AS contract,
           (a.asset->>'value')::numeric AS amount,
           CASE WHEN side.sent THEN t.signer ELSE t.caller END AS sender,
           CASE WHEN a.asset->>'beneficiary' = '0x0000000000000000000000000000000000000000'
                THEN CASE WHEN side.sent THEN t.caller ELSE t.signer END
                ELSE a.asset->>'beneficiary' END AS recipient
    FROM execution e JOIN shop.trade_event t USING (network, block_number, log_index)
    CROSS JOIN LATERAL (VALUES (true,t.assets->'sent'),(false,t.assets->'received')) side(sent,legs)
    CROSS JOIN LATERAL jsonb_array_elements(side.legs) WITH ORDINALITY a(asset,ordinality)
    WHERE e.kind = 'trade'
), trade_payment AS MATERIALIZED (
    SELECT *, (row_number() OVER (PARTITION BY network,block_number,log_index ORDER BY sent DESC,ordinality)-1)::int AS ordinal,
           -- Polygon and Ethereum mutate the asset in memory before emitting Traded:
           -- the converted leg carries the MANA contract. An unconverted leg
           -- remains denominated in USD and cannot enter MANA totals.
           CASE WHEN typ='2' AND contract<>$8 THEN 'usd' ELSE contract END AS currency
    FROM trade_leg WHERE typ IN ('1','2')
), trade_asset AS (
    SELECT *, (row_number() OVER w-1)::int AS ordinal, count(*) OVER (PARTITION BY network,block_number,log_index) AS n
    FROM trade_leg WHERE typ IN ('3','4')
    WINDOW w AS (PARTITION BY network,block_number,log_index ORDER BY sent DESC,ordinality)
), payments AS (
    INSERT INTO shop.sale_payment (network,block_number,log_index,ordinal,currency,gross,payer,beneficiary,bid_fee,consideration)
    SELECT network,block_number,log_index,ordinal,$8,price,payer,payee,bid_fee,true FROM fixed_price
    UNION ALL
    SELECT network,block_number,log_index,ordinal,currency,amount,sender,recipient,NULL,
        NOT EXISTS (SELECT FROM trade_asset a WHERE (a.network,a.block_number,a.log_index,a.sent)=
                          (p.network,p.block_number,p.log_index,p.sent))
    FROM trade_payment p
    UNION ALL
    SELECT e.network,e.block_number,e.log_index,0,$8,n.price,n.caller,NULL,NULL,true
    FROM execution e JOIN shop.name_purchase n USING (block_number,log_index) WHERE e.kind = 'name_fee'
), assets (network,block_number,log_index,ordinal,kind,contract_address,token_id,item_id,seller,buyer,price,currency) AS (
SELECT network,block_number,log_index,ordinal,kind,contract_address,token_id,item_id,seller,buyer,price,$8 FROM fixed_price WHERE kind<>'mint' OR token_id IS NOT NULL
UNION ALL
SELECT a.network,a.block_number,a.log_index,a.ordinal,
       CASE WHEN a.typ='4' THEN 'mint' WHEN a.sent THEN 'order' ELSE 'bid' END,
       a.contract, CASE WHEN a.typ='3' THEN a.amount END, CASE WHEN a.typ='4' THEN a.amount END,
       a.sender,a.recipient,p.price,p.currency
FROM trade_asset a
LEFT JOIN LATERAL (
    SELECT sum(amount) AS price, min(currency) AS currency
    FROM trade_payment t WHERE (t.network,t.block_number,t.log_index) = (a.network,a.block_number,a.log_index)
    -- Classify every leg before checking eligibility. A converted USD leg
    -- can acquire a price only when the receipt proves the execution's split.
    HAVING bool_and(t.typ='1' AND t.currency=$8 AND t.sent<>a.sent) AND sum(t.amount)>0
) p ON a.n = 1
)
INSERT INTO shop.sale_asset (public_id,beneficiary,network,block_number,log_index,ordinal,kind,contract_address,token_id,item_id,seller,buyer,price,currency)
SELECT concat(e.network,'-',e.tx_hash,'-',e.fill,'-',a.ordinal),a.buyer,a.* FROM assets a
JOIN execution e USING (network,block_number,log_index);
