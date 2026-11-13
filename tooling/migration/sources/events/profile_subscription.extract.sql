-- Browser subscriptions have no source timestamps; preserve endpoint identity and keys.
SELECT endpoint, lower("user") AS "user", p256dh, auth
FROM profile_subscriptions
