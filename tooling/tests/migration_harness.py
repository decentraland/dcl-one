"""Loads the migration engine once, so every test module raises and catches the same classes.

`unittest discover -s tooling/tests` imports test files as top-level modules while the case
mixins import `tooling.tests...`; the engine is registered in `sys.modules` under one name so
either import path reuses it.
"""
import contextlib
import importlib.util
import os
import shutil
import socket
import subprocess
import tempfile
import unittest
import urllib.parse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = 'dcl_migration_engine'


def _load():
    module = sys.modules.get(MODULE)
    if module is None:
        spec = importlib.util.spec_from_file_location(MODULE, ROOT / 'tooling/migration/migrate.py')
        module = importlib.util.module_from_spec(spec)
        sys.modules[MODULE] = module
        try:
            spec.loader.exec_module(module)
        except BaseException:
            del sys.modules[MODULE]
            raise
    return module


migrate = _load()
blobs = migrate.load_blobs_module()


SERVER_ENV = 'MIGRATION_TEST_SERVER_URL'
DISPOSABLE_ENV = 'MIGRATION_TEST_SERVER_DISPOSABLE'


def pattern(n):
    return (bytes(range(251)) * (n // 251 + 1))[:n]


def validate_test_server(server):
    parsed = urllib.parse.urlsplit(server)
    host = dict(urllib.parse.parse_qsl(parsed.query)).get('host', parsed.hostname or '')
    local = host in ('', 'localhost', '127.0.0.1', '::1') or host.startswith('/')
    if not local and os.environ.get(DISPOSABLE_ENV) != '1':
        raise RuntimeError('test server is non-local; set MIGRATION_TEST_SERVER_DISPOSABLE=1 only for a disposable server')


@contextlib.contextmanager
def private_postgres_cluster():
    """A caller-owned cluster; fixed fixture database names never reach another suite's server."""
    if not all(shutil.which(binary) for binary in ('psql', 'initdb', 'pg_ctl')):
        if os.environ.get('CI'):
            raise RuntimeError('PostgreSQL binaries are not on PATH')
        raise unittest.SkipTest('PostgreSQL binaries are not on PATH')
    with tempfile.TemporaryDirectory(prefix='migration-pg-') as tmp:
        root = Path(tmp)
        data = root / 'pg'
        initdb = subprocess.run(['initdb', '-D', str(data), '-U', 'postgres', '--auth=trust', '-E', 'UTF8',
                                 '--locale=C'], capture_output=True, text=True)
        if initdb.returncode != 0:
            if os.environ.get('CI'):
                raise RuntimeError(f'initdb failed: {initdb.stderr.strip()}')
            raise unittest.SkipTest(f'initdb cannot create a cluster here: {initdb.stderr.strip()[:200]}')
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        with (data / 'postgresql.conf').open('a') as conf:
            conf.write(f"port={port}\nlisten_addresses='127.0.0.1'\nunix_socket_directories=''\nfsync=off\n")
        try:
            subprocess.run(['pg_ctl', '-D', str(data), '-l', str(root / 'log'), '-w', 'start'],
                           check=True, capture_output=True)
            yield f'postgres://postgres@127.0.0.1:{port}'
        finally:
            subprocess.run(['pg_ctl', '-D', str(data), '-m', 'immediate', 'stop'], capture_output=True)
