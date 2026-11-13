#!/usr/bin/env python3
"""Copies rows from the TypeScript services' Postgres databases into catalyst.

One engine, one manifest per source under sources/<name>/source.toml (README.md
holds the contract). Each step is a keyset walk over its extract query; every
batch commits its rows and its checkpoint in one target transaction, so a run
resumes after the last committed batch. Talks to Postgres through `psql` only.
"""
import argparse
import contextlib
import dataclasses
import datetime
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
import urllib.parse
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCES = HERE / 'sources'
TARGET_ENV = 'TARGET_DATABASE_URL'
# psql error verbosity: `terse` keeps failing rows (personal data) out of logs; --verbose-errors lifts it.
VERBOSITY = 'terse'
NAME = re.compile(r'^[a-z0-9][a-z0-9_-]*$')
STEP_NAME = re.compile(r'^[a-z0-9_]+$')
IDENT = re.compile(r'^[a-z_][a-z0-9_]*$')
QUALIFIED = re.compile(r'^[a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*$')
# Statements a load must not run: the engine owns the transaction and the session.
FORBIDDEN_LOAD = re.compile(
    r'^\s*(COMMIT|ROLLBACK|ABORT|START\s+TRANSACTION|VACUUM|TRUNCATE)\b'
    # BEGIN/END only as transaction statements: PL/pgSQL blocks use both words
    r'|^\s*(BEGIN|END)(\s+(TRANSACTION|WORK))?\s*;'
    r'|^\s*\\',
    re.I | re.M)
URL_PARAMS = {
    'sslmode': 'PGSSLMODE', 'sslrootcert': 'PGSSLROOTCERT', 'sslcert': 'PGSSLCERT',
    'sslkey': 'PGSSLKEY', 'connect_timeout': 'PGCONNECT_TIMEOUT',
    'application_name': 'PGAPPNAME', 'target_session_attrs': 'PGTARGETSESSIONATTRS',
    'options': 'PGOPTIONS',
}

CHECKPOINT_DDL = """
SET client_min_messages = warning;
CREATE SCHEMA IF NOT EXISTS dcl_migration;
CREATE TABLE IF NOT EXISTS dcl_migration.checkpoint (
    source text NOT NULL,
    step text NOT NULL,
    since timestamptz,
    cursor jsonb,
    walk_started_at timestamptz NOT NULL,
    walk_rows bigint NOT NULL DEFAULT 0,
    completed_at timestamptz,
    last_completed_walk_started_at timestamptz,
    total_rows bigint NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source, step)
);
-- Filled by the blob copy (`blobs`, or an external job for sources it does not cover): every
-- upstream object it stored, under the hash the blob store now holds it by. Loads of blob-backed
-- rows join it and refuse objects in neither table.
CREATE TABLE IF NOT EXISTS dcl_migration.blob_inventory (
    source text NOT NULL,
    object_key text NOT NULL,
    hash text NOT NULL,
    size bigint NOT NULL CHECK (size >= 0),
    PRIMARY KEY (source, object_key)
);
-- Objects the blob copy could not store: absent upstream, bytes that do not hash to the key, or a
-- key that is no content id. The node serves them as missing, as upstream did.
CREATE TABLE IF NOT EXISTS dcl_migration.blob_missing (
    source text NOT NULL,
    object_key text NOT NULL,
    reason text NOT NULL,
    PRIMARY KEY (source, object_key)
);
-- The worlds.world_limit_overrides values `worlds-limits` last wrote; a row that no longer
-- matches them was changed on the node and is left alone.
CREATE TABLE IF NOT EXISTS dcl_migration.world_limit_seeded (
    world_name text PRIMARY KEY,
    max_parcels integer,
    max_size_bytes bigint
);
-- What the notifications-workers loads wrote, so its reconcile steps delete only imported rows that
-- upstream no longer holds; a row whose values no longer match was changed on the node and is kept.
CREATE TABLE IF NOT EXISTS dcl_migration.notification_opt_out_loaded (
    address text NOT NULL,
    scope text NOT NULL,
    scope_id text NOT NULL,
    created_at timestamp NOT NULL,
    PRIMARY KEY (address, scope, scope_id)
);
CREATE TABLE IF NOT EXISTS dcl_migration.unconfirmed_email_loaded (
    address text PRIMARY KEY,
    code text NOT NULL
);
-- What the world-storage-service loads wrote: per upstream key, the target row and the updated_at
-- it got. Its reconcile steps delete only those rows, and only while that updated_at still holds.
CREATE TABLE IF NOT EXISTS dcl_migration.wss_world_storage_loaded (
    world_name text NOT NULL,
    place_id uuid NOT NULL,
    key text NOT NULL,
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    updated_at timestamp NOT NULL,
    PRIMARY KEY (world_name, place_id, key)
);
CREATE INDEX IF NOT EXISTS wss_world_storage_loaded_target
    ON dcl_migration.wss_world_storage_loaded (realm, scene_namespace, key);
CREATE TABLE IF NOT EXISTS dcl_migration.wss_player_storage_loaded (
    world_name text NOT NULL,
    place_id uuid NOT NULL,
    player_address text NOT NULL,
    key text NOT NULL,
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    player text NOT NULL,
    updated_at timestamp NOT NULL,
    PRIMARY KEY (world_name, place_id, player_address, key)
);
CREATE INDEX IF NOT EXISTS wss_player_storage_loaded_target
    ON dcl_migration.wss_player_storage_loaded (realm, scene_namespace, player, key);
CREATE TABLE IF NOT EXISTS dcl_migration.wss_env_variable_loaded (
    world_name text NOT NULL,
    place_id uuid NOT NULL,
    key text NOT NULL,
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    updated_at timestamp NOT NULL,
    PRIMARY KEY (world_name, place_id, key)
);
CREATE INDEX IF NOT EXISTS wss_env_variable_loaded_target
    ON dcl_migration.wss_env_variable_loaded (realm, scene_namespace, key);
RESET client_min_messages;
"""


class MigrationError(Exception):
    pass


# ── manifests ───────────────────────────────────────────────────────────────

@dataclasses.dataclass
class Step:
    source: str
    name: str
    targets: list
    extract: Path
    load: Path
    key: list
    watermark: str | None
    columns: list  # [(name, type)]
    notes: str = ''
    whole: bool = False  # one batch holding the whole extract, so the load sees the full upstream set

    @property
    def cursor(self):
        return ([self.watermark] if self.watermark else []) + self.key


@dataclasses.dataclass
class Source:
    name: str
    directory: Path
    repository: str
    revision: str
    database_env: str
    after: list
    steps: list
    notes: str = ''


def load_source(directory):
    path = directory / 'source.toml'
    try:
        data = tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as e:
        raise MigrationError(f'{path}: {e}') from e
    problems = []

    def need(table, field, kind, where):
        value = table.get(field)
        if not isinstance(value, kind):
            problems.append(f'{where}: `{field}` must be a {kind.__name__}')
            return kind()
        return value

    name = need(data, 'name', str, path)
    if name != directory.name:
        problems.append(f'{path}: name {name!r} must match its directory {directory.name!r}')
    if not NAME.match(name or ''):
        problems.append(f'{path}: name {name!r} must match {NAME.pattern}')
    database_env = need(data, 'database_env', str, path)
    if not re.match(r'^[A-Z][A-Z0-9_]*$', database_env or ''):
        problems.append(f'{path}: database_env {database_env!r} must be an environment variable name')
    if database_env == TARGET_ENV:
        problems.append(f'{path}: database_env must not be {TARGET_ENV}')
    after = data.get('after', [])
    if not isinstance(after, list) or not all(isinstance(a, str) for a in after):
        problems.append(f'{path}: `after` must be a list of source names')
        after = []
    steps = []
    seen = set()
    raw_steps = data.get('step', [])
    if not isinstance(raw_steps, list) or not raw_steps:
        problems.append(f'{path}: declares no [[step]]')
        raw_steps = []
    for index, raw in enumerate(raw_steps):
        where = f'{path} step {raw.get("name", index)!r}'
        step_name = need(raw, 'name', str, where)
        if not STEP_NAME.match(step_name or ''):
            problems.append(f'{where}: name must match {STEP_NAME.pattern}')
        if step_name in seen:
            problems.append(f'{where}: duplicate step name')
        seen.add(step_name)
        targets = need(raw, 'targets', list, where)
        for t in targets:
            if not isinstance(t, str) or not QUALIFIED.match(t):
                problems.append(f'{where}: target {t!r} must be schema.table')
        columns = []
        for c in need(raw, 'columns', list, where):
            if not (isinstance(c, dict) and isinstance(c.get('name'), str) and isinstance(c.get('type'), str)):
                problems.append(f'{where}: each column needs `name` and `type`')
                continue
            if not IDENT.match(c['name']) or c['name'] == '_ord':
                problems.append(f'{where}: column {c["name"]!r} is not a plain lowercase identifier')
            if ';' in c['type'] or '--' in c['type']:
                problems.append(f'{where}: column type {c["type"]!r} is not a type')
            columns.append((c['name'], c['type']))
        names = [c for c, _ in columns]
        if len(set(names)) != len(names):
            problems.append(f'{where}: duplicate column names')
        key = need(raw, 'key', list, where)
        if not key:
            problems.append(f'{where}: `key` must name at least one column')
        watermark = raw.get('watermark')
        if watermark is not None and not isinstance(watermark, str):
            problems.append(f'{where}: `watermark` must be a column name')
            watermark = None
        for c in key + ([watermark] if watermark else []):
            if c not in names:
                problems.append(f'{where}: cursor column {c!r} is not in `columns`')
        whole = raw.get('whole', False)
        if not isinstance(whole, bool):
            problems.append(f'{where}: `whole` must be a boolean')
            whole = False
        if whole and watermark:
            problems.append(f'{where}: a `whole` step reads the full set every run and takes no watermark')
        if watermark and dict(columns).get(watermark, '').lower() not in ('timestamptz', 'timestamp with time zone'):
            problems.append(f'{where}: watermark {watermark!r} must be typed timestamptz')
        files = {}
        for field in ('extract', 'load'):
            rel = need(raw, field, str, where)
            file = (directory / rel).resolve()
            if directory.resolve() not in file.parents or not file.is_file():
                problems.append(f'{where}: {field} file {rel!r} is missing or outside {directory.name}/')
            files[field] = file
        steps.append(Step(name, step_name, targets, files['extract'], files['load'], key, watermark,
                          columns, raw.get('notes', ''), whole))
    if problems:
        raise MigrationError('\n'.join(problems))
    source = Source(name, directory, data.get('repository', ''), data.get('revision', ''), database_env,
                    after, steps, data.get('notes', ''))
    for step in source.steps:
        problems.extend(lint_sql(step))
    if problems:
        raise MigrationError('\n'.join(problems))
    return source


