"""Loads the migration engine once, so every test module raises and catches the same classes.

`unittest discover -s tooling/tests` imports test files as top-level modules while the case
mixins import `tooling.tests...`; the engine is registered in `sys.modules` under one name so
either import path reuses it.
"""
import importlib.util
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
