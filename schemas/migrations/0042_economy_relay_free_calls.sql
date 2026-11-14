-- economy.transactions marks the relayed calls that are no sale or credits spend
-- (free_call), the only ones the relayer's daily budget counts
-- (services/catalyst/shop/src/evm/transaction.rs). schemas/31_economy.sql declares
-- the column; a database that created the table before it gains it here (IF EXISTS: a
-- database stamped before the table gains it from the missing-objects pass).
ALTER TABLE IF EXISTS economy.transactions ADD COLUMN IF NOT EXISTS free_call boolean NOT NULL DEFAULT false;
