-- Shared psql input validation. Run via grants.sql or preflight.sql, not alone.
\set ON_ERROR_STOP on
\if :{?source}
\else
  \echo 'Required: -v source=places|events|social-settings'
  SELECT 1 / 0 AS preflight_failure;
\endif
\if :{?writer_role}
\else
  \echo 'Required: -v writer_role=<existing source writer>'
  SELECT 1 / 0 AS preflight_failure;
\endif
\if :{?reader_role}
\else
  \echo 'Required: -v reader_role=<existing dedicated capture reader>'
  SELECT 1 / 0 AS preflight_failure;
\endif
SELECT :'source' IN ('places','events','social-settings')
 AND :'writer_role' <> :'reader_role'
 AND EXISTS (SELECT FROM pg_roles WHERE rolname=:'writer_role')
 AND EXISTS (SELECT FROM pg_roles WHERE rolname=:'reader_role') AS valid \gset
\if :valid
\else
  \echo 'Unknown source, missing roles, or shared reader/writer role'
  SELECT 1 / 0 AS preflight_failure;
\endif
SELECT CASE :'source'
 WHEN 'places' THEN 'migration_capture'
 WHEN 'events' THEN 'migration_capture_events'
 ELSE 'migration_capture_social_settings' END AS capture_schema,
 CASE WHEN :'source'='places' THEN 3 ELSE 1 END AS adapter_version \gset
