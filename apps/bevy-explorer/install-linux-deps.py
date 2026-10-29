#!/usr/bin/env python3
"""Install CI packages from Ubuntu's archive without the slow Azure mirror."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import time


def configure(apt_root):
    # Hosted runners point both source formats at mirror+file:.../apt-mirrors.txt.
    # Replace the Azure entry there as well as direct URLs in older images.
    # Keep suites, components, signing keys, security and third-party sources.
    sources = [apt_root / 'apt-mirrors.txt', apt_root / 'sources.list',
               *sorted((apt_root / 'sources.list.d').glob('*.list')),
               *sorted((apt_root / 'sources.list.d').glob('*.sources'))]
    for path in sources:
        if not path.is_file():
            continue
        original = path.read_text()
        updated = re.sub(r'https?://azure\.archive\.ubuntu\.com/ubuntu(?=[/\s]|$)',
                         'https://archive.ubuntu.com/ubuntu', original)
        if updated != original:
            path.write_text(updated)
            print(f'Using the Ubuntu archive in {path.name}', flush=True)
    config = apt_root / 'apt.conf.d/80-bevy-ci-network'
    config.write_text('Acquire::Retries "3";\n'
                      'Acquire::http::Timeout "30";\n'
                      'Acquire::https::Timeout "30";\n')


def install(packages, apt_root=Path('/etc/apt')):
    configure(apt_root)
    env = {**os.environ, 'DEBIAN_FRONTEND': 'noninteractive'}
    started = time.monotonic()
    # Do not proceed with stale indexes when any repository update fails.
    subprocess.run(['apt-get', '-o', 'APT::Update::Error-Mode=any', 'update'],
                   env=env, check=True)
    subprocess.run(['apt-get', 'install', '--yes', '--no-install-recommends', *packages],
                   env=env, check=True)
    print(f'Linux dependency installation: {time.monotonic() - started:.1f}s', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('packages', nargs='+')
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error('Run with sudo on the disposable Ubuntu CI runner')
    install(args.packages)


if __name__ == '__main__':
    main()
