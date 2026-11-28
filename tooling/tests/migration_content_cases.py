"""Builder, camera-reel, badges and events imports against the schema."""
import contextlib
import io
import os
import shlex
import shutil
import time
import unittest.mock
from unittest.mock import patch

from tooling.tests import migration_harness as harness

COLLECTION = '11111111-1111-4111-8111-111111111111'
ITEM = '22222222-2222-4222-8222-222222222222'
PHOTO = '55555555-5555-4555-8555-555555555555'
OWNER = '0x' + 'a' * 40


class ContentMigrationCases:
    def content_load(self, db, source, step_name, rows):
        """Stages `rows` (SQL value tuples, in manifest column order) and runs the step's load as a
        step's first batch does."""
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / source).steps
                    if s.name == step_name)
        defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
        values = ', '.join(f'({row})' for row in rows)
        variables = {'camera-reel-service': "\\set camera_api_url 'https://camera.example.test'\n",
                     'builder-server': "\\set forum_url 'https://forum.example.test'\n"}.get(source, '')
        checks = ''.join(line + '\n' for line in harness.migrate.builder_checks(step, True))
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.sql(db, variables + f'BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\n'
                         f'INSERT INTO stage VALUES {values};\n{checks}{step.load.read_text()}\nCOMMIT;\n')
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

    def import_builder(self, db, name, updated, contract='NULL', lock='NULL'):
        collection, item = self.builder_rows(name, updated, contract, lock)
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
        self.content_load(db, 'builder-server', 'item_content', [
            f"'{ITEM}', 'thumbnail.png', 'bafkzthumb', '{other}', 1, '2026-02-01Z'",
            f"'{ITEM}', 'model.glb', 'bafkbmodel', '{other}', 0, '2026-02-01Z'"])
        self.assertEqual(self.sql(db, writers).strip(), f'model.glb {other},thumbnail.png {OWNER.lower()}')

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
        later_save = [f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-03-01Z'"]
        self.content_load(db, 'builder-server', 'item_content', later_save)
        self.assertEqual(self.sql(db, f"SELECT count(*) FROM builder.item_contents WHERE item_id = '{ITEM}';\n").strip(), '0')
        self.import_builder(db, 'Upstream edited', '2026-03-01Z')
        self.content_load(db, 'builder-server', 'item_content', later_save)
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

        def upstream(name, status, updated, lock, managers):
            self.import_builder(db, name, updated, lock=lock)
            self.content_load(db, 'builder-server', 'virtual_third_party', [
                self.virtual_third_party_row(provider, managers, f'tp:1:{name}:', 'false', updated)])
            self.content_load(db, 'builder-server', 'collection_curation', [
                self.collection_curation(collection_curation, COLLECTION, status, updated=updated)])
            self.content_load(db, 'builder-server', 'item_curation', [
                f"'{item_curation}', '{item}', '{status}', '{self.HASH}', false, '2026-01-01Z', '{updated}'"])
        upstream('First', 'pending', '2026-02-01Z', 'NULL', [OWNER, self.STRANGER])
        upstream('Second', 'approved', '2026-03-01Z', "'2026-03-01 12:00'", [OWNER])
        self.assertEqual(self.sql(db, f"""SELECT (SELECT name FROM builder.collections WHERE id = '{COLLECTION}'),
                                                 (SELECT lock FROM builder.collections WHERE id = '{COLLECTION}'),
                                                 (SELECT name FROM builder.items WHERE id = '{ITEM}'),
                                                 (SELECT raw_metadata FROM builder.virtual_third_parties),
                                                 (SELECT managers FROM builder.virtual_third_parties),
                                                 (SELECT status FROM builder.collection_curations),
                                                 (SELECT status FROM builder.item_curations);\n""").strip(),
                         f'Second\t2026-03-01 12:00:00\tSecond\ttp:1:Second:\t{{{OWNER}}}\tapproved\tapproved')

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

    def test_when_upstream_renamed_an_untouched_third_party_collection_it_should_move_it_and_its_items_together(self):
        db = self.curated_world('content_builder_unit_untouched')
        item, other = '66666666-6666-4666-8666-666666666666', '67666666-6666-4666-8666-666666666667'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(item, 'hats')[1], self.linked_rows(other, 'hats')[1]])
        collection, row = self.linked_rows(item, 'caps', '2026-03-02Z', '2026-03-02Z', 'Upstream hat')
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:caps\t{self.PROVIDER}:caps:{item[:8]} Linked hat,"
                                               f"{self.PROVIDER}:caps:{other[:8]} Linked hat\tt")
        self.content_load(db, 'builder-server', 'item', [row])
        self.assertEqual(self.linked_unit(db), f"{self.PROVIDER}:caps\t{self.PROVIDER}:caps:{item[:8]} Upstream hat,"
                                               f"{self.PROVIDER}:caps:{other[:8]} Linked hat\tt")

    def test_when_upstream_holds_an_item_whose_urn_differs_from_its_collections_it_should_leave_it_out(self):
        db = self.curated_world('content_builder_urn_fits')
        fits, diverges = '66666666-6666-4666-8666-666666666667', '66666666-6666-4666-8666-666666666668'
        standard = self.builder_rows('Upstream', '2026-02-01Z')[1].replace(ITEM, fits)
        said = self.content_load(db, 'builder-server', 'item', [standard, self.linked_rows(diverges, 'caps')[1]])
        self.assertEqual(self.sql(db, f"SELECT string_agg(id::text, ',' ORDER BY id) FROM builder.items "
                                      f"WHERE id IN ('{fits}', '{diverges}');\n").strip(), fits)
        self.assertIn(f'1 new items whose URN differs from their collection', said)
        self.assertIn(diverges, said)

    def test_when_a_node_save_lands_while_item_content_runs_it_should_keep_the_saved_files_and_refuse_the_next_run(self):
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
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server').steps
                    if s.name == 'item_content')
        load = step.load.read_text()
        cut = load.index('DO $missing$')
        save = (f"BEGIN; {parent}"
                f"UPDATE builder.items SET updated_at = '2026-04-01' WHERE id = '{ITEM}'; "
                f"INSERT INTO builder.item_contents (item_id, file, hash, position, named_by) "
                f"VALUES ('{ITEM}', 'model.glb', 'bafknodemodel', 0, '{OWNER}') "
                f"ON CONFLICT (item_id, file) DO UPDATE SET hash = EXCLUDED.hash; COMMIT;")
        node = f"\\! (psql {shlex.quote(self.url(db))} -X -q -c {shlex.quote(save)} >/dev/null 2>&1 &)\nSELECT pg_sleep(1);\n"
        defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
        checks = ''.join(line + '\n' for line in harness.migrate.builder_checks(step, True))
        staged = f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-03-01Z'"
        self.sql(db, "\\set forum_url 'https://forum.example.test'\n"
                     f"BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\n"
                     f"INSERT INTO stage VALUES ({staged});\n"
                     + checks + load[:cut] + node + load[cut:] + "COMMIT;\n")
        for _ in range(100):
            if self.sql(db, f"SELECT updated_at FROM builder.items WHERE id = '{ITEM}';\n").strip() == '2026-04-01 00:00:00':
                break
            time.sleep(0.1)
        self.assertEqual(self.sql(db, f"SELECT string_agg(file || ' ' || hash, ','), max(i.updated_at) "
                                      f"FROM builder.item_contents c JOIN builder.items i ON i.id = c.item_id "
                                      f"WHERE c.item_id = '{ITEM}';\n").strip(),
                         'model.glb bafknodemodel\t2026-04-01 00:00:00')
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'builder.items: 1 rows.*import no longer applies'):
            self.content_load(db, 'builder-server', 'item_content', [staged])

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

    def linked_curations(self, db):
        return self.sql(db, f"""SELECT (SELECT count(*) FROM builder.item_curations k JOIN builder.items i ON i.id = k.item_id
                                         WHERE i.collection_id = '{self.LINKED}'),
                                       (SELECT count(*) FROM builder.collection_curations
                                         WHERE collection_id = '{self.LINKED}');\n""").strip()

    def duplicate_urns(self, db):
        """Item parts held twice within one provider, and third-party collection URNs held twice."""
        return self.sql(db, "SELECT (SELECT count(*) FROM (SELECT c.third_party_id, lower(substring(i.urn_suffix FROM '[^:]*$')) "
                            "FROM builder.items i JOIN builder.collections c ON c.id = i.collection_id "
                            "WHERE c.third_party_id IS NOT NULL AND i.urn_suffix IS NOT NULL GROUP BY 1, 2 HAVING count(*) > 1) d), "
                            "(SELECT count(*) FROM (SELECT urn_suffix FROM builder.collections "
                            "WHERE third_party_id IS NOT NULL GROUP BY 1 HAVING count(*) > 1) d);\n").strip()

    def test_when_upstream_deleted_an_item_and_created_it_again_under_its_urn_it_should_leave_one_item_with_it(self):
        db = self.curated_world('content_builder_recreated_urn')
        gone, again = '77777777-7777-4777-8777-777777777773', '88888888-7777-4777-8777-777777777774'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(gone, 'hats')[1]])
        recreated = self.linked_rows(again, 'hats', item_updated='2026-03-01Z')[1].replace(
            f':hats:{again[:8]}', f':hats:{gone[:8]}')
        said = self.content_load(db, 'builder-server', 'item', [recreated])
        self.assertEqual(self.duplicate_urns(db), '0\t0', said)
        self.assertIn(f'1 items whose URN another item holds are left out, first: {again}', said)

    def test_when_another_item_of_the_provider_holds_an_items_part_it_should_leave_the_item_out(self):
        db = self.curated_world('content_builder_item_part_taken')
        caps = '35555555-3333-4333-8333-333333333333'
        self.content_load(db, 'builder-server', 'collection', [self.linked_rows(ITEM, 'caps')[0].replace(self.LINKED, caps)])
        holder, same, other = ('77777777-7777-4777-8777-777777777775', '78777777-7777-4777-8777-777777777776',
                               '79777777-7777-4777-8777-777777777776')
        self.content_load(db, 'builder-server', 'item', [
            self.linked_rows(holder, 'hats')[1].replace(f':hats:{holder[:8]}', ':hats:shared')])
        in_caps = self.linked_rows(other, 'caps')[1].replace(self.LINKED, caps).replace(f':caps:{other[:8]}', ':caps:Shared')
        for id, row in ((same, self.linked_rows(same, 'hats')[1].replace(f':hats:{same[:8]}', ':hats:SHARED')),
                        (other, in_caps)):
            said = self.content_load(db, 'builder-server', 'item', [row])
            self.assertEqual(self.duplicate_urns(db), '0\t0', said)
            self.assertIn(f'1 items whose URN another item holds are left out, first: {id}', said)

        db = self.curated_world('content_builder_moved_urn')
        moved, holder = '77777777-7777-4777-8777-777777777777', '79777777-7777-4777-8777-777777777777'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(moved, 'hats')[1], self.linked_rows(holder, 'hats')[1]])
        refreshed = self.linked_rows(moved, 'hats', item_updated='2026-03-01Z')[1].replace(
            f':hats:{moved[:8]}', f':hats:{holder[:8].upper()}')
        said = self.content_load(db, 'builder-server', 'item', [refreshed])
        self.assertEqual(self.duplicate_urns(db), '0\t0', said)
        self.assertIn(f'1 items whose URN another item holds are left out, first: {moved}', said)

    def test_when_one_upstream_batch_holds_two_items_of_a_provider_under_one_item_part_it_should_leave_both_out(self):
        db = self.curated_world('content_builder_item_part_batch')
        first, second = '77777777-7777-4777-8777-77777777777b', '78777777-7777-4777-8777-77777777777b'
        said = self.content_load(db, 'builder-server', 'item', [
            self.linked_rows(first, 'hats')[1].replace(f':hats:{first[:8]}', ':hats:twin'),
            self.linked_rows(second, 'hats')[1].replace(f':hats:{second[:8]}', ':hats:Twin')])
        self.assertEqual(self.sql(db, f"SELECT count(*) FROM builder.items WHERE id IN ('{first}', '{second}');\n").strip(),
                         '0', said)
        self.assertIn(f'2 items whose URN another item holds are left out, first: {first}, {second}', said)

    def test_when_upstream_holds_providers_differing_by_case_it_should_import_the_collections_and_items_of_each(self):
        db = self.curated_world('content_builder_provider_case')
        variant = self.PROVIDER.replace(':curated', ':Curated')
        upper, lower = '9b000000-0000-4000-8000-000000000004', '9b000000-0000-4000-8000-000000000005'
        hats = '35555555-3333-4333-8333-333333333334'
        self.content_load(db, 'builder-server', 'item', [self.linked_rows(lower, 'hats')[1]])
        said = self.content_load(db, 'builder-server', 'collection', [
            self.linked_rows(ITEM, 'hats')[0].replace(self.LINKED, hats).replace(self.PROVIDER, variant)])
        said += self.content_load(db, 'builder-server', 'item', [
            self.linked_rows(upper, 'hats')[1].replace(self.LINKED, hats).replace(self.PROVIDER, variant)
            .replace(f':hats:{upper[:8]}', f':hats:{lower[:8]}'),
            self.linked_rows(lower, 'hats', item_updated='2026-03-01Z', name='Renamed')[1]])
        self.assertEqual(self.sql(db, f"""SELECT (SELECT count(*) FROM builder.collections WHERE id = '{hats}'),
                                                 (SELECT count(*) FROM builder.items WHERE id = '{upper}'),
                                                 (SELECT name FROM builder.items WHERE id = '{lower}');\n""").strip(),
                         '1\t1\tRenamed', said)

    def test_when_another_collection_holds_a_third_party_collections_urn_it_should_leave_the_collection_out(self):
        db = self.curated_world('content_builder_collection_urn')
        holder, taken, variant, boots, twin = ('77777777-7777-4777-8777-777777777778', '77777777-7777-4777-8777-777777777779',
                                               '77777777-7777-4777-8777-77777777777a', '77777777-7777-4777-8777-77777777777b',
                                               '77777777-7777-4777-8777-77777777777c')
        self.content_load(db, 'builder-server', 'collection', [self.linked_rows(ITEM, 'caps')[0].replace(self.LINKED, holder)])
        said = self.content_load(db, 'builder-server', 'collection', [
            self.linked_rows(ITEM, 'caps')[0].replace(self.LINKED, taken),
            self.linked_rows(ITEM, 'Caps')[0].replace(self.LINKED, variant),
            self.linked_rows(ITEM, 'boots')[0].replace(self.LINKED, boots),
            self.linked_rows(ITEM, 'boots')[0].replace(self.LINKED, twin),
            self.linked_rows(ITEM, 'caps', '2026-03-01Z')[0]])
        self.assertEqual(self.duplicate_urns(db), '0\t0', said)
        self.assertEqual(self.sql(db, f"SELECT string_agg(id || ' ' || urn_suffix, ',' ORDER BY id) FROM builder.collections "
                                      f"WHERE id IN ('{self.LINKED}', '{taken}', '{variant}', '{boots}', '{twin}');\n").strip(),
                         f'{self.LINKED} {self.PROVIDER}:hats,{variant} {self.PROVIDER}:Caps')
        self.assertIn(f'4 third-party collections whose URN another collection holds are left out, first: '
                      f'{self.LINKED}, {taken}, {boots}, {twin}', said)

    SIGNATURE = '0x' + 'ef' * 65
    TP_ITEM = '66666666-6666-4666-8666-666666666666'
    STRANGER = '0x' + 'e' * 40

    def imported_world(self, name):
        """Every table the import loads holding rows a run left there, as with creator writes closed."""
        db = self.curated_world(name)
        self.linked_item(db, self.TP_ITEM)
        self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                     "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
        self.content_load(db, 'builder-server', 'item_content', [
            f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'"])
        self.content_load(db, 'builder-server', 'virtual_third_party', [
            self.virtual_third_party_row(self.PROVIDER, [OWNER], 'tp:1:Curated:', 'false', '2026-02-01Z')])
        self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation('c0000000-0000-4000-8000-000000000001', self.LINKED, 'pending')])
        self.content_load(db, 'builder-server', 'item_curation', [
            f"'d0000000-0000-4000-8000-000000000001', '{self.TP_ITEM}', 'pending', '{self.HASH}', false, "
            f"'2026-01-01Z', '2026-01-02Z'"])
        self.content_load(db, 'builder-server', 'collection_event', [
            f"'e0000000-0000-4000-8000-000000000001', '{self.LINKED}', 'review.comment', 'curator', NULL, '{{}}', "
            f"'2026-01-03Z'"])
        self.content_load(db, 'builder-server', 'slot_usage_cheque', [
            f"'f0000000-0000-4000-8000-000000000001', '{self.LINKED}', '{self.PROVIDER}', '{self.SIGNATURE}', "
            f"'0x{'12' * 32}', 1, '2026-01-03Z', '2026-01-03Z'"])
        return db

    def clone(self, template, name):
        real = self.db_name(name)
        self.sql('postgres', f'DROP DATABASE IF EXISTS {real};\nCREATE DATABASE {real} TEMPLATE {self.db_name(template)};\n')
        return name

    def builder_digest(self, db):
        """Every row of every table the import loads, its ledger and its checkpoints."""
        tables = ['builder.collections', 'builder.items', 'builder.item_contents', 'builder.uploaded_contents',
                  'builder.virtual_third_parties', 'builder.collection_curations', 'builder.item_curations',
                  'builder.collection_events', 'builder.slot_usage_cheques', 'dcl_migration.builder_imported',
                  'dcl_migration.checkpoint']
        return self.sql(db, 'SELECT ' + ', '.join(
            f"(SELECT md5(COALESCE(string_agg(t::text, ',' ORDER BY t::text), '')) FROM {t} t)" for t in tables) + ';\n')

    def node_writes(self):
        """For each table the import loads: a row the node created, changed and deleted, with the id it reports."""
        new = '9e000000-0000-4000-8000-0000000000aa'
        item_content, uploaded = f'{ITEM}:model.glb', f'{OWNER}:bafkamodel'
        return {
            'builder.collections': [
                (f"INSERT INTO builder.collections (id, name, eth_address, salt) VALUES ('{new}', 'Node', '{self.STRANGER}', '')", new),
                (f"UPDATE builder.collections SET forum_claimed_at = '2026-03-01' WHERE id = '{self.LINKED}'", self.LINKED),
                (f"DELETE FROM builder.collections WHERE id = '{COLLECTION}'", COLLECTION)],
            'builder.items': [
                (f"INSERT INTO builder.items (id, eth_address, name, type) VALUES ('{new}', '{self.STRANGER}', 'Loose', 'wearable')", new),
                (f"UPDATE builder.items SET data = '{{\"category\": \"hat\"}}' WHERE id = '{self.TP_ITEM}'", self.TP_ITEM),
                (f"DELETE FROM builder.items WHERE id = '{self.TP_ITEM}'", self.TP_ITEM)],
            'builder.item_contents': [
                (f"INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ('{ITEM}', 'x.png', 'bafkx', '{OWNER}')",
                 f'{ITEM}:x.png'),
                (f"UPDATE builder.item_contents SET position = 3", item_content),
                ("DELETE FROM builder.item_contents", item_content)],
            'builder.uploaded_contents': [
                (f"INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ('bafkx', 1, '{self.STRANGER}')",
                 f'{self.STRANGER}:bafkx'),
                ("UPDATE builder.uploaded_contents SET video = true", uploaded),
                ("DELETE FROM builder.uploaded_contents", uploaded)],
            'builder.virtual_third_parties': [
                (f"INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) "
                 f"VALUES ('{self.PROVIDER}x', ARRAY['{self.STRANGER}'], 'tp:1:Squat::')", f'{self.PROVIDER}x'),
                (f"UPDATE builder.virtual_third_parties SET managers = ARRAY['{self.STRANGER}']", self.PROVIDER),
                ("DELETE FROM builder.virtual_third_parties", self.PROVIDER)],
            'builder.collection_curations': [
                (f"INSERT INTO builder.collection_curations (id, collection_id, status) VALUES ('{new}', '{COLLECTION}', 'approved')", new),
                ("UPDATE builder.collection_curations SET status = 'approved'", 'c0000000-0000-4000-8000-000000000001'),
                ("DELETE FROM builder.collection_curations", 'c0000000-0000-4000-8000-000000000001')],
            'builder.item_curations': [
                (f"INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ('{new}', '{ITEM}', 'pending', '{self.HASH}')", new),
                (f"UPDATE builder.item_curations SET content_hash = '{'cd' * 32}'", 'd0000000-0000-4000-8000-000000000001'),
                ("DELETE FROM builder.item_curations", 'd0000000-0000-4000-8000-000000000001')],
            'builder.collection_events': [
                (f"INSERT INTO builder.collection_events (id, collection_id, type, actor) VALUES ('{new}', '{COLLECTION}', 'review.comment', 'creator')", new),
                ("UPDATE builder.collection_events SET payload = '{\"message\": \"node\"}'", 'e0000000-0000-4000-8000-000000000001'),
                ("DELETE FROM builder.collection_events", 'e0000000-0000-4000-8000-000000000001')],
            'builder.slot_usage_cheques': [
                (f"INSERT INTO builder.slot_usage_cheques (id, collection_id, third_party_id, signature, salt, qty) "
                 f"VALUES ('{new}', '{self.LINKED}', '{self.PROVIDER}', '{self.SIGNATURE}', '0x{'34' * 32}', 1)", new),
                ("UPDATE builder.slot_usage_cheques SET qty = 2", 'f0000000-0000-4000-8000-000000000001'),
                ("DELETE FROM builder.slot_usage_cheques", 'f0000000-0000-4000-8000-000000000001')],
        }

    def test_when_the_node_took_builder_writes_after_an_import_every_step_should_refuse_before_writing(self):
        world = self.imported_world('content_builder_untouched_world')
        upstream = {'collection': self.linked_rows(ITEM, 'hats', '2026-03-01Z')[0],
                    'item': self.linked_rows(self.TP_ITEM, 'hats', item_updated='2026-03-01Z', name='Upstream')[1],
                    'item_content': f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 2, '2026-02-01Z'",
                    'virtual_third_party': self.virtual_third_party_row(self.PROVIDER, [OWNER], 'tp:1:Later:', 'false', '2026-03-01Z'),
                    'collection_curation': self.collection_curation('c0000000-0000-4000-8000-000000000002', self.LINKED,
                                                                    'approved', created='2026-03-01Z', updated='2026-03-01Z'),
                    'item_curation': f"'d0000000-0000-4000-8000-000000000002', '{self.TP_ITEM}', 'approved', '{self.HASH}', "
                                     f"false, '2026-03-01Z', '2026-03-01Z'",
                    'collection_event': f"'e0000000-0000-4000-8000-000000000002', '{self.LINKED}', 'review.comment', "
                                        f"'curator', NULL, '{{}}', '2026-03-01Z'",
                    'slot_usage_cheque': f"'f0000000-0000-4000-8000-000000000002', '{self.LINKED}', '{self.PROVIDER}', "
                                         f"'{self.SIGNATURE}', '0x{'56' * 32}', 1, '2026-03-01Z', '2026-03-01Z'"}
        steps = list(upstream)
        self.assertEqual(self.content_load(self.clone(world, 'content_builder_untouched_rerun'), 'builder-server',
                                           'collection', [upstream['collection']]), '')
        n = 0
        for table, writes in self.node_writes().items():
            for kind, (write, id) in zip(('created', 'changed', 'deleted'), writes):
                step = steps[n % len(steps)]
                n += 1
                with self.subTest(table=table, kind=kind, step=step):
                    db = self.clone(world, 'content_builder_untouched_case')
                    self.sql(db, write + ';\n')
                    before = self.builder_digest(db)
                    with self.assertRaisesRegex(harness.migrate.MigrationError,
                                                f'{table}: 1 rows, first: {id}.*import no longer applies'):
                        self.content_load(db, 'builder-server', step, [upstream[step]])
                    self.assertEqual(self.builder_digest(db), before)

    def test_when_the_node_holds_builder_rows_before_the_first_run_it_should_refuse_before_writing(self):
        db = self.content_db('content_builder_first_run_loose_item')
        loose = '77777777-7777-4777-8777-777777777777'
        self.sql(db, f"INSERT INTO builder.items (id, eth_address, name, type) VALUES ('{loose}', '{OWNER}', 'Node', 'wearable');\n")
        collection, item = self.builder_rows('Upstream', '2026-02-01Z')
        for step, row in (('collection', collection), ('item', item)):
            with self.assertRaisesRegex(harness.migrate.MigrationError, f'builder.items: 1 rows, first: {loose}'):
                self.content_load(db, 'builder-server', step, [row])
        self.assertEqual(self.sql(db, "SELECT (SELECT count(*) FROM builder.collections), "
                                      "(SELECT count(*) FROM dcl_migration.builder_imported);\n").strip(), '0\t0')

    def test_when_a_stranger_took_a_pending_providers_virtual_third_party_on_the_node_the_next_run_should_import_none_of_its_collections(self):
        db = self.imported_world('content_builder_pending_provider_squat')
        pending = 'urn:decentraland:matic:collections-thirdparty:pending'
        victim, theirs = '0x' + 'b' * 40, '9b000000-0000-4000-8000-000000000002'
        self.sql(db, f"""INSERT INTO builder.collections (id, name, eth_address, salt, urn_suffix, third_party_id)
                              VALUES ('9e000000-0000-4000-8000-000000000001', 'Squat', '{self.STRANGER}', '', '{pending}:squat', '{pending}');
                         INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata)
                              VALUES ('{pending}', ARRAY['{self.STRANGER}'], 'tp:1:Squat::');\n""")
        before = self.builder_digest(db)
        for step, row in (('collection', f"'{theirs}', 'Victim', '{victim}', NULL, NULL, '{pending}:hats', '{pending}', "
                                         f"'2026-01-01Z', '2026-02-01Z', NULL, NULL, NULL, NULL, NULL"),
                          ('virtual_third_party', self.virtual_third_party_row(pending, [victim], 'tp:1:Pending:', 'false', '2026-02-01Z'))):
            with self.assertRaisesRegex(harness.migrate.MigrationError, f'builder.virtual_third_parties: 1 rows, first: {pending}'):
                self.content_load(db, 'builder-server', step, [row])
        self.assertEqual(self.builder_digest(db), before)

    def test_when_a_stranger_saved_a_case_variant_providers_rows_on_the_node_the_next_run_should_refuse_before_writing(self):
        db = self.imported_world('content_builder_case_variant_squat')
        variant = self.PROVIDER.replace(':curated', ':Curated')
        mine = '9e000000-0000-4000-8000-000000000003'
        self.sql(db, f"""INSERT INTO builder.collections (id, name, eth_address, salt, urn_suffix, third_party_id)
                              VALUES ('{mine}', 'Squat', '{self.STRANGER}', '', '{variant}:hats', '{variant}');
                         INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix)
                              VALUES ('9e000000-0000-4000-8000-000000000007', '{mine}', '{self.STRANGER}', 'S1', 'wearable',
                                      '{variant}:hats:{self.TP_ITEM[:8]}');\n""")
        before = self.builder_digest(db)
        with self.assertRaisesRegex(harness.migrate.MigrationError, f'builder.collections: 1 rows, first: {mine}; '
                                                                    f'builder.items: 1 rows'):
            self.content_load(db, 'builder-server', 'item', [
                self.linked_rows(self.TP_ITEM, 'hats', item_updated='2026-03-01Z', name='Renamed')[1]])
        self.assertEqual(self.builder_digest(db), before)

    def test_when_one_id_on_the_node_was_not_imported_the_run_should_refuse_before_writing_any_other_row(self):
        db = self.content_db('content_builder_one_node_id')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        squat, other = '9e000000-0000-4000-8000-00000000000b', '9b000000-0000-4000-8000-00000000000c'
        self.sql(db, f"INSERT INTO builder.collections (id, name, eth_address, salt) VALUES ('{squat}', 'Squat', '{self.STRANGER}', '');\n")
        row = lambda i, n: f"'{i}', '{n}', '{OWNER}', NULL, NULL, NULL, NULL, '2026-01-01Z', '2026-03-01Z', NULL, NULL, NULL, NULL, NULL"
        with self.assertRaisesRegex(harness.migrate.MigrationError,
                                    f'builder.collections: 1 rows, first: {squat}; the node has taken builder writes'):
            self.content_load(db, 'builder-server', 'collection', [row(squat, 'Theirs'), row(other, 'Other')])
        self.assertEqual(self.sql(db, f"SELECT count(*) FROM builder.collections WHERE id = '{other}';\n").strip(), '0')

    def test_when_the_node_deleted_an_imported_collection_and_another_wallet_recreated_it_the_next_run_should_refuse(self):
        db = self.content_db('content_builder_recreated_collection')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, f"""DELETE FROM builder.collections WHERE id = '{COLLECTION}';
                         INSERT INTO builder.collections (id, name, eth_address, salt) VALUES ('{COLLECTION}', 'Mine', '{self.STRANGER}', '');\n""")
        fresh = '9b000000-0000-4000-8000-00000000000a'
        before = self.builder_digest(db)
        with self.assertRaisesRegex(harness.migrate.MigrationError, f'builder.collections: 1 rows, first: {COLLECTION}'):
            self.content_load(db, 'builder-server', 'item', [
                f"'{fresh}', '{COLLECTION}', '{OWNER}', 'New', 'd', NULL, NULL, 'wearable', 'Common', '', '', "
                f"'{{}}', NULL, NULL, NULL, NULL, NULL, false, 0, NULL, NULL, '2026-03-01Z', '2026-03-01Z'"])
        self.assertEqual(self.builder_digest(db), before)

    def test_when_upstream_adds_a_curation_at_the_instant_of_an_imported_one_it_should_keep_the_imported_one(self):
        db = self.curated_world('content_builder_curation_twins')
        first, twin = 'c0000000-0000-4000-8000-000000000001', 'c0000000-0000-4000-8000-000000000002'
        item_first, item_twin = 'd0000000-0000-4000-8000-000000000001', 'd0000000-0000-4000-8000-000000000002'
        self.linked_item(db, self.TP_ITEM)
        self.content_load(db, 'builder-server', 'collection_curation', [self.collection_curation(first, self.LINKED, 'approved')])
        self.content_load(db, 'builder-server', 'item_curation', [
            f"'{item_first}', '{self.TP_ITEM}', 'approved', '{self.HASH}', false, '2026-01-01Z', '2026-01-02Z'"])
        self.content_load(db, 'builder-server', 'collection_curation', [
            self.collection_curation(first, self.LINKED, 'approved'), self.collection_curation(twin, self.LINKED, 'rejected')])
        self.content_load(db, 'builder-server', 'item_curation', [
            f"'{item_first}', '{self.TP_ITEM}', 'approved', '{self.HASH}', false, '2026-01-01Z', '2026-01-02Z'",
            f"'{item_twin}', '{self.TP_ITEM}', 'rejected', '{self.HASH}', false, '2026-01-01Z', '2026-01-02Z'"])
        self.assertEqual(self.sql(db, """SELECT (SELECT string_agg(id || ' ' || status, ',') FROM builder.collection_curations),
                                                (SELECT string_agg(id || ' ' || status, ',') FROM builder.item_curations);\n""").strip(),
                         f'{first} approved\t{item_first} approved')

    def test_when_upstream_changes_only_a_collections_forum_topic_it_should_carry_it_on_reimport(self):
        db = self.content_db('content_builder_forum_refresh')
        collection, _ = self.builder_rows('Forum', '2026-02-01Z')
        self.content_load(db, 'builder-server', 'collection', [collection])
        posted = collection.rsplit('NULL, NULL', 1)[0] + "'https://forum.example.test/t/hat/9', 9"
        self.content_load(db, 'builder-server', 'collection', [posted])
        self.assertEqual(self.sql(db, f"SELECT forum_link, forum_id FROM builder.collections WHERE id = '{COLLECTION}';\n").strip(),
                         'https://forum.example.test/t/hat/9\t9')

    def test_when_an_imported_cheque_holds_a_salt_an_earlier_upstream_cheque_names_it_should_keep_the_imported_one(self):
        db = self.curated_world('content_builder_cheque_salt')
        later = '35555555-3333-4333-8333-333333333335'
        salt = '0x' + '12' * 32

        def cheque(n, collection, created):
            return (f"'f0000000-0000-4000-8000-00000000000{n}', '{collection}', '{self.PROVIDER}', '{self.SIGNATURE}', "
                    f"'{salt}', 1, '{created}', '{created}'")
        cheques = [cheque(1, later, '2026-01-01Z'), cheque(2, self.LINKED, '2026-01-02Z')]
        self.content_load(db, 'builder-server', 'slot_usage_cheque', cheques)
        self.content_load(db, 'builder-server', 'collection', [self.linked_rows(ITEM, 'caps')[0].replace(self.LINKED, later)])
        self.content_load(db, 'builder-server', 'slot_usage_cheque', cheques)
        self.assertEqual(self.sql(db, "SELECT string_agg(id::text, ',') FROM builder.slot_usage_cheques;\n").strip(),
                         'f0000000-0000-4000-8000-000000000002')

    def test_when_a_builder_run_meets_a_node_that_took_writes_it_should_stop_each_step_before_writing(self):
        src, tgt = self.builder_source('builder_refuse_src'), self.content_db('builder_refuse_tgt')
        self.sql(tgt, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                      "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
        env = {'BUILDER_SERVER_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt),
               'FORUM_URL': 'https://forum.example.test'}
        with unittest.mock.patch.dict(os.environ, env):
            code, out = self.migrate('run', 'builder-server', sources_dir=harness.migrate.SOURCES)
            self.assertEqual(code, 0, out)
            self.sql(tgt, f"UPDATE builder.items SET in_catalyst = true WHERE id = '{ITEM}';\n")
            before = self.builder_digest(tgt)
            for step in [s.name for s in harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server').steps]:
                with self.subTest(step=step):
                    err = io.StringIO()
                    with contextlib.redirect_stderr(err):
                        code, out = self.migrate('run', 'builder-server', '--step', step,
                                                 sources_dir=harness.migrate.SOURCES)
                    self.assertNotEqual(code, 0, out)
                    self.assertIn(f'builder.items: 1 rows, first: {ITEM}', out + err.getvalue())
                    self.assertEqual(self.builder_digest(tgt), before)

    def builder_source(self, name):
        """An upstream database holding one row of every table the import reads."""
        src = self.fresh_db(name)
        self.sql(src, f"""CREATE TYPE curation_status AS ENUM ('pending', 'approved', 'rejected');
            CREATE TABLE collections (id uuid PRIMARY KEY, name text, eth_address text, salt text, contract_address text,
                urn_suffix text, third_party_id text, is_published boolean, created_at timestamp, updated_at timestamp,
                linked_contract_address text, linked_contract_network text, lock timestamp, forum_link text, forum_id int);
            CREATE TABLE items (id uuid PRIMARY KEY, collection_id uuid, eth_address text, name text, description text,
                thumbnail text, video text, type text, rarity text, price text, beneficiary text, data json, metrics json,
                utility text, mappings json, urn_suffix text, blockchain_item_id text, total_supply int,
                local_content_hash text, contents json, created_at timestamp, updated_at timestamp);
            CREATE TABLE item_curations (id uuid PRIMARY KEY, item_id uuid, status curation_status, content_hash text,
                is_mapping_complete boolean, created_at timestamp, updated_at timestamp);
            CREATE TABLE collection_curations (id uuid PRIMARY KEY, collection_id uuid, status curation_status,
                assignee text, created_at timestamp, updated_at timestamp);
            CREATE TABLE virtual_third_parties (id text PRIMARY KEY, managers text[], raw_metadata text,
                "isProgrammatic" boolean, created_at timestamp, updated_at timestamp);
            CREATE TABLE collection_events (id uuid PRIMARY KEY, collection_id uuid, type text, actor text,
                actor_address text, payload jsonb, created_at timestamp);
            CREATE TABLE slot_usage_cheques (id uuid PRIMARY KEY, collection_id uuid, third_party_id text, signature text,
                salt text, qty int, created_at timestamp, updated_at timestamp);
            INSERT INTO collections VALUES ('{self.LINKED}', 'Linked', '{OWNER}', '', NULL, 'hats', '{self.PROVIDER}', false,
                '2026-01-01', '2026-01-02', NULL, NULL, NULL, NULL, NULL);
            INSERT INTO items VALUES ('{ITEM}', '{self.LINKED}', '{OWNER}', 'Hat', 'd', NULL, NULL, 'wearable', NULL, NULL,
                NULL, '{{}}', NULL, NULL, NULL, 'hat', NULL, 0, NULL, '{{"model.glb": "bafkamodel"}}', '2026-01-01', '2026-01-02');
            INSERT INTO item_curations VALUES ('d0000000-0000-4000-8000-000000000001', '{ITEM}', 'pending', '{self.HASH}',
                NULL, '2026-01-03', '2026-01-03');
            INSERT INTO collection_curations VALUES ('c0000000-0000-4000-8000-000000000001', '{self.LINKED}', 'pending',
                NULL, '2026-01-03', '2026-01-03');
            INSERT INTO virtual_third_parties VALUES ('{self.PROVIDER}', ARRAY['{OWNER}'], 'tp:1:Curated:', false,
                '2026-01-01', '2026-01-01');
            INSERT INTO collection_events VALUES ('e0000000-0000-4000-8000-000000000001', '{self.LINKED}', 'review.comment',
                'curator', NULL, '{{}}', '2026-01-03');
            INSERT INTO slot_usage_cheques VALUES ('f0000000-0000-4000-8000-000000000001', '{self.LINKED}', '{self.PROVIDER}',
                '0x{'ef' * 65}', '0x{'12' * 32}', 1, '2026-01-03', '2026-01-03');\n""")
        return src

    def test_when_dry_running_the_builder_import_twice_it_should_load_every_step_and_write_nothing(self):
        src, tgt = self.builder_source('builder_dry_src'), self.content_db('builder_dry_tgt')
        self.sql(tgt, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                      "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
        env = {'BUILDER_SERVER_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt),
               'FORUM_URL': 'https://forum.example.test'}
        with unittest.mock.patch.dict(os.environ, env):
            for _ in range(2):
                code, out = self.migrate('run', 'builder-server', '--dry-run', sources_dir=harness.migrate.SOURCES)
                self.assertEqual(code, 0, out)
                self.assertEqual(out.count(': 1 rows (inserted or upserted '), 8, out)
        self.assertEqual(self.sql(tgt, """SELECT (SELECT count(*) FROM builder.collections), (SELECT count(*) FROM builder.items),
                                                 (SELECT count(*) FROM dcl_migration.builder_imported);\n""").strip(), '0\t0\t0')

    REFUSED_FILE_NAMES = {'empty': '', 'long': 'x' * 257, 'backslash': 'models\\hat.glb'}

    def test_when_upstream_names_a_file_the_node_refuses_it_should_leave_that_items_contents_out_and_load_the_others(self):
        other = '7a200000-0000-4000-8000-00000000002a'
        for kind, name in self.REFUSED_FILE_NAMES.items():
            with self.subTest(name=kind):
                db = self.curated_world(f'content_builder_refused_file_name_{kind}')
                self.linked_item(db, other)
                self.sql(db, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                             "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
                said = self.content_load(db, 'builder-server', 'item_content', [
                    f"'{ITEM}', 'model.glb', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'",
                    f"'{other}', 'thumbnail.png', 'bafkamodel', '{OWNER}', 0, '2026-02-01Z'",
                    f"'{other}', '{name}', 'bafkamodel', '{OWNER}', 1, '2026-02-01Z'"])
                self.assertEqual(self.sql(db, "SELECT string_agg(item_id || ' ' || file, ',' ORDER BY item_id) "
                                              "FROM builder.item_contents;\n").strip(), f'{ITEM} model.glb', said)
                self.assertIn(f'1 items whose contents name a file the node refuses are left out with their contents, '
                              f'first: {other}', said)

    def test_when_upstream_saved_an_item_naming_a_file_the_node_refuses_the_run_should_leave_the_item_out_and_finish(self):
        refused = '7a300000-0000-4000-8000-00000000003a'
        for kind, name in self.REFUSED_FILE_NAMES.items():
            with self.subTest(name=kind):
                src = self.builder_source(f'builder_refused_file_src_{kind}')
                tgt = self.content_db(f'builder_refused_file_tgt_{kind}')
                key = name.replace('\\', '\\\\')
                self.sql(src, f"""INSERT INTO items VALUES ('{refused}', '{self.LINKED}', '{OWNER}', 'Cap', 'd', NULL, NULL,
                                      'wearable', NULL, NULL, NULL, '{{}}', NULL, NULL, NULL, 'cap', NULL, 0, NULL,
                                      '{{"thumbnail.png": "bafkamodel", "{key}": "bafkamodel"}}',
                                      '2026-01-01', '2026-01-02');\n""")
                self.sql(tgt, "INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size, content_type) VALUES "
                              "('builder-server', 'bafkamodel', 'bafkamodel', 5, 'model/gltf-binary');\n")
                env = {'BUILDER_SERVER_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt),
                       'FORUM_URL': 'https://forum.example.test'}
                err = io.StringIO()
                with unittest.mock.patch.dict(os.environ, env), contextlib.redirect_stderr(err):
                    code, out = self.migrate('run', 'builder-server', sources_dir=harness.migrate.SOURCES)
                self.assertEqual(code, 0, out + err.getvalue())
                self.assertEqual(self.sql(tgt, "SELECT string_agg(i.id || ' ' || COALESCE(c.file, '-'), ',') FROM builder.items i "
                                               "LEFT JOIN builder.item_contents c ON c.item_id = i.id;\n").strip(),
                                 f'{ITEM} model.glb')
                self.assertIn(f'first: {refused}', out + err.getvalue())

    def start_over(self, db):
        self.sql(db, f"\\i {harness.migrate.psql_path(harness.migrate.SOURCES / 'builder-server' / 'start_over.sql')}\n")

    def test_when_the_linked_publications_table_remains_a_start_over_should_refuse_naming_its_release(self):
        db = self.content_db('content_builder_start_over_linked')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, "CREATE TABLE builder.linked_collection_publications "
                     "(collection_id uuid PRIMARY KEY REFERENCES builder.collections (id));\n")
        before = self.builder_digest(db)
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'builder-linked-publications-retired.sql'):
            self.start_over(db)
        self.assertEqual(self.builder_digest(db), before)

    def test_when_an_earlier_import_kept_no_ledger_a_start_over_should_need_the_operator_to_say_so(self):
        db = self.content_db('content_builder_start_over_unrecorded')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, 'DELETE FROM dcl_migration.builder_imported;\n')
        before = self.builder_digest(db)
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'start_over.sql -v unrecorded_import=on'):
            self.start_over(db)
        self.assertEqual(self.builder_digest(db), before)
        self.sql(db, '\\set unrecorded_import on\n'
                     f"\\i {harness.migrate.psql_path(harness.migrate.SOURCES / 'builder-server' / 'start_over.sql')}\n")
        self.assertEqual(self.sql(db, 'SELECT (SELECT count(*) FROM builder.collections) + '
                                      '(SELECT count(*) FROM builder.items);\n').strip(), '0')

    def test_when_the_ledger_holds_rows_the_unrecorded_import_switch_should_not_skip_the_check(self):
        db = self.content_db('content_builder_start_over_recorded')
        self.import_builder(db, 'Upstream', '2026-02-01Z')
        self.sql(db, f"INSERT INTO builder.collections (id, name, eth_address, salt) "
                     f"VALUES ('9e000000-0000-4000-8000-0000000000aa', 'Mine', '{self.STRANGER}', '');\n")
        before = self.builder_digest(db)
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'unrecorded_import'):
            self.sql(db, '\\set unrecorded_import on\n'
                         f"\\i {harness.migrate.psql_path(harness.migrate.SOURCES / 'builder-server' / 'start_over.sql')}\n")
        self.assertEqual(self.builder_digest(db), before)

    def test_when_upstream_swapped_two_items_parts_a_rerun_should_leave_both_out_and_a_start_over_should_carry_both(self):
        db = self.curated_world('content_builder_item_swap')
        a, b = '7a000000-0000-4000-8000-00000000000a', '7b000000-0000-4000-8000-00000000000b'

        def items(updated, a_part, b_part, a_name='Linked hat', b_name='Linked hat'):
            return [self.linked_rows(a, 'hats', item_updated=updated, name=a_name)[1].replace(f':hats:{a[:8]}', f':hats:{a_part}'),
                    self.linked_rows(b, 'hats', item_updated=updated, name=b_name)[1].replace(f':hats:{b[:8]}', f':hats:{b_part}')]
        parts = f"SELECT string_agg(name || ' ' || substring(urn_suffix FROM '[^:]*$'), ',' ORDER BY id) " \
                f"FROM builder.items WHERE id IN ('{a}', '{b}');\n"
        self.content_load(db, 'builder-server', 'item', items('2026-02-01Z', 'left', 'right'))
        swapped = items('2026-03-01Z', 'right', 'left', 'A now right', 'B now left')
        said = self.content_load(db, 'builder-server', 'item', swapped)
        self.assertEqual(self.sql(db, parts).strip(), 'Linked hat left,Linked hat right', said)
        self.assertIn(f'2 items whose URN another item holds are left out, first: {a}, {b}; if upstream holds no such '
                      f'duplicate (a swap, or a row deleted and created again), start over', said)
        self.start_over(db)
        self.content_load(db, 'builder-server', 'collection', [self.linked_rows(a, 'hats')[0]])
        self.content_load(db, 'builder-server', 'item', swapped)
        self.assertEqual(self.sql(db, parts).strip(), 'A now right right,B now left left')

    def test_when_upstream_swapped_two_collections_urns_a_rerun_should_leave_both_out_and_a_start_over_should_carry_both(self):
        db = self.curated_world('content_builder_collection_swap')
        other = '7c000000-0000-4000-8000-00000000000c'
        self.content_load(db, 'builder-server', 'collection', [self.linked_rows(ITEM, 'caps')[0].replace(self.LINKED, other)])
        swapped = [self.linked_rows(ITEM, 'caps', '2026-03-01Z')[0],
                   self.linked_rows(ITEM, 'hats', '2026-03-01Z')[0].replace(self.LINKED, other)]
        urns = (f"SELECT string_agg(substring(urn_suffix FROM '[^:]*$'), ',' ORDER BY id) "
                f"FROM builder.collections WHERE id IN ('{self.LINKED}', '{other}');\n")
        said = self.content_load(db, 'builder-server', 'collection', swapped)
        self.assertEqual(self.sql(db, urns).strip(), 'hats,caps', said)
        self.assertIn('2 third-party collections whose URN another collection holds are left out', said)
        self.assertIn('start over', said)
        self.start_over(db)
        self.content_load(db, 'builder-server', 'collection', swapped)
        self.assertEqual(self.sql(db, urns).strip(), 'caps,hats')

    def test_when_upstream_deleted_a_collection_and_created_it_again_under_its_urn_a_start_over_should_join_its_items(self):
        db = self.curated_world('content_builder_collection_recreated')
        item, again, new_item = ('7d000000-0000-4000-8000-00000000000d', '7e000000-0000-4000-8000-00000000000e',
                                 '7f000000-0000-4000-8000-00000000000f')
        self.linked_item(db, item)
        collection = self.linked_rows(ITEM, 'hats', '2026-03-01Z')[0].replace(self.LINKED, again)
        new_row = self.linked_rows(new_item, 'hats', item_updated='2026-03-01Z')[1].replace(self.LINKED, again)
        joined = (f"SELECT (SELECT count(*) FROM builder.collections WHERE id = '{again}'), "
                  f"(SELECT collection_id FROM builder.items WHERE id = '{new_item}');\n")
        said = self.content_load(db, 'builder-server', 'collection', [collection])
        self.content_load(db, 'builder-server', 'item', [new_row])
        self.assertEqual(self.sql(db, joined).strip('\n'), '0\t', said)
        self.assertIn(f'first: {again}; if upstream holds no such duplicate', said)
        self.start_over(db)
        self.content_load(db, 'builder-server', 'collection', [collection])
        self.content_load(db, 'builder-server', 'item', [new_row])
        self.assertEqual(self.sql(db, joined).strip(), f'1\t{again}')

    def test_when_the_node_took_builder_writes_a_start_over_should_refuse_and_empty_nothing(self):
        world = self.imported_world('content_builder_start_over_world')
        for write, refusal in (
                (f"UPDATE builder.items SET name = 'Node' WHERE id = '{self.TP_ITEM}'",
                 f'builder.items: 1 rows, first: {self.TP_ITEM}.*import no longer applies'),
                (f"INSERT INTO builder.collection_publications (collection_id, preparation, status) "
                 f"VALUES ('{COLLECTION}', '{{\"revision\": 1}}', 'prepared')", 'collection publications'),
                (f"INSERT INTO builder.validation_requests (id, collection_id, trigger, item_ids, created_at) "
                 f"VALUES (gen_random_uuid(), '{COLLECTION}', 'publish', ARRAY['{ITEM}'::uuid], '2026-03-01')",
                 'validation requests')):
            with self.subTest(write=write):
                db = self.clone(world, 'content_builder_start_over_refused')
                self.sql(db, write + ';\n')
                before = self.builder_digest(db)
                with self.assertRaisesRegex(harness.migrate.MigrationError, refusal):
                    self.start_over(db)
                self.assertEqual(self.builder_digest(db), before)

    STARTED_OVER = ['builder.collections', 'builder.items', 'builder.item_contents', 'builder.uploaded_contents',
                    'builder.virtual_third_parties', 'builder.collection_curations', 'builder.item_curations',
                    'builder.collection_events', 'builder.slot_usage_cheques', 'dcl_migration.builder_imported']
    KEPT = ['builder.terms_acceptances', 'builder.newsletter_subscriptions', 'builder.validator_seen_signatures',
            'dcl_migration.blob_inventory']

    def counts(self, db, tables):
        return self.sql(db, 'SELECT ' + ', '.join(f'(SELECT count(*) FROM {t})' for t in tables) + ';\n').split()

    def test_when_starting_over_an_imported_node_it_should_empty_what_the_import_loaded_and_keep_the_rest(self):
        db = self.imported_world('content_builder_start_over_emptied')
        self.sql(db, f"""INSERT INTO dcl_migration.checkpoint (source, step, walk_started_at) VALUES
                             ('builder-server', 'item', now()), ('worlds-content-server', 'world', now());
                         INSERT INTO builder.terms_acceptances (id, collection_id, signer, event, body)
                             VALUES (gen_random_uuid(), '{COLLECTION}', '{OWNER}', 'publish_collection_tos', '{{}}');
                         INSERT INTO builder.newsletter_subscriptions (email, source) VALUES ('jane.doe@example.com', 'builder');
                         INSERT INTO builder.validator_seen_signatures (signature, expires_at) VALUES ('{'ab' * 32}', '2026-12-01');\n""")
        self.assertNotIn('0', self.counts(db, self.STARTED_OVER + self.KEPT))
        self.start_over(db)
        self.assertEqual(self.counts(db, self.STARTED_OVER), ['0'] * len(self.STARTED_OVER))
        self.assertEqual(self.counts(db, self.KEPT), ['1'] * len(self.KEPT))
        self.assertEqual(self.sql(db, 'SELECT string_agg(source, \',\') FROM dcl_migration.checkpoint;\n').strip(),
                         'worlds-content-server')

    def test_when_a_node_write_is_in_flight_a_start_over_should_wait_for_it_and_then_refuse(self):
        db = self.imported_world('content_builder_start_over_in_flight')
        curation = 'd0000000-0000-4000-8000-0000000000f1'
        write = (f"BEGIN; INSERT INTO builder.item_curations (id, item_id, status, content_hash) "
                 f"VALUES ('{curation}', '{self.TP_ITEM}', 'pending', '{'cd' * 32}'); SELECT pg_sleep(2); COMMIT;")
        self.sql(db, f"\\! (psql {shlex.quote(self.url(db))} -X -q -c {shlex.quote(write)} >/dev/null 2>&1 &)\n")
        holding = ("SELECT count(*) FROM pg_locks l JOIN pg_class c ON c.oid = l.relation "
                   "WHERE c.relname = 'item_curations' AND l.mode = 'RowExclusiveLock' AND l.pid <> pg_backend_pid();\n")
        for _ in range(100):
            if self.sql(db, holding).strip() == '1':
                break
            time.sleep(0.05)
        else:
            self.fail('the node write never began')
        with self.assertRaisesRegex(harness.migrate.MigrationError, f'builder.item_curations: 1 rows, first: {curation}'):
            self.start_over(db)
        self.assertEqual(self.sql(db, f"SELECT count(*) FROM builder.item_curations WHERE id = '{curation}';\n").strip(), '1')

    def test_when_a_node_row_appears_between_a_steps_batches_the_next_batch_should_stop_rather_than_adopt_it(self):
        squat = '7e100000-0000-4000-8000-00000000001e'
        provider = f'{self.PROVIDER}-squat'
        collection, item = self.builder_rows('Victim', '2026-02-01Z')
        stranger = f"SELECT name, eth_address FROM builder.{{}} WHERE id = '{squat}';\n"
        for step_name, write, row, key, kept, expected in (
                ('collection', f"INSERT INTO builder.collections (id, name, eth_address, salt) "
                               f"VALUES ('{squat}', 'Stranger', '{self.STRANGER}', '')",
                 collection.replace(COLLECTION, squat), 'collections_pkey', stranger.format('collections'),
                 f'Stranger\t{self.STRANGER}'),
                ('item', f"INSERT INTO builder.items (id, eth_address, name, type) "
                         f"VALUES ('{squat}', '{self.STRANGER}', 'Stranger', 'wearable')",
                 item.replace(ITEM, squat), 'items_pkey', stranger.format('items'), f'Stranger\t{self.STRANGER}'),
                ('virtual_third_party', f"INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) "
                                        f"VALUES ('{provider}', ARRAY['{self.STRANGER}'], 'tp:1:Node:')",
                 self.virtual_third_party_row(provider, [OWNER], 'tp:1:Upstream:', 'false', '2026-02-01Z'),
                 'virtual_third_parties_pkey',
                 f"SELECT managers, raw_metadata FROM builder.virtual_third_parties WHERE id = '{provider}';\n",
                 f'{{{self.STRANGER}}}\ttp:1:Node:'),
                ('collection_curation', f"INSERT INTO builder.collection_curations (id, collection_id, status, created_at, updated_at) "
                                        f"VALUES ('{squat}', '{COLLECTION}', 'rejected', '2026-01-01', '2026-01-01')",
                 self.collection_curation(squat, COLLECTION, 'approved'), 'collection_curations_pkey',
                 f"SELECT status FROM builder.collection_curations WHERE id = '{squat}';\n", 'rejected'),
                ('item_curation', f"INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at, updated_at) "
                                  f"VALUES ('{squat}', '{ITEM}', 'rejected', '{'cd' * 32}', '2026-01-01', '2026-01-01')",
                 f"'{squat}', '{ITEM}', 'approved', '{self.HASH}', true, '2026-01-01Z', '2026-01-02Z'",
                 'item_curations_pkey',
                 f"SELECT status, content_hash FROM builder.item_curations WHERE id = '{squat}';\n",
                 f"rejected\t{'cd' * 32}")):
            with self.subTest(step=step_name):
                db = self.curated_world(f'content_builder_mid_step_{step_name}')
                self.sql(db, write + ';\n')
                step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / 'builder-server').steps
                            if s.name == step_name)
                defs = ', '.join(f'"{n}" {t}' for n, t in step.columns)
                later_batch = ''.join(line + '\n' for line in harness.migrate.builder_checks(step, False))
                with self.assertRaisesRegex(harness.migrate.MigrationError, key):
                    self.sql(db, "\\set forum_url 'https://forum.example.test'\n"
                                 f"BEGIN;\nCREATE TEMP TABLE stage ({defs}) ON COMMIT DROP;\nINSERT INTO stage VALUES ({row});\n"
                                 f"{later_batch}{step.load.read_text()}\nCOMMIT;\n")
                self.assertEqual(self.sql(db, kept).strip(), expected)

    def test_event_notice_import_bounds_locks_in_batches_and_dry_runs(self):
        src, tgt = self.notifications_dbs()
        self.sql(src, "DELETE FROM notifications WHERE address IS NOT NULL; INSERT INTO notifications "
                     "SELECT md5(g::text)::uuid, 'series', 'events_started', "
                     "'0x' || lpad(to_hex(g),40,'0'), '{}', "
                     "1767225600000, NULL, 1767225600000, 1767225600000 "
                     "FROM generate_series(1,1024) g;")
        self.sql(tgt, """
            CREATE FUNCTION public.check_notice_lock_bound() RETURNS trigger LANGUAGE plpgsql AS $f$
            BEGIN
              IF (SELECT count(*) FROM pg_locks WHERE pid=pg_backend_pid()
                  AND locktype='advisory' AND granted) > 257 THEN
                RAISE EXCEPTION 'event import retained more than 256 wallet bucket locks';
              END IF;
              RETURN NULL;
            END $f$;
            CREATE TRIGGER check_notice_lock_bound BEFORE INSERT ON social.notifications
            FOR EACH STATEMENT EXECUTE FUNCTION public.check_notice_lock_bound();
        """)
        args = ('run', 'notifications-workers', '--step', 'notification', '--batch-size', '4096')
        for dry in (True, False):
            with self.subTest(dry_run=dry):
                code, out = self.migrate(*args, *(('--dry-run',) if dry else ()),
                                         sources_dir=harness.migrate.SOURCES)
                self.assertEqual(code, 0, out)
                self.assertEqual(self.count(tgt, 'social.notifications'), 0 if dry else 1024)
                if dry:
                    self.assertEqual(self.sql(tgt, "SELECT to_regnamespace('dcl_migration') IS NULL;").strip(), 't')
                self.assertEqual(self.count(tgt, 'social.notification_deliveries'), 0)

    def test_event_notice_import_preserves_identity_and_newer_runtime_rows(self):
        db = self.content_db('event_notice_history')
        kinds = ('events_starts_soon', 'events_started', 'event_created',
                 'event_approved', 'event_rejected', 'event_deleted')
        for index, kind in enumerate(kinds):
            source_id = f'00000000-0000-4000-8000-{index:012d}'
            event_id = f'event-{index}'
            self.sql(db, f"INSERT INTO social.notifications(id,address,type,metadata,read,read_at,in_app,occurred_at) "
                         f"VALUES('{source_id}','{OWNER}','{kind}','{{}}',true,'2026-01-01',false,'2026-01-01');")
            row = (f"'{source_id}','{event_id}','{OWNER}','{kind}','{{\"name\":\"Imported\"}}',"
                   "'2026-02-01Z',NULL,'2026-01-01Z','2026-02-01Z'")
            self.content_load(db, 'notifications-workers', 'notification', [row])
            query = (f"SELECT id,metadata->>'eventId',broadcast_key,read,in_app FROM social.notifications "
                     f"WHERE address='{OWNER}' AND type='{kind}';")
            self.assertEqual(self.sql(db, query).strip(),
                             f'{source_id}\t{event_id}\tevent-notice:{kind}:{event_id}\tt\tf')
            self.sql(db, f"UPDATE social.notifications SET metadata=metadata || "
                         "'{\"name\":\"Newer node\",\"occurredAt\":\"2026-03-01T00:00:00Z\"}' "
                         f"WHERE id='{source_id}';")
            self.content_load(db, 'notifications-workers', 'notification', [row])
            self.assertEqual(self.sql(db, f"SELECT metadata->>'name' FROM social.notifications "
                                          f"WHERE id='{source_id}';").strip(), 'Newer node')
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM social.notification_deliveries;').strip(), '0')

    def test_when_copying_event_posters_it_should_index_the_named_ones_and_keep_the_nodes(self):
        db = self.content_db('event_posters')
        patcher = patch.dict(os.environ, {'TARGET_DATABASE_URL': self.url(db)})
        patcher.start()
        self.addCleanup(patcher.stop)
        self.sql(db, """INSERT INTO explore.event(id,name,raw,origin,approved) VALUES
            ('a','A','{"image":"/poster/a.png","image_vertical":"/poster-vertical/v.webp"}','local',true),
            ('b','B','{"image":"/poster/a.png","image_vertical":null}','local',true),
            ('c','C','{"image":"/poster/own.png"}','local',true),
            ('d','D','{"image":"/poster/gone.png"}','local',true),
            ('e','E','{"image":"https://elsewhere.example/poster/x.png"}','local',true),
            ('f','F','{"image_vertical":"https://events-assets-099ac00.decentraland.org/poster-vertical/full_name.png"}',
             'upstream',true);
            INSERT INTO explore.poster VALUES
            ('poster','own.png','bafkreinodeown','image/png',1,'local',now());""")
        upstream, node = self.tmp / 'event-bucket', self.tmp / 'event-node'
        self.addCleanup(shutil.rmtree, upstream, True)
        self.addCleanup(shutil.rmtree, node, True)
        png, webp = harness.blobs.PNG_MAGIC + b'poster', b'RIFF\0\0\0\0WEBPvertical'
        full = harness.blobs.PNG_MAGIC + b'full'
        for key, data in (('poster/a.png', png), ('poster-vertical/v.webp', webp), ('poster/own.png', png),
                          ('poster-vertical/full_name.png', full)):
            (upstream / key).parent.mkdir(parents=True, exist_ok=True)
            (upstream / key).write_bytes(data)
        copy = ('blobs', 'events', '--from-dir', str(upstream), '--to', str(node))
        code, out = self.migrate(*copy, sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn('4 posters named by events and not served here', out)
        self.assertIn('poster/gone.png: missing', out)
        self.assertIn('posters copied 3, already in the store 0, absent upstream 1', out)
        self.assertEqual(self.sql(db, "SELECT kind, file, blob_cid, content_type, byte_size, origin FROM explore.poster"
                                      " ORDER BY kind, file;").strip().splitlines(),
                         [f'poster\ta.png\t{harness.blobs.hash_bytes_v1(png)}\timage/png\t{len(png)}\tupstream',
                          'poster\town.png\tbafkreinodeown\timage/png\t1\tlocal',
                          f'poster-vertical\tfull_name.png\t{harness.blobs.hash_bytes_v1(full)}\timage/png\t{len(full)}'
                          '\tupstream',
                          f'poster-vertical\tv.webp\t{harness.blobs.hash_bytes_v1(webp)}\timage/webp\t{len(webp)}'
                          '\tupstream'])
        self.assertEqual(harness.blobs.Store(node).path(harness.blobs.hash_bytes_v1(png)).read_bytes(), png)
        code, out = self.migrate('blobs', 'events', '--verify', '--to', str(node), sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn('poster/gone.png: not indexed', out)
        self.assertIn('poster/own.png: indexed, blob missing', out)
        self.assertIn('5 posters named, 1 not indexed, 1 with their blob missing', out)
        code, out = self.migrate(*copy, '--allow-missing', sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('1 posters named by events and not served here', out)
        self.assertIn('absent upstream 1', out)
        (upstream / 'poster/gone.png').write_bytes(b'<html>not found</html>')
        code, out = self.migrate(*copy, sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn('1 posters named by events and not served here', out)
        self.assertIn('not an image 1', out)

    def test_when_an_upstream_posters_blob_is_gone_it_should_copy_it_again_and_repoint_the_row(self):
        db = self.content_db('event_poster_repair')
        patcher = patch.dict(os.environ, {'TARGET_DATABASE_URL': self.url(db)})
        patcher.start()
        self.addCleanup(patcher.stop)
        self.sql(db, """INSERT INTO explore.event(id,name,raw,origin,approved) VALUES
            ('a','A','{"image":"/poster/a.png"}','local',true),
            ('b','B','{"image":"/poster/own.png"}','local',true);
            INSERT INTO explore.poster VALUES
            ('poster','a.png','bafkreistale','image/png',1,'upstream',now()),
            ('poster','own.png','bafkreinodeown','image/png',1,'local',now());""")
        upstream, node = self.tmp / 'event-repair-bucket', self.tmp / 'event-repair-node'
        self.addCleanup(shutil.rmtree, upstream, True)
        self.addCleanup(shutil.rmtree, node, True)
        png = harness.blobs.PNG_MAGIC + b'repaired'
        for key in ('poster/a.png', 'poster/own.png'):
            (upstream / key).parent.mkdir(parents=True, exist_ok=True)
            (upstream / key).write_bytes(png)
        code, out = self.migrate('blobs', 'events', '--from-dir', str(upstream), '--to', str(node),
                                 sources_dir=harness.migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('1 posters named by events and not served here', out)
        self.assertEqual(self.sql(db, "SELECT file, blob_cid, byte_size, origin FROM explore.poster ORDER BY file;")
                         .strip().splitlines(),
                         [f'a.png\t{harness.blobs.hash_bytes_v1(png)}\t{len(png)}\tupstream',
                          'own.png\tbafkreinodeown\t1\tlocal'])

    def test_when_extracting_events_it_should_rewrite_bucket_posters_to_node_paths(self):
        src = self.fresh_db('event_extract_posters')
        self.sql(src, """CREATE TABLE events (id uuid, name text, start_at timestamptz, finish_at timestamptz,
            next_start_at timestamptz, next_finish_at timestamptz, duration bigint, x int, y int,
            recurrent_until timestamptz, created_at timestamptz, updated_at timestamptz, deleted_at timestamp,
            recurrent_dates timestamptz[], image text, image_vertical text, highlighted boolean, trending boolean,
            approved boolean, community_id text, "user" text, description text, all_day boolean, server text,
            url text, user_name text, estate_id text, estate_name text, scene_name text, rejected boolean,
            world boolean, recurrent boolean, recurrent_frequency text, recurrent_setpos int,
            recurrent_monthday int, recurrent_weekday_mask int, recurrent_month_mask int, recurrent_interval int,
            recurrent_count int, categories text[], schedules text[], contact text, details text, place_id text,
            featured_item text, approved_by text, rejected_by text, rejection_reason text,
            deleted_by_user boolean, deleted_by_admin boolean, deleted_by text, deleted_reason text,
            previous_place_id text);
            INSERT INTO events (id, name, start_at, duration, updated_at, image, image_vertical) VALUES
            ('00000000-0000-4000-8000-000000000001', 'E', '2026-01-01Z', 1, '2026-01-01Z',
             'https://events-assets-099ac00.decentraland.org/poster/with_underscore.png',
             'https://elsewhere.example/poster-vertical/x.png');""")
        extract = harness.migrate.SOURCES / 'events' / 'event.extract.sql'
        self.assertEqual(self.sql(src, f"SELECT raw->>'image', raw->>'image_vertical' FROM ({extract.read_text()}) x;")
                         .strip(), '/poster/with_underscore.png\thttps://elsewhere.example/poster-vertical/x.png')

    def test_event_attendance_reconcile_preserves_later_local_changes(self):
        db = self.content_db('event_attendance_reconcile')
        self.sql(db, "INSERT INTO explore.event(id,name,raw,origin,approved) "
                     "VALUES('series','Series','{\"approved\":true}','local',true);")
        wallets = ['0x' + c * 40 for c in 'abcdef']
        for wallet in wallets[:4]:
            self.content_load(db, 'events', 'attendance',
                              [f"'series','{wallet}','Imported','2026-01-01Z'"])
        self.sql(db, f"UPDATE explore.event_attendance_local SET action='cancel',signed_at='2026-02-01' "
                     f"WHERE signer='{wallets[1]}';"
                     f"UPDATE explore.event_attendance_local SET signed_at='2026-02-01',signed_payload='{{}}' "
                     f"WHERE signer='{wallets[2]}';"
                     "INSERT INTO explore.event_attendance_local(event_id,signer,action,signed_at) "
                     f"VALUES('series','{wallets[4]}','going','2026-02-01');")
        self.content_load(db, 'events', 'attendance_reconcile', [f"'series','{wallets[3]}'"])
        query = 'SELECT signer,action FROM explore.event_attendance_local ORDER BY signer;'
        expected = '\n'.join(f'{wallet}\t{action}' for wallet, action in
                             [(wallets[1], 'cancel'), (wallets[2], 'going'),
                              (wallets[3], 'going'), (wallets[4], 'going')])
        self.assertEqual(self.sql(db, query).strip(), expected)
        self.content_load(db, 'events', 'attendance_reconcile', [f"'series','{wallets[3]}'"])
        self.assertEqual(self.sql(db, query).strip(), expected)
        # Re-import must not reclaim newer local writes.
        for wallet in wallets[1:3]:
            self.content_load(db, 'events', 'attendance',
                              [f"'series','{wallet}','Imported again','2026-01-01Z'"])
        self.assertEqual(self.sql(db, query).strip(), expected)

    def test_event_notice_import_reuses_a_runtime_carrier_and_merges_case_duplicates(self):
        db = self.content_db('event_notice_carrier')
        runtime = '10000000-0000-4000-8000-000000000001'
        source = '20000000-0000-4000-8000-000000000001'
        self.sql(db, f"INSERT INTO social.notifications(id,address,type,metadata,occurred_at,in_app,read,read_at) "
                     f"VALUES('{runtime}','{OWNER}','event_rejected',"
                     "'{\"eventId\":\"series\",\"occurredAt\":\"2026-03-01T00:00:00Z\",\"name\":\"Runtime\"}',"
                     "'2026-03-01',false,false,NULL);")
        row = (f"'{source}','series','{OWNER}','event_rejected','{{\"name\":\"Old source\"}}',"
               "'2026-01-01Z','2026-02-01Z','2026-01-01Z','2026-01-01Z'")
        self.content_load(db, 'notifications-workers', 'notification', [row])
        self.assertEqual(self.sql(db, "SELECT id,metadata->>'name',in_app,read FROM social.notifications;").strip(),
                         f'{runtime}\tRuntime\tf\tf')
        # Different upstream IDs for one normalized recipient/type/event converge in a single load.
        rows = [(f"'30000000-0000-4000-8000-{i:012d}','second','{OWNER}','events_started',"
                 f"'{{\"name\":\"Version {i}\"}}','2026-01-0{i}Z',NULL,'2026-01-01Z','2026-01-0{i}Z'")
                for i in (1, 2)]
        self.content_load(db, 'notifications-workers', 'notification', rows)
        self.assertEqual(self.sql(db, "SELECT count(*),max(metadata->>'name') FROM social.notifications "
                                     "WHERE type='events_started';").strip(), '1\tVersion 2')

    def test_event_attendance_empty_source_requires_explicit_override(self):
        db = self.content_db('event_attendance_empty')
        self.sql(db, "INSERT INTO explore.event(id,name,raw) VALUES('series','Series','{}');")
        self.content_load(db, 'events', 'attendance', [f"'series','{OWNER}','Imported','2026-01-01Z'"])
        step = next(s for s in harness.migrate.load_source(harness.migrate.SOURCES / 'events').steps
                    if s.name == 'attendance_reconcile')
        script = ('BEGIN; CREATE TEMP TABLE stage(event_id text, signer text) ON COMMIT DROP;\n'
                  + step.load.read_text() + '\nCOMMIT;')
        with self.assertRaisesRegex(harness.migrate.MigrationError, 'upstream lists no attendees'):
            self.sql(db, script)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM explore.event_attendance_local;').strip(), '1')
        self.sql(db, "SET dcl_migration.allow_empty='on'; " + script)
        self.assertEqual(self.sql(db, 'SELECT count(*) FROM explore.event_attendance_local;').strip(), '0')
