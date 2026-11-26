-- Number venue executions before filtering their assets against the catalogue.
-- Issue and payment logs describe a fill; neither is an additional venue event.
WITH venue_logs AS (
    SELECT block_number,tx_hash,log_index FROM shop.nft_event
    WHERE network=$1 AND block_number>$2 AND block_number<=$3 AND kind='order_successful' AND emitter=ANY($4)
    UNION ALL
    SELECT block_number,tx_hash,log_index FROM shop.sale_event
    WHERE network=$1 AND block_number>$2 AND block_number<=$3 AND kind='bid_accepted' AND emitter=$5
    UNION ALL
    SELECT DISTINCT block_number,tx_hash,bought_log FROM shop.store_sale_line
    WHERE $1='matic' AND block_number>$2 AND block_number<=$3
    UNION ALL
    SELECT block_number,tx_hash,log_index FROM shop.trade_event
    WHERE network=$1 AND block_number>$2 AND block_number<=$3 AND kind='traded' AND marketplace_address=ANY($6)
    UNION ALL
    SELECT block_number,tx_hash,log_index FROM shop.name_purchase
    WHERE $1='ethereum' AND block_number>$2 AND block_number<=$3 AND venue=ANY($7)
), positions AS (
    SELECT block_number,tx_hash,log_index,row_number() OVER (PARTITION BY block_number,tx_hash ORDER BY log_index)-1 AS fill
    FROM venue_logs
)
