\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $$
DECLARE
    unfit bigint;
    submitted bigint;
    copied bigint;
BEGIN
    IF to_regclass('meta.schema_steps') IS NULL OR NOT EXISTS (
        SELECT 1 FROM meta.schema_steps WHERE step = '0068_builder_linked_publications_retired'
    ) THEN
        RAISE EXCEPTION 'run catalyst schema migrate with third-party publish before retiring the linked publications';
    END IF;
    IF to_regclass('builder.linked_collection_publications') IS NULL THEN
        RAISE NOTICE 'the builder linked publications are already gone';
        RETURN;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'builder' AND table_name = 'linked_collection_publications'
          AND column_name = 'forum_url'
    ) THEN
        RAISE EXCEPTION 'run builder-linked-forum-retired.sql first';
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'builder' AND table_name = 'collections'
          AND column_name IN ('is_published', 'is_approved')
    ) THEN
        RAISE EXCEPTION 'run builder-collection-flags-retired.sql first';
    END IF;
    LOCK TABLE builder.collections, builder.linked_collection_publications, builder.slot_usage_cheques
        IN SHARE ROW EXCLUSIVE MODE;
    EXECUTE $q$
        SELECT count(*) FROM builder.linked_collection_publications p
        JOIN builder.collections c ON c.id = p.collection_id
        WHERE p.status = 'submitted' AND (c.third_party_id IS NULL
          OR lower(p.cheque->>'salt') !~ '^0x[0-9a-f]{64}$'
          OR lower(p.cheque->>'signature') !~ '^0x[0-9a-f]{128}(00|01|1b|1c)$'
          OR jsonb_typeof(p.cheque->'qty') IS DISTINCT FROM 'number'
          OR (p.cheque->>'qty')::numeric NOT BETWEEN 1 AND 2147483647
          OR (p.cheque->>'qty')::numeric <> trunc((p.cheque->>'qty')::numeric)) IS NOT FALSE
    $q$ INTO unfit;
    IF unfit > 0 THEN
        RAISE EXCEPTION '% submitted linked publications hold a cheque a slot cheque row cannot keep', unfit;
    END IF;
    EXECUTE $q$
        INSERT INTO builder.slot_usage_cheques (id, collection_id, third_party_id, signature, salt, qty, created_at, updated_at)
        SELECT gen_random_uuid(), p.collection_id, c.third_party_id,
               left(lower(p.cheque->>'signature'), 130) ||
                 CASE right(lower(p.cheque->>'signature'), 2) WHEN '00' THEN '1b' WHEN '01' THEN '1c'
                   ELSE right(lower(p.cheque->>'signature'), 2) END,
               lower(p.cheque->>'salt'), (p.cheque->>'qty')::int, p.updated_at, p.updated_at
        FROM builder.linked_collection_publications p
        JOIN builder.collections c ON c.id = p.collection_id
        WHERE p.status = 'submitted'
        ON CONFLICT (third_party_id, salt) DO NOTHING
    $q$;
    GET DIAGNOSTICS copied = ROW_COUNT;
    EXECUTE $q$
        SELECT count(*) FROM builder.linked_collection_publications WHERE status = 'submitted'
    $q$ INTO submitted;
    IF copied < submitted THEN
        RAISE NOTICE '% submitted cheques were not copied: their provider already holds a cheque with the same salt',
            submitted - copied;
    END IF;
    UPDATE builder.collections SET publication_pending = false
    WHERE third_party_id IS NOT NULL AND publication_pending;
    DROP TABLE builder.linked_collection_publications;
END $$;
COMMIT;
