\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $$
DECLARE
    kept int;
    keyed int;
BEGIN
    IF to_regclass('meta.schema_steps') IS NULL THEN
        RAISE EXCEPTION 'not a migrated catalyst database (no meta.schema_steps)';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM meta.schema_steps WHERE step = '0062_builder_item_contents_order') THEN
        RAISE EXCEPTION 'run catalyst schema migrate with the binary that keeps item documents in order first';
    END IF;
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE contype = 'f' AND confrelid = 'builder.uploaded_contents'::regclass) THEN
        RAISE EXCEPTION 'a table still names builder.uploaded_contents by key: run schemas/releases/builder-scenes-retired.sql first';
    END IF;
    SELECT count(*) INTO kept
    FROM information_schema.columns
    WHERE table_schema = 'builder' AND table_name = 'items'
      AND column_name IN ('data', 'metrics') AND data_type = 'json';
    SELECT count(*) INTO keyed
    FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey)
    WHERE c.conname = 'uploaded_contents_pkey' AND c.conrelid = 'builder.uploaded_contents'::regclass;
    IF kept = 2 AND keyed = 2 THEN
        RAISE NOTICE 'the builder item documents are json and uploads are kept per uploader already';
        RETURN;
    END IF;
    IF kept = 1 THEN
        RAISE EXCEPTION 'only 1 of the 2 builder item documents is json: restore the backup or retype the other by hand';
    END IF;
    IF kept = 0 THEN
        ALTER TABLE builder.items DROP CONSTRAINT items_data_object_check;
        ALTER TABLE builder.items DROP CONSTRAINT items_metrics_object_check;
        ALTER TABLE builder.items ALTER COLUMN data DROP DEFAULT;
        ALTER TABLE builder.items ALTER COLUMN data TYPE json USING data::json;
        ALTER TABLE builder.items ALTER COLUMN data SET DEFAULT '{}'::json;
        ALTER TABLE builder.items ALTER COLUMN metrics TYPE json USING metrics::json;
        ALTER TABLE builder.items ADD CONSTRAINT items_data_object_check CHECK (json_typeof(data) = 'object');
        ALTER TABLE builder.items ADD CONSTRAINT items_metrics_object_check CHECK (metrics IS NULL OR json_typeof(metrics) = 'object');
    END IF;
    IF keyed <> 2 THEN
        ALTER TABLE builder.uploaded_contents DROP CONSTRAINT uploaded_contents_pkey;
        ALTER TABLE builder.uploaded_contents ADD CONSTRAINT uploaded_contents_pkey PRIMARY KEY (hash, uploaded_by);
    END IF;
END $$;
COMMIT;
