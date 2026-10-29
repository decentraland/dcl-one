-- credits.authorizations became ONE credit per settlement group (the upstream
-- credits protocol, 2026-09-26; src/shop/credits/{authorize,batch,settlement}.rs):
-- the chain and manager it was signed against, the contract's credit hash
-- (the revocation key), the quoted total the signed value was sized from, the
-- oracle rate, the transaction the client reported submitting it in, and the
-- `retired` status (a cancelled credit stays pledged until the chain has been
-- read past its expiry). All declared on the table in schema/60_ops.sql; a
-- database stamped before them takes them here. The new relations
-- (authorization_lines, usage_settlements, revocations) and indexes come from
-- the object pass. Existing rows keep NULLs: none was signed against the
-- contract digest (docs/shop/CREDIT-HASH.md), so none can be consumed.
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS chain_id bigint;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS contract text;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS credit_hash text;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS quoted_cents bigint;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS oracle_rate numeric(78,0);
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS oracle_decimals smallint;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS submitted_tx_hash text;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS submitted_at timestamp without time zone;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS retired_at timestamp without time zone;
ALTER TABLE credits.authorizations ADD COLUMN IF NOT EXISTS consumed_at timestamp without time zone;
ALTER TABLE credits.authorizations DROP CONSTRAINT IF EXISTS authorizations_status_check;
ALTER TABLE credits.authorizations ADD CONSTRAINT authorizations_status_check CHECK (status IN ('authorized', 'retired', 'released', 'consumed', 'expired'));
ALTER TABLE credits.authorizations ADD CONSTRAINT authorizations_credit_hash_check CHECK (credit_hash IS NULL OR credit_hash ~ '^0x[0-9a-f]{64}$');
ALTER TABLE credits.authorizations ADD CONSTRAINT authorizations_submitted_tx_check CHECK (submitted_tx_hash IS NULL OR submitted_tx_hash ~ '^0x[0-9a-f]{64}$');
DROP INDEX IF EXISTS credits.authorizations_expiry_idx;
