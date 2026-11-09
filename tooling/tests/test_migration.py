"""The migration engine resumes, dry-runs and re-imports from a point, and every manifest compiles."""
import contextlib
import dataclasses
import datetime
import gzip
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import textwrap
import unittest
import urllib.error

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('migrate', ROOT / 'tooling/migration/migrate.py')
migrate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migrate)
blobs = migrate.load_blobs_module()
CHUNK = blobs.CHUNK_SIZE


def pattern(n):
    return (bytes(range(251)) * (n // 251 + 1))[:n]


def put_upstream(root, key, data):
    (root / blobs.shard(key)).mkdir(parents=True, exist_ok=True)
    (root / blobs.shard(key) / key).write_bytes(data)

MANIFEST = textwrap.dedent('''
    name = "toy"
    repository = "example/toy"
    revision = "0"
    database_env = "TOY_DATABASE_URL"

    [[step]]
    name = "things"
    targets = ["app.things"]
    extract = "things.extract.sql"
    load = "things.load.sql"
    key = ["id"]
    watermark = "updated_at"
    columns = [
      { name = "id", type = "integer" },
      { name = "title", type = "text" },
      { name = "updated_at", type = "timestamptz" },
    ]

    [[step]]
    name = "tags"
    targets = ["app.tags"]
    extract = "tags.extract.sql"
    load = "tags.load.sql"
    key = ["name"]
    columns = [{ name = "name", type = "text" }, { name = "n", type = "integer" }]
''')

FILES = {
    'source.toml': MANIFEST,
    'things.extract.sql': "SELECT id, title, updated_at AT TIME ZONE 'UTC' AS updated_at FROM things\n",
    'things.load.sql': textwrap.dedent('''
        -- upsert
        INSERT INTO app.things (id, title, updated_at)
        SELECT id, title, updated_at FROM stage
        ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, updated_at = EXCLUDED.updated_at;
    '''),
    'tags.extract.sql': 'SELECT name, n FROM tags\n',
    'tags.load.sql': 'INSERT INTO app.tags SELECT name, n FROM stage ON CONFLICT (name) DO UPDATE SET n = EXCLUDED.n;\n',
}


def write_source(root, files=FILES, name='toy'):
    directory = root / name
    directory.mkdir(parents=True)
    for file, text in files.items():
        (directory / file).write_text(text)
    return directory


class Manifests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)

    def test_when_a_manifest_is_valid_it_should_load_with_its_cursor(self):
        source = migrate.load_source(write_source(self.tmp))
        self.assertEqual([s.cursor for s in source.steps], [['updated_at', 'id'], ['name']])

    def test_when_a_cursor_column_is_undeclared_it_should_refuse(self):
        write_source(self.tmp, {**FILES, 'source.toml': MANIFEST.replace('key = ["id"]', 'key = ["uuid"]')})
        with self.assertRaisesRegex(migrate.MigrationError, "cursor column 'uuid'"):
            migrate.discover(self.tmp)

    def test_when_the_watermark_is_not_timestamptz_it_should_refuse(self):
        manifest = MANIFEST.replace('name = "updated_at", type = "timestamptz"', 'name = "updated_at", type = "timestamp"')
        write_source(self.tmp, {**FILES, 'source.toml': manifest})
        with self.assertRaisesRegex(migrate.MigrationError, 'must be typed timestamptz'):
            migrate.discover(self.tmp)

    def test_when_a_whole_step_has_a_watermark_it_should_refuse(self):
        manifest = MANIFEST.replace('watermark = "updated_at"', 'watermark = "updated_at"\nwhole = true')
        write_source(self.tmp, {**FILES, 'source.toml': manifest})
        with self.assertRaisesRegex(migrate.MigrationError, 'takes no watermark'):
            migrate.discover(self.tmp)

    def test_when_a_load_controls_the_transaction_it_should_refuse(self):
        write_source(self.tmp, {**FILES, 'tags.load.sql': 'COMMIT;\nINSERT INTO app.tags SELECT * FROM stage;\n'})
        with self.assertRaisesRegex(migrate.MigrationError, 'must not control the transaction'):
            migrate.discover(self.tmp)

    def test_when_an_extract_has_two_statements_it_should_refuse(self):
        write_source(self.tmp, {**FILES, 'tags.extract.sql': 'SELECT name, n FROM tags; SELECT 1\n'})
        with self.assertRaisesRegex(migrate.MigrationError, 'single statement'):
            migrate.discover(self.tmp)

    def test_when_sources_depend_on_each_other_it_should_order_them(self):
        write_source(self.tmp)
        write_source(self.tmp, {**FILES, 'source.toml': MANIFEST.replace('name = "toy"', 'name = "aaa"')
                                .replace('database_env = "TOY_DATABASE_URL"', 'database_env = "AAA_URL"\nafter = ["toy"]')},
                     name='aaa')
        self.assertEqual([s.name for s in migrate.ordered(migrate.discover(self.tmp), ['all'])], ['toy', 'aaa'])

    def test_when_an_unrelated_manifest_is_broken_it_should_still_load_the_selected_one(self):
        write_source(self.tmp)
        write_source(self.tmp, {**FILES, 'source.toml': MANIFEST.replace('name = "toy"', 'name = "wip"')
                                .replace('database_env = "TOY_DATABASE_URL"', 'database_env = "WIP_URL"'),
                                'tags.load.sql': 'COMMIT;\n'}, name='wip')
        self.assertEqual(list(migrate.discover(self.tmp, ['toy'])), ['toy'])
        with self.assertRaisesRegex(migrate.MigrationError, 'wip'):
            migrate.discover(self.tmp, ['all'])

    def test_when_sources_form_a_cycle_it_should_refuse(self):
        write_source(self.tmp, {**FILES, 'source.toml': MANIFEST.replace('database_env = "TOY_DATABASE_URL"',
                                                                         'database_env = "TOY_URL"\nafter = ["toy"]')})
        with self.assertRaisesRegex(migrate.MigrationError, 'cycle'):
            migrate.ordered(migrate.discover(self.tmp), ['all'])


class Connections(unittest.TestCase):
    def test_when_given_a_url_it_should_keep_credentials_out_of_argv(self):
        env = migrate.connection_env('postgres://reader:s%40cret@db.example:6543/places?sslmode=require',
                                     read_only=True)
        self.assertEqual((env['PGHOST'], env['PGPORT'], env['PGUSER'], env['PGPASSWORD'], env['PGDATABASE'],
                          env['PGSSLMODE']), ('db.example', '6543', 'reader', 's@cret', 'places', 'require'))
        self.assertIn('default_transaction_read_only=on', env['PGOPTIONS'])
        self.assertIn('TimeZone=UTC', env['PGOPTIONS'])

    def test_when_given_an_unknown_parameter_it_should_refuse(self):
        with self.assertRaisesRegex(migrate.MigrationError, 'unsupported'):
            migrate.connection_env('postgres://h/db?weird=1')

    def test_when_quoting_a_literal_it_should_double_single_quotes(self):
        self.assertEqual(migrate.literal("it's"), "'it''s'")


class Walks(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        steps = migrate.load_source(write_source(self.tmp)).steps
        self.watermarked, self.plain = steps
        self.overlap = datetime.timedelta(minutes=10)

    def plan(self, step, checkpoint, since=None, since_last=False, restart=False):
        return migrate.plan_walk(step, checkpoint, since=since, since_last=since_last, overlap=self.overlap,
                                 restart=restart)

    def test_when_a_walk_was_interrupted_it_should_resume_its_cursor(self):
        cp = migrate.Checkpoint(cursor=['2026-01-01 00:00:00+00', '7'], walk_started_at='2026-02-01T00:00:00Z',
                                walk_rows=10)
        walk = self.plan(self.watermarked, cp)
        self.assertEqual((walk.cursor, walk.fresh, walk.rows), (cp.cursor, False, 10))

    def test_when_a_different_since_is_given_it_should_start_a_new_walk(self):
        cp = migrate.Checkpoint(cursor=['x', '1'], walk_started_at='2026-02-01T00:00:00Z')
        walk = self.plan(self.watermarked, cp, since='2026-01-01T00:00:00Z')
        self.assertEqual((walk.cursor, walk.since, walk.fresh), (None, '2026-01-01T00:00:00Z', True))

    def test_when_since_is_last_it_should_subtract_the_overlap(self):
        cp = migrate.Checkpoint(cursor=['x', '1'], walk_started_at='2026-02-01T00:00:00Z',
                                completed_at='2026-02-01T01:00:00Z',
                                last_completed_walk_started_at='2026-02-01T00:00:00Z')
        self.assertEqual(self.plan(self.watermarked, cp, since_last=True).since, '2026-01-31T23:50:00Z')

    def test_when_a_step_has_no_watermark_it_should_rescan_after_completing(self):
        cp = migrate.Checkpoint(cursor=['T9'], walk_started_at='2026-02-01T00:00:00Z', completed_at='2026-02-01T00:01:00Z')
        self.assertIsNone(self.plan(self.plain, cp).cursor)
        self.assertIsNone(self.plan(self.plain, cp, since='2026-01-01T00:00:00Z').since)

    def test_when_a_whole_step_was_left_part_way_it_should_read_the_whole_set_again(self):
        whole = dataclasses.replace(self.plain, whole=True)
        cp = migrate.Checkpoint(cursor=['T9'], walk_started_at='2026-02-01T00:00:00Z', walk_rows=9)
        walk = self.plan(whole, cp, since_last=True)
        self.assertEqual((walk.cursor, walk.since, walk.fresh), (None, None, True))


class Cids(unittest.TestCase):
    """The golden vectors of library/dcl-auth/src/hash.rs."""

    def test_when_hashing_v1_it_should_match_the_node(self):
        self.assertEqual(blobs.hash_bytes_v1(b''), 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku')
        self.assertEqual(blobs.hash_bytes_v1(b'hello world'),
                         'bafkreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e')
        self.assertTrue(blobs.hash_bytes_v1(b'\xaa' * CHUNK).startswith('bafkrei'))
        self.assertTrue(blobs.hash_bytes_v1(b'\xaa' * (CHUNK + 1)).startswith('bafy'))

    def test_when_hashing_v1_past_one_tree_level_it_should_match_the_node(self):
        self.assertEqual(blobs.hash_bytes_v1(pattern(175 * CHUNK + 7)),
                         'bafybeihh7afuh5inawukv67gg6vxlpvb3zgw6rkpw7tymous2idoydpxpi')

    def test_when_hashing_v0_it_should_match_both_upstream_spellings(self):
        hasher = blobs.Hasher(unixfs_v0=True)
        hasher.update(b'hello world')
        self.assertEqual(hasher.v0(), 'QmaozNR7DZHQK1ZcU9p7QdrshMvXqWK6gpu5rmrkPdT3L4')
        self.assertEqual(hasher.v0_unixfs(), 'Qmf412jQZiuVUtdgnB36FXFX7xg5V6KEbSJ4dpQuhkLyfD')
        self.assertEqual(blobs.hash_bytes_v0_unixfs(b''), 'QmbFMke1KXqnYyBBWxB74N4c5SBnJMVAiMNRcGu6x1AwQH')
        self.assertEqual(blobs.hash_bytes_v0_unixfs(pattern(CHUNK + 1)), 'QmUSjGawaz4ptvREcMKSMJneWCa5j8dAz2wSAAvHtW2rnB')
        self.assertEqual(blobs.hash_bytes_v0_unixfs(pattern(CHUNK * 3)), 'QmSuzytMNMmTQC6hL7FG7CtoqWKoZpDiMHbcGeoDakJvAg')
        self.assertEqual(blobs.hash_bytes_v0_unixfs(pattern(CHUNK * 175)),
                         'Qmbp67kThKoJFnWu7pUgwCu81WttemMnD13oG4uj9DiY5E')

    def test_when_fed_in_pieces_it_should_hash_as_the_whole_buffer(self):
        for size in (0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK + 5):
            data = pattern(size)
            for piece in (7, 4096, CHUNK - 1, CHUNK + 5):
                hasher = blobs.Hasher(unixfs_v0=True)
                for i in range(0, len(data), piece):
                    hasher.update(data[i:i + piece])
                self.assertEqual((hasher.v1(), hasher.v0_unixfs()),
                                 (blobs.hash_bytes_v1(data), blobs.hash_bytes_v0_unixfs(data)), (size, piece))


class BlobCopy(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.upstream = self.tmp / 'wcs'
        self.store = blobs.Store(self.tmp / 'node')

    def put(self, key, data, sharded=True, gzipped=False):
        directory = self.upstream / blobs.shard(key) if sharded else self.upstream
        directory.mkdir(parents=True, exist_ok=True)
        if gzipped:
            (directory / f'{key}.gzip').write_bytes(gzip.compress(data))
        else:
            (directory / key).write_bytes(data)

    def copy(self, key):
        return blobs.copy_one(blobs.FolderSource(self.upstream), self.store, key)

    def staging_files(self):
        return list((self.tmp / 'node').rglob('*.tmp'))

    def test_when_copying_a_cid_it_should_store_it_in_the_node_layout(self):
        data = pattern(CHUNK + 10)
        cid = blobs.hash_bytes_v1(data)
        self.put(cid, data)
        outcome = self.copy(cid)
        self.assertEqual((outcome.status, outcome.cid, outcome.size), ('copied', cid, len(data)))
        stored = self.tmp / 'node' / 'contents' / hashlib.sha1(cid.encode()).hexdigest()[:4] / cid
        self.assertEqual(stored.read_bytes(), data)
        self.assertEqual(self.staging_files(), [])

    def test_when_the_blob_is_already_stored_it_should_not_read_upstream(self):
        cid = blobs.hash_bytes_v1(b'x')
        self.put(cid, b'x')
        self.copy(cid)
        shutil.rmtree(self.upstream)
        self.upstream.mkdir()
        self.assertEqual(self.copy(cid).status, 'present')

    def test_when_the_stored_file_is_truncated_it_should_copy_it_again(self):
        data = pattern(CHUNK + 10)
        cid = blobs.hash_bytes_v1(data)
        self.put(cid, data)
        self.store.path(cid).parent.mkdir(parents=True)
        self.store.path(cid).write_bytes(data[:100])
        outcome = self.copy(cid)
        self.assertEqual((outcome.status, outcome.size), ('copied', len(data)))
        self.assertEqual(self.store.path(cid).read_bytes(), data)

    def test_when_the_stored_file_has_its_recorded_size_it_should_count_as_held(self):
        cid = blobs.hash_bytes_v1(b'abc')
        self.put(cid, b'abc')
        self.copy(cid)
        self.assertTrue(self.store.has(cid, 3))
        self.assertFalse(self.store.has(cid, 4))

    def test_when_probing_a_wrong_folder_it_should_find_nothing_readable(self):
        self.upstream.mkdir()
        keys = [blobs.hash_bytes_v1(str(i).encode()) for i in range(40)]
        result = blobs.probe(blobs.FolderSource(self.upstream), keys + ['not a key'])
        self.assertEqual((result.readable, len(result.unreadable)), (0, blobs.PROBE_KEYS))

    def test_when_probing_a_folder_holding_some_keys_it_should_count_them(self):
        keys = [blobs.hash_bytes_v1(str(i).encode()) for i in range(4)]
        self.put(keys[2], b'2')
        result = blobs.probe(blobs.FolderSource(self.upstream), keys)
        self.assertEqual((result.readable, len(result.unreadable)), (1, 3))

    def test_when_every_new_key_is_lost_but_an_earlier_key_reads_it_should_count_the_source_readable(self):
        earlier = [blobs.hash_bytes_v1(str(i).encode()) for i in range(10)]
        for i, key in enumerate(earlier):
            self.put(key, str(i).encode())
        lost = blobs.hash_bytes_v1(b'lost')
        result = blobs.probe(blobs.FolderSource(self.upstream), [lost], reference=earlier)
        self.assertEqual((result.readable, list(result.unreadable)),
                         (blobs.PROBE_REFERENCE_KEYS, [lost]))

    def test_when_upstream_is_flat_or_gzipped_it_should_find_the_bytes(self):
        flat, packed = b'flat file', b'packed file'
        self.put(blobs.hash_bytes_v1(flat), flat, sharded=False)
        self.put(blobs.hash_bytes_v1(packed), packed, gzipped=True)
        self.assertEqual(self.copy(blobs.hash_bytes_v1(flat)).status, 'copied')
        self.assertEqual(self.copy(blobs.hash_bytes_v1(packed)).status, 'copied')

    def test_when_a_cidv0_names_unixfs_bytes_it_should_accept_them(self):
        data = b'legacy'
        key = blobs.hash_bytes_v0_unixfs(data)
        self.put(key, data)
        self.assertEqual(self.copy(key).cid, key)

    def test_when_a_thumbnail_is_keyed_by_sha256_it_should_store_it_under_its_cid(self):
        data = pattern(CHUNK * 2 + 3)
        key = hashlib.sha256(data).hexdigest()
        self.put(key, data)
        outcome = self.copy(key)
        self.assertEqual((outcome.status, outcome.cid), ('copied', blobs.hash_bytes_v1(data)))
        self.assertTrue(self.store.has(outcome.cid))
        self.assertEqual(self.staging_files(), [])

    def test_when_the_bytes_do_not_hash_to_the_key_it_should_store_nothing(self):
        key = blobs.hash_bytes_v1(b'expected')
        self.put(key, b'tampered')
        self.assertEqual(self.copy(key).status, 'corrupt')
        self.assertFalse(self.store.has(key))
        self.assertEqual(self.staging_files(), [])

    def test_when_upstream_lacks_the_object_or_the_key_is_no_id_it_should_say_so(self):
        self.upstream.mkdir()
        self.assertEqual(self.copy(blobs.hash_bytes_v1(b'gone')).status, 'missing')
        self.assertEqual(self.copy('../escape').status, 'invalid')

    def test_when_a_key_only_starts_with_an_id_it_should_be_invalid(self):
        self.upstream.mkdir()
        cid, sha = blobs.hash_bytes_v1(b'x'), hashlib.sha256(b'x').hexdigest()
        for key in (cid + '\n', cid + '\nmore', sha + '\n', 'Qm' + 'a' * 44 + '\n'):
            self.assertEqual(self.copy(key).status, 'invalid', repr(key))

    def test_when_a_read_keeps_failing_mid_stream_it_should_remove_the_staging_file(self):
        retry = unittest.mock.patch.object(blobs, 'RETRY_DELAY', 0)
        retry.start()
        self.addCleanup(retry.stop)
        data = pattern(blobs.READ_SIZE * 2)
        source = FakeFolder({blobs.hash_bytes_v1(data): data}, fail_reads={blobs.hash_bytes_v1(data): 99})
        outcome = blobs.copy_one(source, self.store, blobs.hash_bytes_v1(data))
        self.assertEqual(outcome.status, 'failed')
        self.assertIn('read failed 3 times', outcome.reason)
        self.assertEqual(source.opened, 3)
        self.assertEqual(self.staging_files(), [])

    def test_when_the_store_has_a_staging_file_of_that_name_it_should_not_follow_or_reuse_it(self):
        target = self.tmp / 'elsewhere'
        target.write_bytes(b'keep')
        cid = blobs.hash_bytes_v1(b'x')
        directory = self.tmp / 'node' / 'contents' / blobs.shard(cid)
        directory.mkdir(parents=True)
        seq = next(blobs._seq) + 1
        (directory / f'{cid}.{os.getpid()}.{seq}.tmp').symlink_to(target)
        with self.assertRaises(FileExistsError):
            self.store.staging(cid)
        self.assertEqual(target.read_bytes(), b'keep')

    def test_when_copying_many_keys_it_should_submit_a_bounded_window_in_order(self):
        keys = [blobs.hash_bytes_v1(str(i).encode()) for i in range(200)]
        source = FakeFolder({k: str(i).encode() for i, k in enumerate(keys)})
        consumed = []

        def feed():
            for k in keys:
                consumed.append(k)
                yield k

        outcomes = blobs.copy_all(source, self.store, feed(), workers=2)
        first = next(outcomes)
        self.assertLessEqual(len(consumed), 2 * 4 + 1)
        rest = list(outcomes)
        self.assertEqual([o.key for o in [first, *rest]], keys)
        self.assertTrue(all(o.status == 'copied' for o in [first, *rest]))


class FakeFolder:
    """An upstream whose reads can fail: `fail_reads[key]` reads raise after the first chunk."""

    def __init__(self, objects, fail_reads=None):
        self.objects, self.fail_reads, self.opened = objects, dict(fail_reads or {}), 0

    def open(self, key):
        self.opened += 1
        if key not in self.objects:
            raise blobs.Missing('absent upstream')
        body = FakeBody(self.objects[key], fail=self.fail_reads.get(key, 0) > 0)
        if key in self.fail_reads:
            self.fail_reads[key] -= 1
        return body


class FakeBody:
    def __init__(self, data, fail=False):
        self.stream, self.fail, self.reads = io.BytesIO(data), fail, 0

    def read(self, n=-1):
        self.reads += 1
        if self.fail and self.reads > 1:
            raise TimeoutError('read timed out')
        return self.stream.read(n)

    def close(self):
        pass


class FakeClientError(Exception):
    def __init__(self, code, status):
        super().__init__(code)
        self.response = {'Error': {'Code': code}, 'ResponseMetadata': {'HTTPStatusCode': status}}


class FakeS3:
    """get_object as boto3 answers it: ClientError with `response`, a streaming Body."""

    def __init__(self):
        self.objects, self.errors, self.calls = {}, {}, 0

    def get_object(self, Bucket, Key):
        self.calls += 1
        queued = self.errors.get(Key)
        if queued:
            error = queued.pop(0)
            if error is not None:
                raise error
        if Key not in self.objects:
            raise FakeClientError('NoSuchKey', 404)
        data, encoding, fail = self.objects[Key]
        body = FakeBody(data, fail=fail and self.calls == 1)
        return {'Body': body, **({'ContentEncoding': encoding} if encoding else {})}


class S3Copy(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.store = blobs.Store(self.tmp / 'node')
        self.client = FakeS3()
        self.source = blobs.S3Source('bucket', 'prefix/', client=self.client)
        retry = unittest.mock.patch.object(blobs, 'RETRY_DELAY', 0)
        retry.start()
        self.addCleanup(retry.stop)

    def copy(self, key):
        return blobs.copy_one(self.source, self.store, key)

    def test_when_the_key_is_absent_it_should_record_it_missing(self):
        outcome = self.copy(blobs.hash_bytes_v1(b'gone'))
        self.assertEqual((outcome.status, outcome.reason), ('missing', 'absent upstream'))

    def test_when_access_is_denied_after_the_bucket_read_it_should_record_it_missing_with_the_reason(self):
        key = blobs.hash_bytes_v1(b'hidden')
        self.client.errors['prefix/' + key] = [FakeClientError('AccessDenied', 403)]
        self.source.readable = True
        outcome = self.copy(key)
        self.assertEqual(outcome.status, 'missing')
        self.assertIn('access denied', outcome.reason)

    def test_when_access_is_denied_before_any_key_read_it_should_stop_the_copy(self):
        key = blobs.hash_bytes_v1(b'hidden')
        self.client.errors['prefix/' + key] = [FakeClientError('AccessDenied', 403)]
        with self.assertRaises(blobs.Denied):
            self.copy(key)

    def test_when_every_probed_key_is_denied_it_should_leave_the_source_unreadable(self):
        keys = [blobs.hash_bytes_v1(str(i).encode()) for i in range(3)]
        for k in keys:
            self.client.errors['prefix/' + k] = [FakeClientError('AccessDenied', 403)]
        result = blobs.probe(self.source, keys)
        self.assertEqual((result.readable, len(result.unreadable), self.source.readable), (0, 3, False))

    def test_when_a_probed_key_reads_it_should_mark_the_source_readable(self):
        data = b'present'
        key = blobs.hash_bytes_v1(data)
        self.client.objects['prefix/' + key] = (data, None, False)
        result = blobs.probe(self.source, [key, blobs.hash_bytes_v1(b'gone')])
        self.assertEqual((result.readable, self.source.readable), (1, True))

    def test_when_s3_throttles_or_fails_once_it_should_retry_and_store_the_object(self):
        for error in (FakeClientError('SlowDown', 503), FakeClientError('InternalError', 500),
                      FakeClientError('Unrecognized', 502)):
            data = repr(error.response).encode()
            key = blobs.hash_bytes_v1(data)
            self.client.objects['prefix/' + key] = (data, None, False)
            self.client.errors['prefix/' + key] = [error]
            self.assertEqual(self.copy(key).status, 'copied', error)

    def test_when_s3_keeps_failing_with_5xx_it_should_report_the_key_failed(self):
        key = blobs.hash_bytes_v1(b'x')
        self.client.errors['prefix/' + key] = [FakeClientError('ServiceUnavailable', 503)] * 3
        outcome = self.copy(key)
        self.assertEqual(outcome.status, 'failed')
        self.assertIn('ServiceUnavailable', outcome.reason)

    def test_when_the_clock_is_skewed_it_should_stop_the_copy_without_retrying(self):
        key = blobs.hash_bytes_v1(b'x')
        self.client.errors['prefix/' + key] = [FakeClientError('RequestTimeTooSkewed', 403)]
        with self.assertRaisesRegex(blobs.SourceError, 'sync the system clock'):
            self.copy(key)
        self.assertEqual(self.client.calls, 1)

    def test_when_another_s3_error_occurs_it_should_stop_the_copy(self):
        key = blobs.hash_bytes_v1(b'x')
        self.client.errors['prefix/' + key] = [FakeClientError('InvalidAccessKeyId', 400)]
        with self.assertRaisesRegex(blobs.SourceError, 'InvalidAccessKeyId'):
            self.copy(key)

    def test_when_the_object_is_gzip_encoded_it_should_store_the_inflated_bytes(self):
        data = pattern(5000)
        key = blobs.hash_bytes_v1(data)
        self.client.objects['prefix/' + key] = (gzip.compress(data), 'gzip', False)
        outcome = self.copy(key)
        self.assertEqual(outcome.status, 'copied')
        self.assertEqual(self.store.path(key).read_bytes(), data)

    def test_when_a_stream_fails_once_it_should_retry_and_store_the_object(self):
        data = pattern(blobs.READ_SIZE + 10)
        key = blobs.hash_bytes_v1(data)
        self.client.objects['prefix/' + key] = (data, None, True)
        outcome = self.copy(key)
        self.assertEqual((outcome.status, self.client.calls), ('copied', 2))
        self.assertEqual(self.store.path(key).read_bytes(), data)
        self.assertEqual(list((self.tmp / 'node').rglob('*.tmp')), [])


class FakeResponse(io.BytesIO):
    pass


class BadgeArt(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.art = blobs.ArtDir(self.tmp / 'art')
        self.calls = []
        self.answers = {}
        self.source = blobs.HttpArtSource('https://cdn.example/', opener=self.open)

    def open(self, request, timeout):
        self.calls.append(request.full_url)
        answer = self.answers[request.full_url].pop(0)
        if isinstance(answer, int):
            raise urllib.error.HTTPError(request.full_url, answer, 'x', {}, None)
        return FakeResponse(answer)

    def test_when_a_value_is_a_texture_path_or_a_url_under_the_cdn_base_it_should_give_its_path(self):
        self.assertEqual(blobs.art_path('traveler/starter/2d/normal.png'), 'traveler/starter/2d/normal.png')
        self.assertEqual(blobs.art_path('https://assets-cdn.decentraland.org/traveler/starter/2d/normal.png'),
                         'traveler/starter/2d/normal.png')

    def test_when_a_value_is_elsewhere_or_escapes_the_directory_it_should_give_no_path(self):
        for url in ('https://badges.decentraland.org/assets/walkabout/2d/normal.png',
                    'https://assets-cdn.decentraland.org/../etc/2d/x.png', '../x/2d/normal.png', '/x/2d/normal.png',
                    'https://assets-cdn.decentraland.org/x/2d/normal.png?y', ''):
            self.assertIsNone(blobs.art_path(url), url)

    def test_when_the_cdn_answers_a_png_it_should_store_it_by_path(self):
        self.answers['https://cdn.example/a/2d/normal.png'] = [blobs.PNG_MAGIC + b'x']
        outcome = blobs.copy_art(self.source, self.art, 'a/2d/normal.png')
        self.assertEqual(outcome.status, 'copied')
        self.assertEqual((self.tmp / 'art/a/2d/normal.png').read_bytes(), blobs.PNG_MAGIC + b'x')
        self.assertEqual(list((self.tmp / 'art/a/2d').iterdir()), [self.tmp / 'art/a/2d/normal.png'])

    def test_when_the_texture_is_already_there_it_should_not_fetch_it(self):
        (self.tmp / 'art/a/2d').mkdir(parents=True)
        (self.tmp / 'art/a/2d/normal.png').write_bytes(b'held')
        self.assertEqual(blobs.copy_art(self.source, self.art, 'a/2d/normal.png').status, 'present')
        self.assertEqual(self.calls, [])

    def test_when_the_cdn_answers_404_it_should_report_the_texture_missing(self):
        self.answers['https://cdn.example/a/2d/normal.png'] = [404]
        outcome = blobs.copy_art(self.source, self.art, 'a/2d/normal.png')
        self.assertEqual((outcome.status, outcome.reason), ('missing', 'HTTP 404 upstream'))

    def test_when_the_cdn_fails_once_it_should_retry_and_store_the_texture(self):
        self.answers['https://cdn.example/a/2d/normal.png'] = [503, blobs.PNG_MAGIC]
        with unittest.mock.patch.object(blobs, 'RETRY_DELAY', 0):
            self.assertEqual(blobs.copy_art(self.source, self.art, 'a/2d/normal.png').status, 'copied')
        self.assertEqual(len(self.calls), 2)

    def test_when_the_cdn_answers_something_else_than_a_png_it_should_store_nothing(self):
        self.answers['https://cdn.example/a/2d/normal.png'] = [b'<html>']
        self.assertEqual(blobs.copy_art(self.source, self.art, 'a/2d/normal.png').status, 'corrupt')
        self.assertFalse((self.tmp / 'art/a').exists())


class MissingThreshold(unittest.TestCase):
    def setUp(self):
        self.args = migrate.parser().parse_args(['blobs', 'worlds-content-server'])

    def test_when_a_small_batch_loses_a_few_keys_it_should_stay_under_the_count_floor(self):
        self.assertFalse(migrate.too_many_missing(1, 5, self.args))
        self.assertFalse(migrate.too_many_missing(10, 10, self.args))

    def test_when_a_batch_loses_more_than_both_limits_it_should_fail(self):
        self.assertTrue(migrate.too_many_missing(11, 100, self.args))
        self.assertFalse(migrate.too_many_missing(11, 2000, self.args))


class RecordOutcomes(unittest.TestCase):
    def test_when_there_are_more_than_a_batch_of_outcomes_it_should_flush_in_batches(self):
        outcomes = [blobs.Outcome(f'k{i}', 'copied', f'c{i}', 1) if i % 3 else
                    blobs.Outcome(f'k{i}', 'missing', reason='absent upstream') for i in range(2500)]
        outcomes.append(blobs.Outcome('flaky', 'failed', reason='read failed 3 times'))
        flushed = []
        counts, report, missing = migrate.record_outcomes(
            iter(outcomes), lambda s, m: flushed.append((len(s), len(m))), len(outcomes), lambda line: None)
        self.assertEqual(flushed, [(1000, 0), (666, 0)])
        self.assertEqual(len(missing), 834)
        self.assertEqual(counts['failed'], 1)
        self.assertIn("  'flaky': read failed 3 times", report)
        migrate.record_missing(lambda s, m: flushed.append((len(s), len(m))), missing)
        self.assertEqual(flushed[2:], [(0, 834)])

    def test_when_the_copy_stops_part_way_it_should_record_what_it_finished(self):
        def outcomes():
            yield blobs.Outcome('a', 'copied', 'ca', 1)
            raise blobs.SourceError('expired credentials')
        flushed = []
        with self.assertRaises(blobs.SourceError):
            migrate.record_outcomes(outcomes(), lambda s, m: flushed.append(s), 1, lambda line: None)
        self.assertEqual(flushed, [[('a', 'ca', 1)]])


class WhitelistRows(unittest.TestCase):
    def test_when_an_entry_sets_limits_it_should_convert_megabytes(self):
        rows = migrate.whitelist_rows({'big.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 200}}, None)
        self.assertEqual(rows, {'big.dcl.eth': (50, 200 * 1024 * 1024)})

    def test_when_an_entry_has_no_size_it_should_take_the_old_max_size(self):
        rows = migrate.whitelist_rows({'a.dcl.eth': {'allow_sdk6': True}, 'b.dcl.eth': {'max_size_in_mb': 0}}, 100)
        self.assertEqual(rows, {'a.dcl.eth': (None, 100 * 1024 * 1024), 'b.dcl.eth': (None, 100 * 1024 * 1024)})

    def test_when_an_entry_has_no_size_and_no_max_size_is_given_it_should_refuse(self):
        with self.assertRaisesRegex(migrate.MigrationError, '--max-size-mb'):
            migrate.whitelist_rows({'a.dcl.eth': {'max_parcels': 9}}, None)

    def test_when_an_ens_world_is_listed_it_should_keep_only_its_parcels(self):
        rows = migrate.whitelist_rows({'x.eth': {'max_parcels': 9, 'max_size_in_mb': 500}, 'y.eth': {}}, None)
        self.assertEqual(rows, {'x.eth': (9, None)})

    def test_when_a_key_is_not_lowercase_and_trimmed_it_should_warn_and_skip_it(self):
        warnings = []
        rows = migrate.whitelist_rows({'Big.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 200},
                                       ' big.dcl.eth': {'max_parcels': 60, 'max_size_in_mb': 300},
                                       'big.dcl.eth': {'max_parcels': 10, 'max_size_in_mb': 20}}, None,
                                      warn=warnings.append)
        self.assertEqual(rows, {'big.dcl.eth': (10, 20 * 1024 * 1024)})
        self.assertEqual(len(warnings), 2)

    def test_when_a_limit_exceeds_the_column_range_it_should_cap_it(self):
        rows = migrate.whitelist_rows({'a.dcl.eth': {'max_parcels': 2 ** 40, 'max_size_in_mb': str(2 ** 60)}}, None)
        self.assertEqual(rows, {'a.dcl.eth': (2 ** 31 - 1, (2 ** 63 - 1) // (1024 * 1024) * 1024 * 1024)})

    def test_when_limits_are_integer_strings_it_should_read_them_as_upstream_does(self):
        rows = migrate.whitelist_rows({'a.dcl.eth': {'max_parcels': '12', 'max_size_in_mb': '30'},
                                       'b.dcl.eth': {'max_parcels': '1.5', 'max_size_in_mb': 'big'}}, 100)
        self.assertEqual(rows, {'a.dcl.eth': (12, 30 * 1024 * 1024), 'b.dcl.eth': (None, 100 * 1024 * 1024)})


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


# A throwaway server to use instead of initdb (e.g. a disposable container); its databases are dropped.
SERVER_ENV = 'MIGRATION_TEST_SERVER_URL'


@unittest.skipUnless(shutil.which('psql') and (os.environ.get(SERVER_ENV) or
                                               (shutil.which('initdb') and shutil.which('pg_ctl'))),
                     'PostgreSQL binaries are not on PATH')
class AgainstPostgres(unittest.TestCase):
    """A throwaway cluster: the toy source end to end, and every real manifest's load against the schema."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp())
        cls.data = None
        server = os.environ.get(SERVER_ENV)
        if server:
            cls.base = server.rstrip('/').rsplit('/', 1)[0] if server.count('/') > 2 else server.rstrip('/')
            return
        data = cls.tmp / 'pg'
        initdb = subprocess.run(['initdb', '-D', str(data), '-U', 'postgres', '--auth=trust', '-E', 'UTF8',
                                 '--locale=C'], capture_output=True, text=True)
        if initdb.returncode != 0:
            shutil.rmtree(cls.tmp)
            # CI's dev shell has a working server; locally a client-only initdb (libpq) cannot start one
            if os.environ.get('CI'):
                raise RuntimeError(f'initdb failed: {initdb.stderr.strip()}')
            raise unittest.SkipTest(f'initdb cannot create a cluster here: {initdb.stderr.strip()[:200]}')
        cls.port = free_port()
        with (data / 'postgresql.conf').open('a') as conf:
            conf.write(f"port={cls.port}\nlisten_addresses='127.0.0.1'\nunix_socket_directories=''\nfsync=off\n")
        subprocess.run(['pg_ctl', '-D', str(data), '-l', str(cls.tmp / 'log'), '-w', 'start'], check=True,
                       capture_output=True)
        cls.data = data
        cls.base = f'postgres://postgres@127.0.0.1:{cls.port}'

    @classmethod
    def tearDownClass(cls):
        if cls.data:
            subprocess.run(['pg_ctl', '-D', str(cls.data), '-m', 'immediate', 'stop'], capture_output=True)
        shutil.rmtree(cls.tmp)

    def url(self, db):
        return f'{self.base}/{db}'

    def sql(self, db, script):
        return migrate.psql(self.url(db), script)

    def fresh_db(self, name):
        self.sql('postgres', f'DROP DATABASE IF EXISTS {name};\nCREATE DATABASE {name};\n')
        return name

    def migrate(self, *args, sources_dir):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = migrate.main(['--sources-dir', str(sources_dir), *args])
        return code, out.getvalue()

    def toy(self):
        sources = self.tmp / 'sources'
        if not sources.exists():
            write_source(sources)
        src, tgt = self.fresh_db('toy_src'), self.fresh_db('toy_tgt')
        self.sql(src, textwrap.dedent('''
            CREATE TABLE things (id int PRIMARY KEY, title text, updated_at timestamp NOT NULL);
            INSERT INTO things SELECT g, 'it''s ' || g || E'\\n"q",x', timestamp '2026-01-01' + (g % 7) * interval '1 hour'
              FROM generate_series(1, 250) g;
            CREATE TABLE tags (name text PRIMARY KEY, n int);
            INSERT INTO tags SELECT 't' || g, g FROM generate_series(1, 30) g;
        '''))
        self.sql(tgt, 'CREATE SCHEMA app;\nCREATE TABLE app.things (id int PRIMARY KEY, title text, updated_at timestamp);\n'
                      'CREATE TABLE app.tags (name text PRIMARY KEY, n int);\n')
        env = {'TOY_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)}
        patcher = unittest.mock.patch.dict(os.environ, env)
        patcher.start()
        self.addCleanup(patcher.stop)
        return sources, src, tgt

    def count(self, db, table):
        return int(self.sql(db, f'SELECT count(*) FROM {table};\n').strip())

    def test_when_dry_running_it_should_write_nothing(self):
        sources, _, tgt = self.toy()
        code, out = self.migrate('run', 'toy', '--dry-run', '--batch-size', '100', sources_dir=sources)
        self.assertEqual(code, 0, out)
        self.assertIn('inserted or upserted 250', out)
        self.assertEqual(self.count(tgt, 'app.things'), 0)
        self.assertEqual(self.sql(tgt, "SELECT to_regnamespace('dcl_migration') IS NULL;\n").strip(), 't')

    def test_when_dry_running_a_step_that_needs_an_earlier_one_it_should_see_its_rows(self):
        sources, _, tgt = self.toy()
        self.sql(tgt, 'ALTER TABLE app.tags ADD COLUMN thing int REFERENCES app.things (id);\n')
        dependent = {**FILES, 'tags.load.sql': 'INSERT INTO app.tags SELECT name, n, n FROM stage '
                                               'ON CONFLICT (name) DO UPDATE SET n = EXCLUDED.n;\n'}
        dep_dir = self.tmp / 'dependent'
        if not dep_dir.exists():
            write_source(dep_dir, dependent)
        code, out = self.migrate('run', 'toy', '--dry-run', sources_dir=dep_dir)
        self.assertEqual(code, 0, out)
        self.assertIn('toy/tags: first walk: 30 rows', out)
        self.assertEqual(self.count(tgt, 'app.tags'), 0)

    def test_when_a_batch_fails_it_should_resume_after_the_last_committed_one(self):
        sources, _, tgt = self.toy()
        self.sql(tgt, 'ALTER TABLE app.things ADD CONSTRAINT stop_here CHECK (id <> 150);\n')
        code, _ = self.migrate('run', 'toy', '--step', 'things', '--batch-size', '100', sources_dir=sources)
        self.assertEqual(code, 1)
        committed = self.count(tgt, 'app.things')
        self.assertEqual(committed, 100)
        self.sql(tgt, 'ALTER TABLE app.things DROP CONSTRAINT stop_here;\n')
        code, out = self.migrate('run', 'toy', '--batch-size', '100', sources_dir=sources)
        self.assertEqual(code, 0, out)
        self.assertIn('resuming interrupted walk', out)
        self.assertEqual(self.count(tgt, 'app.things'), 250)
        self.assertEqual(self.sql(tgt, 'SELECT walk_rows FROM dcl_migration.checkpoint WHERE step = \'things\';\n')
                         .strip(), '250')

    def test_when_rows_change_it_should_reimport_from_since(self):
        sources, src, tgt = self.toy()
        self.assertEqual(self.migrate('run', 'toy', sources_dir=sources)[0], 0)
        self.sql(src, "UPDATE things SET title = 'changed', updated_at = timestamp '2026-03-01' WHERE id <= 3;\n"
                      "UPDATE tags SET n = -1 WHERE name = 't1';\n")
        code, out = self.migrate('run', 'toy', '--since', '2026-02-01T00:00:00Z', sources_dir=sources)
        self.assertEqual(code, 0, out)
        self.assertIn('batch 1: 3 rows', out)
        self.assertEqual(self.sql(tgt, "SELECT count(*) FROM app.things WHERE title = 'changed';\n").strip(), '3')
        self.assertEqual(self.sql(tgt, "SELECT n FROM app.tags WHERE name = 't1';\n").strip(), '-1')

    def test_when_continuing_without_since_it_should_pick_up_rows_past_the_cursor(self):
        sources, src, tgt = self.toy()
        self.assertEqual(self.migrate('run', 'toy', sources_dir=sources)[0], 0)
        self.sql(src, "INSERT INTO things VALUES (999, 'late', timestamp '2026-04-01');\n")
        code, out = self.migrate('run', 'toy', '--step', 'things', sources_dir=sources)
        self.assertEqual(code, 0, out)
        self.assertIn('batch 1: 1 rows', out)
        self.assertEqual(self.count(tgt, 'app.things'), 251)

    def test_when_a_checkpoint_moves_under_a_run_it_should_stop(self):
        sources, _, tgt = self.toy()
        self.assertEqual(self.migrate('run', 'toy', '--step', 'things', sources_dir=sources)[0], 0)
        step = migrate.load_source(sources / 'toy').steps[0]
        walk = migrate.Walk(None, ['2026-01-01 06:00:00+00', '250'], '2026-01-01T00:00:00Z', 0, False, '')
        script = migrate.batch_script(step, walk, batch_size=10,
                                      expected=(['stale', '1'], '2026-01-01T00:00:00Z'))
        with tempfile.NamedTemporaryFile('w', suffix='.sql') as f, open(os.devnull, 'rb') as empty:
            f.write(script)
            f.flush()
            with self.assertRaisesRegex(migrate.MigrationError, 'changed under this run'):
                migrate.psql(self.url(tgt), None, stdin=empty, extra=['-f', f.name])

    def test_every_manifest_should_load_into_the_current_schema(self):
        sources = migrate.discover(migrate.SOURCES)
        if not sources:
            self.skipTest('no manifests')
        db = self.fresh_db('catalyst_schema')
        self.sql('postgres', "SELECT 1;\n")
        schema = ''.join(p.read_text() + '\n' for p in sorted((ROOT / 'schemas').glob('*.sql')))
        try:
            self.sql(db, schema)
        except migrate.MigrationError as e:
            self.skipTest(f'schemas/ does not apply on this cluster: {str(e)[:200]}')
        env = {s.database_env: '' for s in sources.values()}
        env['TARGET_DATABASE_URL'] = self.url(db)
        with unittest.mock.patch.dict(os.environ, env):
            code, out = self.migrate('check', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)

    # ── worlds-content-server: blobs, quota blocks, limits ──

    WCS_SCHEMA = textwrap.dedent('''
        CREATE TABLE worlds (name text PRIMARY KEY, owner text, access jsonb, spawn_coordinates text, title text,
          description text, content_rating text, skybox_time int, categories text[], single_player boolean,
          show_in_places boolean, thumbnail_hash text, settings_version bigint, created_at timestamp NOT NULL,
          updated_at timestamp NOT NULL);
        CREATE TABLE world_scenes (world_name text, entity_id text, status text, deployment_auth_chain json,
          entity jsonb, deployer text, parcels text[], size bigint, created_at timestamp NOT NULL,
          updated_at timestamp NOT NULL);
        CREATE TABLE blocked (wallet text PRIMARY KEY, created_at timestamp NOT NULL, updated_at timestamp NOT NULL);
    ''')

    # The worlds storage-quota tables, for a schemas/ that predates them (a no-op once it has them).
    QUOTA_BLOCKS = ('CREATE TABLE IF NOT EXISTS worlds.quota_blocks (wallet text PRIMARY KEY, '
                    'created_at timestamp NOT NULL, updated_at timestamp NOT NULL);\n')
    LIMIT_OVERRIDES = ('CREATE TABLE IF NOT EXISTS worlds.world_limit_overrides (world_name text PRIMARY KEY, '
                       'max_parcels integer, max_size_bytes bigint, created_at timestamp NOT NULL DEFAULT now(), '
                       'updated_at timestamp NOT NULL DEFAULT now());\n')

    def catalyst_db(self, name):
        """A fresh database holding schemas/, cloned from a template built once per class."""
        cls = type(self)
        if not getattr(cls, 'template_ready', False):
            self.fresh_db('catalyst_template')
            schema = ''.join(p.read_text() + '\n' for p in sorted((ROOT / 'schemas').glob('*.sql')))
            try:
                self.sql('catalyst_template', schema)
            except migrate.MigrationError as e:
                self.skipTest(f'schemas/ does not apply on this cluster: {str(e)[:200]}')
            cls.template_ready = True
        self.sql('postgres', f'DROP DATABASE IF EXISTS {name};\nCREATE DATABASE {name} TEMPLATE catalyst_template;\n')
        return name

    def wcs(self):
        src, tgt = self.fresh_db('wcs_src'), self.catalyst_db('wcs_tgt')
        self.sql(src, self.WCS_SCHEMA)
        env = {'WORLDS_CONTENT_SERVER_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)}
        patcher = unittest.mock.patch.dict(os.environ, env)
        patcher.start()
        self.addCleanup(patcher.stop)
        return src, tgt

    def test_when_importing_worlds_it_should_copy_their_blobs_and_rewrite_thumbnails(self):
        src, tgt = self.wcs()
        upstream, node = self.tmp / 'wcs-store', self.tmp / 'node-store'
        self.addCleanup(shutil.rmtree, upstream, True)
        self.addCleanup(shutil.rmtree, node, True)

        def put(data, key=None):
            key = key or blobs.hash_bytes_v1(data)
            (upstream / blobs.shard(key)).mkdir(parents=True, exist_ok=True)
            (upstream / blobs.shard(key) / key).write_bytes(data)
            return key

        upload = pattern(CHUNK + 99)
        upload_key = put(upload, hashlib.sha256(upload).hexdigest())
        model, picture = put(b'glb bytes'), put(b'png bytes')
        lost = blobs.hash_bytes_v1(b'never stored upstream')
        lost_thumbnail = hashlib.sha256(b'thumbnail upstream lost').hexdigest()
        bad = f'{model}\nbroken'
        scenes = {}
        for world, content in (('alpha.dcl.eth', {'a.glb': model, 'lost.png': lost, 'bad.png': bad}),
                               ('beta.dcl.eth', {'thumb.png': picture})):
            entity = json.dumps({'type': 'scene', 'pointers': ['0,0'],
                                 'content': [{'file': f, 'hash': h} for f, h in content.items()]}).encode()
            scenes[world] = put(entity)
            self.sql(src, f"INSERT INTO world_scenes VALUES ('{world}', '{scenes[world]}', 'DEPLOYED', '[]', "
                          f"'{entity.decode()}', '0x{'1' * 40}', '{{0,0}}', 10, now(), now());\n")
        self.sql(src, f"INSERT INTO worlds (name, owner, thumbnail_hash, created_at, updated_at) VALUES "
                      f"('alpha.dcl.eth', '0x{'1' * 40}', '{upload_key}', now(), now()), "
                      f"('beta.dcl.eth', '0x{'1' * 40}', '{picture}', now(), now()), "
                      f"('gamma.dcl.eth', '0x{'1' * 40}', '{lost_thumbnail}', now(), now());\n")
        steps = ('run', 'worlds-content-server', '--step', 'world', '--step', 'world_scene',
                 '--storage-root', str(node))

        code, out = self.migrate(*steps, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1)
        copy = ('blobs', 'worlds-content-server', '--from-dir', str(upstream), '--to', str(node))
        code, out = self.migrate(*copy, '--max-missing-count', '0', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn('2 of 8 new objects absent or unreadable upstream, over --max-missing 1%'
                      ' and --max-missing-count 0', out)
        self.assertEqual(self.count(tgt, 'dcl_migration.blob_missing'), 0)
        code, out = self.migrate(*copy, '--allow-missing', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('copied 0, already in the store 0, absent or unreadable upstream 2, hash mismatch 0,'
                      ' not a content id 1', out)
        # A re-run neither retries nor counts the keys recorded missing.
        code, out = self.migrate(*copy, '--max-missing-count', '0', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('8 objects named, 5 already copied, 3 recorded missing; copying 0', out)
        missing = self.sql(tgt, 'SELECT reason FROM dcl_migration.blob_missing WHERE object_key IN '
                                f'({migrate.literal(lost)}, {migrate.literal(lost_thumbnail)}, {migrate.literal(bad)})'
                                ' ORDER BY 1;\n')
        self.assertEqual(missing.splitlines(), ['absent upstream', 'absent upstream', 'not a content id'])
        code, out = self.migrate(*steps, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('0 rows with missing blobs', out)

        thumbs = self.sql(tgt, 'SELECT name, thumbnail_hash FROM worlds.worlds ORDER BY 1;\n').splitlines()
        self.assertEqual(thumbs, [f'alpha.dcl.eth\t{blobs.hash_bytes_v1(upload)}', f'beta.dcl.eth\t{picture}',
                                  'gamma.dcl.eth\t'])
        self.assertEqual(self.count(tgt, 'worlds.world_scenes'), 2)
        code, out = self.migrate('blobs', 'worlds-content-server', '--verify', '--to', str(node),
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn(f'alpha.dcl.eth/{scenes["alpha.dcl.eth"]}: 2 missing: ', out)
        self.assertIn(lost, out)
        self.assertNotIn('beta.dcl.eth', out)

        # A GC between `blobs` and `run` deletes copied blobs: the run fails instead of loading.
        blobs.Store(node).path(picture).unlink()
        code, out = self.migrate(*steps, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn(f'beta.dcl.eth/{scenes["beta.dcl.eth"]}: 1 missing: {picture}', out)

    def test_when_the_blob_source_is_wrong_it_should_fail_and_record_nothing(self):
        src, tgt = self.wcs()
        empty, node = self.tmp / 'wcs-wrong', self.tmp / 'node-wrong'
        empty.mkdir()
        self.addCleanup(shutil.rmtree, empty, True)
        self.addCleanup(shutil.rmtree, node, True)
        thumbs = [blobs.hash_bytes_v1(str(i).encode()) for i in range(3)]
        self.sql(src, 'INSERT INTO worlds (name, owner, thumbnail_hash, created_at, updated_at) VALUES '
                      + ', '.join(f"('w{i}.dcl.eth', '0x{'1' * 40}', '{t}', now(), now())" for i, t in enumerate(thumbs))
                      + ';\n')
        copy = ('blobs', 'worlds-content-server', '--from-dir', str(empty), '--to', str(node))
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code, out = self.migrate(*copy, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn('none of 3 sampled objects reads', err.getvalue())
        self.assertEqual(self.count(tgt, 'dcl_migration.blob_missing'), 0)
        with contextlib.redirect_stderr(io.StringIO()):
            code, out = self.migrate(*copy, '--allow-missing', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertEqual(self.count(tgt, 'dcl_migration.blob_missing'), 0)
        code, out = self.migrate(*copy, '--skip-probe', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.count(tgt, 'dcl_migration.blob_missing'), 3)

    def test_when_rerunning_blobs_it_should_count_only_new_keys_and_still_probe_the_source(self):
        src, tgt = self.wcs()
        upstream, empty, node = self.tmp / 'wcs-rerun', self.tmp / 'wcs-rerun-wrong', self.tmp / 'node-rerun'
        empty.mkdir()
        for d in (upstream, empty, node):
            self.addCleanup(shutil.rmtree, d, True)

        def world(i, stored):
            key = blobs.hash_bytes_v1(f'thumbnail {i}'.encode())
            if stored:
                put_upstream(upstream, key, f'thumbnail {i}'.encode())
            self.sql(src, f"INSERT INTO worlds (name, owner, thumbnail_hash, created_at, updated_at) VALUES "
                          f"('w{i}.dcl.eth', '0x{'1' * 40}', '{key}', now(), now());\n")
            return key

        def copy(*extra, source=upstream):
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                code, out = self.migrate('blobs', 'worlds-content-server', '--from-dir', str(source),
                                         '--to', str(node), *extra, sources_dir=migrate.SOURCES)
            return code, out + err.getvalue()

        def missing():
            return set(self.sql(tgt, 'SELECT object_key FROM dcl_migration.blob_missing;\n').split())

        for i in range(3):
            world(i, stored=True)
        lost = world(3, stored=False)
        code, out = copy()
        self.assertEqual(code, 0, out)
        self.assertEqual(missing(), {lost})

        code, out = copy('--max-missing-count', '0')
        self.assertEqual(code, 0, out)
        self.assertIn('copying 0', out)

        # One new key, absent upstream: the probe reads keys copied before.
        lost_again = world(4, stored=False)
        code, out = copy()
        self.assertEqual(code, 0, out)
        self.assertEqual(missing(), {lost, lost_again})

        later = world(5, stored=False)
        for extra in ((), ('--allow-missing',), ('--retry-missing',)):
            code, out = copy(*extra, source=empty)
            self.assertEqual(code, 1, (extra, out))
            self.assertIn('none of 4 sampled objects reads', out)
        self.assertEqual(missing(), {lost, lost_again})

        put_upstream(upstream, lost, b'thumbnail 3')
        code, out = copy('--retry-missing')
        self.assertEqual(code, 0, out)
        self.assertIn('2 recorded missing (retrying); copying 3', out)
        self.assertEqual(missing(), {lost_again, later})
        self.assertTrue(blobs.Store(node).has(lost))

    def test_when_a_places_world_image_names_a_copied_thumbnail_it_should_name_its_cid(self):
        tgt = self.catalyst_db('places_images')
        step = next(s for s in migrate.load_source(migrate.SOURCES / 'places').steps if s.name == 'place')
        data = b'uploaded thumbnail'
        key, cid = hashlib.sha256(data).hexdigest(), blobs.hash_bytes_v1(data)
        unknown = hashlib.sha256(b'not copied').hexdigest()
        base = 'https://worlds.example.org/contents/'
        defs = ', '.join(f'{migrate.ident(c)} {t}' for c, t in step.columns)
        rows = []
        for name, image in (('copied.dcl.eth', base + key), ('pending.dcl.eth', base + unknown),
                            ('scene.dcl.eth', base + 'bafkreiexample')):
            raw = json.dumps({'id': name, 'world_name': name, 'image': image, 'world': True, 'source': 'places'})
            rows.append(f"('{name}', '0,0', 't', '', NULL, NULL, '{{}}', now(), false, false, '{name}', "
                        f"'{raw}'::jsonb, false, NULL, NULL)")
        self.sql(tgt, migrate.CHECKPOINT_DDL
                 + f"INSERT INTO dcl_migration.blob_inventory VALUES ('worlds-content-server', '{key}', '{cid}', 1);\n"
                 + 'BEGIN;\n' + '\n'.join(migrate.step_vars(step, False))
                 + f'\nCREATE TEMP TABLE stage ({defs});\nINSERT INTO stage VALUES {", ".join(rows)};\n'
                 + f'\\i {step.load}\nCOMMIT;\n')
        images = self.sql(tgt, "SELECT id, raw->>'image' FROM explore.place ORDER BY 1;\n").splitlines()
        self.assertEqual(images, [f'copied.dcl.eth\t{base}{cid}', f'pending.dcl.eth\t{base}{unknown}',
                                  f'scene.dcl.eth\t{base}bafkreiexample'])

    def test_when_unbanning_quota_blocks_it_should_delete_only_the_former_steps_rows(self):
        src, tgt = self.wcs()
        imported, node_ban, other = f'0x{"a" * 40}', f'0x{"b" * 40}', f'0x{"c" * 40}'
        self.sql(src, f"INSERT INTO blocked VALUES ('{imported}', '2026-01-01 00:00:00.123456', '2026-01-02'), "
                      f"('{node_ban}', '2026-01-01', '2026-01-01');\n")
        migrate.ensure_checkpoint_table(self.url(tgt))
        self.sql(tgt, f"INSERT INTO worlds.blocked_wallets VALUES ('{imported}', '2026-01-01 00:00:00.123456', "
                      f"'2026-01-02'), ('{node_ban}', '2026-05-05', '2026-05-05'), ('{other}', now(), now());\n")
        code, out = self.migrate('worlds-unban-quota-blocks', sources_dir=migrate.SOURCES)
        self.assertIn('nothing to do', out)
        self.sql(tgt, "INSERT INTO dcl_migration.checkpoint (source, step, walk_started_at) "
                      "VALUES ('worlds-content-server', 'blocked_wallet', now());\n")
        code, out = self.migrate('worlds-unban-quota-blocks', '--dry-run', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('deleted 1 quota blocks loaded as bans (rolled back)', out)
        self.assertEqual(self.count(tgt, 'worlds.blocked_wallets'), 3)
        code, out = self.migrate('worlds-unban-quota-blocks', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('kept 1 rows', out)
        self.assertEqual(self.sql(tgt, 'SELECT wallet FROM worlds.blocked_wallets ORDER BY 1;\n').split(),
                         [node_ban, other])
        self.assertEqual(self.count(tgt, 'dcl_migration.checkpoint'), 0)

    def test_when_a_wallet_was_banned_again_on_the_node_it_should_keep_the_ban(self):
        src, tgt = self.wcs()
        imported, rebanned = f'0x{"a" * 40}', f'0x{"d" * 40}'
        self.sql(src, f"INSERT INTO blocked VALUES ('{imported}', '2026-01-01', '2026-01-02'), "
                      f"('{rebanned}', '2026-01-01', '2026-01-02');\n")
        migrate.ensure_checkpoint_table(self.url(tgt))
        # The admin ban upsert keeps created_at and bumps updated_at.
        self.sql(tgt, f"INSERT INTO worlds.blocked_wallets VALUES ('{imported}', '2026-01-01', '2026-01-02'), "
                      f"('{rebanned}', '2026-01-01', '2026-06-01');\n"
                      "INSERT INTO dcl_migration.checkpoint (source, step, walk_started_at) "
                      "VALUES ('worlds-content-server', 'blocked_wallet', '2026-03-01T00:00:00Z');\n")
        code, out = self.migrate('worlds-unban-quota-blocks', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('deleted 1 quota blocks', out)
        self.assertIn(f'kept 1 rows written on the node after the former step ran (a ban made there): {rebanned}',
                      out)
        self.assertEqual(self.sql(tgt, 'SELECT wallet FROM worlds.blocked_wallets;\n').split(), [rebanned])

    def test_when_importing_quota_blocks_it_should_not_ban_the_wallet(self):
        src, tgt = self.wcs()
        self.sql(src, f"INSERT INTO blocked VALUES ('0x{'AB' * 20}', now(), now());\n")
        self.sql(tgt, self.QUOTA_BLOCKS + 'ALTER TABLE worlds.quota_blocks RENAME TO quota_blocks_aside;\n')
        code, out = self.migrate('run', 'worlds-content-server', '--step', 'quota_block', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.sql(tgt, 'ALTER TABLE worlds.quota_blocks_aside RENAME TO quota_blocks;\n')
        code, out = self.migrate('run', 'worlds-content-server', '--step', 'quota_block', '--restart',
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT wallet FROM worlds.quota_blocks;\n').strip(), f'0x{"ab" * 20}')
        self.assertEqual(self.count(tgt, 'worlds.blocked_wallets'), 0)

    def test_when_the_node_already_holds_a_quota_block_it_should_keep_its_start(self):
        src, tgt = self.wcs()
        node_first, upstream_first = f'0x{"a" * 40}', f'0x{"b" * 40}'
        self.sql(src, f"INSERT INTO blocked VALUES ('{node_first}', '2026-05-01', '2026-05-01'), "
                      f"('{upstream_first}', '2026-01-01', '2026-01-01');\n")
        self.sql(tgt, self.QUOTA_BLOCKS
                      + f"INSERT INTO worlds.quota_blocks VALUES ('{node_first}', '2026-03-01', '2026-06-01'), "
                      f"('{upstream_first}', '2026-03-01', '2026-03-01');\n")
        code, out = self.migrate('run', 'worlds-content-server', '--step', 'quota_block', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        rows = self.sql(tgt, 'SELECT wallet, created_at, updated_at FROM worlds.quota_blocks ORDER BY 1;\n')
        self.assertEqual(rows.splitlines(), [f'{node_first}\t2026-03-01 00:00:00\t2026-06-01 00:00:00',
                                             f'{upstream_first}\t2026-01-01 00:00:00\t2026-03-01 00:00:00'])

    def test_when_seeding_limits_it_should_upsert_the_whitelist(self):
        _, tgt = self.wcs()
        self.sql(tgt, self.LIMIT_OVERRIDES)
        whitelist = self.tmp / 'whitelist.json'
        whitelist.write_text(json.dumps({'big.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 200}}))
        code, out = self.migrate('worlds-limits', str(whitelist), '--dry-run', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.count(tgt, 'worlds.world_limit_overrides'), 0)
        code, out = self.migrate('worlds-limits', str(whitelist), sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        rows = self.sql(tgt, 'SELECT world_name, max_parcels, max_size_bytes FROM worlds.world_limit_overrides;\n')
        self.assertEqual(rows.strip(), f'big.dcl.eth\t50\t{200 * 1024 * 1024}')

    def test_when_seeding_limits_again_it_should_keep_overrides_set_on_the_node(self):
        _, tgt = self.wcs()
        self.sql(tgt, self.LIMIT_OVERRIDES
                      + "INSERT INTO worlds.world_limit_overrides VALUES ('node.dcl.eth', 7, 7, now(), now());\n")
        whitelist = self.tmp / 'whitelist-again.json'
        entries = {'seeded.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 1},
                   'edited.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 1},
                   'node.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 1}}
        whitelist.write_text(json.dumps(entries))
        code, out = self.migrate('worlds-limits', str(whitelist), sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('2 rows written', out)
        self.assertIn('kept 1 rows set on the node that differ from the whitelist: node.dcl.eth', out)
        self.sql(tgt, "UPDATE worlds.world_limit_overrides SET max_parcels = 9 WHERE world_name = 'edited.dcl.eth';\n")
        whitelist.write_text(json.dumps({n: {**e, 'max_parcels': 60} for n, e in entries.items()}))
        code, out = self.migrate('worlds-limits', str(whitelist), sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('1 rows written', out)
        rows = self.sql(tgt, 'SELECT world_name, max_parcels FROM worlds.world_limit_overrides ORDER BY 1;\n')
        self.assertEqual(rows.splitlines(), ['edited.dcl.eth\t9', 'node.dcl.eth\t7', 'seeded.dcl.eth\t60'])


    # ── notifications-workers: broadcast audience, deletions ──

    NW_SCHEMA = textwrap.dedent("""
        CREATE TABLE notifications (id uuid PRIMARY KEY, event_key varchar NOT NULL, type varchar NOT NULL,
          address varchar, metadata jsonb NOT NULL, "timestamp" bigint NOT NULL, read_at bigint,
          created_at bigint NOT NULL, updated_at bigint NOT NULL);
        CREATE TABLE broadcast_read (notification_id uuid NOT NULL REFERENCES notifications, address varchar NOT NULL,
          read_at bigint NOT NULL, PRIMARY KEY (notification_id, address));
        CREATE TABLE subscriptions (address varchar PRIMARY KEY, email varchar, details jsonb NOT NULL,
          created_at bigint NOT NULL, updated_at bigint NOT NULL);
        CREATE TABLE unconfirmed_emails (address varchar PRIMARY KEY, email varchar NOT NULL, code varchar NOT NULL,
          created_at bigint NOT NULL, updated_at bigint NOT NULL);
        CREATE TABLE notification_opt_outs (address varchar NOT NULL, scope varchar NOT NULL, scope_id varchar NOT NULL,
          created_at bigint NOT NULL, updated_at bigint NOT NULL, PRIMARY KEY (address, scope, scope_id));
    """)
    BROADCAST = '00000000-0000-4000-8000-0000000000b1'
    COMMUNITY = '00000000-0000-4000-8000-0000000000c1'

    @staticmethod
    def wallet(digit):
        return f'0x{digit * 40}'

    def notifications_dbs(self):
        src, tgt = self.fresh_db('nw_src'), self.catalyst_db('nw_tgt')
        self.sql(src, self.NW_SCHEMA)
        w = self.wallet
        t = 1767225600000  # 2026-01-01
        self.sql(src, textwrap.dedent(f"""
            INSERT INTO subscriptions VALUES ('{w("a")}', NULL, '{{}}', {t}, {t});
            INSERT INTO notifications VALUES
              ('00000000-0000-4000-8000-0000000000a1', 'k1', 'item_sold', '{w("b")}', '{{}}', {t}, NULL, {t}, {t}),
              ('{self.BROADCAST}', 'news', 'announcement', NULL, '{{"title": "hi"}}', {t}, NULL, {t}, {t});
            INSERT INTO broadcast_read VALUES ('{self.BROADCAST}', '{w("1")}', {t + 1000});
            INSERT INTO notification_opt_outs VALUES
              ('{w("c")}', 'community', '{self.COMMUNITY}', {t}, {t}),
              ('{w("d")}', 'community', '{self.COMMUNITY}', {t}, {t});
            INSERT INTO unconfirmed_emails VALUES
              ('{w("e")}', 'e@example.com', '{"e" * 32}', {t}, {t}),
              ('{w("f")}', 'f@example.com', '{"f" * 32}', {t}, {t});
        """))
        # A wallet only the node has seen: it polled the node's bell.
        self.sql(tgt, f"INSERT INTO social.notification_reader_seen VALUES ('{w('2')}', now());\n")
        env = {'NOTIFICATIONS_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)}
        patcher = unittest.mock.patch.dict(os.environ, env)
        patcher.start()
        self.addCleanup(patcher.stop)
        return src, tgt

    def broadcast_copies(self, tgt):
        return self.sql(tgt, "SELECT address, read, id = md5(%s || ':' || address)::uuid FROM social.notifications "
                             "WHERE broadcast_key = 'notifications-workers:%s' ORDER BY address;\n"
                        % (f"'{self.BROADCAST}'", self.BROADCAST)).splitlines()

    def test_when_importing_a_broadcast_it_should_reach_every_wallet_the_source_or_node_knows(self):
        _, tgt = self.notifications_dbs()
        w = self.wallet
        code, out = self.migrate('run', 'notifications-workers', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        expected = [f'{w(d)}\t{"t" if d == "1" else "f"}\tt' for d in '12abcdef']
        self.assertEqual(self.broadcast_copies(tgt), expected)
        code, out = self.migrate('run', 'notifications-workers', '--step', 'broadcast', '--restart',
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.broadcast_copies(tgt), expected)

    def test_when_upstream_drops_an_opt_out_or_pending_email_it_should_delete_only_the_imported_row(self):
        src, tgt = self.notifications_dbs()
        w = self.wallet
        self.assertEqual(self.migrate('run', 'notifications-workers', sources_dir=migrate.SOURCES)[0], 0)
        # The node's own opt-out, and a code the node issued for f after the import.
        self.sql(tgt, f"INSERT INTO social.subscription_opt_outs VALUES ('{w('9')}', 'community', "
                      f"'{self.COMMUNITY}', now());\n"
                      f"UPDATE social.unconfirmed_emails SET code = '{'9' * 32}' WHERE address = '{w('f')}';\n")
        self.sql(src, f"DELETE FROM notification_opt_outs WHERE address = '{w('d')}';\n"
                      f"DELETE FROM unconfirmed_emails;\n"
                      f"INSERT INTO unconfirmed_emails VALUES ('{w('3')}', 'g@example.com', '{'3' * 32}', "
                      f"1767225600000, (extract(epoch from now()) * 1000)::bigint);\n")
        code, out = self.migrate('run', 'notifications-workers', '--since', 'last', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT address FROM social.subscription_opt_outs ORDER BY 1;\n').split(),
                         [w('9'), w('c')])
        self.assertEqual(self.sql(tgt, 'SELECT address FROM social.unconfirmed_emails ORDER BY 1;\n').split(),
                         [w('3'), w('f')])
        self.assertEqual(self.sql(tgt, 'SELECT address FROM dcl_migration.unconfirmed_email_loaded;\n').split(),
                         [w('3')])

    def test_when_upstream_lists_no_opt_outs_it_should_refuse_to_delete_the_imported_ones(self):
        src, tgt = self.notifications_dbs()
        self.assertEqual(self.migrate('run', 'notifications-workers', sources_dir=migrate.SOURCES)[0], 0)
        self.sql(src, 'DELETE FROM notification_opt_outs;\n')
        code, out = self.migrate('run', 'notifications-workers', '--step', 'opt_out_reconcile',
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertEqual(self.count(tgt, 'social.subscription_opt_outs'), 2)


    # ── badges: catalog, presence configuration, progress ──

    BADGES_SCHEMA = textwrap.dedent("""
        CREATE TABLE badge_definitions (id text PRIMARY KEY, name text NOT NULL, category text NOT NULL,
          description text NOT NULL, has_3d boolean NOT NULL, published_at timestamptz, archived_at timestamptz,
          created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
          updated_at timestamptz NOT NULL DEFAULT clock_timestamp());
        CREATE TABLE badge_tiers (badge_id text NOT NULL REFERENCES badge_definitions ON DELETE CASCADE,
          tier_id text NOT NULL, position integer NOT NULL, name text NOT NULL, description text NOT NULL,
          steps integer NOT NULL, asset_slot text NOT NULL, PRIMARY KEY (badge_id, tier_id));
        CREATE TABLE presence_badges (badge_id text PRIMARY KEY REFERENCES badge_definitions,
          location_kind varchar(16) NOT NULL, parcel varchar(32), world_name varchar(255),
          window_start timestamptz NOT NULL, window_end timestamptz NOT NULL, mode varchar(16) NOT NULL,
          required_seconds integer NOT NULL DEFAULT 0, created_at timestamptz NOT NULL DEFAULT now());
        CREATE TABLE user_progress (user_address varchar(255) NOT NULL, badge_id varchar(255) NOT NULL,
          progress jsonb NOT NULL, achieved_tiers jsonb, completed_at bigint, updated_at bigint NOT NULL,
          PRIMARY KEY (user_address, badge_id));
    """)

    # The badge catalog schema, for a schemas/ that predates it (a no-op once it has it).
    BADGES_TARGET = textwrap.dedent("""
        ALTER TABLE social.badge_definitions
          ADD COLUMN IF NOT EXISTS criteria_steps integer CHECK (criteria_steps > 0),
          ADD COLUMN IF NOT EXISTS published_at timestamp,
          ADD COLUMN IF NOT EXISTS archived_at timestamp,
          ADD COLUMN IF NOT EXISTS created_at timestamp NOT NULL DEFAULT now();
        ALTER TABLE social.user_badge_progress ADD COLUMN IF NOT EXISTS progress jsonb NOT NULL DEFAULT '{}';
        ALTER TABLE social.badge_tiers ADD CONSTRAINT test_badge_tiers_steps CHECK (criteria_steps > 0);
        CREATE UNIQUE INDEX IF NOT EXISTS test_badge_tiers_ordinal ON social.badge_tiers (badge_id, ordinal);
        CREATE TABLE IF NOT EXISTS social.presence_badges (
          badge_id text PRIMARY KEY REFERENCES social.badge_definitions (id), location_kind varchar(16) NOT NULL,
          parcel varchar(32), world_name varchar(255), window_start timestamptz NOT NULL,
          window_end timestamptz NOT NULL, mode varchar(16) NOT NULL, required_seconds integer NOT NULL DEFAULT 0,
          created_at timestamptz NOT NULL DEFAULT now());
    """)

    T0 = 1767225600000  # 2026-01-01T00:00:00Z

    @staticmethod
    def tiered(badge, slots):
        prefix = badge.replace('_', '-')
        return ', '.join(f"('{badge}', '{prefix}-{slot}', {i + 1}, '{slot.title()}', '{slot}', {steps}, '{slot}')"
                         for i, (slot, steps) in enumerate(slots))

    def badges_dbs(self, target_schema=True):
        src, tgt = self.fresh_db('badges_src'), self.catalyst_db('badges_tgt')
        self.sql(src, self.BADGES_SCHEMA)
        t = self.T0
        self.sql(src, textwrap.dedent(f"""
            INSERT INTO badge_definitions (id, name, category, description, has_3d, published_at, archived_at,
                                           created_at) VALUES
              ('dclcitizen', 'Decentraland Citizen', 'Explorer', 'Landed', true, to_timestamp({t} / 1000.0), NULL,
               to_timestamp({t} / 1000.0)),
              ('open_for_business', 'Open for Business', 'Creator', 'Store and collection', true,
               to_timestamp({t} / 1000.0), NULL, to_timestamp(({t} + 1) / 1000.0)),
              ('emotionista', 'Emotionista', 'Collector', 'Emotes purchased', true, to_timestamp({t} / 1000.0), NULL,
               to_timestamp(({t} + 2) / 1000.0)),
              ('walkabout_wanderer', 'Walkabout Wanderer', 'Explorer', 'Steps', true, to_timestamp({t} / 1000.0),
               NULL, to_timestamp(({t} + 3) / 1000.0)),
              ('halloweeks_2026', 'Halloweeks 2026', 'Explorer', 'Visited', true, to_timestamp({t} / 1000.0),
               timestamptz '2026-11-08 12:00:00+00', to_timestamp(({t} + 4) / 1000.0)),
              ('stay_fest', 'Stay Fest', 'Explorer', 'Stayed', true, NULL, NULL, to_timestamp(({t} + 5) / 1000.0));
            INSERT INTO badge_tiers VALUES
              ('dclcitizen', 'dclcitizen-root', 1, 'Decentraland Citizen', 'Landed', 1, 'root'),
              ('open_for_business', 'open-for-business-root', 1, 'Open for Business', 'x', 2, 'root'),
              ('halloweeks_2026', 'halloweeks-2026-root', 1, 'Halloweeks 2026', 'Visited', 1, 'root'),
              ('stay_fest', 'stay-fest-root', 1, 'Stay Fest', 'Stayed', 600, 'root'),
              {self.tiered('emotionista', [('starter', 1), ('bronze', 5), ('silver', 10)])},
              {self.tiered('walkabout_wanderer', [('starter', 10000), ('bronze', 50000)])};
            INSERT INTO presence_badges VALUES
              ('stay_fest', 'scene', '10,-20', NULL, '2026-11-01 00:00:00+00', '2026-11-02 00:00:00+00', 'stay', 600,
               '2026-10-30 00:00:00+00');
        """))
        if target_schema:
            self.sql(tgt, self.BADGES_TARGET)
        # Only the seed rows the import replaces or deletes, whichever catalog the schema seeds.
        self.sql(tgt, "DELETE FROM social.badge_definitions WHERE id NOT IN ('dclcitizen', 'open_for_business', "
                      "'emotionista', 'decentraland_citizen', 'walkabout');\n")
        env = {'BADGES_DATABASE_URL': self.url(src), 'TARGET_DATABASE_URL': self.url(tgt)}
        patcher = unittest.mock.patch.dict(os.environ, env)
        patcher.start()
        self.addCleanup(patcher.stop)
        return src, tgt

    def progress(self, src, address, badge, document, tiers=None, completed=None, updated=0):
        tiers_sql = 'NULL' if tiers is None else "'" + json.dumps(
            [{'tier_id': k, 'completed_at': self.T0 + v} for k, v in tiers]) + "'"
        completed_sql = 'NULL' if completed is None else str(self.T0 + completed)
        self.sql(src, f"INSERT INTO user_progress VALUES ('{address}', '{badge}', '{json.dumps(document)}', "
                      f"{tiers_sql}, {completed_sql}, {self.T0 + updated});\n")

    def test_when_importing_the_catalog_it_should_keep_publication_steps_and_upstream_asset_urls(self):
        _, tgt = self.badges_dbs()
        code, out = self.migrate('run', 'badges', '--step', 'catalog', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        rows = self.sql(tgt, "SELECT id, is_tier, criteria_steps, published_at IS NOT NULL, archived_at, "
                             "to_char(created_at, 'YYYY-MM-DD HH24:MI:SS.MS') FROM social.badge_definitions "
                             "ORDER BY created_at, id;\n").splitlines()
        self.assertEqual(rows, [
            'dclcitizen\tf\t1\tt\t\t2026-01-01 00:00:00.000',
            'open_for_business\tf\t2\tt\t\t2026-01-01 00:00:00.001',
            'emotionista\tt\t\tt\t\t2026-01-01 00:00:00.002',
            'walkabout_wanderer\tt\t\tt\t\t2026-01-01 00:00:00.003',
            'halloweeks_2026\tf\t1\tt\t2026-11-08 12:00:00\t2026-01-01 00:00:00.004',
            'stay_fest\tf\t600\tf\t\t2026-01-01 00:00:00.005',
        ])
        assets = self.sql(tgt, "SELECT id, assets FROM social.badge_definitions WHERE id IN "
                               "('dclcitizen', 'emotionista') ORDER BY id;\n").splitlines()
        self.assertEqual(json.loads(assets[0].split('\t')[1]), {
            '2d': {'normal': 'dclcitizen/2d/normal.png'},
            '3d': {'normal': 'dclcitizen/3d/normal.png', 'hrm': 'dclcitizen/3d/hrm.png',
                   'basecolor': 'dclcitizen/3d/basecolor.png'}})
        self.assertEqual(assets[1], 'emotionista\t{}')
        tiers = self.sql(tgt, "SELECT tier_id, ordinal, criteria_steps, assets->'2d'->>'normal' FROM social.badge_tiers "
                              "WHERE badge_id = 'emotionista' ORDER BY ordinal;\n").splitlines()
        self.assertEqual(tiers, ['emotionista-starter\t0\t1\temotionista/starter/2d/normal.png',
                                 'emotionista-bronze\t1\t5\temotionista/bronze/2d/normal.png',
                                 'emotionista-silver\t2\t10\temotionista/silver/2d/normal.png'])
        self.assertEqual(self.sql(tgt, "SELECT count(*) FROM social.badge_tiers WHERE badge_id IN "
                                       "('dclcitizen', 'open_for_business', 'halloweeks_2026');\n").strip(), '0')
        self.assertEqual(self.sql(tgt, "SELECT count(*) FROM social.badge_definitions WHERE id IN "
                                       "('decentraland_citizen', 'walkabout');\n").strip(), '0')
        code, out = self.migrate('run', 'badges', '--step', 'catalog', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('catalog: done, 6 rows in this walk; no writes', out)

    def test_when_importing_presence_badges_it_should_copy_their_configuration(self):
        _, tgt = self.badges_dbs()
        code, out = self.migrate('run', 'badges', '--step', 'catalog', '--step', 'presence_badge',
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        # Epoch seconds: the same instants whether the node's columns carry a time zone or hold UTC.
        self.assertEqual(self.sql(tgt, 'SELECT badge_id, location_kind, parcel, world_name, extract(epoch FROM '
                                       'window_start)::bigint, extract(epoch FROM window_end)::bigint, mode, '
                                       'required_seconds, extract(epoch FROM created_at)::bigint '
                                       'FROM social.presence_badges;\n').strip(),
                         'stay_fest\tscene\t10,-20\t\t1793491200\t1793577600\tstay\t600\t1793318400')

    def test_when_importing_progress_it_should_keep_the_document_partial_steps_and_archived_badges(self):
        src, tgt = self.badges_dbs()
        w1, w2 = self.wallet('1'), self.wallet('2')
        self.progress(src, w1, 'open_for_business', {'steps': 1, 'store_completed': True})
        self.progress(src, w1, 'emotionista', {'steps': 6, 'transactions_emotes_purchase': ['0xa', '0xb']},
                      tiers=[('emotionista-starter', 10), ('emotionista-bronze', 20)], updated=30)
        self.progress(src, w2, 'halloweeks_2026', {'steps': 1}, completed=5, updated=5)
        self.progress(src, w2, 'stay_fest', {'steps': 120, 'intervals': [[1, 121]]}, updated=7)
        code, out = self.migrate('run', 'badges', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        rows = self.sql(tgt, 'SELECT address, badge_id, steps_done, progress, completed_at IS NOT NULL, '
                             'last_completed_tier_id FROM social.user_badge_progress ORDER BY 1, 2;\n').splitlines()
        self.assertEqual([r.split('\t') for r in rows], [
            [w1, 'emotionista', '6', '{"steps": 6, "transactions_emotes_purchase": ["0xa", "0xb"]}', 'f',
             'emotionista-bronze'],
            [w1, 'open_for_business', '1', '{"steps": 1, "store_completed": true}', 'f', ''],
            [w2, 'halloweeks_2026', '1', '{"steps": 1}', 't', ''],
            [w2, 'stay_fest', '120', '{"steps": 120, "intervals": [[1, 121]]}', 'f', ''],
        ])
        self.assertEqual(self.sql(tgt, 'SELECT tier_id FROM social.user_achieved_tiers ORDER BY 1;\n').split(),
                         ['emotionista-bronze', 'emotionista-starter'])

    def test_when_a_wallet_has_rows_in_two_cases_it_should_merge_them(self):
        src, tgt = self.badges_dbs()
        lower = self.wallet('a')
        mixed = '0x' + 'A' * 40
        self.progress(src, lower, 'emotionista', {'steps': 2, 'transactions_emotes_purchase': ['0xa', '0xb']},
                      tiers=[('emotionista-starter', 10)], updated=40)
        self.progress(src, mixed, 'emotionista',
                      {'steps': 7, 'transactions_emotes_purchase': ['0xc', '0xa'], 'note': 'mixed'},
                      tiers=[('emotionista-starter', 5), ('emotionista-bronze', 30)], updated=35)
        code, out = self.migrate('run', 'badges', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        row = self.sql(tgt, "SELECT address, steps_done, progress, last_completed_tier_id, "
                            "extract(epoch FROM updated_at) * 1000 FROM social.user_badge_progress;\n").strip()
        address, steps, document, last, updated = row.split('\t')
        self.assertEqual((address, steps, last, int(float(updated))), (lower, '7', 'emotionista-bronze', self.T0 + 40))
        self.assertEqual(json.loads(document),
                         {'steps': 7, 'note': 'mixed', 'transactions_emotes_purchase': ['0xa', '0xb', '0xc']})
        tiers = self.sql(tgt, "SELECT tier_id, (extract(epoch FROM completed_at) * 1000)::bigint "
                              "FROM social.user_achieved_tiers ORDER BY 1;\n").splitlines()
        self.assertEqual(tiers, [f'emotionista-bronze\t{self.T0 + 30}', f'emotionista-starter\t{self.T0 + 5}'])

    def test_when_the_node_holds_more_steps_it_should_keep_them_and_take_the_source_document(self):
        src, tgt = self.badges_dbs()
        w = self.wallet('3')
        self.assertEqual(self.migrate('run', 'badges', '--step', 'catalog', sources_dir=migrate.SOURCES)[0], 0)
        self.sql(tgt, f"INSERT INTO social.user_badge_progress (address, badge_id, steps_done, granted_by) "
                      f"VALUES ('{w}', 'open_for_business', 2, 'console');\n")
        self.progress(src, w, 'open_for_business', {'steps': 1, 'collection_submitted': True})
        code, out = self.migrate('run', 'badges', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT steps_done, progress, granted_by FROM social.user_badge_progress;\n')
                         .strip(), '2\t{"steps": 1, "collection_submitted": true}\tconsole')

    def test_when_the_schema_lacks_the_badge_columns_it_should_stop_instead_of_dropping_data(self):
        src, tgt = self.badges_dbs()
        self.progress(src, self.wallet('4'), 'dclcitizen', {'steps': 1}, completed=1, updated=1)
        self.sql(tgt, 'ALTER TABLE social.badge_definitions DROP COLUMN archived_at;\n')
        code, out = self.migrate('run', 'badges', '--step', 'catalog', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertEqual(self.sql(tgt, "SELECT count(*) FROM social.badge_definitions WHERE id = 'stay_fest';\n")
                         .strip(), '0')
        self.sql(tgt, 'ALTER TABLE social.badge_definitions ADD COLUMN archived_at timestamp;\n'
                      'ALTER TABLE social.user_badge_progress DROP COLUMN progress;\n'
                      'ALTER TABLE social.presence_badges RENAME TO presence_badges_aside;\n')
        self.assertEqual(self.migrate('run', 'badges', '--step', 'catalog', sources_dir=migrate.SOURCES)[0], 0)
        code, out = self.migrate('run', 'badges', '--step', 'presence_badge', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        code, out = self.migrate('run', 'badges', '--step', 'user_progress', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertEqual(self.count(tgt, 'social.user_badge_progress'), 0)

    def test_when_mirroring_badge_art_it_should_copy_every_texture_the_catalog_names(self):
        _, tgt = self.badges_dbs()
        self.assertEqual(self.migrate('run', 'badges', '--step', 'catalog', sources_dir=migrate.SOURCES)[0], 0)
        upstream, art = self.tmp / 'badge-cdn', self.tmp / 'badge-art'
        self.addCleanup(shutil.rmtree, upstream, True)
        self.addCleanup(shutil.rmtree, art, True)
        dirs = ['dclcitizen', 'open_for_business', 'halloweeks_2026', 'stay_fest',
                *[f'emotionista/{s}' for s in ('starter', 'bronze', 'silver')],
                *[f'walkabout_wanderer/{s}' for s in ('starter', 'bronze')]]
        textures = ['2d/normal.png', '3d/normal.png', '3d/hrm.png', '3d/basecolor.png']
        expected = sorted(f'{d}/{t}' for d in dirs for t in textures)
        for path in expected[1:]:
            (upstream / path).parent.mkdir(parents=True, exist_ok=True)
            (upstream / path).write_bytes(blobs.PNG_MAGIC + path.encode())
        code, out = self.migrate('blobs', 'badges', '--to', str(art), '--verify', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn(f'{len(expected)} textures checked', out)
        code, out = self.migrate('blobs', 'badges', '--to', str(art), '--from-dir', str(upstream),
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn(f'copied {len(expected) - 1}', out)
        self.assertIn(f'{expected[0]} (', out)
        stored = sorted(str(p.relative_to(art)) for p in art.rglob('*.png'))
        self.assertEqual(stored, expected[1:])
        self.assertEqual((art / expected[1]).read_bytes(), blobs.PNG_MAGIC + expected[1].encode())
        (upstream / expected[0]).parent.mkdir(parents=True, exist_ok=True)
        (upstream / expected[0]).write_bytes(blobs.PNG_MAGIC)
        code, out = self.migrate('blobs', 'badges', '--to', str(art), '--from-dir', str(upstream),
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn(f'copied 1, already there {len(expected) - 1}', out)
        code, out = self.migrate('blobs', 'badges', '--to', str(art), '--verify', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)


import unittest.mock  # noqa: E402

if __name__ == '__main__':
    unittest.main()
