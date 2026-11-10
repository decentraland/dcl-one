"""Validate the checked HUD dependency graph without resolving imports."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess

WEB = 'rendering/bevy-explorer/source/react-web/'
MANIFEST = WEB + 'shared-inputs.json'
DOMAINS = (WEB, 'library/ui/', 'library/shop/', 'library/catalyst-api/')
CONTROLS = tuple(WEB + name for name in ('package.json', 'package-lock.json', 'tsconfig.json',
                                       'vite.config.ts', 'vitest.config.ts', 'vite.shared.ts',
                                       'scripts/shared-inputs.mjs')) + ('library/ui/package.json', 'library/ui/foundations/typography/font-source.json')


class ManifestError(ValueError):
    pass


class StaleManifestError(ManifestError):
    pass


class MissingManifest(ManifestError):
    pass


def sha(data):
    return hashlib.sha256(data).hexdigest()


def safe_path(value):
    if (not isinstance(value, str) or not value or '\\' in value or any(ord(c) < 32 for c in value)
            or str(PurePosixPath(value)) != value or PurePosixPath(value).is_absolute()
            or any(part in ('.', '..') for part in value.split('/'))):
        raise ManifestError('Unsafe HUD input path: ' + repr(value))
    return value


class Tree:
    def __init__(self, root, revision=None):
        self.root, self.revision = Path(root).resolve(), revision
        self.git = (self.root / '.git').exists()
        self.entries = {}
        self.blobs = {}
        if revision:
            output = self.command('ls-tree', '-rz', revision)
            for row in output.split(b'\0'):
                if row:
                    attrs, name = row.split(b'\t', 1)
                    mode, kind, oid = attrs.decode().split()
                    self.entries[name.decode()] = (mode, kind, oid)
        elif self.git:
            output = self.command('ls-files', '--stage', '-z')
            for row in output.split(b'\0'):
                if row:
                    attrs, name = row.split(b'\t', 1)
                    mode, oid, stage = attrs.decode().split()
                    if stage != '0':
                        raise ManifestError('Unmerged HUD input inventory')
                    self.entries[name.decode()] = (mode, 'blob', oid)
        else:
            self.entries = {p.relative_to(self.root).as_posix(): None
                            for p in self.root.rglob('*') if (not p.is_dir() or p.is_symlink())
                            and not p.relative_to(self.root).as_posix().startswith(WEB + 'src/engine/generated/')
                            and 'node_modules' not in p.relative_to(self.root).parts}

    def command(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE)

    def prefetch(self, names):
        if not self.revision:
            return
        entries = [(name, self.entries[name][2]) for name in sorted(set(names))
                   if name in self.entries and self.entries[name][1] == 'blob']
        data = subprocess.check_output(['git', 'cat-file', '--batch'], cwd=self.root,
                                       input=''.join(oid + '\n' for _, oid in entries).encode())
        stream = io.BytesIO(data)
        for name, oid in entries:
            actual, kind, size = stream.readline().decode().split()
            if actual != oid or kind != 'blob':
                raise ManifestError('Invalid HUD input blob: ' + name)
            self.blobs[name] = stream.read(int(size))
            if stream.read(1) != b'\n':
                raise ManifestError('Invalid HUD input blob framing: ' + name)

    def read(self, name):
        safe_path(name)
        if name not in self.entries:
            raise ManifestError('Missing or untracked HUD input: ' + name)
        entry = self.entries[name]
        if entry and (entry[0] not in ('100644', '100755') or entry[1] != 'blob'):
            raise ManifestError('Unsupported HUD input mode: ' + name)
        if self.revision:
            return self.blobs[name] if name in self.blobs else self.command('cat-file', 'blob', entry[2])
        path = self.root / name
        parents = [self.root.joinpath(*PurePosixPath(name).parts[:index])
                   for index in range(1, len(PurePosixPath(name).parts) + 1)]
        if any(p.is_symlink() for p in parents) or not path.is_file():
            raise ManifestError('Missing or linked HUD input: ' + name)
        return path.read_bytes()


def root_paths(names):
    return sorted(name for name in names if
                  name.startswith(WEB + 'src/')
                  or name in (WEB + 'app.html', WEB + 'index.html'))


def resolver_controls(names, inputs):
    found = set()
    for name in inputs:
        for directory in PurePosixPath(name).parents:
            for control in ('package.json', 'tsconfig.json', 'jsconfig.json'):
                candidate = str(directory / control)
                if candidate in names:
                    found.add(candidate)
    return found


def read_manifest(root, revision=None):
    tree = Tree(root, revision)
    if MANIFEST not in tree.entries:
        raise MissingManifest('Missing checked HUD input manifest' + (': ' + revision if revision else ''))
    try:
        manifest = json.loads(tree.read(MANIFEST))
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        raise ManifestError('Invalid HUD input manifest') from exc
    if (not isinstance(manifest, dict) or set(manifest) != {'schema', 'controls', 'roots', 'files', 'edges'}
            or type(manifest['schema']) is not int or manifest['schema'] != 1):
        raise ManifestError('Unsupported HUD input manifest schema')
    roots = manifest['roots']
    if not isinstance(roots, list) or any(not isinstance(name, str) for name in roots):
        raise ManifestError('Invalid HUD root inventory')
    for name in roots:
        safe_path(name)
    if roots != sorted(set(roots)) or roots != root_paths(roots):
        raise ManifestError('Invalid HUD declared root inventory')
    stale = []
    if not all(isinstance(manifest[group], list) for group in ('controls', 'files')):
        raise ManifestError('Invalid HUD input lists')
    controls = {row.get('path') for row in manifest['controls']
                if isinstance(row, dict) and isinstance(row.get('path'), str)}
    if not set(CONTROLS) <= controls:
        raise ManifestError('Missing HUD resolver controls')
    current_roots = root_paths(tree.entries)
    candidate_inputs = current_roots + [row.get('path') for row in manifest['files'] if isinstance(row, dict) and isinstance(row.get('path'), str)]
    current_controls = resolver_controls(tree.entries, candidate_inputs)
    candidates = set(current_roots) | current_controls
    declared = {row['path'] for group in ('controls', 'files') for row in manifest[group]
                if isinstance(row, dict) and isinstance(row.get('path'), str)}
    tree.prefetch(candidates | declared)
    # A cache miss may rebuild only after every newly discovered input is safe.
    for name in sorted(candidates):
        tree.read(name)
    if roots != current_roots:
        stale.append('HUD root inventory changed; regenerate shared-inputs.json')
    if not current_controls <= controls:
        stale.append('New HUD resolver control; regenerate shared-inputs.json')
    hashes = {}
    for group in ('controls', 'files'):
        rows = manifest[group]
        if not isinstance(rows, list):
            raise ManifestError('Invalid HUD input list: ' + group)
        names = []
        for row in rows:
            if not isinstance(row, dict) or set(row) - {'path', 'sha256', 'lfs'} or not {'path', 'sha256'} <= row.keys():
                raise ManifestError('Invalid HUD input record')
            name = safe_path(row['path'])
            if name == MANIFEST or (group == 'files' and not name.startswith(DOMAINS)):
                raise ManifestError('HUD input outside source domains: ' + name)
            expected = row['sha256']
            if not isinstance(expected, str) or not re.fullmatch('[0-9a-f]{64}', expected):
                raise ManifestError('Invalid HUD input hash: ' + name)
            content = tree.read(name)
            lfs = row.get('lfs')
            if lfs is not None:
                if (not isinstance(lfs, dict) or set(lfs) != {'oid', 'size'} or lfs['oid'] != expected
                        or type(lfs['size']) is not int or lfs['size'] < 0):
                    raise ManifestError('Invalid HUD LFS identity: ' + name)
                pointer = re.fullmatch(rb'version https://git-lfs.github.com/spec/v1\noid sha256:([0-9a-f]{64})\nsize ([0-9]+)\n?', content)
                valid = ((pointer[1].decode() == expected and int(pointer[2]) == lfs['size']) if pointer
                         else sha(content) == expected and len(content) == lfs['size'])
            else:
                valid = not content.startswith(b'version https://git-lfs.github.com/spec/v1\n') and sha(content) == expected
            if not valid:
                stale.append('Stale HUD input: ' + name)
            if name in hashes and hashes[name] != expected:
                raise ManifestError('Conflicting HUD input identity: ' + name)
            hashes[name] = expected
            names.append(name)
        if names != sorted(set(names)):
            raise ManifestError('Unsorted or duplicate HUD input list: ' + group)
    if not set(roots) <= hashes.keys():
        raise ManifestError('HUD root missing from checked files')
    edges = manifest['edges']
    if not isinstance(edges, list):
        raise ManifestError('Invalid HUD edges')
    ordering = []
    for edge in edges:
        if (not isinstance(edge, dict) or set(edge) != {'from', 'to', 'kind'}
                or not isinstance(edge['kind'], str) or not edge['kind']):
            raise ManifestError('Invalid HUD edge')
        if safe_path(edge['from']) not in hashes or safe_path(edge['to']) not in hashes:
            raise ManifestError('Unresolved HUD edge')
        ordering.append((edge['from'], edge['to'], edge['kind']))
    if ordering != sorted(set(ordering)):
        raise ManifestError('Unsorted or duplicate HUD edges')
    if stale:
        raise StaleManifestError(stale[0])
    return manifest


def input_hashes(root, revision=None):
    manifest = read_manifest(root, revision)
    return {row['path']: row['sha256'] for group in ('controls', 'files') for row in manifest[group]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--newer-than', type=Path)
    args = parser.parse_args()
    try:
        paths = input_hashes(args.root)
        paths[MANIFEST] = ''
        if args.newer_than:
            try:
                stamp = args.newer_than.stat()
            except FileNotFoundError:
                raise SystemExit(1)
            if not stat.S_ISREG(stamp.st_mode):
                raise OSError('Native cache stamp is not a regular file')
            if any((args.root / p).stat().st_mtime > stamp.st_mtime for p in paths):
                raise SystemExit(1)
        else:
            print(json.dumps(sorted(paths)))
    except StaleManifestError as error:
        if not args.newer_than:
            raise
        parser.exit(1, str(error) + '\n')
    except (ManifestError, OSError, subprocess.SubprocessError) as error:
        if not args.newer_than:
            raise
        parser.exit(2, str(error) + '\n')


if __name__ == '__main__':
    main()