def strip_sql_comments(sql):
    sql = re.sub(r'/\*.*?\*/', ' ', sql, flags=re.S)
    return re.sub(r'--[^\n]*', ' ', sql)


def lint_sql(step):
    problems = []
    where = f'{step.source}/{step.name}'
    extract = strip_sql_comments(step.extract.read_text()).strip()
    if not re.match(r'^(SELECT|WITH)\b', extract, re.I):
        problems.append(f'{where}: extract must be one SELECT (or WITH ... SELECT)')
    if extract.endswith(';') or ';' in extract:
        problems.append(f'{where}: extract must be a single statement without `;`')
    if re.search(r'^\s*\\', extract, re.M):
        problems.append(f'{where}: extract must not contain psql backslash commands')
    load = strip_sql_comments(step.load.read_text())
    if FORBIDDEN_LOAD.search(load):
        problems.append(f'{where}: load must not control the transaction, truncate or use psql commands')
    if re.search(r'\b(CREATE|ALTER|DROP)\s+(SCHEMA|TABLE)\s+(IF\s+(NOT\s+)?EXISTS\s+)?dcl_migration\b', load, re.I):
        problems.append(f'{where}: load must not define dcl_migration objects; the engine owns them')
    if not re.search(r'\bstage\b', load):
        problems.append(f'{where}: load never reads `stage`')
    return problems


def discover(sources_dir, names=('all',)):
    """The valid manifests. Problems fail only for the sources `names` selects and the sources they
    run after, so an unfinished manifest blocks its own runs, not every other source's."""
    sources, problems = {}, {}
    if not sources_dir.is_dir():
        return sources
    for directory in sorted(p for p in sources_dir.iterdir() if p.is_dir()):
        if not (directory / 'source.toml').exists():
            continue
        try:
            source = load_source(directory)
            sources[source.name] = source
        except MigrationError as e:
            problems[directory.name] = str(e)
    for source in sources.values():
        for dep in source.after:
            if dep not in sources and dep not in problems:
                problems.setdefault(source.name, f'{source.name}: `after` names unknown source {dep!r}')
    names = list(names)
    if names == ['all'] or not names:
        needed = set(sources) | set(problems)
    else:
        needed, queue = set(), list(names)
        while queue:
            name = queue.pop()
            if name not in needed:
                needed.add(name)
                queue += sources[name].after if name in sources else []
    failing = [problems[n] for n in sorted(needed) if n in problems]
    if failing:
        raise MigrationError('\n'.join(failing))
    return {n: src for n, src in sources.items() if n not in problems}


def ordered(sources, names):
    """`names` and nothing else, each after the sources it names in `after`."""
    selected = set(sources) if names == ['all'] else set(names)
    unknown = selected - set(sources)
    if unknown:
        raise MigrationError(f'unknown source(s): {", ".join(sorted(unknown))}')
    out, state = [], {}

    def visit(name, chain):
        if state.get(name) == 'done':
            return
        if state.get(name) == 'visiting':
            raise MigrationError(f'`after` cycle: {" -> ".join(chain + [name])}')
        state[name] = 'visiting'
        for dep in sources[name].after:
            if dep in selected:
                visit(dep, chain + [name])
        state[name] = 'done'
        out.append(sources[name])

    for name in sorted(selected):
        visit(name, [])
    return out


# ── psql ────────────────────────────────────────────────────────────────────

def connection_env(url, read_only=False):
    """libpq environment for `url`, so credentials never appear in argv."""
    env = {k: v for k, v in os.environ.items() if not k.startswith('PG')}
    parsed = urllib.parse.urlsplit(url)
    options = []
    if parsed.scheme in ('postgres', 'postgresql'):
        if parsed.hostname:
            env['PGHOST'] = parsed.hostname
        if parsed.port:
            env['PGPORT'] = str(parsed.port)
        if parsed.username:
            env['PGUSER'] = urllib.parse.unquote(parsed.username)
        if parsed.password:
            env['PGPASSWORD'] = urllib.parse.unquote(parsed.password)
        database = urllib.parse.unquote(parsed.path.lstrip('/'))
        if database:
            env['PGDATABASE'] = database
        for key, value in urllib.parse.parse_qsl(parsed.query):
            if key == 'host':
                env['PGHOST'] = value
            elif key == 'options':
                options.append(value)
            elif key in URL_PARAMS:
                env[URL_PARAMS[key]] = value
            else:
                raise MigrationError(f'unsupported connection parameter {key!r}')
    else:
        raise MigrationError('database URLs must be postgres:// or postgresql:// URIs')
    options += ['-c TimeZone=UTC', '-c standard_conforming_strings=on', '-c DateStyle=ISO']
    if read_only:
        options.append('-c default_transaction_read_only=on')
    env['PGOPTIONS'] = ' '.join(options)
    return env


def psql(url, script=None, *, read_only=False, stdin=None, stdout=None, extra=()):
    """Runs `script` through psql -f - (or stdin passthrough when stdin is a file)."""
    args = ['psql', '-X', '-At', '-F', '\t', '-v', 'ON_ERROR_STOP=1', '-v', f'VERBOSITY={VERBOSITY}', *extra]
    env = connection_env(url, read_only)
    try:
        if stdin is not None:
            proc = subprocess.run(args, stdin=stdin, stdout=stdout or subprocess.PIPE,
                                  stderr=subprocess.PIPE, env=env)
        else:
            proc = subprocess.run(args + ['-f', '-'], input=script.encode(), stdout=stdout or subprocess.PIPE,
                                  stderr=subprocess.PIPE, env=env)
    except FileNotFoundError as e:
        raise MigrationError('psql is not on PATH') from e
    if proc.returncode != 0:
        raise MigrationError(proc.stderr.decode(errors='replace').strip() or f'psql exited {proc.returncode}')
    for line in proc.stderr.decode(errors='replace').splitlines():
        if line.strip():
            print(f'    {line.split("NOTICE:  ", 1)[-1] if "NOTICE:" in line else line}', file=sys.stderr, flush=True)
    return proc.stdout.decode(errors='replace') if stdout is None else ''


def literal(value):
    if value is None:
        return 'NULL'
    return "'" + str(value).replace("'", "''") + "'"


def ident(name):
    return '"' + name.replace('"', '""') + '"'


def database_url(env_name):
    url = os.environ.get(env_name)
    if not url:
        raise MigrationError(f'{env_name} is not set')
    return url


# ── checkpoints ─────────────────────────────────────────────────────────────

@dataclasses.dataclass
class Checkpoint:
    since: str | None = None
    cursor: list | None = None
    walk_started_at: str | None = None
    walk_rows: int = 0
    completed_at: str | None = None
    last_completed_walk_started_at: str | None = None
    total_rows: int = 0


