-- Preserve populated attribution rows and source identifiers at cutover.
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS id uuid NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS invited_user_ip text;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS signed_up_at timestamp without time zone;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS tier_granted boolean NOT NULL DEFAULT false;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS tier_granted_at timestamp without time zone;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS login_days date[] NOT NULL DEFAULT '{}';
UPDATE social.referral_progress SET tier_granted = true WHERE status = 'tier_granted' AND NOT tier_granted;
ALTER TABLE social.referral_progress ADD CONSTRAINT referral_progress_id_key UNIQUE (id);
