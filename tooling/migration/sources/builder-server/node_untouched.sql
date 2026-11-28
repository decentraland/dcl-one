DO $node_untouched$ DECLARE t record; n bigint; ids text; found text[] := '{}'; BEGIN
  FOR t IN SELECT * FROM (VALUES
      ('collection', 'builder.collections', 'r.id::text'),
      ('item', 'builder.items', 'r.id::text'),
      ('item_content', 'builder.item_contents', 'r.item_id::text || '':'' || r.file'),
      ('uploaded_content', 'builder.uploaded_contents', 'r.uploaded_by || '':'' || r.hash'),
      ('virtual_third_party', 'builder.virtual_third_parties', 'r.id'),
      ('collection_curation', 'builder.collection_curations', 'r.id::text'),
      ('item_curation', 'builder.item_curations', 'r.id::text'),
      ('collection_event', 'builder.collection_events', 'r.id::text'),
      ('slot_usage_cheque', 'builder.slot_usage_cheques', 'r.id::text')) AS v (step, relation, key)
  LOOP
    EXECUTE format(
      'SELECT count(*), string_agg(k, '', '' ORDER BY k) FILTER (WHERE rn <= 5)
         FROM (SELECT COALESCE(n.k, m.id) AS k, row_number() OVER (ORDER BY COALESCE(n.k, m.id)) AS rn
                 FROM (SELECT %3$s AS k, pg_temp.builder_%1$s_fingerprint(r) AS fp FROM %2$s r) n
                 FULL JOIN (SELECT id, fingerprint FROM dcl_migration.builder_imported WHERE step = %1$L) m ON m.id = n.k
                WHERE n.fp IS DISTINCT FROM m.fingerprint) d', t.step, t.relation, t.key) INTO n, ids;
    IF n > 0 THEN found := found || format('%s: %s rows, first: %s', t.relation, n, ids); END IF;
  END LOOP;
  IF cardinality(found) > 0 THEN RAISE EXCEPTION
    'builder-server: the node holds builder rows the import did not leave there (created, changed or deleted on the node): %; the node has taken builder writes and the import no longer applies to it%',
    array_to_string(found, '; '),
    CASE WHEN NOT EXISTS (SELECT 1 FROM dcl_migration.builder_imported) THEN
      '. The node holds no ledger of an import: if these rows came from an earlier import and creator writes never opened on this node, start over with start_over.sql -v unrecorded_import=on'
    ELSE '' END;
  END IF;
END $node_untouched$;
