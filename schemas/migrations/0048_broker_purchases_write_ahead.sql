-- shop.broker_purchases records each leg's signed transaction before it is
-- broadcast (`signed_tx` for the buy, `forward_signed_tx` for the escrow
-- forward), so a resume re-sends the same bytes instead of signing a second
-- transaction. `write_ahead` marks a claim made under that rule; a claim
-- kept from before it stays false and is never bought afresh, because its
-- 'pending' or 'error' status cannot prove that nothing was sent. A
-- recorded leg that looks dead (a finalized transaction holds its nonce, and
-- it has no receipt) is first noted in `dead_seen_at`; it is released only
-- when a second look at least ten minutes later agrees, and the released
-- hash is kept in `released_tx_hashes` (buy) or `released_forward_tx_hashes`
-- (forward). Nullable columns and a constant
-- default change the catalog only; no rewrite.
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS signed_tx text;
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS forward_signed_tx text;
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS write_ahead boolean NOT NULL DEFAULT false;
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS dead_seen_at timestamp without time zone;
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS released_tx_hashes text[];
ALTER TABLE IF EXISTS shop.broker_purchases ADD COLUMN IF NOT EXISTS released_forward_tx_hashes text[];