def read_checkpoints(target_url, source=None):
    exists = psql(target_url, "SELECT to_regclass('dcl_migration.checkpoint') IS NOT NULL;\n").strip()
    if exists != 't':
        return {}
    where = f' WHERE source = {literal(source)}' if source else ''
    out = psql(target_url, f"""SELECT source, step, json_build_object(
        'since', since, 'cursor', cursor, 'walk_started_at', walk_started_at, 'walk_rows', walk_rows,
        'completed_at', completed_at, 'last_completed_walk_started_at', last_completed_walk_started_at,
        'total_rows', total_rows) FROM dcl_migration.checkpoint{where} ORDER BY 1, 2;\n""")
    checkpoints = {}
    for line in out.splitlines():
        if line:
            src, step, payload = line.split('\t', 2)
            checkpoints[(src, step)] = Checkpoint(**json.loads(payload))
    return checkpoints


def same_instant(a, b):
    if a is None or b is None:
        return a is b
    return parse_timestamp(a) == parse_timestamp(b)


def parse_timestamp(value):
    text = value.strip().replace(' ', 'T', 1)
    if text.endswith('Z'):
        text = text[:-1] + '+00:00'
    text = re.sub(r'([+-]\d\d)$', r'\1:00', text)
    try:
        moment = datetime.datetime.fromisoformat(text)
    except ValueError as e:
        raise MigrationError(f'not an ISO 8601 timestamp: {value!r}') from e
    if moment.tzinfo is None:
        raise MigrationError(f'timestamp {value!r} needs a time zone (e.g. Z)')
    return moment.astimezone(datetime.timezone.utc)


def parse_duration(text):
    match = re.fullmatch(r'(\d+)([smhd])', text.strip())
    if not match:
        raise argparse.ArgumentTypeError(f'duration {text!r}: use e.g. 30s, 10m, 2h, 1d')
    unit = {'s': 'seconds', 'm': 'minutes', 'h': 'hours', 'd': 'days'}[match.group(2)]
    return datetime.timedelta(**{unit: int(match.group(1))})


def iso(moment):
    return moment.astimezone(datetime.timezone.utc).isoformat().replace('+00:00', 'Z')


@dataclasses.dataclass
class Walk:
    """Where one run of a step starts: a fresh walk or the resumption of one."""
    since: str | None
    cursor: list | None
    started_at: str | None  # None: take the source clock when the walk begins
    rows: int
    fresh: bool
    reason: str


def plan_walk(step, checkpoint, *, since, since_last, overlap, restart):
    """The walk a run takes for `step`, given its stored checkpoint (or None)."""
    if step.whole:
        # Never a partial set: its load treats what the extract lacks as gone upstream.
        return Walk(None, None, None, 0, True, 'whole set')
    if since_last:
        if checkpoint and checkpoint.last_completed_walk_started_at:
            since = iso(parse_timestamp(checkpoint.last_completed_walk_started_at) - overlap)
        else:
            return Walk(None, None, None, 0, True, 'no completed walk yet: full walk')
    if since is not None and not step.watermark:
        since_note = ' (step has no watermark: full walk)'
        since = None
        if checkpoint and not checkpoint.completed_at and checkpoint.since is None and not restart:
            return Walk(None, checkpoint.cursor, checkpoint.walk_started_at, checkpoint.walk_rows, False,
                        'resuming interrupted walk' + since_note)
        return Walk(None, None, None, 0, True, 'full walk' + since_note)
    if restart or checkpoint is None:
        return Walk(since, None, None, 0, True,
                    'restart' if restart else (f'since {since}' if since else 'first walk'))
    if not checkpoint.completed_at and (since is None or same_instant(since, checkpoint.since)):
        return Walk(checkpoint.since, checkpoint.cursor, checkpoint.walk_started_at, checkpoint.walk_rows,
                    False, 'resuming interrupted walk')
    if since is not None:
        return Walk(since, None, None, 0, True, f'since {since}')
    if step.watermark:
        return Walk(None, checkpoint.cursor, None, 0, True, 'continuing after the last cursor')
    return Walk(None, None, None, 0, True, 'full re-scan (no watermark)')


# ── a step's batches ────────────────────────────────────────────────────────

def extract_sql(step, walk_since, cursor, batch_size):
    extract = strip_sql_comments(step.extract.read_text()).strip()
    cols = ', '.join(f'e.{ident(c)}' for c, _ in step.columns)
    conditions = []
    if cursor is not None:
        if len(cursor) != len(step.cursor):
            raise MigrationError(f'{step.source}/{step.name}: stored cursor {cursor} does not match {step.cursor};'
                                 ' rerun with --restart')
        left = ', '.join(f'e.{ident(c)}' for c in step.cursor)
        right = ', '.join(literal(v) for v in cursor)
        conditions.append(f'({left}) > ({right})' if len(cursor) > 1 else f'{left} > {right}')
    if walk_since is not None:
        conditions.append(f'e.{ident(step.watermark)} >= {literal(walk_since)}::timestamptz')
    where = f' WHERE {" AND ".join(conditions)}' if conditions else ''
    order = ', '.join(f'e.{ident(c)}' for c in step.cursor)
    limit = f' LIMIT {int(batch_size)}' if batch_size is not None else ''
    return (f'COPY (SELECT {cols} FROM (\n{extract}\n) AS e{where} ORDER BY {order}{limit})'
            ' TO STDOUT WITH (FORMAT csv);\n')


def null_check(step):
    cols = ' OR '.join(f'{ident(c)} IS NULL' for c in step.cursor)
    return ('DO $nulls$ BEGIN\n'
            f'  IF EXISTS (SELECT 1 FROM stage WHERE {cols}) THEN\n'
            f"    RAISE EXCEPTION 'a cursor column ({', '.join(step.cursor)}) is null in the extract';\n"
            '  END IF;\n'
            'END $nulls$;')


def step_vars(step, dry_run):
    return [f'\\set source {literal(step.source)}', f'\\set step {literal(step.name)}',
            f"\\set dry_run {'true' if dry_run else 'false'}"]


def batch_script(step, walk, *, batch_size, expected):
    """The target-side script of one batch; the batch's CSV arrives on psql's stdin.

    `expected` is the checkpoint row this batch must find, (cursor, walk_started_at), or None
    for no row: a run that finds anything else stops instead of overwriting another run's work.
    """
    defs = ', '.join(f'{ident(c)} {t}' for c, t in step.columns)
    col_list = ', '.join(ident(c) for c, _ in step.columns)
    last_cursor = ', '.join(f'{ident(c)}::text' for c in step.cursor)
    src, name = literal(step.source), literal(step.name)
    lines = ['\\set ON_ERROR_STOP 1', *step_vars(step, False), "\\set batch_cursor ''", 'BEGIN;']
    if expected is None:
        found = 'NOT EXISTS (SELECT 1 FROM dcl_migration.checkpoint WHERE source = {0} AND step = {1})'
    else:
        cursor = literal(json.dumps(expected[0])) + '::jsonb' if expected[0] is not None else 'NULL'
        found = ('EXISTS (SELECT 1 FROM dcl_migration.checkpoint WHERE source = {0} AND step = {1}'
                 f' AND cursor IS NOT DISTINCT FROM {cursor}'
                 f' AND walk_started_at = {literal(expected[1])}::timestamptz)')
    lines += [
        f"SELECT pg_advisory_xact_lock(hashtextextended('dcl_migration:' || {src} || ':' || {name}, 0)) \\g /dev/null",
        'DO $guard$ BEGIN\n'
        f'  IF NOT {found.format(src, name)} THEN\n'
        f"    RAISE EXCEPTION 'checkpoint of %/% changed under this run; is another run active?', {src}, {name};\n"
        '  END IF;\n'
        'END $guard$;',
    ]
    lines += [
        f'CREATE TEMP TABLE stage ({defs}, _ord bigint GENERATED ALWAYS AS IDENTITY) ON COMMIT DROP;',
        f'\\copy stage ({col_list}) FROM pstdin WITH (FORMAT csv)',
        null_check(step),
        "SELECT '@@rows', count(*) FROM stage;",
        f"SELECT COALESCE((SELECT json_build_array({last_cursor})::text FROM stage ORDER BY _ord DESC LIMIT 1), '')"
        ' AS batch_cursor \\gset',
        "\\echo '@@cursor' :batch_cursor",
        'ALTER TABLE stage DROP COLUMN _ord;',
        "\\echo '@@load'",
        f'\\i {step.load}',
        "\\echo '@@loaded'",
    ]
    lines.append(checkpoint_upsert(step, walk, batch_size, first_batch=expected is None or walk.fresh))
    lines.append('COMMIT;')
    return '\n'.join(lines) + '\n'


