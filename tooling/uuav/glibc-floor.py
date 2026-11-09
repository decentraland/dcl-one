#!/usr/bin/env python3
"""Check that a Linux runtime set loads on the oldest glibc the packages support.

Every `GLIBC_x.y` symbol version an ELF file names must be at or below the
floor. A build host newer than the floor can raise it silently: a header
change there (`sscanf` becoming `__isoc23_sscanf` on glibc 2.38) makes the
same source need a newer glibc.

Usage: glibc-floor.py <directory> [--floor X.Y]
  directory  the runtime set, e.g. library/uuav/dist/linux-x86_64
  --floor    default 2.35 (Ubuntu 22.04; Debian 12 has 2.36)
Exit status: 0 every file is at or below the floor, 1 one is above it, 2 usage.
"""

import argparse
import re
import sys
from pathlib import Path

FLOOR = '2.35'
VERSION = re.compile(rb'GLIBC_(\d+)\.(\d+)')


def newest(path):
    """The newest glibc version the file names, as (major, minor), or None."""
    versions = {(int(major), int(minor)) for major, minor in VERSION.findall(path.read_bytes())}
    return max(versions, default=None)


def main(argv):
    parser = argparse.ArgumentParser(description='Check a Linux runtime set against a glibc floor.')
    parser.add_argument('directory', type=Path)
    parser.add_argument('--floor', default=FLOOR)
    args = parser.parse_args(argv)
    floor = tuple(int(part) for part in args.floor.split('.'))
    elves = sorted(path for path in args.directory.iterdir()
                   if path.is_file() and path.read_bytes()[:4] == b'\x7fELF')
    if not elves:
        print(f'no ELF files in {args.directory}', file=sys.stderr)
        return 2
    above = False
    for path in elves:
        version = newest(path)
        shown = 'none' if version is None else f'{version[0]}.{version[1]}'
        if version is not None and version > floor:
            above = True
            print(f'{path.name}: needs glibc {shown}, above the {args.floor} floor')
        else:
            print(f'{path.name}: glibc {shown}')
    return 1 if above else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
