SELECT id, lower(referrer) AS referrer, reward_image_url, tier,
       to_timestamp(created_at / 1000.0) AS created_at
FROM referral_reward_images
WHERE lower(referrer) ~ '^0x[0-9a-f]{40}$'