def checkpoint_upsert(step, walk, batch_size, first_batch):
    """Records the batch staged in `stage` (its last cursor is :'batch_cursor')."""
    src, name = literal(step.source), literal(step.name)
    since = f'{literal(walk.since)}::timestamptz' if walk.since else 'NULL'
    started = f'{literal(walk.started_at)}::timestamptz'
    prior = literal(json.dumps(walk.cursor)) + '::jsonb' if walk.cursor is not None else 'NULL'
    rows = '(SELECT count(*) FROM stage)'
    done = 'true' if batch_size is None else f'({rows} < {int(batch_size)})'
    walk_rows = 'EXCLUDED.walk_rows' if first_batch else 'c.walk_rows + EXCLUDED.walk_rows'
    return f"""INSERT INTO dcl_migration.checkpoint AS c
  (source, step, since, cursor, walk_started_at, walk_rows, completed_at, last_completed_walk_started_at,
   total_rows, updated_at)
SELECT {src}, {name}, {since}, COALESCE(NULLIF(:'batch_cursor', '')::jsonb, {prior}), {started}, {rows},
       CASE WHEN {done} THEN now() END, CASE WHEN {done} THEN {started} END, {rows}, now()
ON CONFLICT (source, step) DO UPDATE SET
  since = EXCLUDED.since,
  cursor = EXCLUDED.cursor,
  walk_started_at = EXCLUDED.walk_started_at,
  walk_rows = {walk_rows},
  completed_at = EXCLUDED.completed_at,
  last_completed_walk_started_at = COALESCE(EXCLUDED.last_completed_walk_started_at,
                                            c.last_completed_walk_started_at),
  total_rows = c.total_rows + EXCLUDED.total_rows,
  updated_at = now();"""


def parse_batch_output(out):
    rows, cursor, tags, in_load = 0, None, [], False
    for line in out.splitlines():
        if line.startswith('@@rows\t'):
            rows = int(line.split('\t', 1)[1])
        elif line == '@@cursor' or line.startswith('@@cursor '):
            value = line[len('@@cursor'):].strip()
            cursor = json.loads(value) if value else None
        elif line == '@@load':
            in_load = True
        elif line == '@@loaded':
            in_load = False
        elif in_load and re.match(r'^(INSERT \d+ \d+|UPDATE \d+|DELETE \d+|MERGE \d+)$', line):
            tags.append(line)
    return rows, cursor, tags


def summarize_tags(tags):
    totals = {}
    for tag in tags:
        verb, *nums = tag.split()
        totals[verb] = totals.get(verb, 0) + int(nums[-1])
    labels = {'INSERT': 'inserted or upserted', 'UPDATE': 'updated', 'DELETE': 'deleted', 'MERGE': 'merged'}
    return ', '.join(f'{labels[v]} {n}' for v, n in totals.items()) or 'no writes'


def source_now(source_url):
    return psql(source_url, 'SELECT now();\n', read_only=True).strip()


def run_step(step, source_url, target_url, walk, checkpoint, *, batch_size, log):
    """Walks `step` batch by batch from `walk`; returns (rows, write summary)."""
    if step.whole:
        batch_size = None
    started_at = walk.started_at or iso(parse_timestamp(source_now(source_url)))
    walk = dataclasses.replace(walk, started_at=started_at)
    # The checkpoint row the first batch must find: what this run read, untouched.
    expected = (checkpoint.cursor, checkpoint.walk_started_at) if checkpoint else None
    cursor, rows_total, batch_no, tag_totals = walk.cursor, walk.rows, 0, []
    while True:
        batch_no += 1
        batch_walk = dataclasses.replace(walk, cursor=cursor, fresh=walk.fresh and batch_no == 1)
        with tempfile.TemporaryDirectory(prefix='dcl-migration-') as tmp:
            data = Path(tmp) / 'batch.csv'
            # The whole batch lands on disk before the target sees it: a source that fails
            # mid-stream must not commit a truncated batch as if the walk had ended.
            with data.open('wb') as fh:
                psql(source_url, extract_sql(step, walk.since, cursor, batch_size), read_only=True, stdout=fh)
            script = Path(tmp) / 'batch.sql'
            script.write_text(batch_script(step, batch_walk, batch_size=batch_size, expected=expected))
            with data.open('rb') as fh:
                out = psql(target_url, None, stdin=fh, extra=['-f', str(script)])
        rows, last, tags = parse_batch_output(out)
        tag_totals += tags
        rows_total += rows
        if last is not None:
            cursor = last
        expected = (cursor, started_at)
        log(f'    batch {batch_no}: {rows} rows ({summarize_tags(tags)})'
            + (f', cursor {json.dumps(cursor)}' if cursor else ''))
        if batch_size is None or rows < batch_size:
            return rows_total, summarize_tags(tag_totals)


def dry_run(plans, target_url, log):
    """Loads every planned step in one target transaction and rolls it back, so a step (or a source
    named in `after`) sees the rows the steps before it would have written. Extracts are whole
    walks, staged on disk first; `--batch-size` does not apply."""
    with tempfile.TemporaryDirectory(prefix='dcl-migration-dry-') as tmp:
        lines = ['\\set ON_ERROR_STOP 1', 'BEGIN;', CHECKPOINT_DDL.strip()]
        for index, (step, source_url, walk) in enumerate(plans):
            data = Path(tmp) / f'{index}.csv'
            with data.open('wb') as fh:
                psql(source_url, extract_sql(step, walk.since, walk.cursor, None), read_only=True, stdout=fh)
            defs = ', '.join(f'{ident(c)} {t}' for c, t in step.columns)
            col_list = ', '.join(ident(c) for c, _ in step.columns)
            lines += [
                *step_vars(step, True),
                f"\\echo '@@step' {step.source}/{step.name}",
                f'CREATE TEMP TABLE stage ({defs});',
                f"\\copy stage ({col_list}) FROM '{data}' WITH (FORMAT csv)",
                null_check(step),
                "SELECT '@@rows', count(*) FROM stage;",
                "\\echo '@@load'",
                f'\\i {step.load}',
                "\\echo '@@loaded'",
                'DROP TABLE stage;',
            ]
        lines.append('ROLLBACK;')
        script = Path(tmp) / 'dry-run.sql'
        script.write_text('\n'.join(lines) + '\n')
        out = psql(target_url, None, stdin=subprocess.DEVNULL, extra=['-f', str(script)])
    segments, current = {}, None
    for line in out.splitlines():
        if line.startswith('@@step '):
            current = line.split(' ', 1)[1]
            segments[current] = []
        elif current:
            segments[current].append(line)
    for step, _, walk in plans:
        key = f'{step.source}/{step.name}'
        rows, _, tags = parse_batch_output('\n'.join(segments.get(key, [])))
        log(f'  {key}: {walk.reason}: {rows} rows ({summarize_tags(tags)}, rolled back)')


# Financial state is imported only by explicitly naming its source. The
# guard must accompany any selected financial step, even with --step.
EXCLUDED_FROM_ALL = {'credits-server', 'shop-server', 'marketplace-server', 'marketplace-favorites-server'}
CUTOVER_STEPS = {
    'credits-server': {'cutover_guard', 'wallet', 'purchase', 'ledger'},
    'shop-server': {'treasury_guard', 'treasury_ledger', 'treasury_pending_refill'},
}
CUTOVER_GUARDS = {'credits-server': 'cutover_guard', 'shop-server': 'treasury_guard'}


def run_sources(sources, names, steps=None):
    selected = ordered(sources, names)
    if names == ['all']:
        selected = [src for src in selected if src.name not in EXCLUDED_FROM_ALL]
    unknown = set(steps or []) - {step.name for src in selected for step in src.steps}
    if unknown:
        raise MigrationError(f'unknown or excluded step(s): {", ".join(sorted(unknown))};'
                             ' cutover steps require an explicit source, not `all`')
    if steps:
        filtered = []
        for src in selected:
            wanted = set(steps)
            if wanted & CUTOVER_STEPS.get(src.name, set()):
                wanted.add(CUTOVER_GUARDS[src.name])
            filtered.append(dataclasses.replace(src, steps=[step for step in src.steps if step.name in wanted]))
        selected = [src for src in filtered if src.steps]
    return selected


# ── commands ────────────────────────────────────────────────────────────────

def ensure_checkpoint_table(target_url):
    psql(target_url, CHECKPOINT_DDL)


def cmd_list(args, sources):
    for source in ordered(sources, ['all']):
        configured = 'set' if os.environ.get(source.database_env) else 'unset'
        after = f' (after {", ".join(source.after)})' if source.after else ''
        print(f'{source.name}{after}: {source.repository} @ {source.revision[:12] or "?"}'
              f' [{source.database_env} {configured}]')
        for step in source.steps:
            mode = (f'watermark {step.watermark}' if step.watermark
                    else 'whole set, one batch' if step.whole else 'full re-scan')
            cutover = ' [cutover only; excluded from run all]' if source.name in EXCLUDED_FROM_ALL else ''
            print(f'  {step.name}: {", ".join(step.targets)} (key {", ".join(step.key)}; {mode}){cutover}')
    return 0


