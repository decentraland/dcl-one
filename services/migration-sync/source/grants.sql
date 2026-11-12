-- Source-only permissions. Run after explicit installation while writers remain fenced.
-- Repeat for EVERY application/job/operator writer role. Does not create roles.
\ir roles-config.sql
BEGIN;
SELECT format('REVOKE ALL ON SCHEMA %I FROM PUBLIC', :'capture_schema') \gexec
SELECT format('REVOKE ALL ON ALL TABLES IN SCHEMA %I FROM PUBLIC', :'capture_schema') \gexec
SELECT format('REVOKE ALL ON ALL SEQUENCES IN SCHEMA %I FROM PUBLIC', :'capture_schema') \gexec
SELECT format('REVOKE EXECUTE ON ALL FUNCTIONS IN SCHEMA %I FROM PUBLIC', :'capture_schema') \gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'reader_role') \gexec
SELECT format('GRANT USAGE ON SCHEMA public, %I TO %I, %I', :'capture_schema', :'writer_role', :'reader_role') \gexec
SELECT format('GRANT SELECT ON %I.control, %I.scope, %I.transactions TO %I', :'capture_schema', :'capture_schema', :'capture_schema', :'writer_role') \gexec
SELECT format('GRANT UPDATE ON %I.control TO %I', :'capture_schema', :'writer_role') \gexec
SELECT format('GRANT INSERT ON %I.transactions, %I.changes TO %I', :'capture_schema', :'capture_schema', :'writer_role') \gexec
SELECT format('GRANT USAGE ON ALL SEQUENCES IN SCHEMA %I TO %I', :'capture_schema', :'writer_role') \gexec
SELECT format('GRANT EXECUTE ON FUNCTION %s TO %I', p.oid::regprocedure, :'writer_role')
FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
WHERE n.nspname=:'capture_schema' AND p.proname NOT IN ('execute_favorite','record_command') \gexec
SELECT format('GRANT SELECT ON ALL TABLES IN SCHEMA %I TO %I', :'capture_schema', :'reader_role') \gexec
SELECT format('GRANT SELECT ON public.%I TO %I', name, :'reader_role')
FROM unnest(CASE :'source' WHEN 'places' THEN ARRAY['places','user_favorites','user_likes']
 WHEN 'events' THEN ARRAY['schedule'] ELSE ARRAY['social_settings'] END) AS names(name) \gexec
SELECT format('GRANT EXECUTE ON FUNCTION %s TO %I', p.oid::regprocedure, :'reader_role')
FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
WHERE n.nspname=:'capture_schema' AND p.proname='catalogue_document' \gexec
COMMIT;
