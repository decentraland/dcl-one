#!/usr/bin/env python3
"""Move the nixpkgs pin of flake.nix to the tip of a nixpkgs branch and relock it.

flake.nix pins nixpkgs by commit, so `nix flake update` alone never moves it. This resolves
the branch tip (nixos-unstable unless --branch or --rev says otherwise), rewrites the pin,
runs `nix flake update nixpkgs` and reports the old and new revisions; with GITHUB_OUTPUT
set it also writes changed, old, new and date for the Update nixpkgs workflow.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FLAKE = ROOT / 'flake.nix'
LOCK = ROOT / 'flake.lock'
NIXPKGS = 'https://github.com/NixOS/nixpkgs.git'
PIN = re.compile(r'(nixpkgs\.url = "github:NixOS/nixpkgs/)([0-9a-f]{40})(")')


def current_pin(flake_text):
    pins = PIN.findall(flake_text)
    if len(pins) != 1:
        raise ValueError('flake.nix must pin nixpkgs.url to exactly one github:NixOS/nixpkgs/<commit>')
    return pins[0][1]


def replace_pin(flake_text, rev):
    current_pin(flake_text)
    return PIN.sub(lambda match: match.group(1) + rev + match.group(3), flake_text)


def locked_nixpkgs(lock_text):
    """(rev, date) of the locked nixpkgs input."""
    nodes = json.loads(lock_text)['nodes']
    locked = nodes[nodes['root']['inputs']['nixpkgs']]['locked']
    date = datetime.datetime.fromtimestamp(locked['lastModified'], datetime.timezone.utc).date()
    return locked['rev'], date.isoformat()


def branch_tip(branch):
    listing = subprocess.run(['git', 'ls-remote', '--exit-code', NIXPKGS, f'refs/heads/{branch}'],
                             check=True, capture_output=True, text=True).stdout
    return listing.split()[0]


def write_outputs(values):
    print(json.dumps(values, sort_keys=True))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            for key, value in values.items():
                output.write(f'{key}={json.dumps(value) if isinstance(value, bool) else value}\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--branch', default='nixos-unstable', help='nixpkgs branch whose tip becomes the pin')
    parser.add_argument('--rev', help='pin this commit instead of the branch tip')
    args = parser.parse_args()
    old = current_pin(FLAKE.read_text(encoding='utf-8'))
    new = args.rev or branch_tip(args.branch)
    if not re.fullmatch(r'[0-9a-f]{40}', new):
        raise SystemExit(f'Not a full commit hash: {new}')
    if new != old:
        FLAKE.write_text(replace_pin(FLAKE.read_text(encoding='utf-8'), new), encoding='utf-8')
        subprocess.run(['nix', 'flake', 'update', 'nixpkgs'], check=True, cwd=ROOT)
    rev, date = locked_nixpkgs(LOCK.read_text(encoding='utf-8'))
    if rev != new:
        raise SystemExit(f'flake.lock pins {rev}, expected {new}')
    write_outputs({'changed': new != old, 'old': old, 'new': new, 'date': date})


if __name__ == '__main__':
    main()
