"""What of the install tree ships, and where the built pieces land.

Imported by `build-base-blob.py`, never run. The install tree is the tree's
own `@dcl/*` packages (`library/js-sdk-toolchain`) plus the registry extras
pinned in `src/vendor/registry`; the evidence for every prune is in that
script's module docstring, and this module is the policy it describes - the
entry points, the per-package allowlists, the drop lists - and the
reachability walk (`reachable()` -> `collect()`) that turns them into the
file set, plus the resolver check (`resolvable()`) that proves the set is
closed under its own imports: node's for the `.js`, tsc's for the
declarations, which may also land on a module the rollup declares
(`ambient_modules()`). The layout constants at the top are where the two
built products land; `blob_overlays.py` reads them to verify the chunks.
"""
from __future__ import annotations

import json
import os
import re

PREBUILT_DIR = 'node_modules/@dcl/sdk/prebuilt'
CORE_CHUNK = f'{PREBUILT_DIR}/core.js'
SMART_CHUNK = f'{PREBUILT_DIR}/smart.js'
CHUNK_REGISTRY = f'{PREBUILT_DIR}/registry.json'
TYPES_ROLLUP = 'node_modules/@dcl/js-runtime/index.d.ts'

ENTRY_PACKAGES = ['@dcl/sdk', '@dcl/js-runtime', '@dcl/ecs', '@dcl/smart-items',
                  '@dcl/inspector-service', 'typescript']

ENTRY_REASON = {
    '@dcl/sdk': 'types/tsconfig.ecs7.json + the prebuilt runtime chunks',
    '@dcl/js-runtime': 'scene ambient types (the rolled-up .d.ts)',
    '@dcl/ecs': "manifest only: the runtime is inside the chunks and the service "
                'bundle, the types inside the rollup',
    '@dcl/smart-items': "dist/**/*.d.ts: the declarations tsconfig.ecs7.json's paths "
                        'and scene imports resolve',
    '@dcl/inspector-service': 'dist/index.js: the data-layer host, its rpc server, the '
                              'composite dumpers and the @dcl/ecs they run on, which '
                              'data-layer-host.mjs and code-to-composite.mjs load',
    'typescript': 'build --production type check',
}

# `@dcl/inspector` is the editor UI: 119 MB, served from `DCL_ONE_INSPECTOR_DIR`
# when configured, never from the blob.
NEVER = {'@dcl/sdk-commands', '@dcl/explorer', '@dcl/inspector'}

TS_LIB_RE = re.compile(
    r'^lib/lib\.(?:es5|es20\d\d|esnext|decorators)(?:\.(?!full\b)[a-z0-9.]+)?\.d\.ts$')

FILE_ALLOWLIST = {
    'typescript': lambda rel: rel in ('package.json', 'LICENSE.txt')
    or rel in ('lib/tsc.js', 'lib/_tsc.js')
    or bool(TS_LIB_RE.match(rel)),
    '@dcl/sdk': lambda rel: rel in ('package.json', 'LICENSE')
    or (rel.startswith('types/') and rel.endswith('.json')),
    '@dcl/js-runtime': lambda rel: rel in ('package.json', 'LICENSE'),
    '@dcl/ecs': lambda rel: rel in ('package.json', 'LICENSE'),
    '@dcl/smart-items': lambda rel: rel in ('package.json', 'LICENSE')
    or (rel.startswith('dist/') and rel.endswith('.d.ts')),
    '@dcl/inspector-service': lambda rel: rel in ('package.json', 'LICENSE', 'dist/index.js'),
}

# In the install tree for the chunk build and the declaration rollup, never in
# the blob: their code is inside the chunks (and the service bundle), their
# declarations inside the rollup. A shipped manifest's dependency on one is
# dropped (`UNSHIPPED` in build-base-blob.py).
DROP_PACKAGES = {
    '@dcl/react-ecs', '@dcl/ecs-math',
    'react', 'react-reconciler', 'scheduler',
    'loose-envify', 'js-tokens',
    'protobufjs', 'long', '@protobufjs/utf8', 'mitt',
}

NODE_BUILTINS = {
    'assert', 'async_hooks', 'buffer', 'child_process', 'cluster', 'console',
    'constants', 'crypto', 'dgram', 'diagnostics_channel', 'dns', 'domain',
    'events', 'fs', 'http', 'http2', 'https', 'inspector', 'module', 'net',
    'os', 'path', 'perf_hooks', 'process', 'punycode', 'querystring',
    'readline', 'repl', 'stream', 'string_decoder', 'timers', 'tls',
    'trace_events', 'tty', 'url', 'util', 'v8', 'vm', 'wasi', 'worker_threads',
    'zlib',
}

VIRTUAL_PREFIXES = ('~sdk/', '~system/')

# Required inside a try/catch by the service bundle: ws's native accelerators and
# node-fetch's `encoding`, the only packages its build leaves external.
OPTIONAL_SPECS = {'bufferutil', 'utf-8-validate', 'encoding'}

