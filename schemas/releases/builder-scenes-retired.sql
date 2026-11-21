\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $$
DECLARE
    present int;
BEGIN
    IF to_regclass('meta.schema_steps') IS NULL THEN
        RAISE EXCEPTION 'not a migrated catalyst database (no meta.schema_steps)';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM meta.schema_steps WHERE step = '0054_builder_scenes_retired') THEN
        RAISE EXCEPTION 'run catalyst schema migrate with the binary without the scene routes first';
    END IF;
    SELECT count(*) INTO present
    FROM unnest(ARRAY['builder.projects', 'builder.project_manifests', 'builder.project_files',
                      'builder.asset_packs', 'builder.assets', 'builder.asset_contents',
                      'builder.asset_catalog']) AS t(name)
    WHERE to_regclass(t.name) IS NOT NULL;
    IF present = 0 THEN
        RAISE NOTICE 'the builder scene tables are already gone';
        RETURN;
    END IF;
    IF present <> 7 THEN
        RAISE EXCEPTION 'only % of the 7 builder scene tables remain: restore the backup or drop them by hand', present;
    END IF;
    IF EXISTS (SELECT 1 FROM builder.projects WHERE updated_at > timezone('UTC', now()) - interval '10 minutes')
       OR EXISTS (SELECT 1 FROM builder.asset_packs WHERE updated_at > timezone('UTC', now()) - interval '10 minutes') THEN
        RAISE EXCEPTION 'a scene project or asset pack changed in the last ten minutes: an old serve is still running';
    END IF;
    CREATE TEMP TABLE retired_scene_uploads ON COMMIT DROP AS
        SELECT hash FROM builder.project_files
        UNION SELECT thumbnail FROM builder.projects WHERE thumbnail IS NOT NULL
        UNION SELECT thumbnail FROM builder.asset_packs WHERE thumbnail IS NOT NULL
        UNION SELECT thumbnail FROM builder.assets WHERE thumbnail IS NOT NULL
        UNION SELECT hash FROM builder.asset_contents;
    DROP TABLE builder.asset_contents, builder.assets, builder.asset_packs, builder.project_files,
        builder.project_manifests, builder.projects, builder.asset_catalog;
    DELETE FROM builder.uploaded_contents u
    USING (SELECT hash FROM retired_scene_uploads
           EXCEPT SELECT hash FROM builder.item_contents
           EXCEPT SELECT unnest(ARRAY[thumbnail, video, local_content_hash, content_hash,
                                      catalyst_content_hash])
                  FROM builder.items) r
    WHERE u.hash = r.hash;
END $$;
COMMIT;
