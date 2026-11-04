"""The migration engine resumes, dry-runs and re-imports from a point, and every manifest compiles."""
import contextlib
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

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('migrate', ROOT / 'tooling/migration/migrate.py')
migrate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migrate)
blobs = migrate.load_blobs_module()
CHUNK = blobs.CHUNK_SIZE


def pattern(n):
    return (bytes(range(251)) * (n // 251 + 1))[:n]

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


class WhitelistRows(unittest.TestCase):
    def test_when_an_entry_sets_limits_it_should_convert_megabytes(self):
        rows = migrate.whitelist_rows({'Big.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 200}}, None)
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
        scenes = {}
        for world, content in (('alpha.dcl.eth', {'a.glb': model, 'lost.png': lost}),
                               ('beta.dcl.eth', {'thumb.png': picture})):
            entity = json.dumps({'type': 'scene', 'pointers': ['0,0'],
                                 'content': [{'file': f, 'hash': h} for f, h in content.items()]}).encode()
            scenes[world] = put(entity)
            self.sql(src, f"INSERT INTO world_scenes VALUES ('{world}', '{scenes[world]}', 'DEPLOYED', '[]', "
                          f"'{entity.decode()}', '0x{'1' * 40}', '{{0,0}}', 10, now(), now());\n")
        self.sql(src, f"INSERT INTO worlds (name, owner, thumbnail_hash, created_at, updated_at) VALUES "
                      f"('alpha.dcl.eth', '0x{'1' * 40}', '{upload_key}', now(), now()), "
                      f"('beta.dcl.eth', '0x{'1' * 40}', '{picture}', now(), now());\n")
        steps = ('run', 'worlds-content-server', '--step', 'world', '--step', 'world_scene')

        code, out = self.migrate(*steps, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1)
        copy = ('blobs', 'worlds-content-server', '--from-dir', str(upstream), '--to', str(node))
        code, out = self.migrate(*copy, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertIn('copied 5, already in the store 0, absent upstream 1', out)
        code, out = self.migrate(*copy, sources_dir=migrate.SOURCES)
        self.assertIn('6 objects named, 5 already copied', out)
        code, out = self.migrate(*steps, sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)

        thumbs = self.sql(tgt, 'SELECT name, thumbnail_hash FROM worlds.worlds ORDER BY 1;\n').split()
        self.assertEqual(thumbs, ['alpha.dcl.eth', blobs.hash_bytes_v1(upload), 'beta.dcl.eth', picture])
        self.assertEqual(self.count(tgt, 'worlds.world_scenes'), 2)
        code, out = self.migrate('blobs', 'worlds-content-server', '--verify', '--to', str(node),
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.assertIn(f'alpha.dcl.eth/{scenes["alpha.dcl.eth"]}: 1 missing: {lost}', out)
        self.assertNotIn('beta.dcl.eth', out)

    def test_when_importing_quota_blocks_it_should_not_ban_the_wallet(self):
        src, tgt = self.wcs()
        self.sql(src, f"INSERT INTO blocked VALUES ('0x{'AB' * 20}', now(), now());\n")
        code, out = self.migrate('run', 'worlds-content-server', '--step', 'quota_block', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 1, out)
        self.sql(tgt, 'CREATE TABLE worlds.quota_blocks (wallet text PRIMARY KEY, created_at timestamp NOT NULL, '
                      'updated_at timestamp NOT NULL);\n')
        code, out = self.migrate('run', 'worlds-content-server', '--step', 'quota_block', '--restart',
                                 sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.sql(tgt, 'SELECT wallet FROM worlds.quota_blocks;\n').strip(), f'0x{"ab" * 20}')
        self.assertEqual(self.count(tgt, 'worlds.blocked_wallets'), 0)

    def test_when_seeding_limits_it_should_upsert_the_whitelist(self):
        _, tgt = self.wcs()
        self.sql(tgt, 'CREATE TABLE worlds.world_limit_overrides (world_name text PRIMARY KEY, max_parcels integer, '
                      'max_size_bytes bigint, created_at timestamp NOT NULL DEFAULT now(), '
                      'updated_at timestamp NOT NULL DEFAULT now());\n')
        whitelist = self.tmp / 'whitelist.json'
        whitelist.write_text(json.dumps({'Big.dcl.eth': {'max_parcels': 50, 'max_size_in_mb': 200}}))
        code, out = self.migrate('worlds-limits', str(whitelist), '--dry-run', sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        self.assertEqual(self.count(tgt, 'worlds.world_limit_overrides'), 0)
        code, out = self.migrate('worlds-limits', str(whitelist), sources_dir=migrate.SOURCES)
        self.assertEqual(code, 0, out)
        rows = self.sql(tgt, 'SELECT world_name, max_parcels, max_size_bytes FROM worlds.world_limit_overrides;\n')
        self.assertEqual(rows.strip(), f'big.dcl.eth\t50\t{200 * 1024 * 1024}')


import unittest.mock  # noqa: E402

if __name__ == '__main__':
    unittest.main()
