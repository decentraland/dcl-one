"""Canonical Camera import with synthetic rows and an isolated disposable database."""
import base64
import contextlib
import csv
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.parse
import uuid

from tooling.tests.test_migration import migrate, blobs, pattern

PHOTO = '12345678-1234-5678-9012-123456789012'
OWNER = '0x' + 'a' * 40
API = 'https://camera.example.test'
META = dict(userName='Camera', userAddress=OWNER, dateTime='opaque source time', realm='realm',
            scene=dict(name='scene', location=dict(x='1', y='2')),
            visiblePeople=[dict(userName='Guest', userAddress=OWNER, wearables=[], isGuest=True,
                                isEmoting=True, screenRect=dict(x=0.1, y=0.2, width=0.3, height=0.4))],
            placeId='place',
            opaque={'KeepCase': True})


def row(**changes):
    values = dict(id=PHOTO, owner=OWNER, url=f'{API}/api/images/{PHOTO}-photo.png',
                  thumbnail=f'{API}/api/images/{PHOTO}-thumbnail-photo.png', public='f',
                  metadata=json.dumps(META), created='2023-07-03 17:42:28.123456+00')
    values.update(changes)
    return list(values.values())


def reference_reduce(nodes, cid_of):
    while len(nodes) > 1:
        parents = []
        for i in range(0, len(nodes), blobs.MAX_CHILDREN):
            batch = nodes[i:i + blobs.MAX_CHILDREN]
            size = sum(c.file_size for c in batch)
            block = blobs._pb_node(blobs._unixfs_file(size, [c.file_size for c in batch]), [(c.cid, c.tsize) for c in batch])
            parents.append(blobs._Node(cid_of(hashlib.sha256(block).digest()), size,
                                 len(block) + sum(c.tsize for c in batch)))
        nodes = parents
    return nodes[0]


