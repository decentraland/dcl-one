#!/usr/bin/env python3
"""Stage the committed UUAV runtime set next to a binary that will spawn it.

The uuav-helper is located as a sibling of the executable that links the
uuav client, and the FFmpeg libraries resolve from the helper's own
directory. Anything that runs the client - the explorer, the headless
examples, a test binary - therefore needs `library/uuav/dist/<target>/`
copied next to it. This script does that copy and refuses to stage an
unfetched Git LFS pointer, which would otherwise fail at spawn time with an
opaque loader error, or a set whose files are not the bytes
`tooling/uuav/uuav-binaries.lock.json` pins for the target.

Usage: stage-dist.py <target> <directory> [--repo ROOT]
  target     a lock target (macos-universal, windows-x86_64, linux-x86_64)
             or `host` for the running machine's target
  directory  created if missing; existing files of the same names are
             replaced
Exit status: 0 staged, 1 a file is missing, is an LFS pointer or differs from
the lock, 2 usage.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import stat
import sys

LFS_POINTER_PREFIX = b"version https://git-lfs.github.com/spec/v1"
DIST = os.path.join("library", "uuav", "dist")
LOCK = os.path.join("tooling", "uuav", "uuav-binaries.lock.json")
TARGETS = ("macos-universal", "windows-x86_64", "linux-x86_64")


def host_target() -> str | None:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin":
        return "macos-universal"
    if system == "Windows" and machine in ("amd64", "x86_64"):
        return "windows-x86_64"
    if system == "Linux" and machine in ("x86_64", "amd64"):
        return "linux-x86_64"
    return None


def is_lfs_pointer(path: str) -> bool:
    with open(path, "rb") as handle:
        return handle.read(len(LFS_POINTER_PREFIX)) == LFS_POINTER_PREFIX


def sha256(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def locked_hashes(repo: str, target: str) -> dict[str, str]:
    """The lock's sha256 of every file of the target's runtime set, by file name."""
    with open(os.path.join(repo, LOCK), encoding="utf-8") as handle:
        lock = json.load(handle)
    return {os.path.basename(artifact["path"]): artifact["sha256"]
            for artifact in lock["targets"][target]["artifacts"]}


def stage(repo: str, target: str, directory: str) -> list[str]:
    """Copies every file of the target's dist directory once each matches the lock; returns
    problems."""
    source = os.path.join(repo, DIST, target)
    if not os.path.isdir(source):
        return [f"no runtime set at {os.path.relpath(source, repo)}; build it with "
                f"library/uuav/build.sh or fetch it from a uuav-native.yml run"]
    names = sorted(name for name in os.listdir(source)
                   if os.path.isfile(os.path.join(source, name)) and not name.endswith(".sha256"))
    if not names:
        return [f"{os.path.relpath(source, repo)} is empty"]
    problems = [f"{name} is an unfetched Git LFS pointer; run "
                f"`git lfs pull --include 'library/uuav/dist/**'`"
                for name in names if is_lfs_pointer(os.path.join(source, name))]
    if problems:
        return problems
    try:
        locked = locked_hashes(repo, target)
    except (OSError, KeyError, TypeError, ValueError) as error:
        return [f"cannot read {LOCK} for {target}: {error}"]
    for name in names:
        expected = locked.get(name)
        if expected is None:
            problems.append(f"{name} is not in the lock; relock the target or remove the file")
            continue
        actual = sha256(os.path.join(source, name))
        if actual != expected:
            problems.append(f"{name} differs from the lock (sha256 {actual[:12]}, locked "
                            f"{expected[:12]}); the runtime set changed without a relock")
    problems += [f"{name} is in the lock but missing from {os.path.relpath(source, repo)}"
                 for name in sorted(set(locked) - set(names))]
    if problems:
        return problems
    os.makedirs(directory, exist_ok=True)
    for name in names:
        src = os.path.join(source, name)
        dst = os.path.join(directory, name)
        shutil.copyfile(src, dst)
        # the helper must stay executable; dylibs keep their mode too
        os.chmod(dst, os.stat(src).st_mode | stat.S_IRUSR | stat.S_IWUSR)
        print(f"  staged  {name}")
    return []


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("target", help="lock target or `host`")
    parser.add_argument("directory", help="where the binary that spawns the helper lives")
    parser.add_argument("--repo", default=None,
                        help="repository root (default: two levels above this script)")
    args = parser.parse_args()

    repo = args.repo or os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))))
    target = host_target() if args.target == "host" else args.target
    if target not in TARGETS:
        print(f"FAIL: unknown target '{args.target}'; one of {', '.join(TARGETS)} or host",
              file=sys.stderr)
        return 2

    problems = stage(repo, target, args.directory)
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"Staged {target} into {args.directory}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
