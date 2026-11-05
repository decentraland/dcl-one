-- Authenticated Explorer login retries share one receipt per invited wallet and
-- login ID. The FK shares referral attribution's restore/deletion lifecycle.
-- At most 120 unexpired IDs per wallet are admitted under its referral row lock;
-- the existing referral worker removes a bounded expiry batch each minute.
CREATE TABLE IF NOT EXISTS social.referral_login_receipts (
    invited_user text NOT NULL REFERENCES social.referral_progress(invited_user) ON DELETE CASCADE,
    login_id uuid NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    PRIMARY KEY (invited_user, login_id)
);
CREATE INDEX IF NOT EXISTS referral_login_receipts_expiry_idx
    ON social.referral_login_receipts (expires_at, invited_user, login_id);
