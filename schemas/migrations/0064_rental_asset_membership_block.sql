-- Existing assets acquire the LAND membership version on their next indexed fill.
ALTER TABLE IF EXISTS signatures.rental_assets
    ADD COLUMN IF NOT EXISTS membership_block bigint NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS metadata_block bigint NOT NULL DEFAULT 0;
