-- A bid's fingerprint (the BidCreated `_fingerprint`, which an estate bid
-- checks its contents against), kept by the sales walk and its bid fold.
-- Creations walked before this step keep NULL. A node upgraded from before
-- these tables existed gets the columns and the validated check from the
-- schema object pass; existing rows all hold NULL, so the check skips their
-- scan under the migration lock.
ALTER TABLE IF EXISTS shop.sale_event ADD COLUMN IF NOT EXISTS fingerprint text;
ALTER TABLE IF EXISTS shop.sale_event ADD CONSTRAINT sale_event_fingerprint_check
    CHECK (fingerprint IS NULL OR kind = 'bid_created') NOT VALID;
ALTER TABLE IF EXISTS shop.nft_bid ADD COLUMN IF NOT EXISTS fingerprint text;
