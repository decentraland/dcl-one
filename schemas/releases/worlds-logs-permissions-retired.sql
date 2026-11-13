\set ON_ERROR_STOP on
-- Retires the storage collaborator function `worlds.logs_permissions` and its
-- two GIN indexes, superseded by `worlds.logs_permissions_decoded` and its
-- `_decoded_idx` pair. The object pass creates the new objects and leaves the
-- old ones, which `catalyst schema status` does not report. Safe once every
-- running binary queries the decoded function: an older `serve` still calls
-- `worlds.logs_permissions` from its storage grant and `GET /collaborator`,
-- which fail after the drop. Run it once, with the administrative role, after
-- that binary runs on every replica. It refuses a database without the decoded
-- function and both of its indexes built and valid (a failed concurrent build
-- leaves an invalid one), then drops each old index concurrently, so reads and
-- writes on both tables continue; each drop waits at most 10 s for its locks.
-- It runs outside a transaction: an interrupted run may leave an old index
-- invalid, and running it again finishes the drop. A no-op once the objects
-- are gone.
--   Dropped:
--   DROP INDEX worlds.world_scenes_logs_permissions_idx
--   DROP INDEX content.deployments_scene_logs_permissions_idx
--   DROP FUNCTION worlds.logs_permissions(text)
-- Rollback: re-create the three objects from the previous `schemas/20_worlds.sql`.
--   psql "$DATABASE_URL" -X -f schemas/releases/worlds-logs-permissions-retired.sql
SET lock_timeout = '10s';

DO $$
BEGIN
    IF to_regprocedure('worlds.logs_permissions_decoded(text)') IS NULL
       OR NOT EXISTS (
           SELECT 1 FROM pg_index
           WHERE indexrelid = to_regclass('worlds.world_scenes_logs_permissions_decoded_idx')
             AND indisvalid AND indisready)
       OR NOT EXISTS (
           SELECT 1 FROM pg_index
           WHERE indexrelid = to_regclass('content.deployments_scene_logs_permissions_decoded_idx')
             AND indisvalid AND indisready) THEN
        RAISE EXCEPTION 'run catalyst schema migrate with the binary using worlds.logs_permissions_decoded first, and rebuild an invalid _decoded_idx index';
    END IF;
    IF to_regprocedure('worlds.logs_permissions(text)') IS NULL THEN
        RAISE NOTICE 'worlds.logs_permissions is already gone';
    END IF;
END $$;

DROP INDEX CONCURRENTLY IF EXISTS worlds.world_scenes_logs_permissions_idx;

DROP INDEX CONCURRENTLY IF EXISTS content.deployments_scene_logs_permissions_idx;

DROP FUNCTION IF EXISTS worlds.logs_permissions(text);
