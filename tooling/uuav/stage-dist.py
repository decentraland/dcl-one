#!/usr/bin/env python3
"""Stage a UUAV runtime set next to a binary that will spawn it.

The uuav-helper is located as a sibling of the executable that links the
uuav client, and the FFmpeg libraries resolve from the helper's own
directory. Anything that runs the client - the explorer, the headless
examples, a test binary - therefore needs a complete runtime set copied next
to it. The set is the output of `nix build .#uuav-dist` (Linux, macOS; the
`result` link) or of `library/uuav/build.sh` (`library/uuav/dist/<target>/`,
the Windows path).

Usage: stage-dist.py <set> <directory>
  set        a runtime set directory, or `host` for
             library/uuav/dist/<host target>
  directory  created if missing; existing files of the same names are
             replaced
Exit status: 0 staged, 1 the set is missing or incomplete, 2 usage.
"""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import stat
import sys

DIST = os.path.join("library", "uuav", "dist")
HELPERS = ("uuav-helper", "uuav-helper.exe")
LIBRARIES = ("avcodec", "avformat", "avutil", "swresample", "swscale")


def host_target() -> str | None:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin" and machine == "arm64":
        return "macos-arm64"
    if system == "Windows" and machine in ("amd64", "x86_64"):
        return "windows-x86_64"
    if system == "Linux" and machine in ("x86_64", "amd64"):
        return "linux-x86_64"
    if system == "Linux" and machine == "aarch64":
        return "linux-aarch64"
    return None


def missing(names: list[str]) -> list[str]:
    """What a complete set has that `names` lacks: the helper, the five FFmpeg
    libraries in its import closure and a license text."""
    problems = []
    if not any(name in names for name in HELPERS):
        problems.append("no uuav-helper")
    for library in LIBRARIES:
        if not any(library in name and ("." in name) for name in names):
            problems.append(f"no {library} library")
    if not any(name.startswith("LICENSE-") for name in names):
        problems.append("no license text")
    return problems


def stage(source: str, directory: str) -> list[str]:
    """Copies every file of the runtime set at `source`; returns problems."""
    if not os.path.isdir(source):
        return [f"no runtime set at {source}; build it with `nix build .#uuav-dist` or "
                f"library/uuav/build.sh"]
    names = sorted(name for name in os.listdir(source)
                   if os.path.isfile(os.path.join(source, name)))
    problems = [f"{source} is incomplete: {problem}" for problem in missing(names)]
    if problems:
        return problems
    os.makedirs(directory, exist_ok=True)
    for name in names:
        src = os.path.join(source, name)
        dst = os.path.join(directory, name)
        shutil.copyfile(src, dst)
        os.chmod(dst, os.stat(src).st_mode | stat.S_IRUSR | stat.S_IWUSR)
        print(f"  staged  {name}")
    return []


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("set", help="runtime set directory or `host`")
    parser.add_argument("directory", help="where the binary that spawns the helper lives")
    parser.add_argument("--repo", default=None,
                        help="repository root (default: two levels above this script)")
    args = parser.parse_args()

    repo = args.repo or os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))))
    if args.set == "host":
        target = host_target()
        if target is None:
            print(f"FAIL: no UUAV target for {platform.system()} {platform.machine()}",
                  file=sys.stderr)
            return 2
        source = os.path.join(repo, DIST, target)
    else:
        source = args.set

    problems = stage(source, args.directory)
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"Staged {source} into {args.directory}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
