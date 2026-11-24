SELECT indexer, last_block, last_block_hash AS hash, folded_block, folded_at,
       sale_input_epoch AS epoch, sale_history_revision AS revision
FROM shop.chain_cursor
WHERE network = $1 AND chain_id = $2 AND full_history
  AND quarantined_at IS NULL AND diverged_at IS NULL AND rebuild_requested_at IS NULL
  AND last_block_hash IS NOT NULL AND folded_block IS NOT NULL
  AND folded_at BETWEEN $4::timestamp - interval '30 minutes' AND $4
  AND sale_input_version = $3 AND sale_covered_rewinds = reorg_rewinds
  AND sale_verified_through >= folded_block
  AND ((indexer IN ('nft','sales') AND start_block <= $5)
    OR (indexer = 'trade_events' AND start_block <= $6))
ORDER BY indexer
