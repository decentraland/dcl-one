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
                          columns, raw.get('notes', '')))
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
    done = f'({rows} < {int(batch_size)})'
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
        if rows < batch_size:
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
            mode = f'watermark {step.watermark}' if step.watermark else 'full re-scan'
            print(f'  {step.name}: {", ".join(step.targets)} (key {", ".join(step.key)}; {mode})')
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


def cmd_run(args, sources):
    selected = ordered(sources, args.sources)
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
            if args.step and step.name not in args.step:
                continue
            walk = plan_walk(step, checkpoints.get((source.name, step.name)), since=since,
                             since_last=args.since == 'last', overlap=args.overlap, restart=args.restart)
            plans.append((step, source_url, walk))
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
    unknown_steps = set(args.step or []) - {s.name for src in selected for s in src.steps}
    if unknown_steps:
        raise MigrationError(f'unknown step(s): {", ".join(sorted(unknown_steps))}')
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
    spec = importlib.util.spec_from_file_location('dcl_migration_blobs', HERE / 'blobs.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Per source: the upstream object keys its imported rows name (on the source), and the
# (owner, keys) its imported rows name now (on the target), for --verify.
BLOB_SETS = {
    'worlds-content-server': {
        'source': """
SELECT DISTINCT k FROM (
    SELECT entity_id::text AS k FROM world_scenes WHERE status = 'DEPLOYED'
    UNION ALL
    SELECT c->>'hash' FROM world_scenes s
    CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(s.entity::jsonb->'content') = 'array'
                                                 THEN s.entity::jsonb->'content' ELSE '[]'::jsonb END) c
    WHERE s.status = 'DEPLOYED'
    UNION ALL
    SELECT thumbnail_hash::text FROM worlds WHERE coalesce(thumbnail_hash::text, '') <> ''
) x WHERE coalesce(k, '') <> '' ORDER BY k""",
        'target': """
SELECT ws.world_name || '/' || ws.entity_id, h.k
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
ORDER BY 1, 2""",
    },
}


def storage_root(args):
    root = args.to or os.environ.get('STORAGE_ROOT_FOLDER')
    if not root:
        raise MigrationError('pass --to STORAGE_ROOT_FOLDER (the node\'s storage root) or set STORAGE_ROOT_FOLDER')
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


def cmd_blobs(args, sources):
    if args.source not in BLOB_SETS:
        raise MigrationError(f'no blob set for {args.source!r}; known: {", ".join(sorted(BLOB_SETS))}')
    blobs = load_blobs_module()
    store = blobs.Store(storage_root(args))
    target_url = database_url(args.target_env)
    log = lambda line: print(line, flush=True)  # noqa: E731
    if args.verify:
        return verify_blobs(args, blobs, store, target_url, log)
    if bool(args.from_dir) == bool(args.from_s3):
        raise MigrationError('pass exactly one of --from-dir DIR or --from-s3 BUCKET')
    try:
        upstream = blobs.FolderSource(args.from_dir) if args.from_dir else blobs.S3Source(args.from_s3,
                                                                                         args.s3_prefix)
    except (OSError, RuntimeError) as e:
        raise MigrationError(str(e)) from e
    source_url = database_url(sources[args.source].database_env)
    keys = [k for k in psql(source_url, BLOB_SETS[args.source]['source'] + ';\n', read_only=True).splitlines() if k]
    ensure_checkpoint_table(target_url)
    inventory = dict(line.split('\t', 1) for line in psql(
        target_url, f'SELECT object_key, hash FROM dcl_migration.blob_inventory WHERE source = {literal(args.source)};\n'
    ).splitlines() if line)
    todo = [k for k in keys if not (k in inventory and store.has(inventory[k]))]
    log(f'{args.source}: {len(keys)} objects named, {len(keys) - len(todo)} already copied;'
        f' copying {len(todo)} from {upstream.describe()} into {store.contents}')
    reasons = {'missing': 'absent upstream', 'corrupt': 'bytes do not hash to the key', 'invalid': 'not a content id'}
    counts = dict.fromkeys(['copied', 'present', *reasons], 0)
    failures, stored, missing = [], [], []
    for n, outcome in enumerate(blobs.copy_all(upstream, store, todo, args.workers), 1):
        counts[outcome.status] += 1
        if outcome.cid:
            stored.append((outcome.key, outcome.cid, outcome.size))
        else:
            missing.append((outcome.key, reasons[outcome.status]))
            failures.append(f'  {outcome.key}: {reasons[outcome.status]}')
        if len(stored) + len(missing) >= 1000:
            record_blobs(target_url, args.source, stored, missing)
            stored, missing = [], []
        if n % 10000 == 0:
            log(f'  {n}/{len(todo)}')
    record_blobs(target_url, args.source, stored, missing)
    log(f'{args.source}: copied {counts["copied"]}, already in the store {counts["present"]},'
        f' absent upstream {counts["missing"]}, hash mismatch {counts["corrupt"]}, not a content id {counts["invalid"]}')
    if failures:
        log('not stored (recorded in dcl_migration.blob_missing; the node serves them as missing):')
        for line in failures:
            log(line)
    return 0


def verify_blobs(args, blobs, store, target_url, log):
    """Lists every imported row whose blobs the store lacks; existence only, the copy hashed them."""
    out = psql(target_url, BLOB_SETS[args.source]['target'] + ';\n')
    lacking, checked = {}, {}
    for line in out.splitlines():
        if not line:
            continue
        owner, key = line.split('\t', 1)
        if key not in checked:
            checked[key] = store.has(key)
        if not checked[key]:
            lacking.setdefault(owner, []).append(key)
    for owner, keys in lacking.items():
        log(f'{owner}: {len(keys)} missing: {", ".join(keys[:5])}{" ..." if len(keys) > 5 else ""}')
    log(f'{args.source}: {len(checked)} blobs checked in {store.contents}, {len(lacking)} rows with missing blobs')
    return 1 if lacking else 0


# ── worlds limits ───────────────────────────────────────────────────────────

MIB = 1024 * 1024


def positive_int(value):
    """A JSON number upstream would use as a limit; 0, null and non-integers mean "the default"."""
    if isinstance(value, float) and value.is_integer():
        value = int(value)
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        return None
    return int(value)


def whitelist_rows(whitelist, max_size_mb):
    """WCS `WHITELIST_URL` entries as worlds.world_limit_overrides rows (limits-manager semantics:
    a listed world's size cap is `max_size_in_mb || MAX_SIZE`, fixed, and ENS worlds keep
    ENS_MAX_SIZE; `max_parcels || MAX_PARCELS`)."""
    if not isinstance(whitelist, dict):
        raise MigrationError('the whitelist must be a JSON object keyed by world name')
    rows, unsized = {}, []
    for raw_name, entry in whitelist.items():
        name = raw_name.strip().lower()
        if not name or not isinstance(entry, dict):
            continue
        parcels, size_mb = positive_int(entry.get('max_parcels')), positive_int(entry.get('max_size_in_mb'))
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
    out = psql(target_url, f"""BEGIN;
DO $t$ BEGIN
  IF to_regclass('worlds.world_limit_overrides') IS NULL THEN
    RAISE EXCEPTION 'worlds.world_limit_overrides is not in this schema';
  END IF;
END $t$;
INSERT INTO worlds.world_limit_overrides (world_name, max_parcels, max_size_bytes)
SELECT n, p::integer, s::bigint FROM (VALUES
{values}) v(n, p, s)
ON CONFLICT (world_name) DO UPDATE SET
    max_parcels = EXCLUDED.max_parcels,
    max_size_bytes = EXCLUDED.max_size_bytes,
    updated_at = timezone('UTC', now());
{'ROLLBACK' if args.dry_run else 'COMMIT'};
""")
    written = next((line for line in out.splitlines() if line.startswith('INSERT')), 'INSERT 0 0').split()[-1]
    print(f'worlds.world_limit_overrides: {written} rows upserted' + (' (rolled back)' if args.dry_run else ''))
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
    run = sub.add_parser('run', help='migrate sources (`all` for every one)')
    run.add_argument('sources', nargs='+')
    run.add_argument('--dry-run', action='store_true', help='load every batch, then roll it back')
    run.add_argument('--step', action='append', help='only this step (repeatable)')
    run.add_argument('--since', help="ISO 8601 timestamp, or 'last'")
    run.add_argument('--overlap', type=parse_duration, default=datetime.timedelta(minutes=10),
                     help="margin subtracted for --since last (default 10m)")
    run.add_argument('--restart', action='store_true', help='ignore stored checkpoints')
    run.add_argument('--batch-size', type=int, default=5000)
    run.add_argument('--skip-unconfigured', action='store_true',
                     help='skip sources whose database variable is unset')
    blobs = sub.add_parser('blobs', help="copy a source's blobs into the node's store, or --verify them")
    blobs.add_argument('source', help=f'one of: {", ".join(sorted(BLOB_SETS))}')
    blobs.add_argument('--to', help='the node\'s STORAGE_ROOT_FOLDER (default: that variable)')
    blobs.add_argument('--from-dir', help='upstream storage folder (or a local sync of its bucket)')
    blobs.add_argument('--from-s3', metavar='BUCKET', help='upstream S3 bucket (needs boto3)')
    blobs.add_argument('--s3-prefix', default='', help=argparse.SUPPRESS)
    blobs.add_argument('--workers', type=int, default=8)
    blobs.add_argument('--verify', action='store_true',
                       help='list imported rows whose blobs are missing from the store; copies nothing')
    limits = sub.add_parser('worlds-limits', help='seed worlds.world_limit_overrides from a WCS whitelist JSON')
    limits.add_argument('whitelist', help='a copy of the document at the old service\'s WHITELIST_URL')
    limits.add_argument('--max-size-mb', type=int,
                        help="the old service's MAX_SIZE: the cap of a listed world without max_size_in_mb")
    limits.add_argument('--dry-run', action='store_true', help='write, then roll back')
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
        sources = discover(args.sources_dir, selected or ['all'])
        return {'list': cmd_list, 'check': cmd_check, 'run': cmd_run, 'status': cmd_status, 'blobs': cmd_blobs,
                'worlds-limits': cmd_worlds_limits}[args.command](args, sources)
    except MigrationError as e:
        print(f'error: {e}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    with contextlib.suppress(KeyboardInterrupt):
        sys.exit(main())
    sys.exit(130)