DROP_SUFFIXES = ('.map', '.md', '.markdown', '.flow')
DECLARATION_SUFFIXES = ('.d.ts', '.d.cts', '.d.mts')
DROP_DIRS = {
    'test', 'tests', '__tests__', 'docs', 'example', 'examples',
    'bench', 'benchmark', 'benchmarks', 'spec', '.github',
}
DROP_FILES = {'eslint.config.js', '.eslintrc.js', '.eslintrc.cjs', 'karma.conf.js', '.npmignore'}
DROP_FILE_RE = re.compile(r'(^|\.)(test|tests|spec)(-d)?\.(js|cjs|mjs|ts)$')

SPEC_RE = re.compile(
    r'''(?:require\(\s*|(?:^|[\s;{}])(?:import|export)[^'"()]{0,200}?from\s*|import\(\s*)'''
    r'''['"]([^'"\n]+)['"]''',
    re.M,
)
REF_TYPES_RE = re.compile(r'///\s*<reference\s+types\s*=\s*"([^"]+)"')

BLOCK_COMMENT_RE = re.compile(r'/\*[\s\S]*?\*/')
LINE_COMMENT_RE = re.compile(r'^[ \t]*//.*$', re.M)

def strip_comments(text: str) -> str:
    return LINE_COMMENT_RE.sub('', BLOCK_COMMENT_RE.sub('', text))

def scan(text: str) -> set[str]:
    """Bare/deep specifiers and `/// <reference types>` names in one file.

    A reference-types name is tagged `types:x` rather than turned into a
    specifier, because tsc satisfies it from *either* `@types/x` or a package
    literally named `x` - `@dcl/js-runtime` is the latter.
    """
    found = {m.group(1) for m in SPEC_RE.finditer(strip_comments(text))}
    found |= {f'types:{m.group(1)}' for m in REF_TYPES_RE.finditer(text)}
    return found

def types_candidates(spec: str) -> list[str]:
    name = spec[len('types:'):]
    return [name, f'@types/{name}']

SCAN_SKIP_DIRS = {'node_modules'}
SCAN_SUFFIXES = ('.js', '.cjs', '.mjs', '.ts', '.tsx', '.d.ts', '.d.cts', '.d.mts')

def log(msg: str) -> None:
    print(msg, flush=True)

def pkg_of_spec(spec: str) -> str | None:
    if not spec or spec[0] in './' or spec.startswith('node:'):
        return None
    parts = spec.split('/')
    return f'{parts[0]}/{parts[1]}' if parts[0].startswith('@') else parts[0]

def list_packages(nm: str) -> list[str]:
    out = []
    for name in sorted(os.listdir(nm)):
        if name.startswith('.'):
            continue
        full = os.path.join(nm, name)
        if name.startswith('@') and os.path.isdir(full):
            out += [f'{name}/{s}' for s in sorted(os.listdir(full))]
        elif os.path.isdir(full):
            out.append(name)
    return out

def wanted(pkg: str, rel: str) -> bool:
    allow = FILE_ALLOWLIST.get(pkg)
    if allow is not None:
        return allow(rel)
    if rel.endswith(DROP_SUFFIXES):
        return False
    parts = rel.split('/')
    if parts[-1] in DROP_FILES or DROP_FILE_RE.search(parts[-1]):
        return False
    return not any(p in DROP_DIRS for p in parts[:-1])

def specifiers_in(nm: str, pkg: str) -> set[str]:
    """Bare/deep imports of the files this package will actually ship.

    Declarations count: `build --production` type-checks the scene, and tsc
    follows `.d.ts` imports into other packages exactly like node follows
    `require`. A package reached only from a `.d.ts` still has to be there.
    """
    found: set[str] = set()
    root = os.path.join(nm, pkg)
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SCAN_SKIP_DIRS]
        for fn in filenames:
            if not fn.endswith(SCAN_SUFFIXES):
                continue
            path = os.path.join(dirpath, fn)
            rel = os.path.relpath(path, root).replace(os.sep, '/')
            if os.path.islink(path) or not wanted(pkg, rel):
                continue
            with open(path, encoding='utf-8', errors='ignore') as fh:
                found |= scan(fh.read())
    return found

def deps_of_spec(spec: str) -> list[str]:
    if spec.startswith('types:'):
        return types_candidates(spec)
    pkg = pkg_of_spec(spec)
    return [] if pkg is None or pkg in NODE_BUILTINS else [pkg]

def reachable(nm: str, present: set[str]) -> tuple[set[str], dict[str, str]]:
    keep = {p for p in ENTRY_PACKAGES if p in present}
    why = {p: ENTRY_REASON.get(p, 'entry point') for p in keep}
    queue = list(keep)
    while queue:
        pkg = queue.pop()
        for spec in specifiers_in(nm, pkg):
            for dep in deps_of_spec(spec):
                if (
                    dep in present
                    and dep not in keep
                    and dep not in NEVER
                    and dep not in DROP_PACKAGES
                ):
                    keep.add(dep)
                    why[dep] = f'{pkg} imports {spec!r}'
                    queue.append(dep)
    return keep, why

