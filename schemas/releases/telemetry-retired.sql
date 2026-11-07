\set ON_ERROR_STOP on
-- Retires the telemetry sink's schema: the Sentry/Segment sinks, their
-- console, the retention job and the Shop analytics sink wrote it, and no
-- binary reads it once `ops::telemetry` is gone. Precondition: every `serve`
-- of the old build is stopped before this runs; an old replica left running
-- keeps its ingest writer and `telemetry-purge` job writing the schema this
-- drops. Run it once, with the administrative role, after the binary without
-- the sink runs on every replica; it refuses a database without a stamp and
-- notes a schema that is already gone.
--   Dropped (CASCADE), 22 objects:
--   schema  telemetry
--   tables  telemetry.events, issue_state, issue_merges, release_state,
--           ingest_settings, project_quotas, experiment_overrides,
--           flag_overrides, flag_groups, flag_group_targets,
--           experiment_group_targets, product_areas
--   indexes events_received_at_idx, events_source_kind_received_idx,
--           events_fingerprint_received_idx, events_level_received_idx,
--           events_experiments_idx, events_scene_errors_genesis_idx,
--           events_scene_errors_world_idx, events_client_error_received_idx,
--           issue_merges_canonical_idx
-- Kept: ops.admin_audit, its rows with domain 'telemetry' and the CHECK value
-- that admits them. The read-only role the deployment created for the
-- console's SQL reader (USAGE + SELECT on schema telemetry) loses its grants
-- with the schema; dropping the role is the deployment's step.
-- The drop is not reversible: rollback needs the matching backup.
--   psql "$DATABASE_URL" -X -f schemas/releases/telemetry-retired.sql
BEGIN;
DO $$
BEGIN
    IF to_regclass('meta.schema') IS NULL THEN
        RAISE EXCEPTION 'not a stamped catalyst database (no meta.schema)';
    END IF;
    IF to_regnamespace('telemetry') IS NULL THEN
        RAISE NOTICE 'schema telemetry is already gone';
    END IF;
END $$;
DROP SCHEMA IF EXISTS telemetry CASCADE;
COMMIT;
