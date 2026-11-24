#!/usr/bin/env python3
"""Key and prune the Explorer's Cargo caches for the Actions cache: the native jobs' and the wasm relay wake test's.

Cargo rebuilds a crate when its fingerprint changes: the compiler, an environment variable a
build script read (the CC and PKG_CONFIG_PATH the tool steps export), or an input file. A cache
saved under a key blind to one of those keeps stale fingerprints for good: every run restores
it, rebuilds the same crates and never saves, because the key still matches. The key here is
what the fingerprints depend on. The cache holds the workspace crates too: tooling/bevy-mtimes.py
gives an unchanged file its recorded time, so Cargo compiles only what a change reaches.

  key --lane <name>   key, dependencies and restore-keys for actions/cache/restore, into
                      $GITHUB_OUTPUT:
                      bevy-native-v1-<lane>-<os>-<arch>-<rustc>-<tools>-<sources>-<members>
                        rustc    `rustc -vV`
                        tools    the environment the tool steps export for build scripts
                        sources  Cargo.lock, the workspace manifests, the Cargo configs (the
                                 workspace's and the repository root's) and the tracked sources
                                 of the path dependencies outside the workspace: the in-tree
                                 crates and the vendored trees. Cargo.lock names the revision of
                                 each patched source
                        members  the workspace tree and the protocol definitions its build
                                 scripts read
                      `dependencies` is the key without its last part. The fallback keys drop
                      the members, then the sources, then the tools. Main saves an entry for
                      each new key. A pull request restores the newest entry of main for its
                      dependencies and saves an entry of its own only when the restored key does
                      not start with `dependencies`
  prune <target-dir>  before the save: what no later run reads (final binaries, incremental
                      state, reports) goes, and so does every unit that no message in
                      <target-dir>/cargo-messages names: the generations a dependency change left
                      behind. The artifacts, build script outputs and fingerprints of the named
                      units stay, with the bevy-mtimes manifest. Without messages every generation
                      stays. It removes nothing outside <target-dir>: the save step names the
                      Cargo home paths it stores, and the rest of the Cargo home is not in the cache

Run `key` after the tool steps, so it sees their environment, and before the first Cargo
invocation; run `prune` after the last upload that reads the target directory. Every Cargo call of
the job writes its `--message-format=json-render-diagnostics` output into
<target-dir>/cargo-messages/<name>.jsonl.
"""
import argparse
import filecmp
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'rendering/bevy-explorer/source'
# The tracked sources of the path dependencies Cargo judges by file time (tooling/bevy-mtimes.py).
VENDORED = ('rendering/bevy-explorer/vendor', 'library/vendor/rusty_enet',
            'rendering/bevy-explorer/source/vendor/basisu')
# What the workspace crates compile from: their own tree and the protocol definitions (tooling/bevy-mtimes.py).
MEMBERS = ('rendering/bevy-explorer/source', 'library/protocol/proto')
SOURCE_FILES = ('Cargo.lock', 'Cargo.toml', '.cargo/config.toml')
# What build scripts record with rerun-if-env-changed and the tool steps export (CC and CXX by
# install-clang.sh, PKG_CONFIG_PATH by install-tools.sh, CEF_PATH by the CEF export, LIBCLANG_PATH
# and FFMPEG_DIR on Windows), plus the flags Cargo hashes into every unit. A tool step that
# exports a new variable for build scripts adds it here.
TRACKED_ENV = ('CC', 'CXX', 'AR', 'CFLAGS', 'CXXFLAGS', 'CMAKE', 'PKG_CONFIG_PATH', 'LIBCLANG_PATH', 'FFMPEG_DIR',
               'CEF_PATH', 'RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS')
KEEP_IN_PROFILE = ('build', 'deps', '.fingerprint')
KEEP_IN_TARGET = ('CACHEDIR.TAG', '.rustc_info.json')
MESSAGES = 'cargo-messages'
UNIT_HASH = re.compile(r'-([0-9a-f]{16})(?=\.|$)')


