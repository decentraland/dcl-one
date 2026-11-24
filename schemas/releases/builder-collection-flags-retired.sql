\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $$
DECLARE
    present int;
    unbacked bigint;
BEGIN
    IF to_regclass('meta.schema_steps') IS NULL THEN
        RAISE EXCEPTION 'not a migrated catalyst database (no meta.schema_steps)';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM meta.schema_steps WHERE step = '0064_builder_collection_flags_retired') THEN
        RAISE EXCEPTION 'run catalyst schema migrate with the binary that derives collection state first';
    END IF;
    SELECT count(*) INTO present
    FROM information_schema.columns
    WHERE table_schema = 'builder' AND table_name = 'collections'
      AND column_name IN ('is_published', 'is_approved');
    IF present = 0 THEN
        RAISE NOTICE 'the builder collection flags are already gone';
        RETURN;
    END IF;
    IF present <> 2 THEN
        RAISE EXCEPTION 'only % of the 2 builder collection flags remain: restore the backup or drop it by hand', present;
    END IF;
    EXECUTE $q$
        SELECT count(*) FROM builder.collections c
        WHERE c.is_published
          AND NOT EXISTS (SELECT 1 FROM shop.collections s
                          WHERE s.network = 'matic' AND s.contract_address = lower(c.contract_address))
          AND NOT EXISTS (SELECT 1 FROM builder.collection_publications p
                          WHERE p.collection_id = c.id AND p.status = 'published')
          AND NOT EXISTS (SELECT 1 FROM builder.linked_collection_publications l
                          WHERE l.collection_id = c.id AND l.status = 'submitted')
          AND NOT EXISTS (SELECT 1 FROM builder.items i
                          WHERE i.collection_id = c.id AND i.blockchain_item_id IS NOT NULL)
    $q$ INTO unbacked;
    IF unbacked > 0 THEN
        RAISE EXCEPTION '% collections are flagged published with no copy row, receipt, submitted linked publication or minted item: the derived state would read them as drafts', unbacked;
    END IF;
    ALTER TABLE builder.collections DROP COLUMN is_published, DROP COLUMN is_approved;
END $$;
COMMIT;
