-- Why a cancelled rental listing closed (`nonce`, `transferred`, `dissolved`,
-- `expired`), so a rebuild of the rentals index reopens only what a nonce
-- move cancelled. Existing rows keep NULL: a cancellation of unknown cause,
-- which stays cancelled. No backfill and no rewrite.
ALTER TABLE signatures.rentals ADD COLUMN IF NOT EXISTS cancel_reason text;
ALTER TABLE signatures.rentals ADD CONSTRAINT rentals_cancel_reason_known
    CHECK (cancel_reason IN ('nonce', 'transferred', 'dissolved', 'expired'));
ALTER TABLE signatures.rentals ADD CONSTRAINT rentals_cancel_reason_cancelled
    CHECK (cancel_reason IS NULL OR status = 'cancelled');
