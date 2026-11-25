-- Existing nodes can prebuild these read indexes concurrently.
DO $$ BEGIN
    IF to_regclass('shop.sale_asset') IS NOT NULL THEN
        CREATE INDEX IF NOT EXISTS sale_asset_read_seller ON shop.sale_asset (network,seller,block_number) WHERE price>0;
        CREATE INDEX IF NOT EXISTS sale_asset_read_buyer ON shop.sale_asset (network,buyer,block_number) WHERE price>0;
        CREATE INDEX IF NOT EXISTS sale_asset_read_item ON shop.sale_asset (network,contract_address,item_id) WHERE price>0;
        CREATE INDEX IF NOT EXISTS sale_asset_read_price ON shop.sale_asset (price DESC,public_id) WHERE price>0;
    END IF;
    IF to_regclass('shop.sale_payment') IS NOT NULL THEN
        CREATE INDEX IF NOT EXISTS sale_payment_read_beneficiary ON shop.sale_payment (network,beneficiary,block_number) WHERE consideration;
    END IF;
    IF to_regclass('shop.sale_execution') IS NOT NULL THEN
        CREATE INDEX IF NOT EXISTS sale_execution_read_time ON shop.sale_execution (block_time DESC,network,block_number,log_index);
    END IF;
END $$;
