-- Run once on a Sepolia/Amoy database whose graph mirrors used the legacy
-- mainnet defaults, with catalogue workers stopped. DATABASE.md describes
-- how to check the database and configure its replacement sources.
DO $rebind$
BEGIN
    IF current_setting('rebind.confirm_testnet', true) IS DISTINCT FROM 'yes' THEN
        RAISE EXCEPTION 'confirm the testnet rebind with rebind.confirm_testnet=yes';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM land_authz.index_cursor WHERE network='ethereum-sepolia') THEN
        RAISE EXCEPTION 'missing Sepolia LAND cursor for the testnet rebind';
    END IF;
    LOCK TABLE shop.worker_progress, shop.chain_cursor, shop.mirror_state,
               shop.collections, shop.items IN SHARE ROW EXCLUSIVE MODE;
    IF EXISTS (SELECT 1 FROM shop.worker_progress WHERE worker='nft_catalogue_pair' AND scope=0)
       OR EXISTS (SELECT 1 FROM shop.chain_cursor WHERE indexer='nft')
       OR EXISTS (SELECT 1 FROM shop.nft_event LIMIT 1) THEN
        RAISE EXCEPTION 'catalogue has a pair binding or NFT history';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM shop.mirror_state)
       OR EXISTS (SELECT 1 FROM shop.mirror_state WHERE
           (network,source_url) NOT IN (
             ('ethereum','https://subgraph.decentraland.org/collections-ethereum-mainnet'),
             ('matic','https://subgraph.decentraland.org/collections-matic-mainnet'))
           OR pass_block_hash IS NOT NULL
           OR chain_id IS DISTINCT FROM CASE network WHEN 'ethereum' THEN 1 ELSE 137 END)
       THEN
        RAISE EXCEPTION 'expected only the legacy mainnet default mirror sources after migration 0070';
    END IF;
    DELETE FROM shop.items;
    DELETE FROM shop.collections;
    DELETE FROM shop.mirror_state;
    INSERT INTO shop.worker_progress(worker,scope,key_position)
        VALUES ('nft_catalogue_pair',0,ARRAY['11155111','80002']);
    RAISE LOG 'operator rebound legacy marketplace mirrors to Sepolia/Amoy';
    RAISE NOTICE 'legacy marketplace mirrors cleared and catalogue bound to Sepolia/Amoy';
END
$rebind$;
