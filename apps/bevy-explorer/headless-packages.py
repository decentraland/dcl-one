#!/usr/bin/env python3
"""Pin the launcher only after all four platform archives agree on a version."""
import argparse
import json
import re
from pathlib import Path
import tarfile


def pin_launcher(archives, launcher, revision):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("Invalid source revision")
    package = json.loads(launcher.read_text())
    expected = {package['name'] + '-' + platform for platform in
                ('darwin-arm64', 'darwin-x64', 'linux-x64', 'win32-x64')}
    if set(package['optionalDependencies']) != expected:
        raise ValueError('The launcher must declare exactly the four supported platforms')
    versions = {}
    for path in sorted(archives.glob('*.tgz')):
        with tarfile.open(path, 'r:gz') as archive:
            members = [m for m in archive.getmembers() if m.name == 'package/package.json']
            if len(members) != 1 or not members[0].isfile() or members[0].size > 1024 * 1024:
                raise ValueError(f'Invalid package manifest: {path.name}')
            info = json.load(archive.extractfile(members[0]))
        name, version = info['name'], info['version']
        if name not in expected or name in versions or not isinstance(version, str) or not version:
            raise ValueError(f'Unexpected or duplicate platform package: {name}')
        versions[name] = version
    if set(versions) != expected or len(set(versions.values())) != 1:
        raise ValueError('All four platform packages must exist with the same version')
    version = next(iter(versions.values()))
    commit = re.search(r'\.commit-([0-9a-f]{7,40})$', version)
    if not commit or not revision.startswith(commit[1]):
        raise ValueError('Platform version does not identify the checked-out source revision')
    package['version'] = version
    package['commit'] = revision
    package['optionalDependencies'] = dict.fromkeys(package['optionalDependencies'], package['version'])
    launcher.write_text(json.dumps(package, indent=2) + '\n')
    return package['version']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archives', type=Path)
    parser.add_argument('launcher', type=Path)
    parser.add_argument('--revision', required=True)
    args = parser.parse_args()
    print(f'Validated four platforms at {pin_launcher(args.archives, args.launcher, args.revision)}')


if __name__ == '__main__':
    main()
