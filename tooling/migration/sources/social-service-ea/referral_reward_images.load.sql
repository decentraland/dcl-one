-- Immutable historical reward snapshots; importing them issues no reward.
INSERT INTO social.referral_reward_images (id, referrer, reward_image_url, tier, created_at)
SELECT id, referrer, reward_image_url, tier, created_at AT TIME ZONE 'UTC' FROM stage
ON CONFLICT (id) DO NOTHING;
