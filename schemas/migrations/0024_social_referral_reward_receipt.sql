-- Preserve the pre-POST claim and confirmed provider response independently
-- from downstream notification/image delivery. Nullable columns do not rewrite.
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS reward_requested_at timestamp without time zone;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS reward_response jsonb;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS reward_retry_at timestamp without time zone;
-- Preserve confirmed paid receipts and their notification completion flag.
-- An incomplete image does not make a known payment outcome ambiguous.
UPDATE social.referral_progress
    SET reward_tier=COALESCE(reward_tier, CASE
            WHEN pg_input_is_valid(reward_paid->>'tier', 'integer')
            THEN (reward_paid->>'tier')::integer END),
        reward_response=COALESCE(reward_response, CASE
            WHEN jsonb_typeof(reward_paid->'reward')='object'
            THEN reward_paid->'reward' END),
        reward_requested_at=COALESCE(reward_requested_at,timezone('UTC',now()))
    WHERE reward_paid IS NOT NULL;
-- An older pending marker without a confirmed response cannot prove whether
-- its POST already succeeded. Quarantine it until the outcome is reconciled.
UPDATE social.referral_progress SET reward_requested_at=timezone('UTC',now())
    WHERE reward_tier IS NOT NULL AND reward_requested_at IS NULL;
