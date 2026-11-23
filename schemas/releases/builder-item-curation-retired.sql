\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $$
DECLARE
    present int;
    unhashed bigint;
    recorded bigint;
    unbacked bigint;
BEGIN
    IF to_regclass('meta.schema_steps') IS NULL THEN
        RAISE EXCEPTION 'not a migrated catalyst database (no meta.schema_steps)';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM meta.schema_steps WHERE step = '0065_builder_collection_forum') THEN
        RAISE EXCEPTION 'run catalyst schema migrate with the binary that keeps item curations first';
    END IF;
    SELECT count(*) INTO present
    FROM information_schema.columns
    WHERE table_schema = 'builder' AND table_name = 'items'
      AND column_name IN ('curation_status', 'is_approved');
    IF present = 0 THEN
        RAISE NOTICE 'the builder item curation columns are already gone';
        RETURN;
    END IF;
    IF present <> 2 THEN
        RAISE EXCEPTION 'only % of the 2 builder item curation columns remain: restore the backup or drop it by hand', present;
    END IF;
    LOCK TABLE builder.items, builder.item_curations IN SHARE ROW EXCLUSIVE MODE;
    SELECT count(*) INTO unhashed
    FROM builder.items i JOIN builder.collections c ON c.id = i.collection_id
    WHERE c.third_party_id IS NOT NULL AND i.is_published
      AND NOT EXISTS (SELECT 1 FROM builder.item_curations k WHERE k.item_id = i.id)
      AND (i.local_content_hash IS NULL OR i.local_content_hash !~ '^[0-9a-f]{64}$');
    IF unhashed > 0 THEN
        RAISE EXCEPTION '% third-party items submitted here have no curation and no keccak content hash to record one with', unhashed;
    END IF;
    EXECUTE $q$
        INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at)
        SELECT gen_random_uuid(), i.id, i.curation_status, i.local_content_hash, i.mappings IS NOT NULL,
               i.updated_at, i.updated_at
        FROM builder.items i JOIN builder.collections c ON c.id = i.collection_id
        WHERE c.third_party_id IS NOT NULL AND i.is_published
          AND NOT EXISTS (SELECT 1 FROM builder.item_curations k WHERE k.item_id = i.id)
    $q$;
    GET DIAGNOSTICS recorded = ROW_COUNT;
    RAISE NOTICE '% third-party items submitted here now have the curation they recorded', recorded;
    EXECUTE $q$
        SELECT count(*) FROM builder.items i
        WHERE i.curation_status <> 'pending'
          AND NOT EXISTS (SELECT 1 FROM builder.item_curations k WHERE k.item_id = i.id)
    $q$ INTO unbacked;
    RAISE NOTICE '% items lose a stored curation status that no curation row holds', unbacked;
    ALTER TABLE builder.items DROP COLUMN curation_status, DROP COLUMN is_approved;
END $$;
COMMIT;