def digest(*parts):
    hasher = hashlib.sha256()
    for part in parts:
        hasher.update(part.encode('utf-8') + b'\0')
    return hasher.hexdigest()[:16]


def rustc(env):
    return subprocess.check_output([env.get('RUSTC', 'rustc'), '-vV'], text=True)


def tools(env):
    return json.dumps({name: env[name] for name in TRACKED_ENV if name in env}, sort_keys=True)


def workspace(source=SOURCE):
    """The manifests of the workspace members."""
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=source, text=True))
    return sorted(Path(package['manifest_path']) for package in metadata['packages'])


def sources(manifests, root, source):
    parts = [subprocess.check_output(['git', 'rev-parse', f'HEAD:{tree}'], cwd=root, text=True).strip()
             for tree in VENDORED]
    files = {source / name for name in SOURCE_FILES} | set(manifests)
    files |= set(source.glob('rust-toolchain*')) | set(root.glob('rust-toolchain*')) | {root / '.cargo/config.toml'}
    for path in sorted(files):
        if path.is_file():
            parts.append(f'{Path(os.path.relpath(path, root)).as_posix()}:{hashlib.sha256(path.read_bytes()).hexdigest()}')
    return digest(*parts)


def members(root):
    return digest(*(subprocess.check_output(['git', 'rev-parse', f'HEAD:{tree}'], cwd=root, text=True).strip()
                    for tree in MEMBERS))


def key(lane, env, root, source):
    """The exact key and its fallbacks: any members, then any sources, then any tools, of the same compiler on the same lane."""
    platform_name = f'{env.get("RUNNER_OS") or platform.system()}-{env.get("RUNNER_ARCH") or platform.machine()}'
    compiler = f'bevy-native-v1-{lane}-{platform_name}-{digest(rustc(env))}-'
    with_tools = f'{compiler}{digest(tools(env))}-'
    dependencies = f'{with_tools}{sources(workspace(source), root, source)}'
    return f'{dependencies}-{members(root)}', [dependencies, with_tools, compiler]


def remove(path, removed):
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink()
    removed.append(path)


def is_profile(directory):
    return directory.is_dir() and any((directory / name).is_dir() for name in KEEP_IN_PROFILE)


def prune_profile(profile, removed):
    for entry in profile.iterdir():
        if entry.name not in KEEP_IN_PROFILE:
            remove(entry, removed)


def prune_target(target, removed):
    """Profiles lie in target/<profile> and target/<triple>/<profile>; the rest is reports and scratch."""
    for entry in target.iterdir():
        if is_profile(entry):
            prune_profile(entry, removed)
        elif entry.is_dir() and any(is_profile(child) for child in entry.iterdir()):
            prune_target(entry, removed)
        elif entry.name not in KEEP_IN_TARGET:
            remove(entry, removed)


def messages(target):
    for path in sorted((target / MESSAGES).glob('*.jsonl')):
        # Not splitlines(): serde_json leaves U+2028/U+2029 unescaped inside a message.
        for line in path.read_text(encoding='utf-8', errors='replace').split('\n'):
            try:
                message = json.loads(line)
            except ValueError:
                continue
            if isinstance(message, dict):
                yield message


def unit_hashes(path):
    return {found for name in (path.name, path.parent.name) for found in UNIT_HASH.findall(name)}


def uplifted(path):
    """The hash of deps/<crate>-<hash>, the unit Cargo copied or linked to a binary it reports only at the profile's top level.

    Every candidate stays when the binary is gone or none matches it."""
    stem, suffix = (path.name[:-4], '.exe') if path.name.endswith('.exe') else (path.name, '')
    crate = stem.replace('-', '_')
    # An MSVC executable keeps no hash: the hashed files beside it are its test harnesses, named by their own messages.
    if (path.parent / 'deps' / (crate + suffix)).is_file():
        return set()
    pattern = re.compile(re.escape(crate) + r'-([0-9a-f]{16})' + re.escape(suffix))
    candidates = {}
    for entry in (path.parent / 'deps').glob(f'{crate}-*'):
        match = pattern.fullmatch(entry.name)
        if match and entry.is_file():
            candidates[match.group(1)] = entry
    if not path.is_file():
        return set(candidates)
    # The link first: a RUSTFLAGS change renames a unit without changing its bytes, so an older copy can be equal.
    same = {unit for unit, entry in candidates.items() if os.path.samefile(entry, path)}
    return same or {unit for unit, entry in candidates.items() if filecmp.cmp(entry, path, shallow=False)} or set(candidates)


