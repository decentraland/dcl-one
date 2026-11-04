-- A granted referral that reached a reward tier keeps that tier until the
-- rewards request succeeds, then the paid reward until its notification and
-- image are done, so a redelivered login resumes a failed reward without
-- requesting it again. Nullable with no default: no table rewrite.
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS reward_tier integer;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS reward_paid jsonb;
