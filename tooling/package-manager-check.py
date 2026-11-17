#!/usr/bin/env python3
"""Fail on an npm or yarn lockfile, a project or workflow off the pinned pnpm, a setting pnpm ignores in .npmrc, or npm or yarn in a workflow."""
import argparse
import json
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
# pnpm 11 reads only registry, auth and network keys from .npmrc; every other setting lives in pnpm-workspace.yaml.
NPMRC_READ = {'ca', 'cafile', 'cert', 'key', 'registry', '_auth', '_authToken', '_password', 'email', 'username',
              'https-proxy', 'proxy', 'no-proxy', 'http-proxy', 'local-address', 'strict-ssl'}
NPMRC_KEY = re.compile(r'^\s*([^#;\s=][^=]*?)\s*=', re.M)
WORKFLOWS = ('.github/workflows/', '.github/actions/')
# The pnpm every project pins in packageManager and every workflow fetches.
PNPM_VERSION = '11.28.12'
PNPM_NAMED = re.compile(r'\bpnpm@(\d+\.\d+\.\d+)')
# Copies of projects the service test installs with the pnpm its machine carries, and the Creator Hub,
# whose CI takes pnpm from .github/actions/setup-pnpm.
UNPINNED = ('infra/dev/service-test/fixture/', 'apps/creator-hub/')
# A job may fetch a newer npm to publish; that npm is checked like any other.
NPX_NPM = re.compile(r'\bnpx\s+--yes\s+npm@\S+')
# npm commands that install or run a project, after any global flags; publishing and registry queries stay on npm.
NPM_USE = re.compile(r'\bnpm(?:\s+-{1,2}[\w-]+(?:[= ][^\s-]\S*)?)*\s+(?:ci|install|i|add|uninstall|remove|update|up|dedupe'
                     r'|prune|run|run-script|test|t|start|stop|restart|exec|x|rebuild|link|init|create)(?![\w-])'
                     r'|\bnpx\b|(?:^|[\s;&|(])(?:yarn|yarnpkg)(?=\s|$)')


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
        elif name == 'pnpm-lock.yaml' and not rel.startswith(UNPINNED):
            manifest = root / rel.rsplit('/', 1)[0] / 'package.json' if '/' in rel else root / 'package.json'
            pinned = json.loads(manifest.read_text(encoding='utf-8')).get('packageManager', '') if manifest.is_file() else ''
            if pinned.split('+', 1)[0] != f'pnpm@{PNPM_VERSION}':
                found.append(f'{manifest.relative_to(root).as_posix()}: pins `{pinned or "nothing"}`; set packageManager to pnpm@{PNPM_VERSION}')
        elif name == '.npmrc':
            for match in NPMRC_KEY.finditer((root / rel).read_text(encoding='utf-8', errors='replace')):
                key = match.group(1)
                if not (key.startswith(('@', '//')) or key in NPMRC_READ):
                    found.append(f'{rel}: pnpm does not read `{key}` here; move it to pnpm-workspace.yaml or delete it')
        elif rel.startswith(WORKFLOWS) and name.endswith(('.yml', '.yaml', '.sh')):
            for number, line in enumerate((root / rel).read_text(encoding='utf-8', errors='replace').splitlines(), 1):
                if line.lstrip().startswith('#'):
                    continue
                if NPM_USE.search(NPX_NPM.sub('npm', line)):
                    found.append(f'{rel}:{number}: installs or runs with npm or yarn; use pnpm')
                for version in PNPM_NAMED.findall(line):
                    if version != PNPM_VERSION:
                        found.append(f'{rel}:{number}: fetches pnpm {version}; the projects pin {PNPM_VERSION}')
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
