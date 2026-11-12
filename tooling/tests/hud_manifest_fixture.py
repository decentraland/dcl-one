"""Synthetic resolved graphs for artifact tests; no source import resolution."""
import hashlib
import importlib.util
import subprocess
from types import SimpleNamespace
import json
from pathlib import Path

_spec = importlib.util.spec_from_file_location('fixture_hud_contract', Path(__file__).parents[1] / 'hud_shared_inputs.py')
_contract = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_contract)

WEB = 'rendering/bevy-explorer/source/react-web/'
MANIFEST = WEB + 'shared-inputs.json'
SHARED = ('library/shop/model/money.ts', 'library/catalyst-api/credits/BalanceOut.ts',
          'library/ui/foundations/index.ts', 'library/ui/foundations/colors/colors.css')
CONTROLS = tuple(WEB + name for name in ('package.json', 'package-lock.json', 'tsconfig.json',
                                       'vite.config.ts', 'vitest.config.ts', 'vite.shared.ts',
                                       'scripts/shared-inputs.mjs')) + ('library/ui/package.json', 'library/ui/foundations/typography/font-source.json')


def seal(root, shared=SHARED):
    root = Path(root)
    for name in CONTROLS:
        path = root / name
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture resolver control')
    roots = sorted(p.relative_to(root).as_posix() for p in (root / WEB).rglob('*') if p.is_file()
                   and 'node_modules' not in p.relative_to(root).parts
                   and (p.relative_to(root).as_posix().startswith(WEB + 'src/')
                        or p.relative_to(root).as_posix() in (WEB + 'app.html', WEB + 'index.html')))
    names = sorted({p for p in shared if (root / p).is_file()})
    rows = [{'path': p, 'sha256': hashlib.sha256((root / p).read_bytes()).hexdigest()} for p in names]
    target = root / MANIFEST
    target.parent.mkdir(parents=True, exist_ok=True)
    controls = [{'path': p, 'sha256': hashlib.sha256((root / p).read_bytes()).hexdigest()} for p in sorted(set(CONTROLS) | set(roots))]
    directories = [{'path': directory, 'entries': _contract.physical_namespace(SimpleNamespace(root=root), directory)}
                   for directory in _contract.resolution_directory_paths(row['path'] for row in rows)]
    target.write_text(json.dumps({'schema': 2, 'controls': controls, 'roots': roots, 'files': rows,
                                 'edges': [], 'resolutionDirectories': directories}, sort_keys=True) + '\n')


def corpus_files(root):
    root = Path(root)
    payload = b'wOF2' + bytes(60)
    oid = hashlib.sha256(payload).hexdigest()
    decimal = 'library/shop/decimal-lfs.woff2'
    pointer = 'library/shop/pointer.woff2'
    for name, content in ((decimal, payload), (pointer, ('version https://git-lfs.github.com/spec/v1\noid sha256:' + oid + '\nsize 64\n').encode())):
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    crlf = 'library/shop/pointer-crlf.woff2'
    (root / crlf).write_bytes((root / pointer).read_bytes().replace(b'\n', b'\r\n'))
    return {'decimal': decimal, 'pointer': pointer, 'crlf': crlf, 'oid': oid, 'size': 64}


def index_corpus_payload(root, files):
    root = Path(root)
    oid = subprocess.check_output(['git', 'hash-object', '-w', str(root / files['pointer'])], cwd=root).decode().strip()
    subprocess.check_output(['git', 'update-index', '--add', '--cacheinfo', '100644', oid, files['decimal']], cwd=root)


def corpus_manifest(baseline, case, root, lfs_files):
    root = Path(root)
    node_module = 'library/shop/node_modules/fixture.ts'
    data = json.loads(json.dumps(baseline))
    row = data['files'][0]
    mutation = case['mutation']
    if mutation == 'null-lfs':
        row['lfs'] = None
    elif mutation in ('extra-lfs-key', 'unsafe-lfs-size'):
        row['lfs'] = {'oid': row['sha256'], 'size': 9007199254740992 if mutation == 'unsafe-lfs-size' else 0}
        if mutation == 'extra-lfs-key':
            row['lfs']['extra'] = True
    elif mutation == 'duplicate-groups':
        data['files'].append(dict(data['controls'][0]))
    elif mutation == 'owned-file':
        owned = next(row for row in data['controls'] if row['path'] == WEB + 'src/main.ts')
        data['controls'].remove(owned)
        data['files'].append(owned)
    elif mutation == 'node-modules-file':
        data['files'].append({'path': node_module, 'sha256': _contract.sha((root / node_module).read_bytes())})
    elif mutation in ('duplicate-edge', 'unsorted-edges'):
        first = {'from': WEB + 'src/main.ts', 'to': data['files'][0]['path'], 'kind': 'import'}
        second = {'from': WEB + 'src/main.ts', 'to': data['files'][0]['path'], 'kind': 'z-import'}
        data['edges'] = [first, first] if mutation == 'duplicate-edge' else [second, first]
    elif mutation in ('integral-decimal', 'fractional-lfs-size', 'boolean-lfs-size', 'unmaterialized-lfs'):
        size = 64.0 if mutation == 'integral-decimal' else 64.5 if mutation == 'fractional-lfs-size' else True if mutation == 'boolean-lfs-size' else 64
        data['schema'] = 2.0 if mutation == 'integral-decimal' else 2
        data['files'].append({'path': lfs_files['pointer'] if mutation == 'unmaterialized-lfs' else lfs_files['decimal'],
                              'sha256': lfs_files['oid'], 'lfs': {'oid': lfs_files['oid'], 'size': size}})
    elif mutation in ('forged-pointer-raw-hash', 'forged-crlf-pointer-lfs', 'wrong-materialized-lfs-size'):
        name = lfs_files['decimal'] if mutation == 'wrong-materialized-lfs-size' else lfs_files['crlf'] if mutation == 'forged-crlf-pointer-lfs' else lfs_files['pointer']
        content = (root / name).read_bytes()
        row = {'path': name, 'sha256': _contract.sha(content)}
        if mutation != 'forged-pointer-raw-hash':
            row['lfs'] = {'oid': row['sha256'], 'size': 32 if mutation == 'wrong-materialized-lfs-size' else len(content)}
        data['files'].append(row)
    elif mutation == 'missing-resolution-scope':
        data['resolutionDirectories'] = []
    elif mutation == 'extra-resolution-scope':
        data['resolutionDirectories'].append({'path': 'library/ui/unused', 'entries': []})
    elif mutation == 'unsorted-namespace':
        data['resolutionDirectories'][0]['entries'].reverse()
    elif mutation == 'duplicate-namespace-name':
        data['resolutionDirectories'][0]['entries'].append(dict(data['resolutionDirectories'][0]['entries'][0]))
    elif mutation == 'unsafe-namespace-name':
        data['resolutionDirectories'][0]['entries'][0]['name'] = '../escape'
    elif mutation == 'control-character-namespace-name':
        data['resolutionDirectories'][0]['entries'][0]['name'] = 'bad\n\nfile'
    elif mutation == 'boolean-schema':
        data['schema'] = True
    elif mutation == 'fractional-schema':
        data['schema'] = 2.5
    elif mutation == 'invalid-namespace-kind':
        data['resolutionDirectories'][0]['entries'][0]['kind'] = 'symlink'
    else:
        raise ValueError('Unknown shared corpus mutation: ' + mutation)
    data['files'].sort(key=lambda row: row['path'])
    return data
