-- shop.broker_purchases records each leg's signed transaction before it is
-- broadcast (`signed_tx` for the buy, `forward_signed_tx` for the escrow
-- forward), so a resume re-sends the same bytes instead of signing a second
-- transaction. `write_ahead` marks a claim made under that rule; a claim
-- kept from before it stays false and is never bought afresh, because its
-- 'pending' or 'error' status cannot prove that nothing was sent. Nullable
-- columns and a constant default change the catalog only; no rewrite.
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS signed_tx text;
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS forward_signed_tx text;
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS write_ahead boolean NOT NULL DEFAULT false;
