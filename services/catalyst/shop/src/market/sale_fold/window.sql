SELECT coalesce((
  SELECT block_number FROM (
    (SELECT block_number FROM shop.sale_event WHERE network=$1 AND block_number>$2 AND block_number<=$3
         AND kind='bid_accepted' ORDER BY block_number LIMIT 96)
    UNION ALL
    (SELECT block_number FROM shop.nft_event WHERE network=$1 AND block_number>$2 AND block_number<=$3
         AND kind='order_successful' ORDER BY block_number LIMIT 96)
    UNION ALL
    (SELECT block_number FROM shop.trade_event WHERE network=$1 AND block_number>$2 AND block_number<=$3
         AND kind='traded' ORDER BY block_number LIMIT 96)
    UNION ALL
    (SELECT block_number FROM shop.store_sale_line WHERE $1='matic' AND block_number>$2 AND block_number<=$3
         ORDER BY block_number LIMIT 96)
    UNION ALL
    (SELECT block_number FROM shop.name_purchase WHERE $1='ethereum' AND block_number>$2 AND block_number<=$3
         ORDER BY block_number LIMIT 96)
  ) recent ORDER BY block_number OFFSET 95 LIMIT 1
),$3)
