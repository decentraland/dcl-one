-- Schedule the V1 replay after an operator identifies a quarantined legacy
-- Sepolia trade history whose checkpoint cannot be proven. The writer checks
-- its RPC's chain ID and clears the trade projection in its fenced pass.
DO $rebuild$
BEGIN
    IF current_setting('rebuild.confirm_sepolia', true) IS DISTINCT FROM 'yes' THEN
        RAISE EXCEPTION 'confirm the legacy Sepolia trade rebuild with rebuild.confirm_sepolia=yes';
    END IF;
    UPDATE shop.chain_cursor
       SET chain_id = 11155111, start_block = 6770056,
           rebuild_requested_at = timezone('UTC', now()),
           folded_block = NULL, folded_at = NULL,
           owner_generation = owner_generation + 1
     WHERE indexer = 'trade_events' AND network = 'ethereum'
       AND quarantined_at IS NOT NULL AND NOT sepolia_trade_v1
       AND (chain_id IS NULL OR chain_id = 11155111)
       AND start_block = 9455328;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'expected a quarantined Sepolia trade cursor at the legacy V2 start';
    END IF;
END
$rebuild$;
