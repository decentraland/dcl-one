\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $$
BEGIN
    IF to_regclass('meta.schema_steps') IS NULL OR NOT EXISTS (
        SELECT 1 FROM meta.schema_steps WHERE step = '0072_builder_collection_forum_claim'
    ) THEN
        RAISE EXCEPTION 'run catalyst schema migrate with forum claims before retiring the linked forum column';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'builder' AND table_name = 'linked_collection_publications'
          AND column_name = 'forum_url'
    ) THEN
        RETURN;
    END IF;
    LOCK TABLE builder.collections, builder.linked_collection_publications IN SHARE ROW EXCLUSIVE MODE;
    EXECUTE $copy$
        UPDATE builder.collections c SET forum_link = p.forum_url, forum_claimed_at = NULL
        FROM builder.linked_collection_publications p
        WHERE p.collection_id = c.id AND p.forum_url IS NOT NULL AND c.forum_link IS NULL
    $copy$;
    ALTER TABLE builder.linked_collection_publications DROP CONSTRAINT IF EXISTS linked_collection_publications_forum_state_check;
    ALTER TABLE builder.linked_collection_publications DROP COLUMN forum_url;
END $$;
COMMIT;
