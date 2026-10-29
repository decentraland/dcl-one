#!/usr/bin/env python3
"""Fail on a decentraland .proto outside library/protocol/proto, the one proto source."""
import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'library/protocol'  # proto/ is the contract; public/ re-exports it for the npm package
# Copies that predate the one source; each is removed by the change that points its
# consumer at library/protocol. Never add to this list.
ALLOWED = (
    'library/dcl-scene-authoring/proto/',
    'apps/dcl-scene-cli/proto/',
    'services/catalyst/social/src/rpc/proto/',
)
SKIP_DIRS = {'.git', 'node_modules', 'target', 'dist', '.cache', 'out-ts', 'out-js', 'state'}
PACKAGE = re.compile(r'^\s*package\s+(decentraland(?:\.[\w]+)*)\s*;', re.M)


def offenders(root):
    found = []
    for path in sorted(root.rglob('*.proto')):
        rel = path.relative_to(root).as_posix()
        if SKIP_DIRS & set(path.relative_to(root).parts[:-1]):
            continue
        if rel.startswith(SOURCE + '/') or rel.startswith(ALLOWED):
            continue
        if PACKAGE.search(path.read_text(encoding='utf-8-sig', errors='replace')):
            found.append(rel)
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    found = offenders(args.root)
    for rel in found:
        print(f'{rel}: decentraland proto outside {SOURCE}/proto; consume library/protocol instead', file=sys.stderr)
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
