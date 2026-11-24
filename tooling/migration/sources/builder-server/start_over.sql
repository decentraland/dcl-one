\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
LOCK TABLE builder.collections, builder.items, builder.uploaded_contents, builder.item_contents,
           builder.virtual_third_parties, builder.collection_curations, builder.item_curations,
           builder.collection_events, builder.slot_usage_cheques, builder.collection_publications,
           builder.validation_requests IN ACCESS EXCLUSIVE MODE;
\ir fingerprint.sql
\ir node_untouched.sql
DO $start_over$ BEGIN
  IF EXISTS (SELECT 1 FROM builder.collection_publications) OR EXISTS (SELECT 1 FROM builder.validation_requests) THEN
    RAISE EXCEPTION 'builder-server: the node holds collection publications or validation requests, so it has taken builder writes and cannot start over';
  END IF; END $start_over$;
TRUNCATE builder.collections, builder.items, builder.item_contents, builder.uploaded_contents,
         builder.virtual_third_parties, builder.collection_curations, builder.item_curations,
         builder.collection_events, builder.slot_usage_cheques, builder.collection_publications,
         builder.validation_requests;
DELETE FROM dcl_migration.builder_imported;
DELETE FROM dcl_migration.checkpoint WHERE source = 'builder-server';
COMMIT;
