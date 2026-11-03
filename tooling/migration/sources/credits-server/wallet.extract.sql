-- One row per wallet in the program. opted_out_at ends participation;
-- a live captcha or multi-account flag blocks claiming (expiry evaluated on
-- the source clock at extract time).
SELECT lower(u.address) AS address,
       (u.opted_out_at IS NULL) AS has_started_program,
       EXISTS (SELECT 1 FROM flagged_wallets f
                WHERE lower(f.address) = lower(u.address)
                  AND (f.expires_at IS NULL OR f.expires_at > timezone('UTC', now()))) AS is_blocked_for_claiming,
       u.created_at AT TIME ZONE 'UTC' AS created_at,
       u.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM users u
