#!/usr/bin/env python3
"""Move the FFmpeg pin of infra/packages/uuav.nix to the newest FFmpeg release.

uuav.nix pins FFmpeg by commit and hash, so a release never arrives alone, and FFmpeg ships its
security fixes as releases. This takes the highest release tag of FFmpeg (n<major>.<minor> or
n<major>.<minor>.<patch>; -dev tags are not releases), rewrites the commit, the hash and the
version, replaces the version in the documents that name it and checks that
uuav-ffmpeg-hls-https.patch applies to the new source. It prints changed, old and new. The
ffmpeg-sys-next version and the Windows developer prefix stay as they are: the checks of the
pull request show if they must move too.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT / 'infra/packages/uuav.nix'
PATCH = ROOT / 'infra/packages/uuav-ffmpeg-hls-https.patch'
DOCUMENTS = ['library/uuav/README.md', 'library/README.md', 'docs/adr/2026-11-08-uuav-nix-built-runtime.md']
FFMPEG = 'https://github.com/FFmpeg/FFmpeg.git'
RELEASE = re.compile(r'^([0-9a-f]{40})\trefs/tags/n(\d+\.\d+(?:\.\d+)?)(\^\{\})?$')
PIN = re.compile(r'(repo = "FFmpeg";\n\s*rev = ")([0-9a-f]{40})(";\n\s*hash = ")(sha256-[A-Za-z0-9+/=]+)'
                 r'(";\n\s*\};\n\s*ffmpegVersion = ")(\d+\.\d+(?:\.\d+)?)(";)')


def current_pin(package_text):
    """(commit, hash, version) of the FFmpeg source."""
    pins = PIN.findall(package_text)
    if len(pins) != 1:
        raise ValueError('uuav.nix must pin FFmpeg once: rev, hash, then ffmpegVersion')
    return pins[0][1], pins[0][3], pins[0][5]


def replace_pin(package_text, rev, source_hash, version):
    current_pin(package_text)
    return PIN.sub(lambda m: m.group(1) + rev + m.group(3) + source_hash + m.group(5) + version + m.group(7),
                   package_text)


def release_number(version):
    return tuple(int(part) for part in version.split('.'))


def newest_release(listing):
    """(version, commit) of the highest release tag in `git ls-remote --tags` output."""
    commits = {}
    for line in listing.splitlines():
        match = RELEASE.match(line)
        if match and (match.group(3) or match.group(2) not in commits):
            commits[match.group(2)] = match.group(1)  # the peeled line of an annotated tag wins
    if not commits:
        raise ValueError('no FFmpeg release tag in the listing')
    version = max(commits, key=release_number)
    return version, commits[version]


def rename_release(text, old, new):
    return text.replace(f'FFmpeg {old}', f'FFmpeg {new}').replace(f'`n{old}`', f'`n{new}`')


def main():
    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args()
    _, _, old = current_pin(PACKAGE.read_text(encoding='utf-8'))
    listing = subprocess.run(['git', 'ls-remote', '--tags', FFMPEG, 'refs/tags/n*'],
                             check=True, capture_output=True, text=True).stdout
    new, rev = newest_release(listing)
    changed = release_number(new) > release_number(old)
    if changed:
        source = json.loads(subprocess.run(['nix', 'flake', 'prefetch', '--json', f'github:FFmpeg/FFmpeg/{rev}'],
                                           check=True, capture_output=True, text=True).stdout)
        applies = subprocess.run(['patch', '-p1', '--dry-run', '--force', '-d', source['storePath'], '-i', PATCH],
                                 capture_output=True, text=True)
        if applies.returncode:
            raise SystemExit(f'{PATCH.name} does not apply to FFmpeg {new}; refresh it as '
                             f'library/uuav/README.md says.\n{applies.stdout}{applies.stderr}')
        PACKAGE.write_text(replace_pin(PACKAGE.read_text(encoding='utf-8'), rev, source['hash'], new),
                           encoding='utf-8')
        for document in DOCUMENTS:
            path = ROOT / document
            path.write_text(rename_release(path.read_text(encoding='utf-8'), old, new), encoding='utf-8')
    print(json.dumps({'changed': changed, 'old': old, 'new': new}, sort_keys=True))


if __name__ == '__main__':
    main()
