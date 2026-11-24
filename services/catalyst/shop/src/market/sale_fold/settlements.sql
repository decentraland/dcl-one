SELECT e.block_number AS block, e.log_index AS index, e.tx_hash, e.venue, e.kind,
       p.ordinal, p.currency, p.gross::text, p.payer, p.beneficiary, p.bid_fee::text,
       terms.collector, terms.rate, terms.royalty_rate, trade.extra
FROM shop.sale_execution e
JOIN shop.sale_payment p USING (network,block_number,log_index)
LEFT JOIN LATERAL (
    SELECT max(value) FILTER (WHERE field='collector') AS collector,
           max(value::numeric) FILTER (WHERE field='fee_ppm')::int AS rate,
           max(value::numeric) FILTER (WHERE field='royalty_ppm')::int AS royalty_rate
    FROM (
        SELECT DISTINCT ON (field) field,value FROM shop.sale_fee_change f
        WHERE (f.network,f.venue)=(e.network,e.venue)
          AND (f.block_number,f.log_index)<(e.block_number,e.log_index)
        ORDER BY field,block_number DESC,log_index DESC
    ) h
) terms ON true
LEFT JOIN LATERAL (
    SELECT a.asset->>'extra' AS extra FROM shop.trade_event t
    CROSS JOIN LATERAL jsonb_array_elements((t.assets->'sent') || (t.assets->'received')) WITH ORDINALITY a(asset,n)
    WHERE (t.network,t.block_number,t.log_index)=(e.network,e.block_number,e.log_index)
      AND a.asset->>'assetType' IN ('1','2')
    ORDER BY a.n OFFSET p.ordinal LIMIT 1
) trade ON e.kind='trade'
WHERE e.network=$1 AND ((e.block_number>$2 AND e.block_number<=$3) OR e.tx_hash=ANY($4)) AND NOT e.receipt_read
ORDER BY e.block_number,e.tx_hash,e.log_index,p.ordinal
