-- Live multi-account flags. Captcha flags are short-lived claim throttles
-- (expires_at set) and are not imported.
SELECT lower(f.address) AS address,
       f.reason ->> 'ip' AS ip,
       ARRAY(SELECT lower(x)
             FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(f.reason -> 'linkedAddresses') = 'array'
                                                 THEN f.reason -> 'linkedAddresses' ELSE '[]'::jsonb END) x
       ) AS linked_addresses,
       f.flagged_at AT TIME ZONE 'UTC' AS flagged_at
FROM flagged_wallets f
WHERE f.flag_type = 'multi-account'
  AND (f.expires_at IS NULL OR f.expires_at > timezone('UTC', now()))