class CameraFiles(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.env = patch.dict(os.environ, CAMERA_REEL_IMPORT_API_URL=API)
        self.env.start()
        self.addCleanup(self.env.stop)

    def write(self, values):
        file = self.root / 'rows.csv'
        with file.open('w', newline='') as f:
            csv.writer(f).writerows(values)
        return file

    def test_camera_source_never_uses_gzip_fallback(self):
        (self.root / 'photo.png.gzip').write_bytes(gzip.compress(b'original'))
        source = blobs.FolderSource(self.root, exact_names=True)
        with self.assertRaises(blobs.Missing):
            source.open('photo.png')
        data = gzip.compress(b'these are the actual stored bytes')
        (self.root / 'photo.png').write_bytes(data)
        with source.open('photo.png') as stream:
            self.assertEqual(stream.read(), data)

    def test_exact_source_rejects_outside_symlink_after_metadata_preflight(self):
        snapshot = self.root / 'snapshot'
        snapshot.mkdir()
        key = PHOTO + '-photo.png'
        leaf = snapshot / key
        leaf.write_bytes(b'exported object')
        migrate.camera_preflight(self.write([row()]))
        outside = self.root / 'private-fixture'
        outside.write_bytes(b'harmless private fixture must never be copied')
        leaf.unlink()
        leaf.symlink_to(outside)
        store = blobs.Store(self.root / 'unsafe-store')
        with patch.object(blobs, 'Hasher') as hasher:
            result = blobs.copy_filename(blobs.FolderSource(snapshot, exact_names=True), store, key)
        self.assertEqual(result.status, 'missing')
        hasher.assert_not_called()
        self.assertFalse(store.contents.exists())
        self.assertEqual(outside.read_bytes(), b'harmless private fixture must never be copied')

    def test_exact_source_rejects_fifo_and_directory_without_hashing(self):
        snapshot = self.root / 'special-snapshot'
        snapshot.mkdir()
        for kind in ('fifo', 'directory'):
            with self.subTest(kind=kind):
                key = PHOTO + '-' + kind
                leaf = snapshot / key
                os.mkfifo(leaf) if kind == 'fifo' else leaf.mkdir()
                store = blobs.Store(self.root / ('special-store-' + kind))
                with patch.object(blobs, 'Hasher') as hasher:
                    result = blobs.copy_filename(blobs.FolderSource(snapshot, exact_names=True), store, key)
                self.assertEqual(result.status, 'missing')
                hasher.assert_not_called()
                self.assertFalse(store.contents.exists())

    def test_exact_source_rejects_leaf_replaced_between_stat_and_open(self):
        snapshot = self.root / 'racing-snapshot'
        snapshot.mkdir()
        key = PHOTO + '-photo.png'
        leaf = snapshot / key
        outside = self.root / 'outside-fixture'
        outside.write_bytes(b'never copied')
        original_open = os.open
        for replacement in ('symlink', 'fifo', 'regular'):
            with self.subTest(replacement=replacement):
                leaf.unlink(missing_ok=True)
                leaf.write_bytes(b'preflight regular file')
                def racing_open(path, flags, *args, **kwargs):
                    if path == key:
                        self.assertTrue(flags & os.O_NOFOLLOW)
                        self.assertTrue(flags & os.O_NONBLOCK, 'FIFO race must not block')
                        self.assertIn('dir_fd', kwargs)
                        leaf.unlink()
                        if replacement == 'symlink':
                            leaf.symlink_to(outside)
                        elif replacement == 'fifo':
                            os.mkfifo(leaf)
                        else:
                            leaf.write_bytes(b'changed regular file')
                    return original_open(path, flags, *args, **kwargs)
                store = blobs.Store(self.root / ('racing-store-' + replacement))
                with patch.object(blobs.os, 'open', racing_open), patch.object(blobs, 'Hasher') as hasher:
                    result = blobs.copy_filename(blobs.FolderSource(snapshot, exact_names=True), store, key)
                self.assertEqual(result.status, 'missing')
                hasher.assert_not_called()
                self.assertFalse(store.contents.exists())

    def test_raw_reserved_keys_are_distinct_and_never_url_decoded(self):
        snapshot = self.root / 'reserved-snapshot'
        snapshot.mkdir()
        store = blobs.Store(self.root / 'reserved-store')
        values = []
        names = ('photo%2F?#.png', 'photo%252F?#.png')
        for index, filename in enumerate(names):
            values.append(row(url=f'{API}/api/images/{PHOTO}-{filename}',
                              thumbnail=f'{API}/api/images/{PHOTO}-thumbnail-{filename}'))
            for prefix in ('', 'thumbnail-'):
                key = PHOTO + '-' + prefix + filename
                data = (prefix + str(index)).encode()
                (snapshot / key).write_bytes(data)
                copied = blobs.copy_filename(blobs.FolderSource(snapshot, exact_names=True), store, key)
                self.assertEqual(copied.status, 'copied')
                self.assertEqual(store.path(copied.cid).read_bytes(), data)
        accepted = list(migrate.camera_rows(self.write(values)))
        self.assertEqual([keys[0] for _, keys in accepted], [PHOTO + '-' + name for name in names])
        self.assertNotEqual(accepted[0][1], accepted[1][1])
        self.assertFalse((snapshot / (PHOTO + '-photo')).exists())
        # Literal path separators and a differing raw thumbnail suffix remain unsupported.
        for filename in ('photo/child.png', 'photo\\child.png', 'photo\0.png'):
            with self.assertRaises(migrate.MigrationError):
                list(migrate.camera_rows(self.write([row(url=f'{API}/api/images/{PHOTO}-{filename}',
                     thumbnail=f'{API}/api/images/{PHOTO}-thumbnail-{filename}')])))

    def test_api_base_does_not_accept_query_or_fragment_delimiters(self):
        for suffix in ('?', '#', '?query=value', '#fragment'):
            with patch.dict(os.environ, CAMERA_REEL_IMPORT_API_URL=API + suffix):
                with self.assertRaises(migrate.MigrationError):
                    migrate.camera_api_url()

    def test_api_base_trailing_slash_is_literal_and_not_normalized(self):
        prefix = API + '//api/images/'
        with patch.dict(os.environ, CAMERA_REEL_IMPORT_API_URL=API + '/'):
            accepted = list(migrate.camera_rows(self.write([row(url=prefix + PHOTO + '-photo.png',
                thumbnail=prefix + PHOTO + '-thumbnail-photo.png')])))
            self.assertEqual(accepted[0][0][2], prefix + PHOTO + '-photo.png')
            self.assertEqual(accepted[0][1][0], PHOTO + '-photo.png')
            with self.assertRaises(migrate.MigrationError):
                list(migrate.camera_rows(self.write([row()])))

    def test_object_metadata_manifest_is_strict_and_disk_indexed(self):
        key = PHOTO + '-photo%2F?#.png'
        valid = json.dumps(dict(object_key=key, content_type='image/jpeg'))
        invalid = [valid + '\n' + valid,
                   '{"object_key":"a","object_key":"b","content_type":"image/png"}',
                   json.dumps(dict(object_key=key, content_type='image/png', guessed=True)),
                   json.dumps(dict(object_key='../outside', content_type='image/png')),
                   json.dumps(dict(object_key=key, content_type=None)),
                   json.dumps(dict(object_key=key, content_type='image/gif')),
                   json.dumps(dict(object_key=key, content_type=['image/png'])),
                   ' ' * (migrate.CAMERA_METADATA_LINE_BYTES + 1)]
        manifest = self.root / 'objects.jsonl'
        for content in invalid:
            manifest.write_text(content)
            with tempfile.TemporaryDirectory() as index_dir, self.assertRaises(migrate.MigrationError):
                with migrate.camera_object_metadata(manifest, index_dir):
                    pass
        manifest.write_text(valid + '\n')
        with tempfile.TemporaryDirectory() as index_dir:
            with migrate.camera_object_metadata(manifest, index_dir) as metadata:
                self.assertEqual(migrate.object_content_type(metadata, key), 'image/jpeg')
                self.assertIsNone(migrate.object_content_type(metadata, key.replace('%2F', '%252F')))
                self.assertEqual(metadata.execute('PRAGMA cache_size').fetchone()[0], -2048)
                self.assertTrue((Path(index_dir) / 'object-metadata.sqlite').is_file())
        link = self.root / 'manifest-link.jsonl'
        link.symlink_to(manifest)
        with tempfile.TemporaryDirectory() as index_dir, self.assertRaises(migrate.MigrationError):
            with migrate.camera_object_metadata(link, index_dir):
                pass

    def test_metadata_manifest_requires_both_equal_original_headers(self):
        manifest = self.root / 'paired.jsonl'
        keys = [PHOTO + '-photo.png', PHOTO + '-thumbnail-photo.png']
        for types in (['image/png'], ['image/png', 'image/jpeg']):
            manifest.write_text(''.join(json.dumps(dict(object_key=k, content_type=t)) + '\n'
                                        for k, t in zip(keys, types)))
            with tempfile.TemporaryDirectory() as index_dir:
                with migrate.camera_object_metadata(manifest, index_dir) as metadata:
                    with self.assertRaises(migrate.MigrationError):
                        migrate.camera_preflight(self.write([row()]), lambda line: None, object_metadata=metadata)

    def test_streaming_tree_matches_reference_at_fanout_boundaries(self):
        leaf = blobs._Node(blobs._cid_v1_bytes(0x55, b'x' * 32), 13, 13)
        for count in (1, 173, 174, 175, 174 * 174, 174 * 174 + 1):
            cid_of = lambda digest: blobs._cid_v1_bytes(0x70, digest)
            tree = blobs._Tree(cid_of)
            for _ in range(count - 1):
                tree.add(leaf)
            self.assertTrue(all(len(level) < blobs.MAX_CHILDREN for level in tree.levels))
            self.assertEqual(tree.finish(leaf), reference_reduce([leaf] * count, cid_of))

    def test_metadata_and_exact_filenames(self):
        data = self.write([row()])
        rows = list(migrate.camera_rows(data))
        self.assertEqual(rows[0][0], row())
        self.assertEqual(rows[0][1], (PHOTO + '-photo.png', PHOTO + '-thumbnail-photo.png'))

    def test_unsupported_facts_fail_closed(self):
        for changes in ({'created': ''}, {'thumbnail': ''}, {'owner': OWNER.upper()},
                        {'url': 'https://other.example.test/api/images/' + PHOTO + '-photo.png'},
                        {'url': f'{API}/api/images/{PHOTO}-a%20b.png'}, {'metadata': '{}'},
                        {'metadata': json.dumps({**META, 'visiblePeople': [{'userName': 'missing'}]})}):
            with self.subTest(changes=changes), self.assertRaises(migrate.MigrationError):
                list(migrate.camera_rows(self.write([row(**changes)])))

    def test_preflight_reports_all_unsupported_rows(self):
        report = []
        with self.assertRaisesRegex(migrate.MigrationError, 'rejected 2 rows'):
            migrate.camera_preflight(self.write([row(created=''), row(thumbnail='')]), report.append)
        self.assertEqual(len(report), 2)

    def test_filename_copy_exact_large_bytes_and_stored_thumbnail(self):
        source = self.root / 'large-objects'
        source.mkdir()
        store = blobs.Store(self.root / 'store')
        original = pattern(blobs.CHUNK_SIZE * 175 + 7)
        thumb = b'original thumbnail, deliberately not regenerated'
        for key, data in ((PHOTO + '-photo.png', original), (PHOTO + '-thumbnail-photo.png', thumb)):
            (source / key).write_bytes(data)
            result = blobs.copy_filename(blobs.FolderSource(source), store, key)
            self.assertEqual(result.cid, blobs.hash_bytes_v1(data))
            self.assertEqual(store.path(result.cid).read_bytes(), data)
            self.assertEqual(blobs.copy_one(blobs.FolderSource(source), store, key).status, 'invalid')
        self.assertEqual(blobs.copy_filename(blobs.FolderSource(source), store, '../escape').status, 'invalid')


class CameraDatabase(CameraFiles):
    def setUp(self):
        super().setUp()
        admin = os.environ.get('CATALYST_TEST_DATABASE_URL')
        if not admin:
            self.skipTest('CATALYST_TEST_DATABASE_URL required for disposable Camera database')
        self.admin = admin
        self.name = 'camera_import_test_' + uuid.uuid4().hex
        migrate.psql(admin, f'CREATE DATABASE {self.name};')
        self.addCleanup(lambda: migrate.psql(admin, f'DROP DATABASE {self.name} WITH (FORCE);'))
        parsed = urllib.parse.urlsplit(admin)
        self.url = urllib.parse.urlunsplit(parsed._replace(path='/' + self.name))
        self.store = blobs.Store(self.root / 'store')
        self.step = migrate.load_source(migrate.SOURCES / migrate.CAMERA).steps[0]
        self.env2 = patch.dict(os.environ, CAMERA_REEL_DATABASE_URL=self.url, TARGET_DATABASE_URL=self.url)
        self.env2.start()
        self.addCleanup(self.env2.stop)
        self.sql('''CREATE SCHEMA camera_reel;
CREATE TABLE camera_reel.images (id uuid PRIMARY KEY, user_address text NOT NULL,
 image_hash text NOT NULL, thumbnail_hash text NOT NULL, original_filename text,
 image_content_type text CHECK (image_content_type IN ('image/png', 'image/jpeg')),
 is_public boolean NOT NULL, review_status text NOT NULL, ranking_published boolean NOT NULL,
 metadata jsonb NOT NULL, created_at timestamp NOT NULL);
CREATE TABLE images (id uuid PRIMARY KEY, user_address text, url text, thumbnail_url text,
 is_public boolean, metadata jsonb, created_at timestamp);
CREATE TABLE camera_reel.photo_events (id bigint);
''')
        vals = row()
        self.sql('INSERT INTO images VALUES (' + ','.join(migrate.literal(v) for v in vals) + ');')
        migrate.ensure_checkpoint_table(self.url)
        self.objects = self.root / 'objects'
        self.objects.mkdir()
        for key, data in ((PHOTO + '-photo.png', b'original'), (PHOTO + '-thumbnail-photo.png', b'thumb')):
            (self.objects / key).write_bytes(data)
            result = blobs.copy_filename(blobs.FolderSource(self.objects), self.store, key)
            self.record_camera([(key, result.cid, result.size)], [])

    def record_camera(self, stored, missing=(), *, content_type='image/png'):
        migrate.record_blobs(self.url, migrate.CAMERA, stored, missing,
                             content_types={key: content_type for key, _, _ in stored})

    def object_manifest(self, content_type='image/png'):
        manifest = self.root / 'metadata.jsonl'
        manifest.write_text(''.join(json.dumps(dict(object_key=key.name, content_type=content_type)) + '\n'
                                    for key in self.objects.iterdir()))
        return manifest

    def sql(self, sql):
        return migrate.psql(self.url, sql)

    def run_import(self, dry=False, restart=True):
        cp = migrate.read_checkpoints(self.url).get((migrate.CAMERA, 'image'))
        walk = migrate.plan_walk(self.step, cp, since=None, since_last=False, overlap='0s', restart=restart)
        return migrate.run_step(self.step, self.url, self.url, walk, cp,
                                batch_size=100, log=lambda line: None, store=self.store, dry=dry)

    def test_preservation_replay_moderation_and_conflict(self):
        self.run_import()
        actual = json.loads(self.sql('SELECT row_to_json(i) FROM camera_reel.images i;'))
        self.assertEqual(actual['created_at'], '2023-07-03T17:42:28.123456')
        self.assertEqual(actual['metadata'], META)
        self.assertEqual(actual['original_filename'], 'photo.png')
        self.assertFalse(actual['is_public'])
        self.assertTrue(actual['ranking_published'])
        self.assertEqual(actual['image_content_type'], 'image/png')
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.photo_events;').strip(), '0')
        self.sql("UPDATE camera_reel.images SET review_status='rejected';")
        self.run_import()
        self.assertEqual(self.sql('SELECT review_status FROM camera_reel.images;').strip(), 'rejected')
        self.sql('UPDATE images SET is_public=true;')
        before = self.sql('SELECT row_to_json(c) FROM dcl_migration.checkpoint c;')
        with self.assertRaises(migrate.MigrationError):
            self.run_import()
        self.assertEqual(self.sql('SELECT row_to_json(c) FROM dcl_migration.checkpoint c;'), before)
        self.assertEqual(self.sql('SELECT is_public FROM camera_reel.images;').strip(), 'f')

    def test_disappeared_blob_and_dry_run_leave_no_rows_or_checkpoint(self):
        self.run_import(dry=True)
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '0')
        self.assertEqual(self.sql('SELECT count(*) FROM dcl_migration.checkpoint;').strip(), '0')
        self.store.path(blobs.hash_bytes_v1(b'thumb')).unlink()
        with self.assertRaises(migrate.MigrationError):
            self.run_import()
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '0')
        self.assertEqual(self.sql('SELECT count(*) FROM dcl_migration.checkpoint;').strip(), '0')
        blobs.copy_filename(blobs.FolderSource(self.objects), self.store, PHOTO + '-thumbnail-photo.png')
        self.run_import()
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '1')

    def test_later_batch_failure_resumes_from_last_durable_checkpoint(self):
        self.sql("""INSERT INTO images
SELECT new_id::uuid, user_address, replace(url, id::text, new_id),
       replace(thumbnail_url, id::text, new_id), is_public, metadata, created_at
FROM images CROSS JOIN (SELECT '00000000-0000-0000-0000-' || lpad(g::text, 12, '0') AS new_id
                        FROM generate_series(1, 100) g) generated;
""")
        keys = []
        for i in range(1, 101):
            photo = '00000000-0000-0000-0000-' + str(i).zfill(12)
            keys.extend([(photo + '-photo.png', blobs.hash_bytes_v1(b'original'), 8),
                         (photo + '-thumbnail-photo.png', blobs.hash_bytes_v1(b'thumb'), 5)])
        self.record_camera(keys, [])
        missing = PHOTO + '-thumbnail-photo.png'
        self.sql('DELETE FROM dcl_migration.blob_inventory WHERE object_key=' + migrate.literal(missing) + ';')
        with self.assertRaises(migrate.MigrationError):
            self.run_import()
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '100')
        self.assertEqual(self.sql('SELECT walk_rows FROM dcl_migration.checkpoint;').strip(), '100')
        self.record_camera([(missing, blobs.hash_bytes_v1(b'thumb'), 5)], [])
        self.run_import(restart=False)
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '101')
        self.assertEqual(self.sql('SELECT walk_rows FROM dcl_migration.checkpoint;').strip(), '101')
        self.assertEqual(self.sql('SELECT DISTINCT image_content_type FROM camera_reel.images;').strip(), 'image/png')

    def test_cli_copy_and_run(self):
        self.sql('DELETE FROM dcl_migration.blob_inventory;')
        args = ['blobs', migrate.CAMERA, '--from-dir', str(self.objects), '--to', str(self.store.contents.parent),
                '--object-metadata', str(self.object_manifest())]
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(migrate.main(args), 0)
            self.assertEqual(migrate.main(['run', migrate.CAMERA, '--dry-run', '--storage-root', str(self.store.contents.parent)]), 0)
            self.assertEqual(migrate.main(['run', migrate.CAMERA, '--storage-root', str(self.store.contents.parent)]), 0)
            self.assertEqual(migrate.main(['blobs', migrate.CAMERA, '--verify', '--to', str(self.store.contents.parent)]), 0)
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '1')

    def test_canonical_unicode_and_reserved_basename_is_preserved(self):
        filename = "café 'x$()?#%2F.png"
        for prefix, data in (('', b'original'), ('thumbnail-', b'thumb')):
            key = PHOTO + '-' + prefix + filename
            (self.objects / key).write_bytes(data)
            copied = blobs.copy_filename(blobs.FolderSource(self.objects, exact_names=True), self.store, key)
            self.record_camera([(key, copied.cid, copied.size)], [])
        self.sql('UPDATE images SET url=' + migrate.literal(API + '/api/images/' + PHOTO + '-' + filename)
                 + ', thumbnail_url=' + migrate.literal(API + '/api/images/' + PHOTO + '-thumbnail-' + filename) + ';')
        self.run_import()
        self.assertEqual(self.sql('SELECT original_filename FROM camera_reel.images;').strip(), filename)

    def test_trailing_api_slash_publishes_with_exact_literal_prefix(self):
        self.sql("UPDATE images SET url=replace(url, '/api/images/', '//api/images/'), "
                 "thumbnail_url=replace(thumbnail_url, '/api/images/', '//api/images/');")
        with patch.dict(os.environ, CAMERA_REEL_IMPORT_API_URL=API + '/'):
            self.run_import()
        self.assertEqual(self.sql('SELECT original_filename FROM camera_reel.images;').strip(), 'photo.png')

    def test_missing_manifest_fails_before_copy_or_bookkeeping_mutation(self):
        target = self.root / 'must-not-create'
        args = ['blobs', migrate.CAMERA, '--from-dir', str(self.objects), '--to', str(target)]
        manifest = self.root / 'bad-pairs.jsonl'
        cases = [None,
                 [dict(object_key=PHOTO + '-photo.png', content_type='image/png')],
                 [dict(object_key=PHOTO + '-photo.png', content_type='image/png'),
                  dict(object_key=PHOTO + '-thumbnail-photo.png', content_type='image/jpeg')]]
        for records in cases:
            extra = []
            if records is not None:
                manifest.write_text(''.join(json.dumps(record) + '\n' for record in records))
                extra = ['--object-metadata', str(manifest)]
            with patch.object(migrate, 'ensure_checkpoint_table') as ensure:
                with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(migrate.main(args + extra), 1)
            ensure.assert_not_called()
            self.assertFalse(target.exists())

    def test_mime_inventory_unknown_enrichment_and_known_conflicts(self):
        self.sql('UPDATE dcl_migration.blob_inventory SET content_type=NULL;')
        for mode in ([], ['--dry-run']):
            with patch.object(migrate, 'ensure_checkpoint_table') as ensure:
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(migrate.main(['run', migrate.CAMERA, '--storage-root',
                                                  str(self.store.contents.parent)] + mode), 1)
            ensure.assert_not_called()
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '0')
        args = ['blobs', migrate.CAMERA, '--from-dir', str(self.objects), '--to', str(self.store.contents.parent),
                '--object-metadata', str(self.object_manifest())]
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(migrate.main(args), 0)
        self.assertEqual(self.sql('SELECT DISTINCT content_type FROM dcl_migration.blob_inventory;').strip(), 'image/png')
        key = PHOTO + '-photo.png'
        with self.assertRaises(migrate.MigrationError):
            self.record_camera([(key, blobs.hash_bytes_v1(b'original'), 8)], content_type='image/jpeg')
        self.sql('UPDATE dcl_migration.blob_inventory SET content_type=NULL;')
        with self.assertRaises(migrate.MigrationError):
            self.record_camera([(key, blobs.hash_bytes_v1(b'changed'), 7)], content_type='image/jpeg')
        self.assertEqual(self.sql('SELECT count(*) FROM dcl_migration.blob_inventory WHERE content_type IS NOT NULL;').strip(), '0')

    def test_mislabeled_png_and_same_cid_aliases_keep_explicit_mime(self):
        second = '22345678-1234-5678-9012-123456789012'
        png = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=')
        self.sql('INSERT INTO images SELECT ' + migrate.literal(second) + '::uuid, user_address, '
                 + 'replace(url, id::text, ' + migrate.literal(second) + '), '
                 + 'replace(thumbnail_url, id::text, ' + migrate.literal(second) + '), is_public, metadata, created_at FROM images;')
        self.sql('DELETE FROM dcl_migration.blob_inventory;')
        manifest = self.root / 'mime-aliases.jsonl'
        records = []
        for photo, mime in ((PHOTO, 'image/jpeg'), (second, 'image/png')):
            for suffix in ('photo.png', 'thumbnail-photo.png'):
                key = photo + '-' + suffix
                (self.objects / key).write_bytes(png)
                records.append(dict(object_key=key, content_type=mime))
        manifest.write_text(''.join(json.dumps(record) + '\n' for record in records))
        args = ['blobs', migrate.CAMERA, '--from-dir', str(self.objects), '--to', str(self.store.contents.parent),
                '--object-metadata', str(manifest)]
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(migrate.main(args), 0)
        self.run_import()
        self.assertEqual(self.sql('SELECT image_content_type FROM camera_reel.images ORDER BY id;').split(),
                         ['image/jpeg', 'image/png'])
        self.assertEqual(self.sql('SELECT count(DISTINCT image_hash) FROM camera_reel.images;').strip(), '1')
        self.assertEqual(self.store.path(blobs.hash_bytes_v1(png)).read_bytes(), png)
        self.run_import()
        self.sql("UPDATE camera_reel.images SET image_content_type='image/png' WHERE id=" + migrate.literal(PHOTO) + ';')
        before = self.sql('SELECT row_to_json(c) FROM dcl_migration.checkpoint c;')
        with self.assertRaises(migrate.MigrationError):
            self.run_import()
        self.assertEqual(self.sql('SELECT row_to_json(c) FROM dcl_migration.checkpoint c;'), before)

    def test_mime_capability_and_pair_mismatch_fail_without_publication(self):
        self.sql("UPDATE dcl_migration.blob_inventory SET content_type='image/jpeg' WHERE object_key="
                 + migrate.literal(PHOTO + '-thumbnail-photo.png') + ';')
        with self.assertRaises(migrate.MigrationError):
            self.run_import()
        self.sql('ALTER TABLE camera_reel.images DROP COLUMN image_content_type;')
        with self.assertRaisesRegex(migrate.MigrationError, 'image_content_type schema capability'):
            self.run_import()
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '0')
        self.assertEqual(self.sql('SELECT count(*) FROM dcl_migration.checkpoint;').strip(), '0')

    def test_filename_capability_required(self):
        self.sql('ALTER TABLE camera_reel.images DROP COLUMN original_filename;')
        with self.assertRaisesRegex(migrate.MigrationError, 'original_filename schema capability'):
            self.run_import()
        self.assertEqual(self.sql('SELECT count(*) FROM dcl_migration.checkpoint;').strip(), '0')

    def test_publication_paths_with_quotes_and_shell_characters(self):
        self.store = blobs.Store(self.root / "store with 'quotes; $(echo unexpected)")
        for key in (PHOTO + '-photo.png', PHOTO + '-thumbnail-photo.png'):
            blobs.copy_filename(blobs.FolderSource(self.objects), self.store, key)
        self.run_import()
        self.assertEqual(self.sql('SELECT count(*) FROM camera_reel.images;').strip(), '1')

    def test_inventory_conflict_does_not_overwrite(self):
        with self.assertRaises(migrate.MigrationError):
            self.record_camera([(PHOTO + '-photo.png', blobs.hash_bytes_v1(b'changed'), 7)], [])
        self.assertEqual(self.sql("SELECT hash FROM dcl_migration.blob_inventory WHERE object_key=" + migrate.literal(PHOTO + '-photo.png') + ';').strip(), blobs.hash_bytes_v1(b'original'))

    def test_shared_gc_lock_held_during_client_verification(self):
        # The client helper attempts the collector's exclusive lock through a
        # second connection, then executes the real local-file validation.
        helper = self.root / 'probe.py'
        helper.write_text('''import os, subprocess, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import blobs
probe = subprocess.run(['psql', '-XAt', os.environ['TARGET_DATABASE_URL'], '-c',
 'SELECT pg_try_advisory_xact_lock(%d)'], capture_output=True, text=True, check=True)
assert probe.stdout.strip() == 'f', 'publication lock was not held'
blobs.verify_publication(sys.argv[2], sys.argv[3])
''' % migrate.STORE_LOCK_KEY)
        self.sql(f"""CREATE FUNCTION require_publication_lock() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS (SELECT 1 FROM pg_locks WHERE locktype='advisory' AND mode='ShareLock'
   AND pid=pg_backend_pid() AND classid={migrate.STORE_LOCK_KEY >> 32}::oid
   AND objid={migrate.STORE_LOCK_KEY & 0xffffffff}::oid) THEN
   RAISE EXCEPTION 'shared GC lock absent during publication';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER image_lock BEFORE INSERT ON camera_reel.images FOR EACH ROW EXECUTE FUNCTION require_publication_lock();
CREATE TRIGGER checkpoint_lock BEFORE INSERT ON dcl_migration.checkpoint FOR EACH ROW EXECUTE FUNCTION require_publication_lock();
""")
        original = migrate.camera_publication
        def publication(store, inventory):
            lines = original(store, inventory)
            command = migrate.shlex.join([migrate.sys.executable, str(helper), str(migrate.HERE), str(inventory), str(store.contents.parent)])
            index = next(i for i, line in enumerate(lines) if 'FROM PROGRAM' in line)
            lines[index] = '\\copy camera_verified FROM PROGRAM ' + migrate.literal(command) + ' WITH (FORMAT csv)'
            return lines
        with patch.object(migrate, 'camera_publication', publication):
            self.run_import()
        self.assertEqual(self.sql(f'SELECT pg_try_advisory_xact_lock({migrate.STORE_LOCK_KEY});').strip(), 't')


if __name__ == '__main__':
    unittest.main()
