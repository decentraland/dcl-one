-- BidAccepted carries the fee paid in base units. A NULL in an old row
-- distinguishes an unobserved fee from a zero fee.
ALTER TABLE IF EXISTS shop.sale_event ADD COLUMN IF NOT EXISTS bid_fee numeric(78,0);
-- A node upgraded from before sale_event existed gets the validated check
-- from the schema object pass. Existing rows have no historical fee yet.
ALTER TABLE IF EXISTS shop.sale_event ADD CONSTRAINT sale_event_bid_fee_check
    CHECK (bid_fee IS NULL OR (bid_fee >= 0 AND kind = 'bid_accepted')) NOT VALID;

-- Existing publications predate the sale input decoders. Only a checked
-- replay or a contiguous window written by those decoders advances proof.
ALTER TABLE IF EXISTS shop.chain_cursor
    ADD COLUMN IF NOT EXISTS sale_input_epoch uuid NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN IF NOT EXISTS sale_covered_rewinds integer,
    ADD COLUMN IF NOT EXISTS sale_input_version integer,
    ADD COLUMN IF NOT EXISTS sale_verified_through bigint,
    ADD COLUMN IF NOT EXISTS sale_dirty_from bigint,
    ADD COLUMN IF NOT EXISTS sale_repair_upper bigint,
    ADD COLUMN IF NOT EXISTS sale_history_revision bigint NOT NULL DEFAULT 0;
