#!/usr/bin/env python3
"""Select the Development environment's Rust test platforms: macOS only for changes it can observe.

  python3 tooling/dev-ci.py plan                     in a pull_request run (compares the merge commit's parents)
  python3 tooling/dev-ci.py plan --base origin/main  locally (compares HEAD with its merge base)
"""
import argparse
import importlib.util
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = 'development.yml'
# What the macOS Rust job itself runs: the dev stack, the ENet pin check and dev-test.sh's tests.
MACOS = ('tooling/dev*', 'tooling/enet-provenance.py', 'tooling/tests/test_dev_regressions.py',
         'tooling/tests/test_enet_provenance.py')
# Watched by the workflow for smoke/web only; the Linux leg covers the Rust job's view of them.
LINUX_ONLY = ('apps/sites/**', 'apps/ui3/**', 'rendering/bevy-explorer/upstream.lock.json',
              'tooling/*.py', 'tooling/tests/**')


def load_ci_local():
    spec = importlib.util.spec_from_file_location('ci_local', ROOT / 'tooling/ci-local.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def needs_macos(changed, ci):
    triggers = ci.workflow_paths(WORKFLOW, source=ROOT)
    watched = [ci.glob_regex(p) for p in triggers] if triggers is not None else None
    macos = [ci.glob_regex(p) for p in MACOS]
    linux = [ci.glob_regex(p) for p in LINUX_ONLY]
    relevant = [path for path in changed if watched is None or any(r.match(path) for r in watched)]
    for path in relevant:
        if any(r.match(path) for r in macos) or not any(r.match(path) for r in linux):
            return True, f'macOS input changed: {path}'
    return False, 'Only Linux-covered inputs changed' if relevant else 'No watched inputs changed'


def plan(event, base, head):
    if event not in ('pull_request', 'local'):
        return True, f'{event} event runs every platform'
    try:
        if event == 'local':
            base = subprocess.check_output(['git', 'merge-base', base, head], cwd=ROOT, text=True,
                                           stderr=subprocess.PIPE, timeout=60).strip()
        changed = subprocess.check_output(['git', 'diff', '--name-only', '--no-renames', '-z', base, head],
                                          cwd=ROOT, text=True, stderr=subprocess.PIPE, timeout=60).split('\0')
        return needs_macos([path for path in changed if path], load_ci_local())
    except Exception as exc:
        return True, f'Comparison unavailable, running macOS: {exc}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('plan',))
    parser.add_argument('--base')
    parser.add_argument('--head', default='HEAD')
    args = parser.parse_args()
    event = os.environ.get('GITHUB_EVENT_NAME', 'local')
    if args.base is None and event == 'local':
        parser.error('--base is required outside GitHub Actions')
    macos, reason = plan(event, args.base or 'HEAD^1', args.head)
    print(f'macos={str(macos).lower()}: {reason}')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
            summary.write(f'Rust tests on macOS: {str(macos).lower()} ({reason}).\n')
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.write(f'macos={str(macos).lower()}\n')


if __name__ == '__main__':
    main()
