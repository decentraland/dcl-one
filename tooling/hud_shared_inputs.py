"""Validate the checked HUD dependency graph without resolving imports."""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess

WEB = 'rendering/bevy-explorer/source/react-web/'
MANIFEST = WEB + 'shared-inputs.json'
REGENERATE_COMMAND = 'npm --prefix ' + WEB.rstrip('/') + ' run gen:shared-inputs'
DOMAINS = ('library/ui/', 'library/shop/', 'library/catalyst-api/')
RESOLVER_FILES = ('package.json', 'tsconfig.json', 'jsconfig.json')
CONTROLS = tuple(WEB + name for name in ('package.json', 'package-lock.json', 'tsconfig.json',
                                       'vite.config.ts', 'vitest.config.ts', 'vite.shared.ts',
                                       'scripts/shared-inputs.mjs')) + ('library/ui/package.json', 'library/ui/foundations/typography/font-source.json')


class ManifestError(ValueError):
    pass


class StaleManifestError(ManifestError):
    def __init__(self, message, native_content_paths=None):
        super().__init__(message)
        self.native_content_paths = native_content_paths


class MissingManifest(ManifestError):
    pass


def validation_message(error):
    message = str(error)
    if REGENERATE_COMMAND in message:
        return message
    return message + '; repair invalid inputs, then run ' + REGENERATE_COMMAND + ' from the repository root'


def safe_integer(value):
    if type(value) is int:
        return 0 <= value <= 9007199254740991
    return (type(value) is float and 0 <= value <= 9007199254740991
            and math.isfinite(value) and value == math.floor(value))


def reject_json_constant(value):
    raise ManifestError('Invalid HUD JSON numeric constant: ' + value)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def safe_path(value):
    if (not isinstance(value, str) or not value or '\\' in value or any(ord(c) < 32 for c in value)
            or str(PurePosixPath(value)) != value or PurePosixPath(value).is_absolute()
            or any(part in ('.', '..') for part in value.split('/'))
            or any(0xD800 <= ord(c) <= 0xDFFF for c in value)):
        raise ManifestError('Unsafe HUD input path: ' + repr(value))
    return value


class Tree:
    def __init__(self, root, revision=None):
        self.root, self.revision = Path(root).resolve(), revision
        self.git = (self.root / '.git').exists()
        self.entries = {}
        self.blobs = {}
        self.unmerged = set()
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
                        self.unmerged.add(name.decode())
                        self.entries[name.decode()] = (mode, 'unmerged', oid)
                    else:
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

    def check_regular(self, name, allow_missing=False):
        safe_path(name)
        if name in self.unmerged:
            raise ManifestError('Unmerged HUD input: ' + name)
        tracked = name in self.entries
        if not tracked and not allow_missing:
            raise ManifestError('Missing or untracked HUD input: ' + name)
        entry = self.entries.get(name)
        if entry and (entry[0] not in ('100644', '100755') or entry[1] != 'blob'):
            raise ManifestError('Unsupported HUD input mode: ' + name)
        if self.revision:
            return tracked
        parts = PurePosixPath(name).parts
        for index in range(1, len(parts) + 1):
            path = self.root.joinpath(*parts[:index])
            try:
                mode = path.lstat().st_mode
            except FileNotFoundError:
                if allow_missing:
                    return False
                raise ManifestError('Missing or linked HUD input: ' + name)
            if stat.S_ISLNK(mode) or (index < len(parts) and not stat.S_ISDIR(mode)):
                raise ManifestError('Missing or linked HUD input: ' + name)
            if index == len(parts) and not stat.S_ISREG(mode):
                raise ManifestError('Non-regular HUD input: ' + name)
        if not tracked:
            raise ManifestError('Missing or untracked HUD input: ' + name)
        return True

    def read(self, name):
        self.check_regular(name)
        if self.revision:
            return self.blobs[name] if name in self.blobs else self.command('cat-file', 'blob', self.entries[name][2])
        return (self.root / name).read_bytes()


def root_paths(names):
    return sorted(name for name in names if
                  name.startswith(WEB + 'src/')
                  or name in (WEB + 'app.html', WEB + 'index.html'))


def resolver_controls(names, inputs):
    found = set()
    for name in inputs:
        for directory in PurePosixPath(name).parents:
            for control in RESOLVER_FILES:
                candidate = str(directory / control)
                if candidate in names:
                    found.add(candidate)
    return found


def namespace_excluded(name, kind):
    return name in ('node_modules', '.git', '.agent-work') or (kind == 'directory' and name.startswith(('.consumer-foundations-', '.consumer-ssr-')))


