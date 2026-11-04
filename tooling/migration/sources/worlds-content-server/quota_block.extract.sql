SELECT b.wallet, b.created_at, b.updated_at
FROM (
    SELECT lower(wallet) AS wallet,
           created_at AT TIME ZONE 'UTC' AS created_at,
           updated_at AT TIME ZONE 'UTC' AS updated_at,
           row_number() OVER (PARTITION BY lower(wallet) ORDER BY updated_at DESC, wallet) AS rn
    FROM blocked
    WHERE lower(wallet) ~ '^0x[0-9a-f]{40}$'
) b
WHERE b.rn = 1
