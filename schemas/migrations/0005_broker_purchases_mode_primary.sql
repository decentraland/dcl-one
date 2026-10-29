-- shop.broker_purchases.mode is what PurchaseMode::as_str writes
-- (src/shop/checkout/calls.rs): a CollectionStore mint is 'primary', as the
-- wire and v1 spell it. The table's check listed 'mint' instead, so every
-- primary broker claim and escrow claim failed its INSERT. The check is
-- replaced under a new name; declared on the table in schema/30_shop.sql.
ALTER TABLE shop.broker_purchases DROP CONSTRAINT IF EXISTS broker_purchases_mode_known;
ALTER TABLE shop.broker_purchases ADD CONSTRAINT broker_purchases_mode_valid CHECK (mode IN ('primary', 'secondary', 'trade', 'name-mint', 'name-secondary'));
