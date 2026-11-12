-- Read-only checks after installation/grants; run as the installation owner.
-- Does not validate deployed schemas, payload completeness, or unknown writers.
\ir roles-config.sql
BEGIN READ ONLY;
-- Turn checks into a single fail-closed result below; missing objects abort first.
SELECT format('SELECT count(*)=1 AND bool_and(version=%s) AS control_ok FROM %I.control', :adapter_version, :'capture_schema') \gset control_query_
:control_query_format \gset
SELECT format('SELECT count(*) BETWEEN 1 AND 1000 AS scope_ok FROM %I.scope', :'capture_schema') \gset scope_query_
:scope_query_format \gset
WITH business AS (
 SELECT to_regclass('public.' || name)::oid AS oid
 FROM unnest(CASE :'source' WHEN 'places' THEN ARRAY['places','user_favorites','user_likes']
 WHEN 'events' THEN ARRAY['schedule'] ELSE ARRAY['social_settings'] END) names(name)
), capture AS (
 SELECT c.oid FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
 WHERE n.nspname=:'capture_schema' AND c.relkind IN ('r','p')
), relations AS (SELECT oid FROM business UNION SELECT oid FROM capture)
SELECT :'control_ok'::boolean AND :'scope_ok'::boolean
 AND NOT EXISTS (SELECT FROM business WHERE oid IS NULL)
 AND has_schema_privilege(:'writer_role', :'capture_schema', 'USAGE')
 AND has_schema_privilege(:'reader_role', :'capture_schema', 'USAGE')
 AND has_schema_privilege(:'reader_role', 'public', 'USAGE')
 AND has_database_privilege(:'reader_role', current_database(), 'CONNECT')
 AND NOT has_schema_privilege(:'reader_role', :'capture_schema', 'CREATE')
 AND NOT has_schema_privilege(:'reader_role', 'public', 'CREATE')
 AND NOT EXISTS (SELECT FROM pg_auth_members m JOIN pg_roles r ON r.oid=m.member WHERE r.rolname=:'reader_role')
 AND NOT EXISTS (SELECT FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database() AND r.rolname=:'reader_role')
 AND NOT EXISTS (SELECT FROM pg_roles WHERE rolname=:'reader_role' AND (rolsuper OR rolbypassrls OR rolcreaterole OR rolcreatedb OR rolreplication))
 AND NOT EXISTS (SELECT FROM relations r JOIN pg_class c ON c.oid=r.oid WHERE c.relrowsecurity
   OR NOT has_table_privilege(:'reader_role',r.oid,'SELECT')
   OR has_table_privilege(:'reader_role',r.oid,'INSERT,UPDATE,DELETE,TRUNCATE,TRIGGER')
   OR has_any_column_privilege(:'reader_role',r.oid,'INSERT,UPDATE'))
 AND has_table_privilege(:'writer_role',format('%I.control',:'capture_schema'),'SELECT')
 AND has_table_privilege(:'writer_role',format('%I.control',:'capture_schema'),'UPDATE')
 AND has_table_privilege(:'writer_role',format('%I.scope',:'capture_schema'),'SELECT')
 AND has_table_privilege(:'writer_role',format('%I.transactions',:'capture_schema'),'SELECT')
 AND has_table_privilege(:'writer_role',format('%I.transactions',:'capture_schema'),'INSERT')
 AND has_table_privilege(:'writer_role',format('%I.changes',:'capture_schema'),'INSERT')
 AND NOT EXISTS (SELECT FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
   WHERE n.nspname=:'capture_schema' AND c.relkind='S'
   AND (NOT has_sequence_privilege(:'writer_role',c.oid,'USAGE') OR has_sequence_privilege(:'reader_role',c.oid,'USAGE,UPDATE')))
 AND NOT EXISTS (SELECT FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
   WHERE n.nspname=:'capture_schema' AND (p.prosecdef
    OR (p.proname NOT IN ('execute_favorite','record_command') AND NOT has_function_privilege(:'writer_role',p.oid,'EXECUTE'))
    OR (p.proname='catalogue_document' AND NOT has_function_privilege(:'reader_role',p.oid,'EXECUTE'))
    OR (p.proname<>'catalogue_document' AND has_function_privilege(:'reader_role',p.oid,'EXECUTE'))))
 AND (SELECT count(*) FROM pg_trigger t JOIN pg_proc p ON p.oid=t.tgfoid JOIN pg_namespace n ON n.oid=p.pronamespace
      WHERE n.nspname=:'capture_schema' AND t.tgrelid IN (SELECT oid FROM business) AND t.tgenabled='O')
     = CASE :'source' WHEN 'places' THEN 9 WHEN 'events' THEN 3 ELSE 2 END AS preflight_ok \gset
ROLLBACK;
\if :preflight_ok
  \echo 'Source capture permission/trigger preflight passed for the specified writer and reader'
\else
  \echo 'Source preflight failed; keep writers fenced and inspect adapter, scope, triggers and effective grants'
  SELECT 1 / 0 AS preflight_failure;
\endif
