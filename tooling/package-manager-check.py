#!/usr/bin/env python3
"""Fail on an npm or yarn lockfile, pnpm settings in .npmrc, or a workflow that installs or runs with npm."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SKIP_DIRS = {'.git', 'node_modules', 'target', 'dist', '.cache', '.agent-work'}
LOCKFILES = {'package-lock.json', 'npm-shrinkwrap.json', 'yarn.lock'}
# Test scenes that install as a creator's project does, with npm.
CREATOR_SCENES = ('library/js-sdk-toolchain/test/snapshots/package-lock.json',)
# pnpm reads these from pnpm-workspace.yaml, never from .npmrc.
UNREAD = re.compile(r'^\s*(ignore-scripts|save-exact|save-prefix|node-linker|auto-install-peers)\s*=', re.M)
WORKFLOWS = ('.github/workflows/', '.github/actions/')
# Publishing and registry queries stay on npm; fetching pnpm or a newer npm through npx is how a job bootstraps them.
NPM_USE = re.compile(r'\bnpm\s+(ci|install|i|add|run|run-script|test|start|exec|rebuild|link)\b|\bnpx\b(?!\s+--yes\s+(pnpm|npm)@)')


def files(root):
    # In a checkout only tracked and unignored files count: installs and test runs leave ignored lockfiles behind.
    if (root / '.git').exists():
        listed = subprocess.run(('git', '-C', str(root), 'ls-files', '-z', '--cached', '--others', '--exclude-standard'),
                                capture_output=True, check=True).stdout.decode().split('\0')
        yield from sorted(rel for rel in listed if rel and (root / rel).is_file())
        return
    for directory, names, entries in os.walk(root):
        names[:] = sorted(name for name in names if name not in SKIP_DIRS)
        for entry in sorted(entries):
            yield (Path(directory) / entry).relative_to(root).as_posix()


def offenders(root):
    found = []
    for rel in files(root):
        if rel in CREATOR_SCENES:
            continue
        name = rel.rsplit('/', 1)[-1]
        if name in LOCKFILES:
            found.append(f'{rel}: npm or yarn lockfile; convert it with `pnpm import` and delete it')
        elif name == '.npmrc':
            for match in UNREAD.finditer((root / rel).read_text(encoding='utf-8', errors='replace')):
                found.append(f'{rel}: pnpm does not read `{match.group(1)}` here; set it in pnpm-workspace.yaml')
        elif rel.startswith(WORKFLOWS) and name.endswith(('.yml', '.yaml', '.sh')):
            for number, line in enumerate((root / rel).read_text(encoding='utf-8', errors='replace').splitlines(), 1):
                if not line.lstrip().startswith('#') and NPM_USE.search(line):
                    found.append(f'{rel}:{number}: installs or runs with npm; use pnpm')
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    args = parser.parse_args()
    found = offenders(args.root)
    for line in found:
        print(line, file=sys.stderr)
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