def live_units(target):
    """The hashes of every unit the job's Cargo calls named, built or fresh, or None when they named none."""
    live = set()
    for message in messages(target):
        if message.get('reason') == 'compiler-artifact':
            for name in message.get('filenames') or ():
                path = Path(name)
                live |= unit_hashes(path) or uplifted(path)
        elif message.get('reason') == 'build-script-executed' and message.get('out_dir'):
            live |= unit_hashes(Path(message['out_dir']))
    return live or None


def profiles(target):
    for entry in target.iterdir():
        if is_profile(entry):
            yield entry
        elif entry.is_dir() and not entry.is_symlink():
            yield from (child for child in entry.iterdir() if is_profile(child))


def prune_generations(target, live, removed):
    """Remove the deps and build entries of units outside `live`, and the fingerprints with their hashes; returns bytes freed and failures."""
    stale, freed, failed = set(), 0, []

    def drop(entry):
        nonlocal freed
        try:
            bytes_ = size(entry) if entry.is_dir() and not entry.is_symlink() else entry.lstat().st_size
            remove(entry, removed)
            freed += bytes_
        except OSError as error:
            failed.append(f'{entry}: {error}')

    def entries(name):
        return [entry for profile in profiles(target) if (profile / name).is_dir() for entry in (profile / name).iterdir()]

    for entry in entries('deps') + entries('build'):
        found = UNIT_HASH.search(entry.name)
        if found and found.group(1) not in live:
            stale.add(found.group(1))
            drop(entry)
    # Only the hashes of removed entries: a path cdylib's and an MSVC executable's fingerprint carry one no message names.
    for entry in entries('.fingerprint'):
        found = re.fullmatch(r'.+-([0-9a-f]{16})', entry.name)
        if found and found.group(1) in stale:
            drop(entry)
    return freed, failed


def size(directory):
    """Bytes of the files below directory. A symbolic link is not followed; an entry that cannot be read counts as 0."""
    total = 0
    for parent, _, files in os.walk(directory):
        for name in files:
            try:
                total += os.lstat(os.path.join(parent, name)).st_size
            except OSError:
                pass
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('key').add_argument('--lane', required=True, help='the job: test, headless, clippy or relay-wake')
    commands.add_parser('prune').add_argument('target', type=Path, help="the explorer's Cargo target directory")
    args = parser.parse_args()
    if args.command == 'key':
        exact, fallbacks = key(args.lane, os.environ, ROOT, SOURCE)
        lines = [f'key={exact}', f'dependencies={fallbacks[0]}', 'restore-keys<<KEYS', *fallbacks, 'KEYS']
        if os.environ.get('GITHUB_OUTPUT'):
            with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
                output.write('\n'.join(lines) + '\n')
            print(f'{exact}, falling back to {" then ".join(fallbacks)}')
        else:
            print('\n'.join(lines))
    else:
        target = args.target.resolve()
        # Before prune_target: it removes the messages and the binaries `uplifted` compares.
        live = live_units(target)
        removed = []
        prune_target(target, removed)
        if live is None:
            print(f'::warning::no Cargo message in {target / MESSAGES} names a unit: every build generation stays in the cache')
        else:
            pruned = len(removed)
            freed, failed = prune_generations(target, live, removed)
            for failure in failed:
                print(f'::warning::kept a stale build entry: {failure}')
            print(f'{len(live)} live units; {len(removed) - pruned} entries of stale generations removed '
                  f'({freed / 2**20:.0f} MB)')
        print(f'{len(removed)} entries pruned; {size(target) / 2**20:.0f} MB left in {args.target}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
