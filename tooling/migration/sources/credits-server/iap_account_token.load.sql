-- A token is a one-way derivation of its wallet, so the node's own row for
-- it names the same wallet; the first one written is kept.
INSERT INTO credits.iap_account_tokens (token, address, created_at)
SELECT token, address, created_at AT TIME ZONE 'UTC'
FROM stage
ON CONFLICT (token) DO NOTHING;
