-- A granted referral that reached a reward tier keeps that tier until the
-- reward, its notification and its image are done, so a redelivered login
-- retries a failed reward. Nullable with no default: no table rewrite.
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS reward_tier integer;
