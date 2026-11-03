"""The migration engine resumes, dry-runs and re-imports from a point, and every manifest compiles."""
import contextlib
import datetime
import importlib.util
import io
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


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


@unittest.skipUnless(shutil.which('initdb') and shutil.which('pg_ctl') and shutil.which('psql'),
                     'PostgreSQL binaries are not on PATH')
class AgainstPostgres(unittest.TestCase):
    """A throwaway cluster: the toy source end to end, and every real manifest's load against the schema."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp())
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

    @classmethod
    def tearDownClass(cls):
        subprocess.run(['pg_ctl', '-D', str(cls.data), '-m', 'immediate', 'stop'], capture_output=True)
        shutil.rmtree(cls.tmp)

    def url(self, db):
        return f'postgres://postgres@127.0.0.1:{self.port}/{db}'

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


import unittest.mock  # noqa: E402

if __name__ == '__main__':
    unittest.main()