def resolution_directory_paths(files):
    directories = set()
    for name in files:
        domain = next((prefix.rstrip('/') for prefix in DOMAINS if name.startswith(prefix)), None)
        if domain is None:
            raise ManifestError('Unsupported resolution namespace source: ' + name)
        directory = PurePosixPath(name).parent
        while True:
            if any(namespace_excluded(part, 'directory') for part in directory.parts):
                raise ManifestError('Unsupported resolution namespace: ' + str(directory))
            directories.add(str(directory))
            if str(directory) == domain:
                break
            directory = directory.parent
    return sorted(directories)


def indexed_namespace(tree, directory):
    entries = {}
    for name, entry in tree.entries.items():
        if not name.startswith(directory + '/'):
            continue
        tail = name[len(directory) + 1:]
        part = tail.split('/')[0]
        kind = 'directory' if '/' in tail else 'regular'
        if namespace_excluded(part, kind):
            continue
        if '/' not in tail and entry and (entry[0] not in ('100644', '100755') or entry[1] not in ('blob', 'unmerged')):
            raise ManifestError('Unsafe resolution namespace mode: ' + name)
        entries[part] = 'directory' if '/' in tail else 'regular'
    return [{'name': name, 'kind': entries[name]} for name in sorted(entries)]


def physical_namespace(tree, directory):
    safe_path(directory)
    current = tree.root
    for part in PurePosixPath(directory).parts:
        current /= part
        mode = current.lstat().st_mode
        if not stat.S_ISDIR(mode) or stat.S_ISLNK(mode):
            raise ManifestError('Unsafe resolution directory: ' + directory)
    entries = []
    for path in current.iterdir():
        mode = path.lstat().st_mode
        if namespace_excluded(path.name, 'directory' if stat.S_ISDIR(mode) else 'regular'):
            continue
        safe_path(path.name)
        if stat.S_ISLNK(mode) or not (stat.S_ISDIR(mode) or stat.S_ISREG(mode)):
            raise ManifestError('Unsafe resolution namespace entry: ' + str(path))
        entries.append({'name': path.name, 'kind': 'directory' if stat.S_ISDIR(mode) else 'regular'})
    return sorted(entries, key=lambda entry: entry['name'])


def namespace_subset(actual, expected):
    return all(entry in expected for entry in actual)


def validate_resolution_directories(tree, manifest, stale):
    directories = manifest['resolutionDirectories']
    expected = resolution_directory_paths(row['path'] for row in manifest['files'])
    if (not isinstance(directories, list) or any(not isinstance(row, dict) or set(row) != {'path', 'entries'} for row in directories)
            or [row['path'] for row in directories] != expected):
        raise ManifestError('Invalid HUD resolution directory scope')
    for row in directories:
        entries = row['entries']
        if not isinstance(entries, list):
            raise ManifestError('Invalid HUD resolution namespace')
        names = []
        for entry in entries:
            if not isinstance(entry, dict) or set(entry) != {'name', 'kind'}:
                raise ManifestError('Invalid HUD resolution namespace entry')
            name = safe_path(entry['name'])
            if '/' in name or namespace_excluded(name, entry['kind']) or entry['kind'] not in ('regular', 'directory'):
                raise ManifestError('Invalid HUD resolution namespace entry')
            names.append(name)
        if names != sorted(set(names)):
            raise ManifestError('Unsorted HUD resolution namespace')
        if tree.git or tree.revision:
            indexed = indexed_namespace(tree, row['path'])
            if indexed != entries:
                stale.append('Stale HUD resolution namespace: ' + row['path'])
            if not tree.revision:
                actual = physical_namespace(tree, row['path'])
                if not namespace_subset(actual, indexed):
                    raise ManifestError('Untracked or unsafe HUD resolution namespace entry: ' + row['path'])
                if actual != indexed:
                    stale.append('Missing HUD resolution namespace entry: ' + row['path'])
        elif not namespace_subset(physical_namespace(tree, row['path']), entries):
            raise ManifestError('Unexpected filtered HUD resolution namespace entry: ' + row['path'])


