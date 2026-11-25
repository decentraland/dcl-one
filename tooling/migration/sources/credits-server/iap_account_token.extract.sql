-- The wallet behind each App Store account token upstream registered.
SELECT lower(app_account_token) AS token,
       lower(wallet) AS address,
       to_timestamp(created_at / 1000.0) AS created_at
FROM iap_account_tokens
