#!/usr/bin/env python3
"""Select frontend unit tests; backend contracts and integration checks still run."""
import argparse
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
# Explicit backend implementation boundaries, not every file under catalyst.
# Shared types, configuration, manifests, schemas and unknown crates keep full CI.
BACKEND_CRATES = frozenset({
    'builder', 'camera-reel', 'chain-index', 'comms', 'content', 'db', 'explore',
    'fed', 'jobs', 'land-authz', 'price', 'shop', 'social', 'storage', 'testing', 'worlds',
})
COMPANION_DOCS = frozenset({'DATABASE.md', 'METRICS.md', 'docs/readiness.md'})
FRONTEND_ROOTS = ('apps/ui3', 'apps/sites', 'library')
SOURCE_EXTENSIONS = ('ts', 'tsx', 'js', 'jsx', 'mts', 'mjs', 'cts', 'cjs')
BACKEND_REFERENCE = re.compile(r'services/catalyst(?:/[A-Za-z0-9_.@+-]+)*')


def backend_source(path):
    parts = PurePosixPath(path).parts
    if '..' in parts or not path.endswith('.rs'):
        return False
    if parts[:2] != ('services', 'catalyst'):
        return False
    if len(parts) >= 4 and parts[2] in ('src', 'tests'):
        return True
    return (len(parts) >= 5 and parts[2] in BACKEND_CRATES
            and parts[3] in ('src', 'tests'))


def ensure_commit(sha):
    if not re.fullmatch(r'[0-9a-f]{40}', sha or '') or sha == '0' * 40:
        raise ValueError('Missing comparison commit')
    found = subprocess.run(['git', 'cat-file', '-e', f'{sha}^{{commit}}'],
                           cwd=ROOT, capture_output=True, timeout=60).returncode == 0
    if not found:
        subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', sha],
                       cwd=ROOT, check=True, timeout=60)


def frontend_backend_references(head):
    """Inspect committed frontend sources and helpers, including source-reading tests.

    This deliberately is not an import/JavaScript parser. Literal paths (even in
    comments) count, directory references cover their subtree, and recognizable
    computed/split paths refuse the optimization rather than guessing a target.
    """
    for root in FRONTEND_ROOTS:
        kind = subprocess.check_output(['git', 'cat-file', '-t', f'{head}:{root}'],
                                       cwd=ROOT, text=True, timeout=60).strip()
        if kind != 'tree':
            raise ValueError(f'Frontend source root unavailable: {root}')
    patterns = [f':(glob){root}/**/*.{extension}'
                for root in FRONTEND_ROOTS for extension in SOURCE_EXTENSIONS]
    matches = subprocess.run(
        ['git', 'grep', '-a', '-l', '-z', '-F', '-e', 'services', head, '--', *patterns],
        cwd=ROOT, capture_output=True, text=True, timeout=60,
    )
    if matches.returncode not in (0, 1):
        raise ValueError('Frontend dependency scan failed')
    references = set()
    for blob in filter(None, matches.stdout.split('\0')):
        text = subprocess.check_output(['git', 'show', blob], cwd=ROOT, text=True, timeout=60)
        # join(root, "services", "catalyst", ...) and string concatenation do
        # not give us a complete literal path. Keep the full frontend suites.
        if (re.search(r'''["'`]services/?["'`]''', text)
                and re.search(r'''["'`]catalyst(?:/|["'`])''', text)):
            raise ValueError('Computed frontend backend dependency')
        for match in BACKEND_REFERENCE.finditer(text):
            path = match.group()
            tail = text[match.end():]
            if '..' in PurePosixPath(path).parts or (tail and tail[0] in '/${\\'):
                raise ValueError('Ambiguous frontend backend dependency')
            references.add(path)
    return references


def plan(event, base, head):
    if event != 'pull_request':
        return {'run-web-unit-tests': True, 'reason': 'Full frontend units outside pull requests'}
    try:
        ensure_commit(base)
        ensure_commit(head)
        # Compare the actual PR base to its tested merge tree. --no-renames
        # exposes both paths of a rename, including a removed frontend input.
        changed = subprocess.check_output(
            ['git', 'diff', '--name-only', '--no-renames', '-z', base, head, '--'],
            cwd=ROOT, text=True, timeout=60,
        )
        paths = set(filter(None, changed.split('\0')))
        backend_only = (any(backend_source(path) for path in paths)
                        and all(backend_source(path) or path in COMPANION_DOCS for path in paths))
        if backend_only:
            references = frontend_backend_references(head)
            if any(path == reference or path.startswith(reference + '/')
                   for path in paths for reference in references):
                return {'run-web-unit-tests': True,
                        'reason': 'Frontend sources or helpers reference changed backend inputs'}
        return {
            'run-web-unit-tests': not backend_only,
            'reason': ('Known backend implementation changes only; contracts and smoke still run'
                       if backend_only else 'Shared, frontend, unknown or empty changes keep full frontend units'),
        }
    except (OSError, ValueError, TypeError, subprocess.SubprocessError) as exc:
        print(f'Conservative frontend selection: {exc}')
        return {'run-web-unit-tests': True, 'reason': 'Comparison unavailable; run full frontend units'}


def write_outputs(values):
    print(json.dumps(values, sort_keys=True))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            for key, value in values.items():
                encoded = json.dumps(value) if isinstance(value, bool) else value
                output.write(f'{key}={encoded}\n')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            summary.write(f"Frontend unit tests: {values['reason']}.\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', default=os.environ.get('BASE_SHA', ''))
    parser.add_argument('--head', default=os.environ.get('GITHUB_SHA', ''))
    args = parser.parse_args()
    write_outputs(plan(os.environ.get('GITHUB_EVENT_NAME', ''), args.base, args.head))


if __name__ == '__main__':
    main()
