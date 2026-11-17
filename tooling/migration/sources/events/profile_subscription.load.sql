-- The target retains these API rows without sending browser pushes. With no
-- source timestamp, import only while target subscription writes are disabled.
INSERT INTO explore.event_profile_subscriptions (endpoint, "user", p256dh, auth)
SELECT endpoint, "user", p256dh, auth FROM stage
ON CONFLICT (endpoint) DO UPDATE SET
    "user" = EXCLUDED."user",
    p256dh = EXCLUDED.p256dh,
    auth = EXCLUDED.auth;
