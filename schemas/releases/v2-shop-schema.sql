-- Schema release v2 (2026-09-26): the Shop schema (DATABASE.md, "Bootstrap, schema versions and local writes";
-- deploy/README.md, "Data-safe schema releases", point 3). Run by hand with
-- psql, ON_ERROR_STOP, under catalyst.service stopped, after a catalyst-backup
-- and a catalyst-verify-restore, on a database stamped version 1; then
-- `catalyst schema migrate` (the activation's oneshot) adds what version 2's
-- layout gained since. Renames the schema `marketplace` to `shop` (its tables,
-- types, indexes and sequences follow) and moves the four checkout ledgers
-- out of `economy`; no row, column or index changes. Idempotent by the
-- version guard: a second run raises and applies nothing.
BEGIN;
DO $$
BEGIN
    IF (SELECT version FROM meta.schema WHERE id = 1 AND identity = 'catalyst') IS DISTINCT FROM 1 THEN
        RAISE EXCEPTION 'schema release v2 applies to a catalyst database stamped version 1 (found %)',
            (SELECT identity || '/' || version FROM meta.schema WHERE id = 1);
    END IF;
END $$;
ALTER SCHEMA marketplace RENAME TO shop;
ALTER TABLE economy.broker_purchases SET SCHEMA shop;
ALTER TABLE economy.escrow_actions SET SCHEMA shop;
ALTER TABLE economy.name_transfers SET SCHEMA shop;
ALTER TABLE economy.usage_grants SET SCHEMA shop;
UPDATE meta.schema SET version = 2, applied_at = timezone('UTC', now()) WHERE id = 1;
COMMIT;