def read_manifest(root, revision=None):
    tree = Tree(root, revision)
    if MANIFEST not in tree.entries:
        raise MissingManifest('Missing checked HUD input manifest' + (': ' + revision if revision else ''))
    try:
        manifest = json.loads(tree.read(MANIFEST), parse_constant=reject_json_constant)
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        raise ManifestError('Invalid HUD input manifest') from exc
    if (not isinstance(manifest, dict) or set(manifest) != {'schema', 'controls', 'roots', 'files', 'edges', 'resolutionDirectories'}
            or not safe_integer(manifest['schema']) or manifest['schema'] != 2):
        raise ManifestError('Unsupported HUD input manifest schema')
    roots = manifest['roots']
    if not isinstance(roots, list) or any(not isinstance(name, str) for name in roots):
        raise ManifestError('Invalid HUD root inventory')
    for name in roots:
        safe_path(name)
    if roots != sorted(set(roots)) or roots != root_paths(roots):
        raise ManifestError('Invalid HUD declared root inventory')
    stale = []
    native_content_changes = set()
    if not all(isinstance(manifest[group], list) for group in ('controls', 'files')):
        raise ManifestError('Invalid HUD input lists')
    controls = {row.get('path') for row in manifest['controls']
                if isinstance(row, dict) and isinstance(row.get('path'), str)}
    if not set(CONTROLS) <= controls:
        raise ManifestError('Missing HUD resolver controls')
    current_roots = root_paths(tree.entries)
    candidate_inputs = current_roots + [row.get('path') for group in ('controls', 'files')
                                       for row in manifest[group] if isinstance(row, dict) and isinstance(row.get('path'), str)]
    current_controls = resolver_controls(tree.entries, candidate_inputs)
    candidates = set(current_roots) | current_controls
    declared = {row['path'] for group in ('controls', 'files') for row in manifest[group]
                if isinstance(row, dict) and isinstance(row.get('path'), str)}
    tree.prefetch(candidates | declared)
    # A cache miss may rebuild only after every newly discovered input is safe.
    missing_owned = set()
    owned_roots = {name for name in roots if PurePosixPath(name).name not in RESOLVER_FILES}
    for name in sorted(candidates | owned_roots):
        if name in owned_roots and not tree.check_regular(name, allow_missing=True):
            missing_owned.add(name)
            continue
        tree.read(name)
    if missing_owned:
        stale.append('Removed owned HUD root; regenerate shared-inputs.json')
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
            if name == MANIFEST or (group == 'files' and
                                    (not name.startswith(DOMAINS) or 'node_modules' in PurePosixPath(name).parts)):
                raise ManifestError('HUD input outside source domains: ' + name)
            expected = row['sha256']
            if not isinstance(expected, str) or not re.fullmatch('[0-9a-f]{64}', expected):
                raise ManifestError('Invalid HUD input hash: ' + name)
            lfs = row.get('lfs')
            if 'lfs' in row:
                if (not isinstance(lfs, dict) or set(lfs) != {'oid', 'size'} or lfs['oid'] != expected
                        or not safe_integer(lfs['size'])):
                    raise ManifestError('Invalid HUD LFS identity: ' + name)
            if name in missing_owned:
                valid = False
            else:
                content = tree.read(name)
                is_pointer = content.startswith(b'version https://git-lfs.github.com/spec/')
                if is_pointer and not tree.revision:
                    raise ManifestError('Unmaterialized HUD LFS input: ' + name)
                if lfs is not None:
                    pointer = re.fullmatch(rb'version https://git-lfs.github.com/spec/v1\noid sha256:([0-9a-f]{64})\nsize ([0-9]+)\n?', content)
                    if is_pointer and pointer is None:
                        raise ManifestError('Invalid historical HUD LFS pointer: ' + name)
                    valid = ((pointer[1].decode() == expected and int(pointer[2]) == lfs['size']) if pointer
                             else sha(content) == expected and len(content) == lfs['size'])
                else:
                    valid = not is_pointer and sha(content) == expected
            if not valid:
                stale.append('Stale HUD input: ' + name)
                if (group == 'controls' and name.startswith(WEB + 'src/') and lfs is None
                        and name not in missing_owned and PurePosixPath(name).name not in RESOLVER_FILES):
                    native_content_changes.add(name)
            if name in hashes:
                raise ManifestError('Duplicate HUD input identity: ' + name)
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
    validate_resolution_directories(tree, manifest, stale)
    if stale:
        content_paths = tuple(hashes) if len(native_content_changes) == len(stale) else None
        raise StaleManifestError(validation_message(stale[0]), content_paths)
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
        try:
            paths = input_hashes(args.root)
        except StaleManifestError as error:
            if not args.newer_than or error.native_content_paths is None:
                raise
            # Local owned-body edits use the successful bundle stamp; strict callers still refuse them.
            paths = dict.fromkeys(error.native_content_paths)
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
        parser.exit(1, validation_message(error) + '\n')
    except (ManifestError, OSError, subprocess.SubprocessError) as error:
        parser.exit(2 if args.newer_than else 1, validation_message(error) + '\n')


if __name__ == '__main__':
    main()
