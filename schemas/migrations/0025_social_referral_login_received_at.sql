-- Earlier native-login deployments stored occurrence-based expiry without a
-- server receipt time. Preserve every receipt and conservatively start its
-- quota window when this migration backfills it under the table lock. Use
-- clock_timestamp() so time waiting for that lock cannot backdate the window.
-- If the relation is absent, the following object pass creates it instead.
ALTER TABLE IF EXISTS social.referral_login_receipts ADD COLUMN IF NOT EXISTS received_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', clock_timestamp());