def cmd_check(args, sources):
    selected = ordered(sources, args.sources or ['all'])
    target_url = os.environ.get(args.target_env)
    failures = 0
    for source in selected:
        source_url = os.environ.get(source.database_env)
        for step in source.steps:
            where = f'{source.name}/{step.name}'
            if source_url:
                try:
                    extract = strip_sql_comments(step.extract.read_text()).strip()
                    out = psql(source_url, f'{extract} \\gdesc\n', read_only=True)
                    got = [line.split('\t')[0] for line in out.splitlines() if line]
                    want = [c for c, _ in step.columns]
                    if got != want:
                        raise MigrationError(f'extract returns {got}, manifest declares {want}')
                    print(f'ok   {where}: extract matches columns')
                except MigrationError as e:
                    failures += 1
                    print(f'FAIL {where}: extract: {e}')
            else:
                print(f'skip {where}: extract ({source.database_env} unset)')
            if target_url:
                try:
                    defs = ', '.join(f'{ident(c)} {t}' for c, t in step.columns)
                    psql(target_url, f'BEGIN;\n{CHECKPOINT_DDL}\n' + '\n'.join(step_vars(step, True))
                         + f'\nCREATE TEMP TABLE stage ({defs});\n\\i {step.load}\nROLLBACK;\n')
                    print(f'ok   {where}: load runs on an empty stage')
                except MigrationError as e:
                    failures += 1
                    print(f'FAIL {where}: load: {e}')
            else:
                print(f'skip {where}: load ({args.target_env} unset)')
    print(f'{len(selected)} source(s) checked, {failures} failure(s)')
    return 1 if failures else 0


def validate_worlds_blocked_names(path):
    if path is None:
        raise MigrationError('worlds-content-server requires --worlds-blocked-names: export BLOCKED_WORLD_NAMES as a JSON array (including [] when empty)')
    try:
        names = json.loads(Path(path).read_text())
    except (OSError, ValueError) as error:
        raise MigrationError(f'cannot read world blocked-name export: {error}') from error
    if not isinstance(names, list) or any(not isinstance(name, str) for name in names):
        raise MigrationError('world blocked-name export must be a JSON array of strings')
    if names:
        raise MigrationError('nonempty BLOCKED_WORLD_NAMES cannot yet be migrated with exact-name semantics; resolve the target blocking policy before cutover')


def cmd_run(args, sources):
    selected = run_sources(sources, args.sources, args.step)
    if args.sources == ['all']:
        print('run all: excluding credits-server, shop-server and both marketplace sources; explicitly name the source at cutover',
              flush=True)
    if any(source.name == 'worlds-content-server' and os.environ.get(source.database_env)
           for source in selected):
        validate_worlds_blocked_names(args.worlds_blocked_names)
    target_url = database_url(args.target_env)
    missing = [s.database_env for s in selected if not os.environ.get(s.database_env)]
    if missing and not args.skip_unconfigured:
        raise MigrationError(f'unset: {", ".join(missing)} (or pass --skip-unconfigured)')
    since = None
    if args.since and args.since != 'last':
        since = iso(parse_timestamp(args.since))
    if not args.dry_run:
        ensure_checkpoint_table(target_url)
    checkpoints = read_checkpoints(target_url)
    log = lambda line: print(line, flush=True)  # noqa: E731
    plans = []
    for source in selected:
        if not os.environ.get(source.database_env):
            log(f'{source.name}: skipped ({source.database_env} unset)')
            continue
        source_url = database_url(source.database_env)
        for step in source.steps:
            walk = plan_walk(step, checkpoints.get((source.name, step.name)), since=since,
                             since_last=args.since == 'last', overlap=args.overlap, restart=args.restart)
            plans.append((step, source_url, walk))
    # Sources whose loaded rows name blobs end with the --verify check: a store GC'd or copied
    # into the wrong root since `blobs` fails the run instead of serving missing files.
    verify = sorted({step.source for step, _, _ in plans
                     if step.name in BLOB_SETS.get(step.source, {}).get('steps', ())})
    store = None
    if verify and not args.dry_run:
        store = load_blobs_module().Store(storage_root(args))
    if args.dry_run:
        log('dry run (one transaction, rolled back):')
        dry_run(plans, target_url, log)
    else:
        current = None
        for step, source_url, walk in plans:
            if step.source != current:
                current = step.source
                log(f'{current}:')
            log(f'  {step.name}: {walk.reason}')
            rows, writes = run_step(step, source_url, target_url, walk,
                                    checkpoints.get((step.source, step.name)), batch_size=args.batch_size, log=log)
            log(f'  {step.name}: done, {rows} rows in this walk; {writes}')
        for name in verify:
            log(f'{name}: checking that the store holds every imported blob')
            # Objects upstream itself lost are served as missing, as upstream did.
            if verify_blobs(name, store, target_url, log, recorded_missing(name, target_url)):
                raise MigrationError(f'{name}: imported rows name blobs missing from {store.contents};'
                                     ' re-run `blobs` (and never run the storage GC between `blobs` and `run`)')
    return 0


def cmd_status(args, sources):
    target_url = database_url(args.target_env)
    checkpoints = read_checkpoints(target_url)
    for source in ordered(sources, args.sources or ['all']):
        for step in source.steps:
            cp = checkpoints.get((source.name, step.name))
            if cp is None:
                print(f'{source.name}/{step.name}: never run')
                continue
            state = f'complete at {cp.completed_at}' if cp.completed_at else 'interrupted'
            since = f', since {cp.since}' if cp.since else ''
            print(f'{source.name}/{step.name}: {state}; walk started {cp.walk_started_at}{since},'
                  f' {cp.walk_rows} rows this walk, {cp.total_rows} rows loaded in all,'
                  f' cursor {json.dumps(cp.cursor)}')
    return 0


# ── blobs ───────────────────────────────────────────────────────────────────

def load_blobs_module():
    # Cached, so every caller sees one module and one set of its classes.
    module = sys.modules.get('dcl_migration_blobs')
    if module is None:
        spec = importlib.util.spec_from_file_location('dcl_migration_blobs', HERE / 'blobs.py')
        module = importlib.util.module_from_spec(spec)
        sys.modules['dcl_migration_blobs'] = module
        try:
            spec.loader.exec_module(module)
        except BaseException:
            del sys.modules['dcl_migration_blobs']
            raise
    return module


# Per source: the upstream object keys its imported rows name (on the source), the (owner, key)
# pairs its imported rows name now (on the target), for --verify, and the steps whose rows name
# blobs (a `run` of one of them ends with the --verify check). Output is hex-encoded so a key
# holding a line or field separator stays one value.
BLOB_SETS = {
    'worlds-content-server': {
        'source': """
SELECT encode(convert_to(k, 'UTF8'), 'hex') FROM (
    SELECT DISTINCT k FROM (
        SELECT entity_id::text AS k FROM world_scenes WHERE status = 'DEPLOYED'
        UNION ALL
        SELECT c->>'hash' FROM world_scenes s
        CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(s.entity::jsonb->'content') = 'array'
                                                     THEN s.entity::jsonb->'content' ELSE '[]'::jsonb END) c
        WHERE s.status = 'DEPLOYED'
        UNION ALL
        SELECT thumbnail_hash::text FROM worlds WHERE coalesce(thumbnail_hash::text, '') <> ''
    ) x WHERE coalesce(k, '') <> ''
) y ORDER BY k""",
        'target': """
SELECT encode(convert_to(o, 'UTF8'), 'hex'), encode(convert_to(k, 'UTF8'), 'hex') FROM (
    SELECT ws.world_name || '/' || ws.entity_id AS o, h.k
    FROM worlds.world_scenes ws
    CROSS JOIN LATERAL (
        SELECT ws.entity_id
        UNION
        SELECT c->>'hash' FROM jsonb_array_elements(CASE WHEN jsonb_typeof(ws.entity->'content') = 'array'
                                                         THEN ws.entity->'content' ELSE '[]'::jsonb END) c
    ) h(k)
    WHERE ws.mirror_source IS NULL AND coalesce(h.k, '') <> ''
    UNION ALL
    SELECT name || ' (thumbnail)', thumbnail_hash FROM worlds.worlds
    WHERE mirror_source IS NULL AND coalesce(thumbnail_hash, '') <> ''
) x ORDER BY o, k""",
        'steps': {'world', 'world_scene'},
    },
}
RECORD_BATCH = 1000


def hex_rows(out, width):
    """psql rows of hex-encoded columns, decoded."""
    rows = []
    for line in out.splitlines():
        if line:
            fields = line.split('\t')
            if len(fields) != width:
                raise MigrationError(f'unexpected psql row with {len(fields)} fields')
            rows.append([bytes.fromhex(f).decode() for f in fields])
    return rows


def storage_root(args):
    root = getattr(args, 'to', None) or getattr(args, 'storage_root', None) or os.environ.get('STORAGE_ROOT_FOLDER')
    if not root:
        raise MigrationError('pass the node\'s storage root (--to for blobs, --storage-root for run)'
                             ' or set STORAGE_ROOT_FOLDER')
    return Path(root)


