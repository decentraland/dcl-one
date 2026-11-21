"""Builder, camera-reel, badges and events imports against the schema."""
import contextlib
import io
import shlex
import time

from tooling.tests import migration_harness as harness

COLLECTION = '11111111-1111-4111-8111-111111111111'
ITEM = '22222222-2222-4222-8222-222222222222'
PHOTO = '55555555-5555-4555-8555-555555555555'
OWNER = '0x' + 'a' * 40


class ContentMigrationCases:
    def content_load(self, db, source, step_name, rows):
        """Stages `rows` (SQL value tuples, in manifest column order) and runs the step's load."""
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / source).steps
                    if s.name == step_name)
        defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
        values = ', '.join(f'({row})' for row in rows)
        variables = {'camera-reel-service': "\\set camera_api_url 'https://camera.example.test'\n",
                     'builder-server': "\\set forum_url 'https://forum.example.test'\n"}.get(source, '')
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.sql(db, variables + f'BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\n'
                         f'INSERT INTO stage VALUES {values};\n{step.load.read_text()}\nCOMMIT;\n')
        return err.getvalue()

    def content_db(self, name):
        db = self.catalyst_db(name)
        self.sql(db, harness.migrate.CHECKPOINT_DDL)
        return db

    @staticmethod
    def builder_rows(name, updated, contract='NULL', lock='NULL'):
        collection = (f"'{COLLECTION}', '{name}', '{OWNER}', NULL, {contract}, NULL, NULL, "
                      f"'2026-01-01Z', '{updated}', NULL, NULL, {lock}, NULL, NULL")
        item = (f"'{ITEM}', '{COLLECTION}', '{OWNER}', '{name}', 'd', NULL, NULL, 'wearable', 'Common', '', '', "
                f"'{{}}', NULL, NULL, NULL, NULL, NULL, false, 0, NULL, NULL, '2026-01-01Z', '{updated}'")
        return collection, item

    def import_builder(self, db, name, updated, contract='NULL'):
        collection, item = self.builder_rows(name, updated, contract)
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.content_load(db, 'builder-server', 'item', [item])

    def builder_names(self, db):
        return self.sql(db, f"""SELECT (SELECT name FROM builder.collections WHERE id = '{COLLECTION}'),
                                       (SELECT name FROM builder.items WHERE id = '{ITEM}');\n""").strip()

    def test_when_planning_the_builder_import_it_should_carry_no_scene_step(self):
        source = harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server')
        self.assertEqual([s.name for s in source.steps], ['collection', 'item', 'item_content', 'virtual_third_party',
                                                          'collection_curation', 'item_curation', 'collection_event',
                                                          'slot_usage_cheque'])
        self.assertFalse(any(t.startswith('builder.project') or t.startswith('builder.asset')
                             for s in source.steps for t in s.targets))

    def test_when_importing_builder_rows_it_should_land_them_with_normalized_values(self):
        db = self.content_db('content_builder_import')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.assertEqual(self.builder_names(db), 'Upstream\tUpstream')
        self.assertEqual(self.sql(db, f"SELECT rarity, price IS NULL, beneficiary IS NULL "
                                      f"FROM builder.items WHERE id = '{ITEM}';\n").strip(), 'common\tt\tt')

    def test_when_importing_a_standard_collection_it_should_keep_its_contract_and_store_no_published_state(self):
        db = self.content_db('content_builder_contract')
        contract = '0x' + 'c' * 40
        self.import_builder(db, 'Upstream', '2026-02-01Z', f"'{contract}'")
        self.assertEqual(self.sql(db, f"SELECT contract_address, publication_pending FROM builder.collections "
                                      f"WHERE id = '{COLLECTION}';\n").strip(), f'{contract}\tf')
        self.assertEqual(self.sql(db, "SELECT count(*) FROM information_schema.columns WHERE table_schema = 'builder' "
                                      "AND table_name = 'collections' AND column_name IN ('is_published', 'is_approved');\n"
                                      ).strip(), '0')

    def test_when_importing_a_locked_collection_it_should_keep_its_lock(self):
        db = self.content_db('content_builder_lock')
        collection, _ = self.builder_rows('Locked', '2026-02-01Z', lock="'2025-05-04 10:10:48.005'")
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.assertEqual(self.sql(db, f"SELECT lock FROM builder.collections WHERE id = '{COLLECTION}';\n").strip(),
                         '2025-05-04 10:10:48.005')

    @staticmethod
    def virtual_third_party_row(provider, managers, raw, programmatic, updated):
        listed = ','.join(managers)
        return (f"'{provider}', '{{{listed}}}', '{raw}', {programmatic}, "
                f"'2026-01-01Z', '{updated}'")

    def virtual_third_parties(self, db):
        return self.sql(db, "SELECT id, managers, raw_metadata, is_programmatic FROM builder.virtual_third_parties "
                            "ORDER BY id;\n").strip()

    def test_when_importing_virtual_third_parties_it_should_keep_their_managers_metadata_and_kind(self):
        db = self.content_db('content_builder_virtual_third_parties')
        provider = 'urn:decentraland:matic:collections-thirdparty:virtual'
        self.content_load(db, 'builder-server', 'virtual_third_party', [
            self.virtual_third_party_row(provider, ['0x' + 'AB' * 20], 'tp:1:Virtual:a: b:amoy-0x1', 'true',
                                         '2026-02-01Z'),
            self.virtual_third_party_row('urn:decentraland:matic:collections-thirdparty:' + 'x' * 600, [OWNER],
                                         'tp:1:Long:', 'false', '2026-02-01Z')])
        self.assertEqual(self.virtual_third_parties(db),
                         f"{provider}\t{{0x{'ab' * 20}}}\ttp:1:Virtual:a: b:amoy-0x1\tt")

    def test_when_reimporting_a_virtual_third_party_it_should_keep_what_the_node_wrote_later(self):
        db = self.content_db('content_builder_virtual_rerun')
        provider = 'urn:decentraland:matic:collections-thirdparty:virtual'
        self.content_load(db, 'builder-server', 'virtual_third_party', [
            self.virtual_third_party_row(provider, [OWNER], 'tp:1:First:', 'false', '2026-02-01Z')])
        self.sql(db, "UPDATE builder.virtual_third_parties SET is_programmatic = true;\n")
        self.content_load(db, 'builder-server', 'virtual_third_party', [
            self.virtual_third_party_row(provider, [OWNER], 'tp:1:Second:', 'false', '2026-03-01Z')])
        self.assertEqual(self.virtual_third_parties(db), f"{provider}\t{{{OWNER}}}\ttp:1:First:\tt")

    def test_when_importing_a_third_party_collection_it_should_keep_its_linked_contract(self):
        db = self.content_db('content_builder_linked')
        provider = 'urn:decentraland:matic:collections-thirdparty:tp'
        self.content_load(db, 'builder-server', 'collection', [
            f"'{COLLECTION}', 'Linked', '', NULL, NULL, '{provider}:hats', '{provider}', "
            f"'2026-01-01Z', '2026-02-01Z', '0x{'AB' * 20}', 'mainnet', NULL, NULL, NULL"])
        self.assertEqual(self.sql(db, f"SELECT linked_contract_address, linked_contract_network FROM builder.collections "
                                      f"WHERE id = '{COLLECTION}';\n").strip(), f"0x{'ab' * 20}\tmainnet")

    def test_when_importing_an_item_it_should_keep_its_documents_and_contents_in_the_saved_order(self):
        db = self.content_db('content_builder_item_order')
        collection, _ = self.builder_rows('Ordered', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [collection])
        item = (f"'{ITEM}', '{COLLECTION}', '{OWNER}', 'Ordered', 'd', NULL, NULL, 'wearable', 'common', NULL, NULL, "
                f"'{{\"tags\": [], \"category\": \"hat\"}}', '{{\"triangles\": 1, \"bodies\": 1}}', NULL, NULL, NULL, NULL, "
                f"false, 0, NULL, NULL, '2026-01-01Z', '2026-02-01Z'")
        self.content_load(db, 'builder-server', 'item', [item])
        self.assertEqual(self.sql(db, f"SELECT data::text, metrics::text FROM builder.items WHERE id = '{ITEM}';\n").strip(),
                         '{"tags": [], "category": "hat"}\t{"triangles": 1, "bodies": 1}')
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkzthumb', 'bafkzthumb', 3, 'image/png'),"
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
        self.content_load(db, 'builder-server', 'item_content', [
            f"'{ITEM}', 'thumbnail.png', 'bafkzthumb', '{OWNER}', 0, '2026-02-01Z'",
            f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 1, '2026-02-01Z'",
        ])
        self.assertEqual(self.sql(db, f"SELECT string_agg(file, ',' ORDER BY position) FROM builder.item_contents "
                                      f"WHERE item_id = '{ITEM}';\n").strip(), 'thumbnail.png,model.glb')
        self.assertEqual(self.sql(db, "SELECT string_agg(hash || ' ' || uploaded_by, ',' ORDER BY hash) "
                                      "FROM builder.uploaded_contents;\n").strip(),
                         f'bafkamodel {OWNER},bafkzthumb {OWNER}')

    def test_when_importing_an_items_contents_it_should_attribute_each_name_to_the_items_owner(self):
        db = self.content_db('content_builder_named_by')
        collection, item = self.builder_rows('Named', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.content_load(db, 'builder-server', 'item', [item])
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkzthumb', 'bafkzthumb', 3, 'image/png'),"
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary'),"
                     "('builder-server', 'bafkbmodel', 'bafkbmodel', 5, 'model/gltf-binary');\n")
        rows = [f"'{ITEM}', 'thumbnail.png', 'bafkzthumb', '{OWNER.upper().replace('0X', '0x')}', 0, '2026-02-01Z'",
                f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 1, '2026-02-01Z'"]
        self.content_load(db, 'builder-server', 'item_content', rows)
        writers = f"SELECT string_agg(file || ' ' || named_by, ',' ORDER BY file) FROM builder.item_contents WHERE item_id = '{ITEM}';\n"
        self.assertEqual(self.sql(db, writers).strip(), f'model.glb {OWNER.lower()},thumbnail.png {OWNER.lower()}')
        other = '0x' + 'b' * 40
        self.sql(db, f"UPDATE builder.item_contents SET named_by = '{other}' WHERE item_id = '{ITEM}';\n")
        self.content_load(db, 'builder-server', 'item_content', [
            f"'{ITEM}', 'thumbnail.png', 'bafkzthumb', '{OWNER}', 1, '2026-02-01Z'",
            f"'{ITEM}', 'model.glb', 'bafkbmodel', '{OWNER}', 0, '2026-02-01Z'"])
        self.assertEqual(self.sql(db, writers).strip(), f'model.glb {OWNER.lower()},thumbnail.png {other}')

    def test_when_importing_an_items_video_it_should_attribute_it_to_the_items_owner_with_its_bytes(self):
        db = self.content_db('content_builder_video')
        collection, _ = self.builder_rows('Video', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [collection])

        def item(video, updated):
            return (f"'{ITEM}', '{COLLECTION}', '{OWNER.upper().replace('0X', '0x')}', 'Video', 'd', NULL, {video}, "
                    f"'wearable', 'common', NULL, NULL, '{{}}', NULL, NULL, NULL, NULL, NULL, false, 0, "
                    f"NULL, NULL, '2026-01-01Z', '{updated}'")
        self.content_load(db, 'builder-server', 'item', [item("'bafkvideo'", '2026-02-01Z')])
        writer = f"SELECT video || ' ' || video_named_by FROM builder.items WHERE id = '{ITEM}';\n"
        self.assertEqual(self.sql(db, writer).strip(), f'bafkvideo {OWNER}')
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary'),"
                     "('builder-server', 'bafkvideo', 'bafkvideo', 7, 'video/mp4');\n")
        self.content_load(db, 'builder-server', 'item_content', [
            f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'",
            f"'{ITEM}', 'video.mp4', 'bafkvideo', '{OWNER}', 1, '2026-02-01Z'"])
        self.assertEqual(self.sql(db, "SELECT string_agg(hash || ' ' || uploaded_by || ' ' || size || ' ' || video, ',' "
                                      "ORDER BY hash) FROM builder.uploaded_contents;\n").strip(),
                         f'bafkamodel {OWNER} 5 false,bafkvideo {OWNER} 7 true')
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'not in dcl_migration.blob_inventory'):
            self.content_load(db, 'builder-server', 'item_content', [
                f"'{ITEM}', 'model.glb', 'bafkmissing', '{OWNER}', 0, '2026-02-01Z'"])

    def test_when_an_items_video_is_over_the_nodes_largest_file_it_should_stop_the_batch(self):
        db = self.content_db('content_builder_video_size')
        collection, _ = self.builder_rows('Video size', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [collection])
        item = (f"'{ITEM}', '{COLLECTION}', '{OWNER}', 'Video size', 'd', NULL, 'bafkvideo', 'wearable', 'common', NULL, "
                f"NULL, '{{}}', NULL, NULL, NULL, NULL, NULL, false, 0, NULL, NULL, '2026-01-01Z', '2026-02-01Z'")
        self.content_load(db, 'builder-server', 'item', [item])
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary'),"
                     "('builder-server', 'bafkvideo', 'bafkvideo', 250000001, 'video/mp4');\n")
        rows = [f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'", f"'{ITEM}', 'video.mp4', 'bafkvideo', '{OWNER}', 1, '2026-02-01Z'"]
        with self.assertRaisesRegex(harness.migrate.MigrationError, '1 content hashes are over 250,000,000 bytes'):
            self.content_load(db, 'builder-server', 'item_content', rows)
        self.assertEqual(self.sql(db, "SELECT count(*) FROM builder.uploaded_contents;\n").strip(), '0')
        self.sql(db, "UPDATE dcl_migration.blob_inventory SET size = 250000000 WHERE object_key = 'bafkvideo';\n")
        self.content_load(db, 'builder-server', 'item_content', rows)
        self.assertEqual(self.sql(db, "SELECT string_agg(hash || ' ' || size, ',' ORDER BY hash) "
                                      "FROM builder.uploaded_contents;\n").strip(),
                         'bafkamodel 5,bafkvideo 250000000')

    def test_when_the_item_documents_release_has_not_run_it_should_refuse_to_import_items(self):
        db = self.content_db('content_builder_before_release')
        collection, item = self.builder_rows('Early', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.sql(db, "ALTER TABLE builder.items DROP CONSTRAINT items_data_object_check;\n"
                     "ALTER TABLE builder.items ALTER COLUMN data DROP DEFAULT;\n"
                     "ALTER TABLE builder.items ALTER COLUMN data TYPE jsonb USING data::jsonb;\n")
        with self.assertRaisesRegex(harness.migrate.MigrationError,
                                    'schemas/releases/builder-item-documents-and-uploaders.sql'):
            self.content_load(db, 'builder-server', 'item', [item])
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM builder.items;\n').strip(), '0')

    def test_when_reimporting_it_should_keep_the_rows_the_node_changed(self):
        db = self.content_db('content_builder_node_wins')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, f"""UPDATE builder.collections SET name = 'Node', updated_at = '2026-03-01' WHERE id = '{COLLECTION}';
                         UPDATE builder.items SET name = 'Node', updated_at = '2026-03-01' WHERE id = '{ITEM}';\n""")
        self.import_builder(db, 'Upstream later', '2026-04-01Z')
        self.assertEqual(self.builder_names(db), 'Node\tNode')

    def test_when_the_node_deleted_imported_rows_it_should_not_bring_them_back_on_reimport(self):
        db = self.curated_world('content_builder_node_deletes')
        provider = 'urn:decentraland:matic:collections-thirdparty:virtual'
        virtual = self.virtual_third_party_row(provider, [OWNER], 'tp:1:First:', 'false', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'virtual_third_party', [virtual])
        curation = self.collection_curation('c0000000-0000-4000-8000-000000000001', COLLECTION, 'pending')
        self.content_load(db, 'builder-server', 'collection_curation', [curation])
        self.sql(db, f"""DELETE FROM builder.collections WHERE id = '{COLLECTION}';
                         DELETE FROM builder.virtual_third_parties;\n""")
        self.import_builder(db, 'Upstream again', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'virtual_third_party', [virtual])
        self.content_load(db, 'builder-server', 'collection_curation', [curation])
        self.assertEqual(self.sql(db, f"""SELECT (SELECT count(*) FROM builder.collections WHERE id = '{COLLECTION}'),
                                                 (SELECT count(*) FROM builder.items WHERE id = '{ITEM}'),
                                                 (SELECT count(*) FROM builder.collection_curations),
                                                 (SELECT count(*) FROM builder.virtual_third_parties);\n""").strip(),
                         '0\t0\t0\t0')

    def test_when_the_node_wrote_imported_rows_without_moving_their_time_it_should_keep_them_on_reimport(self):
        db = self.curated_world('content_builder_node_quiet_writes')
        item = '66666666-6666-4666-8666-666666666666'
        self.linked_item(db, item)
        row = 'd0000000-0000-4000-8000-000000000001'
        self.content_load(db, 'builder-server', 'item_curation', [
            f"'{row}', '{item}', 'pending', '{self.HASH}', false, '2026-01-01Z', '2026-01-02Z'"])
        self.sql(db, f"""UPDATE builder.collections SET forum_link = 'https://forum.example.test/t/1', forum_id = 1
                          WHERE id = '{COLLECTION}';
                         UPDATE builder.items SET video = 'bafknodevideo', video_named_by = '{OWNER}' WHERE id = '{ITEM}';
                         UPDATE builder.item_curations SET content_hash = '{'cd' * 32}' WHERE id = '{row}';\n""")
        self.import_builder(db, 'Curated', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'item_curation', [
            f"'{row}', '{item}', 'pending', '{self.HASH}', false, '2026-01-01Z', '2026-01-02Z'"])
        self.assertEqual(self.sql(db, f"""SELECT (SELECT forum_link FROM builder.collections WHERE id = '{COLLECTION}'),
                                                 (SELECT video FROM builder.items WHERE id = '{ITEM}'),
                                                 (SELECT content_hash FROM builder.item_curations WHERE id = '{row}');\n""").strip(),
                         f"https://forum.example.test/t/1\tbafknodevideo\t{'cd' * 32}")

    def test_when_reimporting_it_should_import_the_rows_it_has_not_imported_yet(self):
        db = self.content_db('content_builder_catch_up')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        other = '77777777-7777-4777-8777-777777777777'
        collection, _ = self.builder_rows('Upstream', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [
            collection, collection.replace(COLLECTION, other).replace("'Upstream'", "'Later'")])
        self.assertEqual(self.sql(db, "SELECT string_agg(name, ',' ORDER BY name) FROM builder.collections;\n").strip(),
                         'Later,Upstream')

    def test_when_an_items_upstream_row_moved_before_its_contents_were_imported_it_should_get_them_on_the_next_run(self):
        db = self.content_db('content_builder_contents_late')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
        self.import_builder(db, 'Upstream edited', '2026-03-01Z')
        self.content_load(db, 'builder-server', 'item_content',
                          [f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-03-01Z'"])
        self.assertEqual(self.sql(db, f"SELECT count(*) FROM builder.item_contents WHERE item_id = '{ITEM}';\n").strip(), '1')

    def test_when_an_item_was_imported_before_its_collection_it_should_join_the_collection_on_the_next_run(self):
        db = self.content_db('content_builder_item_before_collection')
        collection, item = self.builder_rows('Upstream', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'item', [item])
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.content_load(db, 'builder-server', 'item', [item])
        self.assertEqual(self.sql(db, f"SELECT collection_id FROM builder.items WHERE id = '{ITEM}';\n").strip(), COLLECTION)

    def test_when_upstream_changed_imported_rows_the_node_left_alone_it_should_carry_the_changes_on_reimport(self):
        db = self.curated_world('content_builder_upstream_changes')
        provider = 'urn:decentraland:matic:collections-thirdparty:virtual'
        item = '66666666-6666-4666-8666-666666666666'
        self.linked_item(db, item)
        collection_curation, item_curation = 'c0000000-0000-4000-8000-000000000001', 'd0000000-0000-4000-8000-000000000001'

        def upstream(name, status, updated):
            self.import_builder(db, name, updated)
            self.content_load(db, 'builder-server', 'virtual_third_party', [
                self.virtual_third_party_row(provider, [OWNER], f'tp:1:{name}:', 'false', updated)])
            self.content_load(db, 'builder-server', 'collection_curation', [
                self.collection_curation(collection_curation, COLLECTION, status, updated=updated)])
            self.content_load(db, 'builder-server', 'item_curation', [
                f"'{item_curation}', '{item}', '{status}', '{self.HASH}', false, '2026-01-01Z', '{updated}'"])
        upstream('First', 'pending', '2026-02-01Z')
        upstream('Second', 'approved', '2026-03-01Z')
        self.assertEqual(self.sql(db, f"""SELECT (SELECT name FROM builder.collections WHERE id = '{COLLECTION}'),
                                                 (SELECT name FROM builder.items WHERE id = '{ITEM}'),
                                                 (SELECT raw_metadata FROM builder.virtual_third_parties),
                                                 (SELECT status FROM builder.collection_curations),
                                                 (SELECT status FROM builder.item_curations);\n""").strip(),
                         'Second\tSecond\ttp:1:Second:\tapproved\tapproved')

    def test_when_the_node_wrote_imported_rows_without_moving_their_time_it_should_keep_them_when_upstream_changes(self):
        db = self.content_db('content_builder_node_quiet_writes_upstream_moved')

        def upstream(name, updated):
            collection, item = self.builder_rows(name, updated)
            self.content_load(db, 'builder-server', 'collection', [collection])
            self.content_load(db, 'builder-server', 'item', [item.replace("'d', NULL, NULL,", "'d', NULL, 'bafkupstreamvideo',")])
        upstream('Upstream', '2026-02-01Z')
        self.sql(db, f"""UPDATE builder.collections SET forum_link = 'https://forum.example.test/t/1', forum_id = 1
                          WHERE id = '{COLLECTION}';
                         UPDATE builder.items SET video = 'bafknodevideo' WHERE id = '{ITEM}' AND video_named_by = '{OWNER}';\n""")
        upstream('Upstream edited', '2026-03-01Z')
        self.assertEqual(self.sql(db, f"""SELECT (SELECT forum_link FROM builder.collections WHERE id = '{COLLECTION}'),
                                                 (SELECT video FROM builder.items WHERE id = '{ITEM}');\n""").strip(),
                         'https://forum.example.test/t/1\tbafknodevideo')
        self.assertEqual(self.builder_names(db), 'Upstream\tUpstream')

    def test_when_the_node_is_posting_a_topic_or_publishing_it_should_keep_the_collection_when_upstream_changes(self):
        db = self.content_db('content_builder_node_claims')
        publishing = '77777777-7777-4777-8777-777777777777'

        def upstream(name, updated, forum='NULL, NULL'):
            collection, _ = self.builder_rows(name, updated)
            posted = collection.removesuffix(', NULL, NULL') + f', {forum}'
            self.content_load(db, 'builder-server', 'collection', [posted, collection.replace(COLLECTION, publishing)])
        upstream('Upstream', '2026-02-01Z')
        self.sql(db, f"""UPDATE builder.collections SET forum_claimed_at = '2026-02-02' WHERE id = '{COLLECTION}';
                         UPDATE builder.collections SET publication_pending = true WHERE id = '{publishing}';\n""")
        upstream('Upstream later', '2026-03-01Z', "'https://forum.example.test/t/9', 9")
        self.assertEqual(self.sql(db, "SELECT name, forum_link IS NULL, forum_claimed_at IS NOT NULL, publication_pending "
                                      "FROM builder.collections ORDER BY id;\n").strip().split('\n'),
                         ['Upstream\tt\tt\tf', 'Upstream\tt\tf\tt'])

    def test_when_the_node_renamed_a_linked_collection_it_should_keep_its_items_urns_on_reimport(self):
        db = self.curated_world('content_builder_node_renames')
        item = '66666666-6666-4666-8666-666666666666'
        self.linked_item(db, item)
        self.sql(db, f"""UPDATE builder.collections SET urn_suffix = '{self.PROVIDER}:caps', updated_at = '2026-03-01'
                          WHERE id = '{self.LINKED}';
                         UPDATE builder.items SET urn_suffix = '{self.PROVIDER}:caps:{item[:8]}' WHERE id = '{item}';\n""")
        self.linked_item(db, item)
        self.assertEqual(self.sql(db, f"SELECT urn_suffix FROM builder.items WHERE id = '{item}';\n").strip(),
                         f'{self.PROVIDER}:caps:{item[:8]}')

    def linked_rows(self, item, urn_part, collection_updated='2026-02-01Z', item_updated='2026-02-01Z', name='Linked hat'):
        collection = (f"'{self.LINKED}', 'Linked', '', NULL, NULL, '{self.PROVIDER}:{urn_part}', '{self.PROVIDER}', "
                      f"'2026-01-01Z', '{collection_updated}', NULL, NULL, NULL, NULL, NULL")
        row = (f"'{item}', '{self.LINKED}', '{OWNER}', '{name}', 'd', NULL, NULL, 'wearable', NULL, NULL, NULL, "
               f"'{{}}', NULL, NULL, NULL, '{self.PROVIDER}:{urn_part}:{item[:8]}', NULL, false, 0, NULL, NULL, "
               f"'2026-01-01Z', '{item_updated}'")
        return collection, row

    def linked_unit(self, db):
        return self.sql(db, f"""SELECT c.urn_suffix, string_agg(i.urn_suffix || ' ' || i.name, ',' ORDER BY i.id),
                                       bool_and(left(i.urn_suffix, length(c.urn_suffix) + 1) = c.urn_suffix || ':')
                                  FROM builder.collections c JOIN builder.items i ON i.collection_id = c.id
                                 WHERE c.id = '{self.LINKED}' GROUP BY c.urn_suffix;\n""").strip()

    def test_when_the_node_edited_an_item_and_upstream_renamed_its_collection_it_should_keep_the_whole_unit(self):
        db = self.curated_world('content_builder_unit_item_owned')
        item, other = '66666666-6666-4666-8666-666666666666', '66666666-6666-4666-8666-666666666667'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1], self.linked_rows(other, 'hats')[1]])
        self.sql(db, f"UPDATE builder.items SET name = 'Node hat', updated_at = '2026-03-01' WHERE id = '{item}';\n")
        collection, row = self.linked_rows(item, 'caps', '2026-03-02Z', '2026-03-02Z', 'Upstream hat')
        said = self.content_load(db, 'builder-server', 'collection', [collection])
        said += self.content_load(db, 'builder-server', 'item', [row, self.linked_rows(other, 'caps', '2026-03-02Z', '2026-03-02Z', 'Upstream hat')[1]])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:hats\t{self.PROVIDER}:hats:{item[:8]} Node hat,"
                                               f"{self.PROVIDER}:hats:{other[:8]} Linked hat\tt")
        self.assertIn(f'1 collections changed upstream are left as the node holds them', said)
        self.assertIn(self.LINKED, said)

    def test_when_the_node_edited_a_collection_and_upstream_renamed_it_it_should_keep_its_items_urns(self):
        db = self.curated_world('content_builder_unit_collection_owned')
        item = '66666666-6666-4666-8666-666666666666'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1]])
        self.sql(db, f"UPDATE builder.collections SET name = 'Node name', updated_at = '2026-03-01' WHERE id = '{self.LINKED}';\n")
        collection, row = self.linked_rows(item, 'caps', '2026-03-02Z', '2026-03-02Z', 'Upstream hat')
        self.content_load(db, 'builder-server', 'collection', [collection])
        said = self.content_load(db, 'builder-server', 'item', [row])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:hats\t{self.PROVIDER}:hats:{item[:8]} Linked hat\tt")
        self.assertIn('1 items changed upstream are left as the node holds them', said)

    def test_when_upstream_renamed_an_untouched_third_party_collection_it_should_move_it_and_its_items_together(self):
        db = self.curated_world('content_builder_unit_untouched')
        item, other = '66666666-6666-4666-8666-666666666666', '66666666-6666-4666-8666-666666666667'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1], self.linked_rows(other, 'hats')[1]])
        collection, row = self.linked_rows(item, 'caps', '2026-03-02Z', '2026-03-02Z', 'Upstream hat')
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:caps\t{self.PROVIDER}:caps:{item[:8]} Linked hat,"
                                               f"{self.PROVIDER}:caps:{other[:8]} Linked hat\tt")
        self.content_load(db, 'builder-server', 'item', [row])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:caps\t{self.PROVIDER}:caps:{item[:8]} Upstream hat,"
                                               f"{self.PROVIDER}:caps:{other[:8]} Linked hat\tt")
        self.assertEqual(self.sql(db, "SELECT count(*) FROM dcl_migration.builder_imported m JOIN builder.items i "
                                      "ON m.step = 'item' AND m.id = i.id::text AND m.fingerprint IS DISTINCT FROM "
                                      "md5(json_build_array(i.collection_id, i.eth_address, i.name, i.description, i.thumbnail, "
                                      "i.video, i.type, i.rarity, i.price, i.beneficiary, i.data, i.metrics, i.utility, "
                                      "i.mappings, i.urn_suffix, i.blockchain_item_id, i.is_published, i.in_catalyst, "
                                      "i.total_supply, i.local_content_hash, i.content_hash, i.catalyst_content_hash, "
                                      "i.created_at, i.updated_at, i.video_named_by)::text);\n").strip(), '0')

    def test_when_upstream_added_items_to_collections_the_node_changed_it_should_add_only_those_whose_urn_fits(self):
        db = self.curated_world('content_builder_unit_new_items')
        item, fits, diverges = ('66666666-6666-4666-8666-666666666666', '66666666-6666-4666-8666-666666666667',
                                '66666666-6666-4666-8666-666666666668')
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1]])
        self.sql(db, f"""UPDATE builder.collections SET name = 'Node name', updated_at = '2026-03-01' WHERE id = '{COLLECTION}';
                         UPDATE builder.collections SET urn_suffix = '{self.PROVIDER}:caps', updated_at = '2026-03-01'
                          WHERE id = '{self.LINKED}';
                         UPDATE builder.items SET urn_suffix = '{self.PROVIDER}:caps:{item[:8]}' WHERE id = '{item}';\n""")
        standard = self.builder_rows('Upstream', '2026-02-01Z')[1].replace(ITEM, fits)
        said = self.content_load(db, 'builder-server', 'item', [standard, self.linked_rows(diverges, 'hats')[1]])
        self.assertEqual(self.sql(db, f"SELECT string_agg(id::text, ',' ORDER BY id) FROM builder.items "
                                      f"WHERE id IN ('{fits}', '{diverges}');\n").strip(), fits)
        self.assertIn('1 new items whose URN differs from their collection', said)
        self.assertIn(diverges, said)
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:caps\t{self.PROVIDER}:caps:{item[:8]} Linked hat\tt")

    def test_when_the_node_reviewed_a_collection_it_should_keep_upstream_from_changing_any_row_of_it(self):
        db = self.curated_world('content_builder_unit_reviewed')
        item = '66666666-6666-4666-8666-666666666666'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1]])
        reviewed, later = 'c0000000-0000-4000-8000-000000000001', 'c0000000-0000-4000-8000-000000000002'
        self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation(reviewed, self.LINKED, 'pending')])
        self.sql(db, f"UPDATE builder.collection_curations SET status = 'approved', updated_at = '2026-03-01' "
                     f"WHERE id = '{reviewed}';\n")
        collection, row = self.linked_rows(item, 'hats', '2026-03-02Z', '2026-03-02Z', 'Upstream hat')
        self.content_load(db, 'builder-server', 'collection', [collection.replace("'Linked'", "'Upstream name'")])
        self.content_load(db, 'builder-server', 'item', [row])
        said = self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation(reviewed, self.LINKED, 'rejected', updated='2026-03-02Z'),
            self.collection_curation(later, self.LINKED, 'rejected', created='2026-03-02Z', updated='2026-03-02Z')])
        self.assertEqual(self.sql(db, f"""SELECT (SELECT name FROM builder.collections WHERE id = '{self.LINKED}'),
                                                 (SELECT name FROM builder.items WHERE id = '{item}'),
                                                 (SELECT string_agg(id || ' ' || status, ',') FROM builder.collection_curations
                                                   WHERE collection_id = '{self.LINKED}');\n""").strip(),
                         f'Linked\tLinked hat\t{reviewed} approved')
        self.assertIn(later, said)

    def test_when_a_node_save_lands_while_item_content_runs_it_should_keep_the_saved_files(self):
        for loose in (False, True):
            with self.subTest(loose=loose):
                self.node_save_during_item_content(loose)

    def node_save_during_item_content(self, loose):
        db = self.content_db(f'content_builder_contents_race_{int(loose)}')
        _, item = self.builder_rows('Upstream', '2026-03-01Z')
        if loose:
            self.content_load(db, 'builder-server', 'item', [item.replace(f"'{COLLECTION}'", 'NULL', 1)])
        else:
            self.import_builder(db, 'Upstream', '2026-03-01Z')
        parent = (f"SELECT id FROM builder.items WHERE id = '{ITEM}' FOR UPDATE; " if loose else
                  f"SELECT id FROM builder.collections WHERE id = '{COLLECTION}' FOR UPDATE; ")
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n"
                     f"INSERT INTO builder.item_contents (item_id, file, hash, position, named_by) "
                     f"VALUES ('{ITEM}', 'model.glb', 'bafkoldmodel', 0, '{OWNER}');\n")
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server').steps
                    if s.name == 'item_content')
        load = step.load.read_text()
        cut = load.index('DO $missing$')
        save = (f"BEGIN; {parent}"
                f"UPDATE builder.items SET updated_at = '2026-04-01' WHERE id = '{ITEM}'; "
                f"UPDATE builder.item_contents SET hash = 'bafknodemodel' WHERE item_id = '{ITEM}'; COMMIT;")
        node = f"\\! (psql {shlex.quote(self.url(db))} -X -q -c {shlex.quote(save)} >/dev/null 2>&1 &)\nSELECT pg_sleep(1);\n"
        defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
        self.sql(db, "\\set forum_url 'https://forum.example.test'\n"
                     f"BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\n"
                     f"INSERT INTO stage VALUES ('{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-03-01Z');\n"
                     + load[:cut] + node + load[cut:] + "COMMIT;\n")
        for _ in range(100):
            if self.sql(db, f"SELECT updated_at FROM builder.items WHERE id = '{ITEM}';\n").strip() == '2026-04-01 00:00:00':
                break
            time.sleep(0.1)
        self.assertEqual(self.sql(db, f"SELECT string_agg(file || ' ' || hash, ','), max(i.updated_at) "
                                      f"FROM builder.item_contents c JOIN builder.items i ON i.id = c.item_id "
                                      f"WHERE c.item_id = '{ITEM}';\n").strip(),
                         'model.glb bafknodemodel\t2026-04-01 00:00:00')

    def test_when_the_node_added_an_item_and_upstream_renamed_its_collection_it_should_keep_the_whole_unit(self):
        db = self.curated_world('content_builder_unit_node_item')
        item = '66666666-6666-4666-8666-666666666666'
        self.sql(db, f"INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix) VALUES "
                     f"('{item}', '{self.LINKED}', '{OWNER}', 'Node hat', 'wearable', '{self.PROVIDER}:hats:{item[:8]}');\n")
        self.content_load(db, 'builder-server', 'collection', [self.linked_rows(item, 'caps', '2026-03-02Z')[0]])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:hats\t{self.PROVIDER}:hats:{item[:8]} Node hat\tt")

    def test_when_a_node_edit_lands_while_the_collection_step_runs_it_should_stay_the_nodes(self):
        db = self.curated_world('content_builder_unit_race')
        item = '66666666-6666-4666-8666-666666666666'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1]])
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server').steps
                    if s.name == 'collection')
        load = step.load.read_text()
        cut = load.index('DO $kept$')
        edit = (f"BEGIN; SELECT id FROM builder.collections WHERE id = '{self.LINKED}' FOR UPDATE; "
                f"UPDATE builder.items SET name = 'Node hat', updated_at = '2026-03-01' WHERE id = '{item}'; COMMIT;")
        node = f"\\! (psql {shlex.quote(self.url(db))} -X -q -c {shlex.quote(edit)} >/dev/null 2>&1 &)\nSELECT pg_sleep(1);\n"
        defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
        collection = self.linked_rows(item, 'caps', '2026-03-02Z')[0]
        self.sql(db, "\\set forum_url 'https://forum.example.test'\n"
                     f"BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\nINSERT INTO stage VALUES ({collection});\n"
                     + load[:cut] + node + load[cut:] + "COMMIT;\n")
        for _ in range(100):
            if self.sql(db, f"SELECT name FROM builder.items WHERE id = '{item}';\n").strip() == 'Node hat':
                break
            time.sleep(0.1)
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'caps', '2026-03-02Z', '2026-03-03Z', 'Upstream hat')[1]])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:caps\t{self.PROVIDER}:caps:{item[:8]} Node hat\tt")

    def test_when_loads_share_the_unit_rule_it_should_be_the_same_text_in_each(self):
        loads = {s.name: s.load.read_text() for s in harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server').steps}

        def shared(text):
            start = text.index('CREATE OR REPLACE FUNCTION pg_temp.builder_collection_fingerprint')
            end = text.index('$$;', text.index('CREATE OR REPLACE FUNCTION pg_temp.builder_unit_untouched')) + 3
            return text[start:end]
        copies = {name: shared(loads[name]) for name in ['collection', 'item', 'collection_curation', 'item_curation']}
        self.assertEqual(len(set(copies.values())), 1, sorted(copies))

    def test_when_the_ledger_predates_fingerprints_it_should_leave_its_rows_alone(self):
        db = self.catalyst_db('content_builder_old_ledger')
        self.sql(db, "CREATE SCHEMA dcl_migration;\n"
                     "CREATE TABLE dcl_migration.builder_imported (step text NOT NULL, id text NOT NULL, "
                     "PRIMARY KEY (step, id));\n")
        self.sql(db, harness.migrate.CHECKPOINT_DDL)
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, "UPDATE dcl_migration.builder_imported SET fingerprint = NULL;\n")
        self.import_builder(db, 'Upstream later', '2026-03-01Z')
        self.assertEqual(self.builder_names(db), 'Upstream\tUpstream')

    def test_when_the_node_changed_an_items_contents_it_should_keep_them_on_reimport(self):
        db = self.content_db('content_builder_node_contents')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary'),"
                     "('builder-server', 'bafkzthumb', 'bafkzthumb', 3, 'image/png');\n")
        upstream = [f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'",
                    f"'{ITEM}', 'thumbnail.png', 'bafkzthumb', '{OWNER}', 1, '2026-02-01Z'"]
        self.content_load(db, 'builder-server', 'item_content', upstream)
        self.sql(db, f"""DELETE FROM builder.item_contents WHERE item_id = '{ITEM}';
                         DELETE FROM builder.uploaded_contents;
                         INSERT INTO builder.item_contents (item_id, file, hash, position, named_by)
                              VALUES ('{ITEM}', 'model.glb', 'bafknodemodel', 0, '{OWNER}');
                         UPDATE builder.items SET updated_at = '2026-03-01' WHERE id = '{ITEM}';\n""")
        self.import_builder(db, 'Upstream again', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'item_content', upstream)
        self.assertEqual(self.sql(db, f"SELECT string_agg(file || ' ' || hash, ',' ORDER BY file) "
                                      f"FROM builder.item_contents WHERE item_id = '{ITEM}';\n").strip(),
                         'model.glb bafknodemodel')
        self.assertEqual(self.sql(db, "SELECT count(*) FROM builder.uploaded_contents;\n").strip(), '0')

    def test_when_the_node_saved_an_item_with_a_video_of_its_own_it_should_still_import_the_other_items_contents(self):
        db = self.content_db('content_builder_node_video')
        other = '77777777-7777-4777-8777-777777777777'
        _, item = self.builder_rows('Upstream', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [self.builder_rows('Upstream', '2026-02-01Z')[0]])
        self.content_load(db, 'builder-server', 'item', [item, item.replace(ITEM, other)])
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n"
                     f"UPDATE builder.items SET video = 'bafknodevideo', video_named_by = '{OWNER}', "
                     f"updated_at = '2026-03-01' WHERE id = '{ITEM}';\n")
        self.content_load(db, 'builder-server', 'item_content', [
            f"'{ITEM}', 'model.glb', 'bafkupstreamonly', '{OWNER}', 0, '2026-02-01Z'",
            f"'{other}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'"])
        self.assertEqual(self.sql(db, "SELECT string_agg(item_id::text, ',') FROM builder.item_contents;\n").strip(), other)

    def test_when_the_node_holds_an_id_the_import_did_not_write_it_should_stop_the_batch(self):
        db = self.content_db('content_builder_taken_ids')
        squatter = '0x' + 'c' * 40
        self.sql(db, f"""INSERT INTO builder.collections (id, name, eth_address, salt)
                              VALUES ('{COLLECTION}', 'Taken', '{squatter}', '');
                         INSERT INTO builder.items (id, collection_id, eth_address, name, type)
                              VALUES ('{ITEM}', '{COLLECTION}', '{squatter}', 'Taken', 'wearable');\n""")
        collection, item = self.builder_rows('Upstream', '2026-02-01Z')
        with self.assertRaisesRegex(harness.migrate.MigrationError, f'did not write.*{COLLECTION}'):
            self.content_load(db, 'builder-server', 'collection', [collection])
        with self.assertRaisesRegex(harness.migrate.MigrationError, f'did not write.*{ITEM}'):
            self.content_load(db, 'builder-server', 'item', [item])
        self.assertEqual(self.builder_names(db), 'Taken\tTaken')
        self.assertEqual(self.sql(db, f"SELECT eth_address FROM builder.items WHERE id = '{ITEM}';\n").strip(),
                         squatter)

    PROVIDER = 'urn:decentraland:matic:collections-thirdparty:curated'
    LINKED = '33333333-3333-4333-8333-333333333333'
    MISSING = '44444444-4444-4444-8444-444444444444'
    HASH = 'ab' * 32

    def curated_world(self, name):
        """A standard collection with an item and a third-party collection with an item, imported."""
        db = self.content_db(name)
        self.import_builder(db, 'Curated', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [
            f"'{self.LINKED}', 'Linked', '', NULL, NULL, '{self.PROVIDER}:hats', '{self.PROVIDER}', "
            f"'2026-01-01Z', '2026-02-01Z', NULL, NULL, NULL, NULL, NULL"])
        return db

    @staticmethod
    def collection_curation(id, collection, status, assignee='NULL', reviewed_by='NULL', reasons='NULL',
                            message='NULL', created='2026-01-01Z', updated='2026-01-02Z'):
        return (f"'{id}', '{collection}', '{status}', {assignee}, {reviewed_by}, {reasons}, {message}, "
                f"'{created}', '{updated}'")

    def test_when_importing_collection_curations_it_should_keep_upstreams_review_and_skip_what_the_node_lacks(self):
        db = self.curated_world('content_builder_collection_curations')
        upper = '0x' + 'AB' * 20
        self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation('c0000000-0000-4000-8000-000000000001', COLLECTION, 'rejected', f"'{upper}'",
                                     f"'{upper}'", "'{clipping,other}'", "'  Fix the hat  '"),
            self.collection_curation('c0000000-0000-4000-8000-000000000002', COLLECTION, 'pending', "'not an address'",
                                     "'validator'", "'{}'", "'   '", '2026-01-03Z', '2026-01-03Z'),
            self.collection_curation('c0000000-0000-4000-8000-000000000003', self.MISSING, 'approved'),
            self.collection_curation('c0000000-0000-4000-8000-000000000004', COLLECTION, 'approved',
                                     created='2026-01-04Z', updated='2026-01-05Z'),
            self.collection_curation('c0000000-0000-4000-8000-000000000005', COLLECTION, 'pending',
                                     created='2026-01-04Z', updated='2026-01-04Z'),
            self.collection_curation('c0000000-0000-4000-8000-000000000006', COLLECTION, 'withdrawn',
                                     created='2026-01-06Z', updated='2026-01-06Z'),
            self.collection_curation('c0000000-0000-4000-8000-000000000007', COLLECTION, 'rejected',
                                     reasons="'{rigging}'", message=f"'{'x' * 3001}'",
                                     created='2026-01-07Z', updated='2026-01-07Z'),
            self.collection_curation('c0000000-0000-4000-8000-000000000008', COLLECTION, 'rejected',
                                     reasons="'{rigging,too_ugly}'", message="'Fix it'",
                                     created='2026-01-08Z', updated='2026-01-08Z')])
        self.assertEqual(self.sql(db, "SELECT id, status, assignee, reviewed_by, rejection_reasons, rejection_message "
                                      "FROM builder.collection_curations ORDER BY created_at;\n").strip('\n').split('\n'), [
            f"c0000000-0000-4000-8000-000000000001\trejected\t{upper.lower()}\t{upper.lower()}\t{{clipping,other}}\tFix the hat",
            "c0000000-0000-4000-8000-000000000002\tpending\t\tvalidator\t\t",
            "c0000000-0000-4000-8000-000000000004\tapproved\t\t\t\t",
            "c0000000-0000-4000-8000-000000000007\trejected\t\t\t{rigging}\t",
            "c0000000-0000-4000-8000-000000000008\trejected\t\t\t\tFix it"])

    def test_when_reimporting_a_collection_curation_it_should_keep_what_the_node_decided_later(self):
        db = self.curated_world('content_builder_collection_curation_rerun')
        row = 'c0000000-0000-4000-8000-000000000001'
        self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation(row, COLLECTION, 'pending')])
        self.sql(db, f"UPDATE builder.collection_curations SET status = 'approved', updated_at = '2026-03-01' "
                     f"WHERE id = '{row}';\n")
        self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation(row, COLLECTION, 'rejected', updated='2026-02-01Z')])
        self.assertEqual(self.sql(db, f"SELECT status FROM builder.collection_curations WHERE id = '{row}';\n").strip(),
                         'approved')

    def linked_item(self, db, item):
        self.content_load(db, 'builder-server', 'item', [
            f"'{item}', '{self.LINKED}', '{OWNER}', 'Linked hat', 'd', NULL, NULL, 'wearable', NULL, NULL, NULL, "
            f"'{{}}', NULL, NULL, NULL, '{self.PROVIDER}:hats:{item[:8]}', NULL, false, 0, NULL, NULL, "
            f"'2026-01-01Z', '2026-02-01Z'"])

    def test_when_importing_item_curations_it_should_keep_one_per_instant_and_skip_what_the_node_lacks(self):
        db = self.curated_world('content_builder_item_curations')
        item = '55555555-5555-4555-8555-555555555551'
        self.linked_item(db, item)
        self.content_load(db, 'builder-server', 'item_curation', [
            f"'d0000000-0000-4000-8000-000000000001', '{item}', 'approved', '{self.HASH}', true, '2026-01-01Z', '2026-01-02Z'",
            f"'d0000000-0000-4000-8000-000000000002', '{item}', 'pending', '{self.HASH.upper()}', NULL, '2026-01-03Z', '2026-01-03Z'",
            f"'d0000000-0000-4000-8000-000000000003', '{item}', 'approved', '{self.HASH}', NULL, '2026-01-03Z', '2026-01-09Z'",
            f"'d0000000-0000-4000-8000-000000000004', '{self.MISSING}', 'approved', '{self.HASH}', NULL, '2026-01-01Z', '2026-01-01Z'",
            f"'d0000000-0000-4000-8000-000000000005', '{item}', 'pending', 'bafkreinotahexdigest', NULL, '2026-01-05Z', '2026-01-05Z'",
            f"'d0000000-0000-4000-8000-000000000006', '{item}', 'approved', '{self.HASH}', NULL, '2026-01-06Z', '2026-01-05Z'"])
        self.assertEqual(self.sql(db, "SELECT id, status, content_hash, is_mapping_complete, updated_at "
                                      "FROM builder.item_curations ORDER BY created_at;\n").strip('\n').split('\n'), [
            f"d0000000-0000-4000-8000-000000000001\tapproved\t{self.HASH}\tt\t2026-01-02 00:00:00",
            f"d0000000-0000-4000-8000-000000000003\tapproved\t{self.HASH}\t\t2026-01-09 00:00:00",
            f"d0000000-0000-4000-8000-000000000006\tapproved\t{self.HASH}\t\t2026-01-06 00:00:00"])

    def test_when_importing_collection_events_it_should_keep_upstreams_and_skip_unknown_ones(self):
        db = self.curated_world('content_builder_collection_events')
        upper = '0x' + 'CD' * 20
        self.content_load(db, 'builder-server', 'collection_event', [
            f"'e0000000-0000-4000-8000-000000000001', '{COLLECTION}', 'review.rejected', 'curator', '{upper}', "
            f"'{{\"rejectionReasons\": [\"clipping\"]}}', '2026-01-01Z'",
            f"'e0000000-0000-4000-8000-000000000002', '{COLLECTION}', 'review.withdrawn', 'curator', NULL, '{{}}', '2026-01-02Z'",
            f"'e0000000-0000-4000-8000-000000000003', '{COLLECTION}', 'review.comment', 'operator', NULL, '{{}}', '2026-01-03Z'",
            f"'e0000000-0000-4000-8000-000000000004', '{self.MISSING}', 'review.comment', 'curator', NULL, '{{}}', '2026-01-04Z'",
            f"'e0000000-0000-4000-8000-000000000005', '{COLLECTION}', 'review.comment', 'curator', 'nobody', '[]', '2026-01-05Z'",
            f"'e0000000-0000-4000-8000-000000000006', '{COLLECTION}', 'collection.published', 'creator', 'nobody', '{{}}', '2026-01-06Z'"])
        self.assertEqual(self.sql(db, "SELECT id, type, actor, actor_address, payload FROM builder.collection_events "
                                      "ORDER BY created_at;\n").strip('\n').split('\n'), [
            f"e0000000-0000-4000-8000-000000000001\treview.rejected\tcurator\t{upper.lower()}\t"
            '{"rejectionReasons": ["clipping"]}',
            "e0000000-0000-4000-8000-000000000006\tcollection.published\tcreator\t\t{}"])

    def test_when_importing_slot_cheques_it_should_keep_each_salt_once_for_its_collections_provider(self):
        db = self.curated_world('content_builder_slot_cheques')
        salt, other = '0x' + '12' * 32, '0x' + '34' * 32
        signature = '0x' + 'ef' * 65

        def cheque(n, collection, provider, salt, qty, created):
            return (f"'f0000000-0000-4000-8000-00000000000{n}', '{collection}', '{provider}', '{signature}', "
                    f"'{salt}', {qty}, '{created}', '{created}'")
        self.content_load(db, 'builder-server', 'slot_usage_cheque', [
            cheque(1, self.LINKED, self.PROVIDER, salt.upper().replace('0X', '0x'), 3, '2026-01-01Z'),
            cheque(2, self.LINKED, self.PROVIDER, salt, 3, '2026-01-02Z'),
            cheque(3, self.LINKED, self.PROVIDER + 'x', other, 1, '2026-01-03Z'),
            cheque(4, self.MISSING, self.PROVIDER, other, 1, '2026-01-04Z'),
            cheque(5, self.LINKED, self.PROVIDER, '0x1234', 1, '2026-01-05Z'),
            cheque(6, self.LINKED, self.PROVIDER, other, 0, '2026-01-06Z')])
        self.assertEqual(self.sql(db, "SELECT id, salt, qty FROM builder.slot_usage_cheques;\n").strip(),
                         f"f0000000-0000-4000-8000-000000000001\t{salt}\t3")

    def test_when_importing_forum_columns_it_should_keep_a_link_only_when_it_is_a_web_url(self):
        db = self.content_db('content_builder_forum')
        cases = [("'https://forum.example.test/t/hat/41395'", 41395, 'https://forum.example.test/t/hat/41395\t41395'),
                 ("'javascript:alert(1)'", 7, ''),
                 *((f"'{elsewhere}'", 8, '') for elsewhere in (
                     'https://elsewhere.example.test/t/hat/1', 'https://forum.example.test.elsewhere.test/t/1',
                     'http://forum.example.test/t/hat/1', 'https://forum.example.test'))]
        for n, (link, forum_id, expected) in enumerate(cases):
            id = f'f{n:07d}-0000-4000-8000-000000000000'
            collection, _ = self.builder_rows('Forum', '2026-02-01Z')
            row = collection.replace(COLLECTION, id).rsplit('NULL, NULL', 1)[0] + f"{link}, {forum_id}"
            self.content_load(db, 'builder-server', 'collection', [row])
            self.assertEqual(self.sql(db, f"SELECT forum_link, forum_id FROM builder.collections WHERE id = '{id}';\n")
                             .strip(), expected, link)

    def test_when_the_source_predates_the_review_columns_and_events_it_should_extract_without_them(self):
        db = self.fresh_db('content_builder_old_source')
        self.sql(db, """CREATE TYPE curation_status AS ENUM ('pending', 'approved', 'rejected');
            CREATE TABLE collection_curations (id uuid PRIMARY KEY, collection_id uuid NOT NULL, status curation_status NOT NULL,
                created_at timestamp NOT NULL, updated_at timestamp NOT NULL, assignee text);
            INSERT INTO collection_curations VALUES ('c0000000-0000-4000-8000-000000000001',
                '11111111-1111-4111-8111-111111111111', 'approved', '2026-01-01', '2026-01-02', NULL);\n""")
        source = harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server')
        steps = {s.name: s for s in source.steps}
        self.assertEqual(self.sql(db, steps['collection_curation'].extract.read_text() + ';\n').strip(),
                         'c0000000-0000-4000-8000-000000000001\t11111111-1111-4111-8111-111111111111\tapproved'
                         '\t\t\t\t\t2026-01-01 00:00:00+00\t2026-01-02 00:00:00+00')
        events = steps['collection_event']
        self.assertEqual(events.requires, 'collection_events')
        absent = harness.migrate.without_missing_source(events, self.url(db))
        self.assertIsNone(absent.extract)
        self.assertEqual(self.sql(db, harness.migrate.extract_text(absent) + ';\n').strip(), '')
        self.sql(db, """ALTER TABLE collection_curations ADD COLUMN reviewed_by text, ADD COLUMN rejection_reasons text[],
                ADD COLUMN rejection_message text;
            INSERT INTO collection_curations VALUES ('c0000000-0000-4000-8000-000000000002',
                '11111111-1111-4111-8111-111111111111', 'rejected', '2026-01-03', '2026-01-04', NULL, 'validator',
                '{rigging,other}', 'Fix it');\n""")
        self.assertEqual(self.sql(db, steps['collection_curation'].extract.read_text() + ' ORDER BY 8;\n')
                         .strip('\n').split('\n'), [
            'c0000000-0000-4000-8000-000000000001\t11111111-1111-4111-8111-111111111111\tapproved'
            '\t\t\t\t\t2026-01-01 00:00:00+00\t2026-01-02 00:00:00+00',
            'c0000000-0000-4000-8000-000000000002\t11111111-1111-4111-8111-111111111111\trejected'
            '\t\tvalidator\t{rigging,other}\tFix it\t2026-01-03 00:00:00+00\t2026-01-04 00:00:00+00'])
        self.sql(db, """CREATE TABLE collection_events (id uuid PRIMARY KEY, collection_id uuid NOT NULL, type text NOT NULL,
                actor text NOT NULL, actor_address text, payload jsonb NOT NULL DEFAULT '{}', created_at timestamp NOT NULL);
            INSERT INTO collection_events VALUES ('e0000000-0000-4000-8000-000000000001',
                '11111111-1111-4111-8111-111111111111', 'review.comment', 'curator', NULL,
                '{"message": "a <b>&amp; \\"quote\\""}', '2026-01-03');\n""")
        present = harness.migrate.without_missing_source(events, self.url(db))
        self.assertEqual(present, events)
        self.assertEqual(self.sql(db, harness.migrate.extract_text(present) + ';\n').strip(),
                         'e0000000-0000-4000-8000-000000000001\t11111111-1111-4111-8111-111111111111\treview.comment'
                         '\tcurator\t\t{"message": "a <b>&amp; \\"quote\\""}\t2026-01-03 00:00:00+00')

    def camera_rows(self, is_public):
        return [f"'{PHOTO}', '{OWNER}', 'https://camera.example.test/api/images/{PHOTO}-photo.png', "
                f"'https://camera.example.test/api/images/{PHOTO}-thumbnail-photo.png', {is_public}, "
                f"'{{\"userAddress\": \"{OWNER}\"}}', '2026-01-01Z'"]

    def camera_db(self, name):
        db = self.content_db(name)
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     f"('camera-reel-service', '{PHOTO}-photo.png', 'bafkreiimage', 10, 'image/png'),"
                     f"('camera-reel-service', '{PHOTO}-thumbnail-photo.png', 'bafkreithumb', 5, 'image/png');\n")
        return db

    def is_public(self, db):
        return self.sql(db, f"SELECT is_public FROM camera_reel.images WHERE id = '{PHOTO}';\n").strip()

    def test_when_importing_a_photo_it_should_name_its_blob_and_follow_upstream_visibility(self):
        db = self.camera_db('content_camera_import')
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('false'))
        self.assertEqual(self.sql(db, f"SELECT user_address, image_hash, thumbnail_hash, is_public, review_status "
                                      f"FROM camera_reel.images WHERE id = '{PHOTO}';\n").strip(),
                         f'{OWNER}\tbafkreiimage\tbafkreithumb\tf\tok')
        self.assertEqual(self.sql(db, 'SELECT id FROM camera_reel.imported_images;\n').strip(), PHOTO)
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('true'))
        self.assertEqual(self.is_public(db), 't')

    def test_when_the_owner_toggled_visibility_on_the_node_it_should_keep_it_on_reimport(self):
        db = self.camera_db('content_camera_node_wins')
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('false'))
        self.sql(db, f"UPDATE camera_reel.images SET is_public = true WHERE id = '{PHOTO}';\n")
        self.content_load(db, 'camera-reel-service', 'image', self.camera_rows('false'))
        self.assertEqual(self.is_public(db), 't')

    @staticmethod
    def schedule_row(name, updated):
        return (f"'s1', '{name}', NULL, NULL, NULL, '[]', NULL, NULL, true, '2026-01-01Z', '{updated}'")

    def test_when_importing_schedules_it_should_keep_a_newer_node_row(self):
        db = self.content_db('content_schedules')
        self.content_load(db, 'events', 'schedule', [self.schedule_row('Upstream', '2026-02-01Z')])
        self.sql(db, "UPDATE explore.schedules_local SET name = 'Node', updated_at = '2026-03-01' WHERE id = 's1';\n")
        self.content_load(db, 'events', 'schedule', [self.schedule_row('Upstream again', '2026-02-15Z')])
        name = "SELECT name FROM explore.schedules_local WHERE id = 's1';\n"
        self.assertEqual(self.sql(db, name).strip(), 'Node')
        self.content_load(db, 'events', 'schedule', [self.schedule_row('Upstream later', '2026-04-01Z')])
        self.assertEqual(self.sql(db, name).strip(), 'Upstream later')

    def test_when_importing_profile_permissions_it_should_grant_them_and_report_the_count(self):
        db = self.content_db('content_profile_permissions')
        row = ("'{user}', NULL, false, NULL, NULL, true, false, false, '{perms}', '2026-01-01Z', '2026-01-01Z'")
        notices = self.content_load(db, 'events', 'profile_settings', [
            row.format(user=OWNER, perms='["approve_any_event", "edit_any_profile"]'),
            row.format(user='0x' + 'b' * 40, perms='[]')])
        self.assertIn('importing 2 permission grants (approve_any_event, edit_any_profile)', notices)
        self.assertEqual(self.sql(db, 'SELECT "user", permissions_granted FROM explore.event_profile_settings '
                                      'ORDER BY 1;\n').splitlines(),
                         [f'{OWNER}\tt', f'0x{"b" * 40}\tf'])

    def test_when_a_badge_was_edited_on_the_node_it_should_keep_it_on_reimport(self):
        src, tgt = self.badges_dbs()
        run = ('run', 'badges', '--step', 'catalog')
        self.assertEqual(self.migrate(*run, sources_dir=harness.migrate.SOURCES)[0], 0)
        self.sql(tgt, "UPDATE social.badge_definitions SET name = 'Node name' WHERE id = 'dclcitizen';\n"
                      "UPDATE social.badge_tiers SET tier_name = 'Node tier' WHERE tier_id = 'emotionista-bronze';\n")
        self.sql(src, "UPDATE badge_definitions SET description = 'Upstream edit' "
                      "WHERE id IN ('dclcitizen', 'emotionista', 'open_for_business');\n")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code, out = self.migrate(*run, sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('kept badge(s) dclcitizen, emotionista as edited on the node', err.getvalue())
        self.assertEqual(self.sql(tgt, "SELECT id, name, description FROM social.badge_definitions WHERE id IN "
                                       "('dclcitizen', 'emotionista', 'open_for_business') ORDER BY id;\n")
                         .splitlines(),
                         ['dclcitizen\tNode name\tLanded', 'emotionista\tEmotionista\tEmotes purchased',
                          'open_for_business\tOpen for Business\tUpstream edit'])
        self.assertEqual(self.sql(tgt, "SELECT tier_name FROM social.badge_tiers "
                                       "WHERE tier_id = 'emotionista-bronze';\n").strip(), 'Node tier')
