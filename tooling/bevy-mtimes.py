#!/usr/bin/env python3
"""Keep the vendored Bevy's cached build across CI checkouts.

Cargo decides whether a path dependency is fresh by file times, and a fresh checkout gives
every file the current time, so the Bevy a CI cache restores would be rebuilt by every job
of every run, changed or not. Crates from git or crates.io are judged by version instead.

  record <target-dir>   after the build: each vendored file's git blob id and mtime, floored
                        to whole seconds, into every profile's .fingerprint directory, so the
                        manifest travels in the same cache archive as the artifacts it vouches for
  restore <target-dir>  after the checkout and the cache restore, before Cargo runs: each file
                        whose blob id still matches gets its recorded mtime back; a changed or
                        new file keeps the checkout's time, so Cargo rebuilds what depends on it

To Cargo this is a checkout that never moved. Floored times stay valid when a cache archive
keeps whole seconds only; in CI the tree is only ever written at checkout, before any build.
Without a manifest (a cold cache, or one rust-cache pruned) restore changes nothing.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TREE = 'rendering/bevy-explorer/bevy'
# rust-cache keeps a .fingerprint entry whose name, up to the last '-', is a dependency's
# package name (cleanup.ts rmExcept); `bevy` is one, so the manifest survives its pruning.
MANIFEST = 'bevy-mtimes.json'
SECOND = 10**9


def blobs(root):
    """{path: blob id} for every tracked file under TREE, from the index."""
    out = subprocess.check_output(['git', 'ls-files', '--stage', '-z', '--', TREE], cwd=root)
    result = {}
    for record in filter(None, out.split(b'\0')):
        meta, path = record.split(b'\t', 1)
        result[path.decode('utf-8')] = meta.split()[1].decode('ascii')
    return result


def fingerprint_dirs(target):
    """The .fingerprint directory of every profile: target/<profile> and target/<triple>/<profile>."""
    target = Path(target)
    if not target.is_dir():
        return []
    return sorted(p for p in [*target.glob('*/.fingerprint'), *target.glob('*/*/.fingerprint')] if p.is_dir())


def record(root, target):
    """Write the manifest into every profile; returns how many profiles got it."""
    files = {path: [blob, (Path(root) / path).stat().st_mtime_ns // SECOND * SECOND]
             for path, blob in blobs(root).items()}
    text = json.dumps({'tree': TREE, 'files': files}, sort_keys=True)
    directories = fingerprint_dirs(target)
    for directory in directories:
        (directory / MANIFEST).write_text(text)
    return len(directories)


def restore(root, target):
    """Give unchanged files their recorded mtime; returns (restored, left with the checkout's time)."""
    manifests = [d / MANIFEST for d in fingerprint_dirs(target) if (d / MANIFEST).is_file()]
    recorded = json.loads(manifests[0].read_text())['files'] if manifests else {}
    restored = left = 0
    for path, blob in blobs(root).items():
        entry = recorded.get(path)
        if entry and entry[0] == blob:
            os.utime(Path(root) / path, ns=(entry[1], entry[1]))
            restored += 1
        else:
            left += 1
    return restored, left


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=('record', 'restore'))
    parser.add_argument('target', help="the explorer's Cargo target directory")
    args = parser.parse_args()
    if args.command == 'record':
        print(f'{TREE}: mtimes recorded in {record(ROOT, args.target)} profile(s) of {args.target}')
    else:
        restored, left = restore(ROOT, args.target)
        print(f'{TREE}: {restored} unchanged file(s) keep their cached mtime; {left} changed or unrecorded')
    return 0


if __name__ == '__main__':
    sys.exit(main())