def record_blobs(target_url, source, stored, missing):
    if not stored and not missing:
        return
    lines = ['BEGIN;']
    if stored:
        values = ',\n'.join(f'({literal(source)}, {literal(k)}, {literal(h)}, {int(n)})' for k, h, n in stored)
        lines += [f'INSERT INTO dcl_migration.blob_inventory (source, object_key, hash, size) VALUES\n{values}\n'
                  'ON CONFLICT (source, object_key) DO UPDATE SET hash = EXCLUDED.hash, size = EXCLUDED.size;',
                  f'DELETE FROM dcl_migration.blob_missing WHERE source = {literal(source)} AND object_key IN '
                  f'({", ".join(literal(k) for k, _, _ in stored)});']
    if missing:
        values = ',\n'.join(f'({literal(source)}, {literal(k)}, {literal(r)})' for k, r in missing)
        lines.append(f'INSERT INTO dcl_migration.blob_missing (source, object_key, reason) VALUES\n{values}\n'
                     'ON CONFLICT (source, object_key) DO UPDATE SET reason = EXCLUDED.reason;')
    lines.append('COMMIT;')
    psql(target_url, '\n'.join(lines) + '\n')


def record_outcomes(outcomes, record, total, log, batch=RECORD_BATCH):
    """Records stored objects through `record(stored, [])` every `batch` and at the end, also when
    the copy stops part-way. Objects not stored are returned, for the caller to record once the
    run is known sound; a `failed` read is reported, never returned, so the next run retries it.
    Returns (counts, report lines, [(key, reason)] not stored)."""
    counts = dict.fromkeys(['copied', 'present', 'missing', 'corrupt', 'invalid', 'failed'], 0)
    report, stored, missing = [], [], []
    try:
        for n, outcome in enumerate(outcomes, 1):
            counts[outcome.status] += 1
            if outcome.cid:
                stored.append((outcome.key, outcome.cid, outcome.size))
            else:
                if outcome.status != 'failed':
                    missing.append((outcome.key, outcome.reason))
                report.append(f'  {outcome.key!r}: {outcome.reason}')
            if len(stored) >= batch:
                record(stored, [])
                stored = []
            if n % 10000 == 0:
                log(f'  {n}/{total}')
    finally:
        record(stored, [])
    return counts, report, missing


def record_missing(record, missing, batch=RECORD_BATCH):
    for i in range(0, len(missing), batch):
        record([], missing[i:i + batch])


def cmd_blobs(args, sources):
    if args.source == 'badges':
        return cmd_badge_art(args)
    if args.source not in BLOB_SETS:
        raise MigrationError(f'no blob set for {args.source!r}; known: {", ".join(sorted(BLOB_SETS))}')
    if args.from_url or args.refresh:
        raise MigrationError('--from-url and --refresh apply to `blobs badges` only')
    blobs = load_blobs_module()
    store = blobs.Store(storage_root(args))
    target_url = database_url(args.target_env)
    log = lambda line: print(line, flush=True)  # noqa: E731
    if args.verify:
        return verify_blobs(args.source, store, target_url, log)
    if bool(args.from_dir) == bool(args.from_s3):
        raise MigrationError('pass exactly one of --from-dir DIR or --from-s3 BUCKET')
    try:
        upstream = blobs.FolderSource(args.from_dir) if args.from_dir else blobs.S3Source(args.from_s3,
                                                                                         args.s3_prefix)
    except (OSError, RuntimeError) as e:
        raise MigrationError(str(e)) from e
    source_url = database_url(sources[args.source].database_env)
    keys = [k for (k,) in hex_rows(psql(source_url, BLOB_SETS[args.source]['source'] + ';\n', read_only=True), 1)]
    ensure_checkpoint_table(target_url)
    inventory = {k: (h, int(n)) for k, h, n in hex_rows(psql(
        target_url, "SELECT encode(convert_to(object_key, 'UTF8'), 'hex'), encode(convert_to(hash, 'UTF8'), 'hex'),"
                    " encode(convert_to(size::text, 'UTF8'), 'hex')"
                    f' FROM dcl_migration.blob_inventory WHERE source = {literal(args.source)};\n'), 3)}
    # A recorded object counts only while the store holds a file of its recorded size.
    held = [k for k in keys if k in inventory and store.has(*inventory[k])]
    known_missing = recorded_missing(args.source, target_url)
    fresh = [k for k in keys if k not in inventory and k not in known_missing
             or k in inventory and not store.has(*inventory[k])]
    retried = [k for k in keys if k in known_missing and k not in inventory] if args.retry_missing else []
    todo = fresh + retried
    log(f'{args.source}: {len(keys)} objects named, {len(held)} already copied,'
        f' {len(known_missing.intersection(keys))} recorded missing{" (retrying)" if retried else ""};'
        f' copying {len(todo)} from {upstream.describe()} into {store.contents}')
    record = lambda stored, missing: record_blobs(target_url, args.source, stored, missing)  # noqa: E731
    newly = {'missing': 0}

    def tally(outcomes):
        for outcome in outcomes:
            if outcome.status == 'missing' and outcome.key not in known_missing:
                newly['missing'] += 1
            yield outcome

    try:
        if todo:
            # Keys already recorded missing say nothing about the source; keys copied before do.
            probe_source(blobs, upstream, fresh, held, args.skip_probe, log)
        counts, report, missing = record_outcomes(tally(blobs.copy_all(upstream, store, todo, args.workers)),
                                                  record, len(todo), log)
    except blobs.SourceError as e:
        raise MigrationError(f'upstream storage: {e}') from e
    log(f'{args.source}: copied {counts["copied"]}, already in the store {counts["present"]},'
        f' absent or unreadable upstream {counts["missing"]}, hash mismatch {counts["corrupt"]},'
        f' not a content id {counts["invalid"]}, read failed {counts["failed"]}')
    if too_many_missing(newly['missing'], len(fresh), args) and not args.allow_missing:
        for line in report[:20]:
            log(line)
        log(f'{args.source}: {newly["missing"]} of {len(fresh)} new objects absent or unreadable upstream, over'
            f' --max-missing {args.max_missing:g}% and --max-missing-count {args.max_missing_count};'
            ' none recorded missing. Check the source and credentials, or pass --allow-missing to record them')
        return 1
    record_missing(record, missing)
    if report:
        log('not stored (recorded in dcl_migration.blob_missing, which the node serves as missing,'
            ' except read failures, which the next run retries):')
        for line in report:
            log(line)
    return 1 if counts['failed'] else 0


# Every string in the badges' and tiers' `assets` documents (on the target).
BADGE_ART_URLS = """
SELECT encode(convert_to(o, 'UTF8'), 'hex'), encode(convert_to(u, 'UTF8'), 'hex') FROM (
    SELECT DISTINCT o, v #>> '{}' AS u FROM (
        SELECT id AS o, assets FROM social.badge_definitions
        UNION ALL
        SELECT badge_id || '/' || tier_id, assets FROM social.badge_tiers
    ) a CROSS JOIN LATERAL jsonb_path_query(a.assets, 'strict $.**') v
    WHERE jsonb_typeof(v) = 'string'
) x ORDER BY u, o"""


def badge_art_paths(target_url, base):
    """{path: [owner, ...]} for every texture path the catalog names (relative, or under `base`)."""
    paths = {}
    blobs = load_blobs_module()
    for owner, url in hex_rows(psql(target_url, BADGE_ART_URLS + ';\n'), 2):
        path = blobs.art_path(url, base)
        if path:
            paths.setdefault(path, []).append(owner)
    return paths


def cmd_badge_art(args):
    """Mirrors the art the imported badge catalog names from upstream's CDN into BADGES_ASSETS_DIR."""
    blobs = load_blobs_module()
    root = args.to or os.environ.get('BADGES_ASSETS_DIR')
    if not root:
        raise MigrationError("pass the node's badge art directory (--to) or set BADGES_ASSETS_DIR")
    art_dir = blobs.ArtDir(root, refresh=args.refresh)
    log = lambda line: print(line, flush=True)  # noqa: E731
    paths = badge_art_paths(database_url(args.target_env), blobs.BADGE_ART_BASE)
    if not paths:
        raise MigrationError('no badge names art to mirror; run `run badges` first')
    if args.verify:
        lacking = [p for p in paths if not art_dir.has(p)]
        for path in lacking:
            log(f'{", ".join(paths[path][:3])}: missing {path}')
        log(f'badges: {len(paths)} textures checked in {art_dir.root}, {len(lacking)} missing')
        return 1 if lacking else 0
    if args.from_s3:
        raise MigrationError('badge art reads from the CDN (--from-url) or a local copy (--from-dir)')
    try:
        upstream = blobs.FolderArtSource(args.from_dir) if args.from_dir else blobs.HttpArtSource(
            args.from_url or blobs.BADGE_ART_BASE)
    except OSError as e:
        raise MigrationError(str(e)) from e
    log(f'badges: {len(paths)} textures named; copying from {upstream.describe()} into {art_dir.root}')
    counts = dict.fromkeys(['copied', 'present', 'missing', 'corrupt', 'invalid', 'failed'], 0)
    report = []
    try:
        for outcome in blobs.copy_all(upstream, art_dir, sorted(paths), args.workers, copy=blobs.copy_art):
            counts[outcome.status] += 1
            if outcome.status not in ('copied', 'present'):
                report.append(f'  {outcome.key} ({", ".join(paths[outcome.key][:3])}): {outcome.reason}')
    except blobs.SourceError as e:
        raise MigrationError(f'upstream art: {e}') from e
    log(f'badges: copied {counts["copied"]}, already there {counts["present"]},'
        f' absent upstream {counts["missing"]}, not a PNG {counts["corrupt"]}, read failed {counts["failed"]}')
    for line in report:
        log(line)
    if counts['failed'] or (report and not args.allow_missing):
        return 1
    return 0


