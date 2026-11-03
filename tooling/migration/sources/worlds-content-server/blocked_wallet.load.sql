INSERT INTO worlds.blocked_wallets (wallet, created_at, updated_at)
SELECT s.wallet, s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
FROM stage s
ON CONFLICT (wallet) DO UPDATE SET
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at;
