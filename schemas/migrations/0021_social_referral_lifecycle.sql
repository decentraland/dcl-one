-- Preserve populated attribution rows and source identifiers at cutover.
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS id uuid NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS invited_user_ip text;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS signed_up_at timestamp without time zone;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS tier_granted boolean NOT NULL DEFAULT false;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS tier_granted_at timestamp without time zone;
ALTER TABLE social.referral_progress ADD COLUMN IF NOT EXISTS login_days date[] NOT NULL DEFAULT '{}';
UPDATE social.referral_progress SET tier_granted = true WHERE status = 'tier_granted' AND NOT tier_granted;
CREATE UNIQUE INDEX IF NOT EXISTS referral_progress_id_key ON social.referral_progress (id);
CREATE INDEX IF NOT EXISTS referral_progress_ip_idx ON social.referral_progress (referrer, invited_user_ip);
CREATE INDEX IF NOT EXISTS referral_progress_granted_idx ON social.referral_progress (referrer) WHERE tier_granted;
CREATE TABLE IF NOT EXISTS social.referral_reward_images (
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    referrer text NOT NULL,
    reward_image_url text NOT NULL,
    tier integer NOT NULL CHECK (tier > 0),
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT referral_reward_images_pkey PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS referral_reward_images_referrer_idx ON social.referral_reward_images (referrer);