def too_many_missing(newly, fresh, args):
    """Newly missing keys over both the --max-missing share of new keys and --max-missing-count."""
    return newly > args.max_missing_count and newly > args.max_missing / 100 * fresh


def probe_source(blobs, upstream, keys, reference, skip_probe, log):
    """Fails before copying when no sampled key reads: a wrong folder or bucket, or credentials
    that cannot read it, would otherwise record every object missing. `reference` keys were read
    by an earlier run, so they read from the right source even when every new key is lost."""
    result = blobs.probe(upstream, keys, reference=reference)
    if result.readable or not result.unreadable:
        return
    reasons = sorted(set(result.unreadable.values()))
    if not skip_probe:
        raise MigrationError(f'none of {len(result.unreadable)} sampled objects reads from {upstream.describe()}'
                             f' ({"; ".join(reasons[:3])}); check the source and credentials, or pass'
                             ' --skip-probe when these objects are lost upstream')
    log(f'warning: none of {len(result.unreadable)} sampled objects reads from {upstream.describe()};'
        ' copying anyway (--skip-probe)')
    upstream.readable = True


def recorded_missing(source, target_url):
    return {k for (k,) in hex_rows(psql(
        target_url, "SELECT encode(convert_to(object_key, 'UTF8'), 'hex') FROM dcl_migration.blob_missing"
                    f' WHERE source = {literal(source)};\n'), 1)}


def verify_blobs(source, store, target_url, log, skip=frozenset()):
    """Lists every imported row whose blobs the store lacks, but for the keys in `skip`; existence
    only, the copy hashed them."""
    lacking, checked = {}, {}
    for owner, key in hex_rows(psql(target_url, BLOB_SETS[source]['target'] + ';\n'), 2):
        if key in skip:
            continue
        if key not in checked:
            checked[key] = store.has(key)
        if not checked[key]:
            lacking.setdefault(owner, []).append(key)
    for owner, keys in lacking.items():
        log(f'{owner}: {len(keys)} missing: {", ".join(keys[:5])}{" ..." if len(keys) > 5 else ""}')
    log(f'{source}: {len(checked)} blobs checked in {store.contents}, {len(lacking)} rows with missing blobs')
    return 1 if lacking else 0


# ── worlds limits ───────────────────────────────────────────────────────────

MIB = 1024 * 1024
DIGITS = re.compile(r'\s*[0-9]+\s*')
INT_MAX, BIGINT_MAX = 2 ** 31 - 1, 2 ** 63 - 1


def positive_int(value, ceiling=None):
    """A limit as upstream's JS reads it (`x || default`, then BigInt(x)): a positive integer number
    or integer string; 0, null and anything else mean "the default". Capped at `ceiling`."""
    if isinstance(value, str) and DIGITS.fullmatch(value):
        value = int(value)
    if isinstance(value, float) and value.is_integer():
        value = int(value)
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        return None
    return min(int(value), ceiling) if ceiling else int(value)