def collect(nm: str, keep: set[str]) -> tuple[dict[str, bytes], dict[str, int]]:
    files: dict[str, bytes] = {}
    kept_bytes: dict[str, int] = {}
    for pkg in sorted(keep):
        root = os.path.join(nm, pkg)
        if not os.path.isdir(root):
            continue
        total = 0
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d != 'node_modules']
            for fn in filenames:
                src = os.path.join(dirpath, fn)
                if os.path.islink(src):
                    continue
                rel = os.path.relpath(src, root).replace(os.sep, '/')
                if not wanted(pkg, rel):
                    continue
                with open(src, 'rb') as fh:
                    data = fh.read()
                files[f'node_modules/{pkg}/{rel}'] = data
                total += len(data)
        kept_bytes[pkg] = total
    return files, kept_bytes

def exports_target(files: dict[str, bytes], pkg: str, sub: str) -> str | None:
    """The file `require('<pkg>/<sub>')` lands on through the package's `exports` map.

    None when the package has no map or the map has no entry for the subpath;
    conditions are taken in the map's own order, `require`, `node` or
    `default`, the way node resolves a `require`.
    """
    manifest = files.get(f'node_modules/{pkg}/package.json')
    if manifest is None:
        return None
    target = json.loads(manifest).get('exports')
    target = target.get(f'./{sub}') if isinstance(target, dict) else None
    while isinstance(target, dict):
        target = next((v for k, v in target.items()
                       if k in ('require', 'node', 'default')), None)
    if not isinstance(target, str):
        return None
    return f'node_modules/{pkg}/{os.path.normpath(target)}'

def resolvable(spec: str, files: dict[str, bytes]) -> bool:
    if spec.startswith(VIRTUAL_PREFIXES):
        return True
    if spec.startswith('types:'):
        return any(
            any(f.startswith(f'node_modules/{c}/') for f in files)
            for c in types_candidates(spec)
        )
    pkg = pkg_of_spec(spec)
    if pkg is None:
        return True
    if pkg in NODE_BUILTINS or spec in OPTIONAL_SPECS:
        return True
    prefix = f'node_modules/{spec}'
    if spec == pkg:
        return any(f.startswith(f'node_modules/{pkg}/') for f in files)
    target = exports_target(files, pkg, spec[len(pkg) + 1:])
    if target is not None:
        return target in files
    return any(
        f == prefix
        or f in (f'{prefix}.js', f'{prefix}.cjs', f'{prefix}.mjs',
                 f'{prefix}.json', f'{prefix}.d.ts')
        or f.startswith(f'{prefix}/')
        for f in files
    )

SCAN_EXEMPT = {
    'node_modules/typescript/lib/_tsc.js',
    CORE_CHUNK,
    SMART_CHUNK,
    TYPES_ROLLUP,
}

# Specifiers a file carries as text, not as modules it loads: the service bundle's
# scene script template (`src/lib/sdk/script-templates.ts`) imports the SDK the way a
# scene does, and the bundled ajv names its runtime helpers in the code it would
# emit for standalone validators (`.code = 'require("ajv/dist/runtime/...")'`).
SCAN_EMBEDDED = {
    'node_modules/@dcl/inspector-service/dist/index.js': {
        '@dcl/sdk/ecs', '@dcl/sdk/math',
        'ajv/dist/runtime/parseJson', 'ajv/dist/runtime/quote',
        'ajv/dist/runtime/timestamp', 'ajv/dist/runtime/uri',
        'ajv/dist/runtime/validation_error',
    },
}

RUNTIME_SUFFIXES = ('.js', '.cjs', '.mjs')
AMBIENT_RE = re.compile(r'^declare module ["\']([^"\']+)["\']', re.M)

def specifiers_in_files(files: dict[str, bytes]) -> tuple[set[str], set[str]]:
    """Bare specifiers of the kept files: node's (`.js`) and tsc's (declarations)."""
    runtime: set[str] = set()
    declared: set[str] = set()
    for name, data in files.items():
        if not name.endswith(SCAN_SUFFIXES) or name in SCAN_EXEMPT:
            continue
        found = scan(data.decode('utf-8', 'ignore')) - SCAN_EMBEDDED.get(name, set())
        (runtime if name.endswith(RUNTIME_SUFFIXES) else declared).update(found)
    return runtime, declared

def ambient_modules(rollup: bytes) -> set[str]:
    """The module names the declaration rollup declares; a shipped `.d.ts` may
    import one of them where node would find no file (`@dcl/sdk/math` from
    `@dcl/smart-items`'s declarations)."""
    return set(AMBIENT_RE.findall(rollup.decode('utf-8', 'ignore')))
