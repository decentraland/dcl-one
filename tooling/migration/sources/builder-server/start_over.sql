\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '10s';
DO $prerequisite$ BEGIN
  IF to_regclass('builder.linked_collection_publications') IS NOT NULL THEN
    RAISE EXCEPTION 'builder-server: builder.linked_collection_publications still references builder.collections; run schemas/releases/builder-linked-publications-retired.sql before starting over';
  END IF; END $prerequisite$;
LOCK TABLE builder.collections, builder.items, builder.uploaded_contents, builder.item_contents,
           builder.virtual_third_parties, builder.collection_curations, builder.item_curations,
           builder.collection_events, builder.slot_usage_cheques, builder.collection_publications,
           builder.validation_requests IN ACCESS EXCLUSIVE MODE;
\ir fingerprint.sql
\if :{?unrecorded_import}
DO $unrecorded$ BEGIN
  IF EXISTS (SELECT 1 FROM dcl_migration.builder_imported) THEN
    RAISE EXCEPTION 'builder-server: unrecorded_import is for a node whose import kept no ledger, and dcl_migration.builder_imported holds rows; start over without it';
  END IF; END $unrecorded$;
\else
\ir node_untouched.sql
\endif
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