def whitelist_rows(whitelist, max_size_mb, warn=lambda line: print(line, file=sys.stderr)):
    """WCS `WHITELIST_URL` entries as worlds.world_limit_overrides rows (limits-manager semantics:
    a listed world's size cap is `max_size_in_mb || MAX_SIZE`, fixed, and ENS worlds keep
    ENS_MAX_SIZE; `max_parcels || MAX_PARCELS`). Upstream looks entries up by the lowercased world
    name, so a key that is not already lowercase and trimmed never applied and is skipped."""
    if not isinstance(whitelist, dict):
        raise MigrationError('the whitelist must be a JSON object keyed by world name')
    rows, unsized = {}, []
    for name, entry in whitelist.items():
        if not name or name != name.strip().lower():
            warn(f'skipped whitelist key {name!r}: upstream only matched lowercase, trimmed names')
            continue
        if not isinstance(entry, dict):
            continue
        # Capped to the columns' range; a larger cap was unlimited in practice.
        parcels = positive_int(entry.get('max_parcels'), INT_MAX)
        size_mb = positive_int(entry.get('max_size_in_mb'), BIGINT_MAX // MIB)
        if name.endswith('.eth') and not name.endswith('.dcl.eth'):
            size = None
        elif size_mb:
            size = size_mb * MIB
        elif max_size_mb:
            size = max_size_mb * MIB
        else:
            size = None
            unsized.append(name)
        if parcels or size:
            rows[name] = (parcels, size)
    if unsized:
        raise MigrationError(f'{len(unsized)} entries have no max_size_in_mb ({", ".join(sorted(unsized)[:5])}...);'
                             ' pass --max-size-mb with the old service\'s MAX_SIZE')
    return rows


def cmd_worlds_limits(args, sources):
    try:
        whitelist = json.loads(Path(args.whitelist).read_text())
    except (OSError, ValueError) as e:
        raise MigrationError(f'{args.whitelist}: {e}') from e
    rows = whitelist_rows(whitelist, args.max_size_mb)
    target_url = database_url(args.target_env)
    if not rows:
        print('no limit overrides in the whitelist')
        return 0
    values = ',\n'.join(f'({literal(n)}, {literal(p)}, {literal(s)})' for n, (p, s) in sorted(rows.items()))
    ensure_checkpoint_table(target_url)
    out = psql(target_url, f"""BEGIN;
DO $t$ BEGIN
  IF to_regclass('worlds.world_limit_overrides') IS NULL THEN
    RAISE EXCEPTION 'worlds.world_limit_overrides is not in this schema';
  END IF;
END $t$;
CREATE TEMP TABLE wl ON COMMIT DROP AS
SELECT n, p::integer AS p, s::bigint AS s FROM (VALUES
{values}) v(n, p, s);
CREATE TEMP TABLE written (n text PRIMARY KEY) ON COMMIT DROP;
-- Only rows this command wrote and nobody changed since; any other existing row is the node's.
WITH u AS (
    UPDATE worlds.world_limit_overrides o SET max_parcels = wl.p, max_size_bytes = wl.s,
        updated_at = timezone('UTC', now())
    FROM wl JOIN dcl_migration.world_limit_seeded m ON m.world_name = wl.n
    WHERE o.world_name = wl.n
      AND (o.max_parcels, o.max_size_bytes) IS NOT DISTINCT FROM (m.max_parcels, m.max_size_bytes)
    RETURNING o.world_name)
INSERT INTO written SELECT world_name FROM u;
WITH i AS (
    INSERT INTO worlds.world_limit_overrides (world_name, max_parcels, max_size_bytes)
    SELECT n, p, s FROM wl ON CONFLICT (world_name) DO NOTHING RETURNING world_name)
INSERT INTO written SELECT world_name FROM i;
INSERT INTO dcl_migration.world_limit_seeded (world_name, max_parcels, max_size_bytes)
SELECT n, p, s FROM wl JOIN written USING (n)
ON CONFLICT (world_name) DO UPDATE SET max_parcels = EXCLUDED.max_parcels, max_size_bytes = EXCLUDED.max_size_bytes;
SELECT '@@written', count(*) FROM written;
SELECT '@@kept', wl.n FROM wl JOIN worlds.world_limit_overrides o ON o.world_name = wl.n
WHERE wl.n NOT IN (SELECT n FROM written)
  AND (o.max_parcels, o.max_size_bytes) IS DISTINCT FROM (wl.p, wl.s) ORDER BY 2;
{'ROLLBACK' if args.dry_run else 'COMMIT'};
""")
    marks = [line.split('\t') for line in out.splitlines() if line.startswith('@@')]
    written = next(v for k, v in marks if k == '@@written')
    kept = [v for k, v in marks if k == '@@kept']
    print(f'worlds.world_limit_overrides: {written} rows written' + (' (rolled back)' if args.dry_run else ''))
    if kept:
        print(f'kept {len(kept)} rows set on the node that differ from the whitelist: {", ".join(kept)}')
    return 0


# ── one-off: quota blocks an older mapping loaded as bans ───────────────────

def cmd_worlds_unban_quota_blocks(args, sources):
    """Deletes from worlds.blocked_wallets the rows the former `blocked_wallet` step loaded there.

    The tool records no per-wallet provenance, so a row counts as that step's when upstream
    `blocked` still holds its wallet with the same created_at (the step copied it verbatim; a ban
    made on the node carries its own) and the row was last written before that step's last walk
    started (a later ban on the node bumps updated_at). Runs only where that step's checkpoint
    exists, and drops it."""
    source = sources['worlds-content-server']
    target_url = database_url(args.target_env)
    old = psql(target_url, "SELECT to_regclass('dcl_migration.checkpoint') IS NOT NULL AND EXISTS ("
                           "SELECT 1 FROM dcl_migration.checkpoint WHERE source = 'worlds-content-server'"
                           " AND step = 'blocked_wallet');\n").strip()
    if old != 't':
        print('no worlds-content-server/blocked_wallet checkpoint: the former step never ran here; nothing to do')
        return 0
    step = next(s for s in source.steps if s.name == 'quota_block')
    source_url = database_url(source.database_env)
    with tempfile.TemporaryDirectory(prefix='dcl-migration-') as tmp:
        data = Path(tmp) / 'blocked.csv'
        with data.open('wb') as fh:
            psql(source_url, extract_sql(step, None, None, None), read_only=True, stdout=fh)
        script = Path(tmp) / 'unban.sql'
        script.write_text(f"""\\set ON_ERROR_STOP 1
BEGIN;
CREATE TEMP TABLE upstream_blocked (wallet text, created_at timestamptz, updated_at timestamptz) ON COMMIT DROP;
\\copy upstream_blocked FROM pstdin WITH (FORMAT csv)
CREATE TEMP TABLE walk ON COMMIT DROP AS
SELECT walk_started_at AT TIME ZONE 'UTC' AS started FROM dcl_migration.checkpoint
WHERE source = 'worlds-content-server' AND step = 'blocked_wallet';
WITH d AS (
    DELETE FROM worlds.blocked_wallets b USING upstream_blocked u, walk w
    WHERE b.wallet = u.wallet AND b.created_at = u.created_at AT TIME ZONE 'UTC' AND b.updated_at <= w.started
    RETURNING 1)
SELECT '@@deleted', count(*) FROM d;
SELECT '@@rebanned', b.wallet FROM worlds.blocked_wallets b JOIN upstream_blocked u ON u.wallet = b.wallet
WHERE b.created_at = u.created_at AT TIME ZONE 'UTC' ORDER BY 2;
SELECT '@@kept', count(*) FROM worlds.blocked_wallets b JOIN upstream_blocked u ON u.wallet = b.wallet
WHERE b.created_at <> u.created_at AT TIME ZONE 'UTC';
DELETE FROM dcl_migration.checkpoint WHERE source = 'worlds-content-server' AND step = 'blocked_wallet';
{'ROLLBACK' if args.dry_run else 'COMMIT'};
""")
        with data.open('rb') as fh:
            out = psql(target_url, None, stdin=fh, extra=['-f', str(script)])
    marks = [line.split('\t') for line in out.splitlines() if line.startswith('@@')]
    counts = {k: v for k, v in marks if k != '@@rebanned'}
    rebanned = [v for k, v in marks if k == '@@rebanned']
    print(f'worlds.blocked_wallets: deleted {counts["@@deleted"]} quota blocks loaded as bans'
          + (' (rolled back)' if args.dry_run else ''))
    if rebanned:
        print(f'kept {len(rebanned)} rows written on the node after the former step ran (a ban made there):'
              f' {", ".join(rebanned)}')
    if counts['@@kept'] != '0':
        print(f'kept {counts["@@kept"]} rows whose wallet is blocked upstream but whose created_at differs'
              ' (a ban made on the node); review them by hand')
    print('a wallet whose upstream block was lifted after the former step ran is no longer in `blocked`'
          ' and cannot be told apart from a node ban; review worlds.blocked_wallets by hand')
    return 0


def parser():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument('--sources-dir', type=Path, default=SOURCES, help=argparse.SUPPRESS)
    p.add_argument('--verbose-errors', action='store_true',
                   help='include DETAIL in database errors (may print row values)')
    p.add_argument('--target-env', default=TARGET_ENV,
                   help=f'environment variable holding the target URL (default {TARGET_ENV})')
    sub = p.add_subparsers(dest='command', required=True)
    sub.add_parser('list', help='sources, steps and their environment variables')
    check = sub.add_parser('check', help='validate manifests; with URLs set, compile extracts and loads')
    check.add_argument('sources', nargs='*')
    status = sub.add_parser('status', help='checkpoints in the target')
    status.add_argument('sources', nargs='*')
    run = sub.add_parser('run', help='migrate sources (`all` excludes credits, shop and marketplace)')
    run.add_argument('sources', nargs='+')
    run.add_argument('--worlds-blocked-names', type=Path, help='JSON export of source BLOCKED_WORLD_NAMES; currently only an empty list is supported')
    run.add_argument('--dry-run', action='store_true', help='load every batch, then roll it back')
    run.add_argument('--step', action='append', help='only this step (repeatable)')
    run.add_argument('--since', help="ISO 8601 timestamp, or 'last'")
    run.add_argument('--overlap', type=parse_duration, default=datetime.timedelta(minutes=10),
                     help="margin subtracted for --since last (default 10m)")
    run.add_argument('--restart', action='store_true', help='ignore stored checkpoints')
    run.add_argument('--batch-size', type=int, default=5000)
    run.add_argument('--skip-unconfigured', action='store_true',
                     help='skip sources whose database variable is unset')
    run.add_argument('--storage-root',
                     help="the node's STORAGE_ROOT_FOLDER (default: that variable); runs of steps whose rows"
                          ' name blobs end by checking the store holds them')
    blobs = sub.add_parser('blobs', help="copy a source's blobs into the node's store, or --verify them")
    blobs.add_argument('source', help=f'one of: {", ".join(sorted([*BLOB_SETS, "badges"]))}')
    blobs.add_argument('--to', help='the node\'s STORAGE_ROOT_FOLDER (default: that variable); for badges,'
                                    ' its BADGES_ASSETS_DIR (default: that variable)')
    blobs.add_argument('--from-dir', help='upstream storage folder (or a local sync of its bucket)')
    blobs.add_argument('--from-url', help='badges: upstream art base URL (default: its production CDN_HOST)')
    blobs.add_argument('--refresh', action='store_true',
                       help='badges: copy every texture again, also those already in the directory')
    blobs.add_argument('--from-s3', metavar='BUCKET', help='upstream S3 bucket (needs boto3)')
    blobs.add_argument('--s3-prefix', default='', help=argparse.SUPPRESS)
    blobs.add_argument('--workers', type=int, default=8)
    blobs.add_argument('--allow-missing', action='store_true',
                       help='record objects absent or unreadable upstream as missing however many there are'
                            ' (the probe still runs)')
    blobs.add_argument('--max-missing', type=float, default=1.0, metavar='PERCENT',
                       help='fail, recording none missing, when more of the new keys are absent or'
                            ' unreadable upstream (default 1) ...')
    blobs.add_argument('--max-missing-count', type=int, default=10, metavar='N',
                       help='... and more than N of them (default 10)')
    blobs.add_argument('--retry-missing', action='store_true',
                       help='also retry keys recorded missing by an earlier run (never counted against'
                            ' --max-missing)')
    blobs.add_argument('--skip-probe', action='store_true',
                       help='copy even when none of the sampled objects reads upstream')
    blobs.add_argument('--verify', action='store_true',
                       help='list imported rows whose blobs are missing from the store; copies nothing')
    limits = sub.add_parser('worlds-limits', help='seed worlds.world_limit_overrides from a WCS whitelist JSON')
    limits.add_argument('whitelist', help='a copy of the document at the old service\'s WHITELIST_URL')
    limits.add_argument('--max-size-mb', type=int,
                        help="the old service's MAX_SIZE: the cap of a listed world without max_size_in_mb")
    limits.add_argument('--dry-run', action='store_true', help='write, then roll back')
    unban = sub.add_parser('worlds-unban-quota-blocks',
                           help='one-off: delete the quota blocks the former blocked_wallet step loaded as bans')
    unban.add_argument('--dry-run', action='store_true', help='delete, report, then roll back')
    return p


def main(argv=None):
    global VERBOSITY
    args = parser().parse_args(argv)
    if args.verbose_errors:
        VERBOSITY = 'default'
    if getattr(args, 'batch_size', 1) < 1 or getattr(args, 'workers', 1) < 1:
        print('--batch-size and --workers must be positive', file=sys.stderr)
        return 2
    try:
        selected = [args.source] if getattr(args, 'source', None) else getattr(args, 'sources', None)
        if args.command == 'worlds-unban-quota-blocks':
            selected = ['worlds-content-server']
        sources = discover(args.sources_dir, selected or ['all'])
        return {'list': cmd_list, 'check': cmd_check, 'run': cmd_run, 'status': cmd_status, 'blobs': cmd_blobs,
                'worlds-limits': cmd_worlds_limits,
                'worlds-unban-quota-blocks': cmd_worlds_unban_quota_blocks}[args.command](args, sources)
    except MigrationError as e:
        print(f'error: {e}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    with contextlib.suppress(KeyboardInterrupt):
        sys.exit(main())
    sys.exit(130)
