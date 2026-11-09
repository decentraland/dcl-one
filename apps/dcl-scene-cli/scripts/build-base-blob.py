#!/usr/bin/env python3
"""Build `src/vendor/node_modules.zip` - the offline scene toolchain.

`init` extracts this blob so a scaffolded scene bundles, type-checks and
previews with no package manager and no network. It is pure JS - no platform
binaries - so one blob serves linux, macOS and Windows.

Where everything in it comes from:

* Every `@dcl/*` package this tree builds is this tree's build
  (`TREE_PACKAGES`): `@dcl/sdk`, `@dcl/ecs`, `@dcl/js-runtime`,
  `@dcl/react-ecs`, `@dcl/smart-items` (the smart-item runtime) and
  `@dcl/inspector-service` (the data-layer host and the composite readers,
  without the editor UI), all from `library/js-sdk-toolchain` (`make -C
  library/js-sdk-toolchain install build`), whose components and scene APIs
  are generated from `library/protocol`. Each package is packed the way npm
  would publish it (`pnpm pack`, so `files` and ignore rules apply) and
  stamped with the scaffold's `@dcl/sdk` pin
  (`src/templates/init/scene/package.json`, the one place the version is
  written); every `workspace:` dependency between them is rewritten to that
  version, and a dependency on a package the blob never carries
  (`UNSHIPPED`: `@dcl/sdk-commands`, and `@dcl/react-ecs` and
  `@dcl/ecs-math`, whose code is inside the chunks and the rollup) is
  dropped from the shipped manifest. `pack_tree_packages()`.

  The pin is a `-dcl-one.N` prerelease the registry does not serve. The rule
  is the one `pinProtocol` in `library/js-sdk-toolchain/scripts/prepare.spec.ts`
  applies: a packed manifest never sends a consumer to the registry for a
  version that is not there, so the blob names that version only for a
  package it ships itself - which is why the unshipped dependencies above
  are dropped rather than stamped.
* The one registry package the blob ships is `typescript`, the scaffold's
  pin. The registry manifest `src/vendor/registry/package.json` names it as
  its dependency and, as devDependencies, what the chunk build and the
  declaration rollup resolve from the tree packages' own dependencies:
  `react` and `react-reconciler` (`@dcl/react-ecs`), `@dcl/ecs-math`,
  `protobufjs`, `long` and `@protobufjs/utf8` (`@dcl/ecs`) and `mitt`
  (`@dcl/smart-items`). Their code ends inside the chunks, their
  declarations inside the rollup, and no file of theirs ships
  (`DROP_PACKAGES`). `registry/pnpm-lock.yaml` pins the set; it is installed
  with `pnpm install --frozen-lockfile` (hoisted, no scripts, no peer
  auto-install: the workspace file beside it records that last setting, so
  a `pnpm install --lockfile-only` there writes a lock the frozen install
  accepts). The registry repeats pins the tree's manifests own:
  `install_registry()` checks every exact `dependencies` version of the
  packed packages and `typescript` against the scaffold, so a bump in the
  tree fails here naming the manifest to align.

The blob does NOT ship the SDK as source. It ships the *build products* that
source was only ever used to derive:

* two prebuilt runtime chunks, `@dcl/sdk/prebuilt/{core,smart}.js`, and
* one rolled-up ambient declaration file, `@dcl/js-runtime/index.d.ts`.

**Why the chunks.** The SDK runtime chunk is scene-independent. Two scenes -
one importing three `@dcl/ecs` symbols, one importing react-ecs UI + tweens +
audio + animator + players + network - produce byte-identical
`bin/sdk-runtime.js`; it is keyed only on which `@dcl/*` packages are
installed, never on what the scene imports. So shipping 3.6 MB of `.js` would
serve one purpose: letting rolldown re-derive the same artifact on every
build. It is built here once instead, by `dcl-scene-cli vendor-chunks` (a
hidden subcommand over `src/prebuilt.rs`), which is the only place either
chunk is ever produced.

The split into *two* chunks keeps the smart-item runtime (`@dcl/smart-items`,
about 30% of the SDK's runtime bytes) out of every scene that has no smart
item. The smart chunk is its own file, and `src/build.rs` installs it only
when `Project::is_editor_scene()` is true or the built scene chunk actually
names `@dcl/smart-items` or its legacy name `@dcl/asset-packs` (a scene can
boot the runtime from its own source with no composite at all, so the
composite test alone is not enough).

Two registry keys exist for the split: `@dcl/sdk/platform` and
`@dcl/sdk/text-codec`. The smart-item runtime requires both, and because it
lives in its own chunk those requires cross a chunk boundary.
`verify_requires()` in `src/prebuilt.rs` confirms every external require of
the smart chunk is a key the core chunk publishes or `~system/*`.

**Why the rolled-up types.** `build --production` type-checks the scene, so the
declarations have to be there even though the implementations do not. Rolling
`@dcl/{sdk,ecs,react-ecs,ecs-math,js-runtime}` plus the `protobufjs`, `long`
and `mitt` declarations they reach into ONE ambient `.d.ts` replaces those
`.d.ts` trees and, more to the point, lets the `.js` beside them go. See
`build_types_rollup()` for the six transforms this forces and why each is
mandatory. Measured when the rollup was introduced: all 60 sdk7-test-scenes
type-checked to byte-identical diagnostics under the rolled-up types and
under their own `node_modules` (55 clean under both, 5 failing identically
for unrelated missing third-party deps).

An ambient `declare module "X"` wins over a real `node_modules/X` whose `types`
entry does not resolve - which is what lets `@dcl/ecs` stay on disk as a
manifest (`@dcl/sdk` depends on it at the scaffold pin, which only the blob
serves) while its types come from the rollup. `mitt` is rolled up too, with
no files on disk: `@dcl/smart-items`' shipped declarations import it
(`ROLLUP_SEED_DEP_PKGS`).

`@dcl/smart-items`' declarations ship as files (`dist/**/*.d.ts`), not in the
rollup: `tsconfig.ecs7.json` maps `@dcl/asset-packs` onto
`../../smart-items/dist/index` through `paths`, and a scene's
`@dcl/smart-items/<module>` import resolves through the package's `exports`.
Their own imports of `@dcl/ecs`, `@dcl/react-ecs` and `@dcl/sdk/math` fall to
the rollup's ambient modules, the way a scene's do; the resolver check
accepts a declaration import that lands on one (`ambient_modules()`).

What gets dropped, and the evidence for each:

* The `.js` *and* `.d.ts` of `@dcl/sdk`, `@dcl/react-ecs`, `@dcl/ecs-math`,
  `react`, `react-reconciler`, `scheduler` and `@dcl/ecs/dist`. Every byte is
  already inside the two prebuilt chunks (runtime) or the rolled-up
  declaration file (types). `@dcl/sdk` keeps only its manifest and
  `types/*.json`: every scene's tsconfig extends
  `@dcl/sdk/types/tsconfig.ecs7.json`, and rolldown reads it too.
* `@dcl/smart-items` down to its manifest and `dist/**/*.d.ts`: the runtime
  is inside `prebuilt/smart.js`.
* `@dcl/inspector-service` down to its manifest and `dist/index.js`: one
  self-contained esbuild bundle of the data-layer host, its WebSocket rpc
  server (`serveDataLayer`), the engine context, the composite dumpers, the
  legacy composite rename and the `@dcl/ecs` they run on, exported as `ecs`
  (`src/lib/ecs.ts` in the package). Its build leaves nothing external but
  node's own modules and three optional natives required in a try/catch
  (`bufferutil`, `utf-8-validate`, `encoding`: `OPTIONAL_SPECS`), so no
  runtime package ships beside it. Its `dist/lib/**/*.d.ts` describe the
  editor's API, which nothing here type-checks against.
* `@dcl/ecs/dist-cjs`. `templates/code-to-composite.mjs`, the crdt dumper in
  `templates/data-layer-host.mjs` and the goldens' runtime need the CRDT wire
  codec in node, outside any chunk; they take it from the service bundle's
  `ecs`, the one copy the host's engines are made of. `@dcl/ecs` keeps its
  manifest so `@dcl/sdk`'s dependency on it resolves in the extracted tree.
* `@dcl/sdk-commands`, `@dcl/explorer` and `@dcl/inspector`, should any ever
  reach the install tree (`NEVER` in `blob_collect.py`): the Rust CLI is the
  first's replacement, the editor UI is served from `DCL_ONE_INSPECTOR_DIR`,
  and `~sdk/script-utils` is generated per scene over
  `@dcl/smart-items/scripts`, which the smart chunk carries.
* `protobufjs`, `long`, `@protobufjs/utf8` and `mitt`, whole: `@dcl/ecs`'s
  and `@dcl/smart-items`' dependencies, in the install tree for the chunk
  build and the rollup. Their code is inside the chunks and the service
  bundle, their declarations inside the rollup (`protobufjs` where
  `@dcl/ecs/dist/*.gen.d.ts` reach it, `mitt` for `@dcl/smart-items`'
  shipped declarations), and the two shipped manifests drop them
  (`UNSHIPPED`). A scene that imports one itself declares it in its own
  manifest, which the scaffold does not.

**The data-layer host.** The visual editor talks to a *data-layer host* over
a WebSocket: an RPC service carrying the engine's CRDT stream in both
directions. `@dcl/inspector-service` is the tree's host - upstream's
`@dcl/inspector` carries the same code under 119 MB of browser UI, which
ships separately as the editor bundle `DCL_ONE_INSPECTOR_DIR` names. `req()`
in `templates/data-layer-host.mjs` loads the configured editor's host when
one is configured and the scene's `@dcl/inspector-service` otherwise. The
wire protocol the host and the editor share (`@dcl/rpc`: port multiplexing,
framing, bidirectional streams) and the transport (`ws`) are inside the
service bundle, which serves itself (`serveDataLayer`); an editor's host
without that export is served by the driver from the `@dcl/rpc` and `ws`
beside it.

Pre-existing prunes, with their evidence:

* `typescript/lib/typescript.js` (8.7 MB), `typescript.d.ts` (0.6 MB),
  `_tsserver.js`, `tsserverlibrary.*`, `typingsInstaller.js`, `watchGuard.js`,
  `typesMap.json`. `build.rs` runs exactly one thing -
  `node typescript/lib/tsc.js -p tsconfig.json --noEmit` - and a run of that
  under a node with `Module._resolveFilename` and the `fs` read family hooked
  resolves precisely two files inside the package: `lib/tsc.js` and the
  `lib/_tsc.js` it requires. `typescript.js` is the programmatic API, the
  `tsserver*` files are the editor language service; nothing in this crate,
  in `data-layer-host.mjs`, or in a scene loads either.
* `typescript/lib/<locale>/diagnosticMessages.generated.json` - 13 locales,
  4.2 MB. tsc reads at most one, only when `--locale`/the host locale asks for
  it, and `localizedDiagnosticMessages` stays undefined when the file is
  absent: messages come out in English instead of failing.
* `ethers` is not installed at all. `@dcl/sdk/ethereum-provider` names it only
  in a doc comment, `@dcl/sdk`'s manifest does not declare it and neither does
  the scaffold's, so a package-manager install in a scaffolded scene does not
  install it either. A scene that wants ethers must add it to its own
  `package.json`. To put it back, add it to the `dependencies` of
  `src/vendor/registry/package.json` and to `ENTRY_PACKAGES` in
  `blob_collect.py`.
* `@types/node` (2.3 MB) and `undici-types` (0.1 MB). The scaffold's tsconfig
  inherits `"types": ["@dcl/js-runtime"]` from `@dcl/sdk/types/tsconfig.ecs7.json`,
  which turns off automatic `@types` inclusion, and the traced tsc run never
  opens a file under `@types/`. Nothing in the kept declaration closure carries
  a `/// <reference types="node" />` either (the resolver check below would
  fail if it did).
* Source maps, `.md`, `test/`, `spec/`, `docs/`, `example/`, `bench*/`,
  `.github/`: nothing loads them at runtime and tsc never reads them.
* `typescript/lib/lib.dom*.d.ts`, `lib.webworker*.d.ts`, `lib.scripthost.d.ts`,
  `lib.d.ts`, `lib.es6.d.ts` and every `lib.*.full.d.ts` - 3.16 MB of the
  9.6 MB the package still weighed, the DOM and its bundles. Every scene's
  tsconfig extends `@dcl/sdk/types/tsconfig.ecs7.json` (`lib: ["ES2020"]`,
  `types: ["@dcl/js-runtime"]`), and the traced tsc run above opens exactly
  45 `lib.*.d.ts`: `lib.es5`, the `lib.es2015`-`lib.es2020` pieces and the
  two `lib.decorators*`. `TS_LIB_RE` in `blob_collect.py` keeps every
  ECMAScript library through `esnext` so a scene raising `lib` to a later
  year still resolves it; a scene that names the DOM has to install it, the
  same divergence as every other prune here.

The registry pins are owned by the tree's manifests and the scaffold, and
checked here, not chosen here (`tree_pins()`); `react` and `react-reconciler`,
which `@dcl/react-ecs` only ranges, are the registry manifest's own.
`typescript` is the scaffold's pin - see `TS_CEILING_NOTE` below and
`src/vendor/README.md`.

Bootstrapping: the chunk build needs a `dcl-scene-cli` binary, and that binary
embeds this blob. There is no cycle - the binary that *builds* the chunks only
has to contain rolldown and `src/split.rs`, not the blob it will later ship. So
the order is: `cargo build -p dcl-scene-cli` (with whatever blob is committed),
run this script, `cargo build -p dcl-scene-cli` again to embed the new blob.

Layout. This file is the pipeline: the two installs, the chunk build, the
declaration rollup and `main()`. Two siblings are imported by name and never
run. `blob_collect.py` decides what of the install tree ships - the entry
points, the allowlists and the reachability walk that prunes the rest - and
`blob_overlays.py` is the checks that the tree-sourced behaviours (the
TypeScript 7 clean tsconfig and the peer-trust gate) reached the blob.
Nothing rewrites a shipped file, which is what lets `src/vendor/README.md`
say so.

Usage:  python3 scripts/build-base-blob.py [--work DIR] [--keep-work]
            [--reuse-install] [--cli-bin PATH] [--out FILE | --check]

The script creates `--work` (default `.cache/scene-cli-blob` under the
repository) and marks it with `WORK_MARKER`; a directory that exists without
the marker is refused, and only a marked one is removed at the end.
`--keep-work` keeps it; `--reuse-install` skips both installs, reuses the
directory as it is and implies `--keep-work`.

`--check` builds the blob in memory and compares it with the committed one,
entry by entry; CI runs it so the archive cannot drift from the tree. When
every entry matches by name, size and CRC and only the deflate bytes differ
(another zlib), it reports "same entries, different compression" and passes.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from blob_collect import (
    CHUNK_REGISTRY, CORE_CHUNK, DROP_PACKAGES, FILE_ALLOWLIST, NEVER, SMART_CHUNK,
    TYPES_ROLLUP, ambient_modules, collect, list_packages, log, reachable,
    resolvable, specifiers_in_files,
)
from blob_overlays import check_chunk_peer_trust, check_ecs7_tsconfig

CRATE = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(CRATE))
REGISTRY = os.path.join(CRATE, 'src/vendor/registry')
SCAFFOLD = os.path.join(CRATE, 'src/templates/init/scene/package.json')
VENDOR_README = os.path.join(CRATE, 'src/vendor/README.md')
OUT = os.path.join(CRATE, 'src/vendor/node_modules.zip')
DEFAULT_WORK = os.path.join(REPO, '.cache/scene-cli-blob')
WORK_MARKER = '.build-base-blob-work'
CLI_BIN = 'dcl-scene-cli'
FIXED_DATE = (1980, 1, 1, 0, 0, 0)
PNPM = os.environ.get('PNPM', 'pnpm')

TS_CEILING_NOTE = """typescript 7.x is the Go port: its `lib/getExePath.js` resolves a
per-platform native package (`@typescript/typescript-darwin-arm64` and friends),
which would make this blob platform-specific. 6.x is the ceiling until that is
addressed."""

TREE_BUILD = 'make -C library/js-sdk-toolchain install build'
SDK_PACKAGES_DIR = 'library/js-sdk-toolchain/packages/@dcl'

# package -> (directory under the repository root, the file its build emits
# last). All share one version, the scaffold pin.
TREE_PACKAGES = {
    '@dcl/sdk': (f'{SDK_PACKAGES_DIR}/sdk', 'index.js'),
    '@dcl/ecs': (f'{SDK_PACKAGES_DIR}/ecs', 'dist/index.js'),
    '@dcl/js-runtime': (f'{SDK_PACKAGES_DIR}/js-runtime', 'apis.d.ts'),
    '@dcl/react-ecs': (f'{SDK_PACKAGES_DIR}/react-ecs', 'dist/index.js'),
    '@dcl/smart-items': (f'{SDK_PACKAGES_DIR}/smart-items', 'dist/index.js'),
    '@dcl/inspector-service': (f'{SDK_PACKAGES_DIR}/inspector-service', 'dist/index.js'),
}

# Tree manifests whose exact `dependencies` pins the registry must repeat.
PINNED_BY_TREE = ('@dcl/sdk', '@dcl/ecs', '@dcl/smart-items', '@dcl/inspector-service')
EXACT_VERSION_RE = re.compile(r'\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?')

# Packages no shipped manifest may depend on: `blob_collect` never ships them,
# and their version is either not on the registry (`@dcl/sdk-commands`,
# `@dcl/react-ecs` at the scaffold pin) or served by the chunks and the rollup
# instead of a package directory (`@dcl/ecs-math`, `protobufjs`, `mitt`, ...).
UNSHIPPED = NEVER | DROP_PACKAGES

PNPM_INSTALL = [
    'install', '--frozen-lockfile', '--ignore-scripts',
    '--config.node-linker=hoisted', '--config.auto-install-peers=false',
]

# Rolled up from their public entry points: package -> (directory of its
# declarations, the entry point).
ROLLUP_PKGS = [
    ('@dcl/sdk', '', 'index'),
    ('@dcl/ecs', 'dist', 'dist/index'),
    ('@dcl/react-ecs', 'dist', 'dist/index'),
    ('@dcl/ecs-math', 'dist', 'dist/index'),
]

# Rolled up only where a declaration above, or one of `ROLLUP_SEED_DEP_PKGS`,
# reaches them.
ROLLUP_DEP_PKGS = [
    ('mitt', '', 'index'),
    ('protobufjs', '', 'index'),
    ('long', '', 'index'),
    ('@protobufjs/aspromise', '', 'index'),
    ('@protobufjs/base64', '', 'index'),
    ('@protobufjs/eventemitter', '', 'index'),
    ('@protobufjs/float', '', 'index'),
    ('@protobufjs/inquire', '', 'index'),
    ('@protobufjs/pool', '', 'index'),
    ('@protobufjs/utf8', '', 'index'),
]

# Rolled up whole: `@dcl/smart-items`' declarations ship as files and import
# them, and nothing of theirs is on disk.
ROLLUP_SEED_DEP_PKGS = ('mitt',)

ROLLUP_FULL_SUBPATH_PKGS = ('@dcl/sdk',)

ROLLUP_SPEC_RE = re.compile(
    r"""(?P<pre>\bfrom\s*|\bimport\s*\(\s*|\bimport\s+|\brequire\s*\(\s*)"""
    r"""(?P<q>['"])(?P<spec>[^'"]+)(?P=q)"""
)
ROLLUP_DECLARE_RE = re.compile(
    r'\bdeclare\s+(?=(?:abstract\s+)?'
    r'(?:const|let|var|function|class|namespace|enum|interface|type|module|global)\b)'
)
ROLLUP_EXPORT_AS_NS_RE = re.compile(r'^\s*export\s+as\s+namespace\s+\w+\s*;?\s*$', re.M)
ROLLUP_REF_RE = re.compile(r'^///\s*<reference[^>]*/>\s*$', re.M)

def run(cmd: list[str], cwd: str, what: str) -> subprocess.CompletedProcess:
    r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f'{what} failed ({" ".join(cmd)} in {cwd}):\n{r.stdout}\n{r.stderr}')
    return r

def read_json(path: str) -> dict:
    with open(path, encoding='utf-8') as fh:
        return json.load(fh)

def scaffold_pins() -> tuple[str, str]:
    """The SDK version the blob is stamped with, and the typescript pin.

    Both live in the scaffold manifest and nowhere else: `init` writes that
    manifest into every scene, so the blob's `@dcl/*` manifests must name the
    same version or the scene's own dependency listing and its `node_modules`
    disagree. `init.rs::blob_tracks_the_scaffold_pin_and_carries_the_ecs_fixes`
    checks the shipped archive against it.
    """
    pins = read_json(SCAFFOLD)['devDependencies']
    sdk = pins['@dcl/sdk']
    if pins.get('@dcl/js-runtime') != sdk:
        raise SystemExit(
            f'{SCAFFOLD}: @dcl/js-runtime is pinned to {pins.get("@dcl/js-runtime")} '
            f'but @dcl/sdk to {sdk}; the SDK packages share one version')
    if not re.fullmatch(r'\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?', sdk):
        raise SystemExit(f'{SCAFFOLD}: @dcl/sdk pin {sdk!r} is not a version')
    log(f'scaffold pins: @dcl/sdk {sdk}, typescript {pins["typescript"]}')
    return sdk, pins['typescript']

def find_cli_bin(explicit: str | None) -> str:
    """The `dcl-scene-cli` that builds the chunks: `--cli-bin`, or the newest
    of the debug and release builds under `CARGO_TARGET_DIR` /
    `services/catalyst/target`."""
    if explicit:
        if not os.path.isfile(explicit):
            raise SystemExit(f'--cli-bin {explicit} does not exist')
        return explicit
    target = os.environ.get('CARGO_TARGET_DIR') or os.path.join(REPO, 'services/catalyst/target')
    found = [p for p in (os.path.join(target, profile, CLI_BIN) for profile in ('release', 'debug'))
             if os.path.isfile(p)]
    if not found:
        raise SystemExit(
            f'no {CLI_BIN} under {target}/{{release,debug}}.\n'
            'The chunk build needs a dcl-scene-cli binary. Build one first:\n'
            '  cargo build -p dcl-scene-cli   (in services/catalyst)\n'
            'There is no bootstrap cycle: that binary only has to contain '
            'rolldown and src/split.rs, not the blob it will later ship.')
    return max(found, key=os.path.getmtime)

def check_tree_built() -> None:
    """Every tree package must carry the file its build emits last."""
    missing = [f'{d}/{marker}' for d, marker in TREE_PACKAGES.values()
               if not os.path.isfile(os.path.join(REPO, d, marker))]
    if missing:
        raise SystemExit(
            f'the tree packages are not built ({", ".join(missing)} missing). '
            f'Run `{TREE_BUILD}` first (SDK_PROTOC=protoc inside the Nix shell).')

def tree_pins() -> dict[str, tuple[str, str]]:
    """name -> (version, owning manifest): every exact-version dependency of
    the `PINNED_BY_TREE` packages on a package the tree does not build. Two
    manifests pinning one name differently is an error here, not a choice."""
    pins: dict[str, tuple[str, str]] = {}
    for pkg in PINNED_BY_TREE:
        manifest = os.path.join(REPO, TREE_PACKAGES[pkg][0], 'package.json')
        data = read_json(manifest)
        for name, want in data.get('dependencies', {}).items():
            if name in TREE_PACKAGES or not EXACT_VERSION_RE.fullmatch(want):
                continue
            prev = pins.get(name)
            if prev is not None and prev[0] != want:
                raise SystemExit(f'{manifest} pins {name} {want} but {prev[1]} pins {prev[0]}')
            pins[name] = (want, manifest)
    return pins

def install_registry(work: str, ts_pin: str) -> None:
    """`pnpm install --frozen-lockfile` of `src/vendor/registry` into `work`.

    The lockfile is the pin set; every pin another file owns (`tree_pins()`
    and the scaffold's typescript) is checked against the registry manifest
    before anything is installed, so a bump in the tree fails here naming the
    manifest to align (then `pnpm install --lockfile-only` in
    `src/vendor/registry` to refresh the lock). The manifest's
    `devDependencies` are the build-time packages (`DROP_PACKAGES`); they are
    installed whatever `NODE_ENV` says. Hoisted so the tree is flat like an
    npm install, no scripts so nothing from the registry runs, and no peer
    auto-install so a peer is only ever served by a package the lockfile pins
    or the tree packs.
    """
    manifest = read_json(os.path.join(REGISTRY, 'package.json'))
    deps = {**manifest.get('devDependencies', {}), **manifest['dependencies']}
    pins = tree_pins()
    pins['typescript'] = (ts_pin, SCAFFOLD)
    for name, (want, owner) in sorted(pins.items()):
        if deps.get(name) != want:
            raise SystemExit(
                f'{REGISTRY}/package.json pins {name} {deps.get(name)} but '
                f'{owner} pins {want}; align the registry manifest and refresh its lock')
    for name in ('package.json', 'pnpm-workspace.yaml', 'pnpm-lock.yaml'):
        shutil.copy(os.path.join(REGISTRY, name), os.path.join(work, name))
    log('installing the registry packages: ' + ', '.join(f'{k}@{v}' for k, v in sorted(deps.items())))
    env = {k: v for k, v in os.environ.items() if k != 'NODE_ENV'}
    r = subprocess.run([PNPM, *PNPM_INSTALL], cwd=work, capture_output=True, text=True, env=env)
    if r.returncode != 0 and 'ERR_PNPM_IGNORED_BUILDS' not in (r.stdout + r.stderr):
        raise SystemExit(f'registry install failed:\n{r.stdout}\n{r.stderr}')

def pack_tree_packages(work: str, versions: dict[str, str]) -> None:
    """Put the tree's build of every `@dcl/*` package into `work/node_modules`.

    `pnpm pack` in each package directory gives exactly what npm would publish
    (`files`, `.gitignore`, no `node_modules`), which is what a scene's own
    install would have resolved; the tarball is extracted into the install
    tree. Then the manifest is stamped: its `version` becomes the scaffold
    pin, every dependency on another tree package (`workspace:*` in the tree)
    names that same version, and a dependency on a package in `UNSHIPPED` is
    dropped, so the
    shipped `@dcl/sdk/package.json` reads like a published one and names only
    packages the extracted tree resolves.
    """
    nm = os.path.join(work, 'node_modules')
    packed = os.path.join(work, 'packed')
    shutil.rmtree(packed, ignore_errors=True)
    for pkg, (d, _marker) in TREE_PACKAGES.items():
        src = os.path.join(REPO, d)
        dest = os.path.join(packed, pkg.replace('/', '__'))
        os.makedirs(dest)
        run([PNPM, 'pack', '--pack-destination', dest], src, f'pnpm pack {pkg}')
        tarballs = [f for f in os.listdir(dest) if f.endswith('.tgz')]
        if len(tarballs) != 1:
            raise SystemExit(f'pnpm pack {pkg} left {tarballs} in {dest}, expected one tarball')
        target = os.path.join(nm, pkg)
        shutil.rmtree(target, ignore_errors=True)
        with tarfile.open(os.path.join(dest, tarballs[0])) as tar:
            members = tar.getmembers()
            for m in members:
                if not m.name.startswith('package/'):
                    raise SystemExit(f'{tarballs[0]}: unexpected entry {m.name}')
                m.name = m.name[len('package/'):]
            if hasattr(tarfile, 'data_filter'):
                tar.extractall(target, members=members, filter='data')
            else:
                tar.extractall(target, members=members)
        manifest = os.path.join(target, 'package.json')
        data = read_json(manifest)
        data['version'] = versions[pkg]
        dropped = []
        for field in ('dependencies', 'devDependencies', 'peerDependencies'):
            for dep in list(data.get(field, {})):
                if dep in UNSHIPPED:
                    del data[field][dep]
                    dropped.append(dep)
                elif dep in versions:
                    data[field][dep] = versions[dep]
        with open(manifest, 'w', encoding='utf-8') as fh:
            json.dump(data, fh, indent=2)
            fh.write('\n')
        n = sum(len(f) for _, _, f in os.walk(target))
        note = f', without {", ".join(dropped)}' if dropped else ''
        log(f'    {pkg} <- {d} ({n} files), stamped {versions[pkg]}{note}')

def exports_types(nm: str, pkg: str, sub: str) -> str | None:
    """The `.d.ts` tsc lands on for `<pkg>/<sub>` through the package's `exports` map.

    `moduleResolution: bundler` takes the `import` and `types` conditions, in
    the map's own order. None without a map or an entry for the subpath.
    """
    manifest = os.path.join(nm, pkg, 'package.json')
    if not os.path.isfile(manifest):
        return None
    target = read_json(manifest).get('exports')
    target = target.get(f'./{sub}') if isinstance(target, dict) else None
    while isinstance(target, dict):
        target = next((v for k, v in target.items()
                       if k in ('types', 'import', 'default')), None)
    if not isinstance(target, str):
        return None
    return os.path.normpath(os.path.join(nm, pkg, target))

def build_types_rollup(nm: str) -> bytes:
    """Roll the scene-facing `.d.ts` of the SDK packages into ONE ambient file.

    Every kept file becomes `declare module "<canonical specifier>" { ... }`
    with its relative imports rewritten to canonical specifiers, so tsc resolves
    `@dcl/sdk/ecs` & co. from ambient module declarations instead of from a
    `node_modules` tree - which is what lets the packages themselves go.

    `@dcl/js-runtime`'s own three declarations are ambient already (global
    interfaces + `declare module '~system/*'` + `declare module '~sdk/*'`) and
    are copied verbatim at top level. That is load-bearing: the bundle then has
    no top-level `import`/`export` and stays a *script*, which is what makes the
    inner `declare module` blocks ambient declarations rather than
    augmentations of modules that no longer exist.

    The keep-set is tsc reachability, not "every `.d.ts` in the package": BFS
    from the public entry points, following declaration imports the way tsc
    does. Shipping the unreachable ones would *add* errors the node_modules tree
    never produced - `@dcl/react-ecs/dist/reconciler/types.d.ts` names the DOM
    type `Document`, which `lib: ["ES2020"]` does not define, and tsc never
    loads that file today because nothing public re-exports it.

    Six transforms are forced by the re-homing. Each is an outright tsc error
    otherwise:

    * `declare` modifiers are stripped inside the module bodies. A `.d.ts` at
      top level is not an ambient context, so upstream writes `export declare
      const`; inside `declare module { }` it already is one and TS1038 rejects
      the modifier.
    * `export as namespace X` (the UMD global of protobufjs and long) is
      dropped: TS1316, "global module exports may only appear at top level".
    * An alias module whose target uses `export =` (protobufjs, long,
      `@protobufjs/*`) is emitted as `import x = require(...); export = x`.
      `export *` against such a module is TS2498.
    * `@dcl/js-runtime/apis.d.ts` reaches @dcl/ecs as `import('../ecs')` - it
      walks out of its own package directory, which only resolves while the two
      are siblings under `node_modules/@dcl/`, exactly what this bundle
      removes. Rewritten to `import('@dcl/ecs')`.
    * A relative specifier written with a `.js` extension (long's `./types.js`)
      resolves to the sibling `.d.ts`.
    * `@dcl/sdk/src/<x>` is aliased onto `@dcl/sdk/<x>`. `@dcl/sdk` ships its
      own TypeScript *sources* under `src/` and real scenes import them -
      `@dcl/sdk/src/players` and `@dcl/sdk/src/network`, two scenes in
      decentraland/sdk7-test-scenes. Those `.ts` carry implementations and
      cannot become `declare module` bodies. With the alias blocks the corpus
      is 60/60; without them, 58/60.
    """
    known: dict[str, str] = {}

    for pkg, subdir, _entry in ROLLUP_PKGS + ROLLUP_DEP_PKGS:
        root = os.path.join(nm, pkg)
        if not os.path.isdir(root):
            continue
        base = os.path.join(root, subdir) if subdir else root
        for dirpath, dirnames, filenames in os.walk(base):
            dirnames[:] = [d for d in dirnames
                           if d not in ('dist-cjs', 'src', 'etc', 'node_modules')]
            for fn in filenames:
                if not fn.endswith('.d.ts'):
                    continue
                p = os.path.join(dirpath, fn)
                rel = os.path.relpath(p, root)[: -len('.d.ts')].replace(os.sep, '/')
                known[p] = f'{pkg}/{rel}'

    def specifiers(text: str) -> list[str]:
        return [m.group('spec') for m in ROLLUP_SPEC_RE.finditer(text)]

    def resolve_rel(from_file: str, spec: str) -> str | None:
        base = os.path.normpath(os.path.join(os.path.dirname(from_file), spec))
        cands = [base + '.d.ts', os.path.join(base, 'index.d.ts')]
        if base.endswith('.js'):
            cands.insert(0, base[:-3] + '.d.ts')
        return next((c for c in cands if c in known), None)

    export_aliases: list[tuple[str, str]] = []

    def resolve_bare(spec: str) -> str | None:
        for p, canon in known.items():
            if canon == spec or canon == spec + '/index':
                return p
        for pkg, _subdir, entry in ROLLUP_PKGS + ROLLUP_DEP_PKGS:
            if spec == pkg:
                p = os.path.join(nm, pkg, entry + '.d.ts')
                if p in known:
                    return p
            if spec.startswith(pkg + '/'):
                p = exports_types(nm, pkg, spec[len(pkg) + 1:])
                if p in known:
                    export_aliases.append((spec, known[p]))
                    return p
        return None

    entry_points = []
    for pkg, _subdir, entry in ROLLUP_PKGS + [
            t for t in ROLLUP_DEP_PKGS if t[0] in ROLLUP_SEED_DEP_PKGS]:
        p = os.path.join(nm, pkg, entry + '.d.ts')
        if p in known:
            entry_points.append(p)
    for pkg in ROLLUP_FULL_SUBPATH_PKGS:
        for dirpath, dirnames, filenames in os.walk(os.path.join(nm, pkg)):
            dirnames[:] = [d for d in dirnames
                           if d not in ('src', 'types', 'bin', 'node_modules')]
            entry_points += [os.path.join(dirpath, f)
                             for f in filenames if f.endswith('.d.ts')]

    seen: set[str] = set()
    texts: dict[str, str] = {}
    queue = list(entry_points)
    while queue:
        p = queue.pop()
        if p in seen or p not in known:
            continue
        seen.add(p)
        with open(p, encoding='utf8') as fh:
            texts[p] = fh.read()
        for spec in specifiers(texts[p]):
            t = resolve_rel(p, spec) if spec.startswith('.') else resolve_bare(spec)
            if t and t not in seen:
                queue.append(t)
    keep = seen

    aliases: list[tuple[str, str]] = list(export_aliases)
    for pkg, _subdir, entry in ROLLUP_PKGS + ROLLUP_DEP_PKGS:
        p = os.path.join(nm, pkg, entry + '.d.ts')
        if p in keep:
            aliases.append((pkg, known[p]))
    for p in keep:
        canon = known[p]
        if canon.endswith('/index'):
            aliases.append((canon[: -len('/index')], canon))
    sdk_src = os.path.join(nm, '@dcl/sdk/src')
    canon_of_keep = {known[p] for p in keep}
    for dirpath, _dn, filenames in os.walk(sdk_src):
        for f in filenames:
            if not f.endswith('.ts') or f.endswith('.d.ts'):
                continue
            rel = os.path.relpath(os.path.join(dirpath, f), sdk_src)[:-3]
            rel = rel.replace(os.sep, '/')
            targets = [f'@dcl/sdk/{rel}']
            if rel.endswith('/index'):
                targets.append(f'@dcl/sdk/{rel[: -len("/index")]}')
            for target in targets:
                if target in canon_of_keep:
                    aliases.append((f'@dcl/sdk/src/{rel}', target))
                    if rel.endswith('/index'):
                        aliases.append(
                            (f'@dcl/sdk/src/{rel[: -len("/index")]}', target))
                    break

    out = [
        '// GENERATED by scripts/build-base-blob.py -- do not edit.',
        '// Ambient declaration bundle: @dcl/sdk, @dcl/ecs, @dcl/react-ecs,',
        '// @dcl/ecs-math, @dcl/js-runtime + the protobufjs, long and mitt',
        '// declarations their .d.ts and @dcl/smart-items\' reach. Replaces those',
        '// packages\' .d.ts trees entirely; the implementations live in',
        f'// {CORE_CHUNK} and {SMART_CHUNK}. @dcl/smart-items ships its own .d.ts.',
        '',
    ]
    for name in ('index.d.ts', 'apis.d.ts', 'sdk.d.ts'):
        p = os.path.join(nm, '@dcl/js-runtime', name)
        if not os.path.exists(p):
            continue
        with open(p, encoding='utf8') as fh:
            txt = ROLLUP_REF_RE.sub('', fh.read())
        txt = txt.replace("'../ecs'", "'@dcl/ecs'").replace('"../ecs"', '"@dcl/ecs"')
        out += [f'// ---- @dcl/js-runtime/{name} (ambient, verbatim) ----', txt, '']

    unresolved: dict[str, list[str]] = {}
    uses_export_eq: set[str] = set()
    for p in sorted(keep):
        body = texts[p]
        bad = [s for s in specifiers(body)
               if s.startswith('.') and not resolve_rel(p, s)]
        if bad:
            unresolved[os.path.relpath(p, nm)] = bad

        def sub(m: re.Match, _p: str = p) -> str:
            spec = m.group('spec')
            if not spec.startswith('.'):
                return m.group(0)
            t = resolve_rel(_p, spec)
            if t is None:
                return m.group(0)
            return f'{m.group("pre")}{m.group("q")}{known[t]}{m.group("q")}'

        body = ROLLUP_SPEC_RE.sub(sub, body)
        body = ROLLUP_REF_RE.sub('', body)
        body = ROLLUP_EXPORT_AS_NS_RE.sub('', body)
        body = ROLLUP_DECLARE_RE.sub('', body)
        if re.search(r'^\s*export\s*=', body, re.M):
            uses_export_eq.add(known[p])
        indented = '\n'.join(('  ' + l) if l.strip() else l
                             for l in body.splitlines())
        out += [f'declare module "{known[p]}" {{', indented, '}', '']

    emitted: set[str] = set()
    for alias, canon in sorted(set(aliases)):
        if alias in emitted or alias == canon or alias in canon_of_keep:
            continue
        emitted.add(alias)
        if canon in uses_export_eq:
            out += [f'declare module "{alias}" {{',
                    f'  import __x = require("{canon}");',
                    '  export = __x;', '}', '']
            continue
        src = texts[next(p for p in keep if known[p] == canon)]
        has_default = bool(re.search(
            r'^\s*export\s+default\b|^\s*export\s*\{[^}]*\bdefault\b', src, re.M))
        body_lines = [f'  export * from "{canon}";']
        if has_default:
            body_lines.append(f'  export {{ default }} from "{canon}";')
        out += [f'declare module "{alias}" {{'] + body_lines + ['}', '']

    if unresolved:
        raise SystemExit(
            'the declaration rollup has unresolved relative specifiers:\n  '
            + '\n  '.join(f'{p}: {u}' for p, u in sorted(unresolved.items())))
    log(f'declaration rollup: {len(keep)} modules of {len(known)} found, '
        f'{len(emitted)} aliases')
    return '\n'.join(out).encode('utf8')

CHUNK_SCENE_JSON = (
    '{"runtimeVersion":"7","main":"bin/index.js",'
    '"scene":{"parcels":["0,0"],"base":"0,0"}}'
)
CHUNK_TSCONFIG = (
    '{"compilerOptions":{"jsx":"react","allowJs":true,"resolveJsonModule":true,'
    '"strict":true},"include":["src/**/*.ts"],'
    '"extends":"@dcl/sdk/types/tsconfig.ecs7.json"}'
)

def build_chunks(work: str, cli_bin: str, files: dict[str, bytes],
                 kept_bytes: dict[str, int]) -> None:
    """Bundle the two prebuilt runtime chunks and add them to the blob.

    The chunks are produced by `dcl-scene-cli vendor-chunks`, not here: the
    rolldown settings (`CodeSplittingMode::Bool(false)`, Cjs, es2020, minify, no
    sourcemap), the registry key lists and the `~sdk/script-utils` alias all
    live in `src/{split,prebuilt}.rs`, and a second copy of them in python would
    drift. This function only stages a throwaway scene against the *unpruned*
    install tree and shells out.

    That tree has to be unpruned: the core chunk bundles all of
    `@dcl/{sdk,ecs,react-ecs,ecs-math}` + react, and the smart chunk bundles
    `@dcl/smart-items`' `dist/*.js` (with `@dcl/smart-items/scripts`, the
    runtime behind every scene's generated `~sdk/script-utils`), none of
    which ship as files.

    `vendor-chunks` verifies both chunks before returning: every `require()`
    either chunk emits must be `~system/*` or a key the loader's registry will
    hold when that chunk is evaluated. `@dcl/sdk/platform` and
    `@dcl/sdk/text-codec` are registry keys for exactly this: the smart-item
    runtime requires both across the chunk boundary, and a key the core chunk
    does not publish is a hard "not in the sdk runtime registry" throw at
    scene start.
    """
    scene = os.path.join(work, 'chunk-scene')
    shutil.rmtree(scene, ignore_errors=True)
    os.makedirs(os.path.join(scene, 'src'))
    os.symlink(os.path.join(work, 'node_modules'),
               os.path.join(scene, 'node_modules'))
    with open(os.path.join(scene, 'scene.json'), 'w') as fh:
        fh.write(CHUNK_SCENE_JSON)
    with open(os.path.join(scene, 'tsconfig.json'), 'w') as fh:
        fh.write(CHUNK_TSCONFIG)
    with open(os.path.join(scene, 'src/index.ts'), 'w') as fh:
        fh.write('export function main() {}\n')

    out = os.path.join(work, 'prebuilt')
    shutil.rmtree(out, ignore_errors=True)
    log(f'building the prebuilt runtime chunks with {cli_bin}')
    r = subprocess.run(
        [cli_bin, 'vendor-chunks', '--dir', scene,
         '--out-core', os.path.join(out, 'core.js'),
         '--out-smart', os.path.join(out, 'smart.js')],
        capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f'vendor-chunks failed:\n{r.stdout}\n{r.stderr}')

    total = 0
    for name, rel in (('core.js', CORE_CHUNK), ('smart.js', SMART_CHUNK),
                      ('registry.json', CHUNK_REGISTRY)):
        with open(os.path.join(out, name), 'rb') as fh:
            data = fh.read()
        files[rel] = data
        total += len(data)
        log(f'    {rel}  {len(data)} B')
    kept_bytes['@dcl/sdk (prebuilt chunks)'] = total

def check_chunk_registry(files: dict[str, bytes]) -> None:
    """The resolver check for the two prebuilt chunks.

    A chunk's `require()`s are served by the split loader, not by node: they
    must be `~system/*` (passed through to the host) or a key some chunk has
    already published. The core chunk is evaluated first and so may only use
    `~system/*`; the smart chunk is layered on it and may also use any core key.
    `registry.json` is written by `vendor-chunks` from the same
    `src/split.rs` lists the chunks were built against, so this cannot drift
    from what the loader will actually hold.
    """
    registry = json.loads(files[CHUNK_REGISTRY])
    for rel, allowed in ((CORE_CHUNK, set()), (SMART_CHUNK, set(registry['core']))):
        code = files[rel].decode('utf-8', 'ignore')
        specs = set(re.findall(r'require\("([^"]+)"\)', code))
        specs |= set(re.findall(r"require\('([^']+)'\)", code))
        bad = sorted(s for s in specs
                     if not s.startswith('~system/') and s not in allowed)
        if bad:
            raise SystemExit(
                f'{rel} requires specifiers no chunk publishes: {", ".join(bad)}\n'
                'Add them to REGISTRY_KEYS in src/split.rs and rebuild.')
        log(f'    {rel}: {len(specs)} requires, all served')

COUNT_WORDS = ('zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven',
               'eight', 'nine', 'ten', 'eleven', 'twelve', 'thirteen',
               'fourteen', 'fifteen', 'sixteen', 'seventeen', 'eighteen',
               'nineteen', 'twenty')
README_ALLOWLIST = re.compile(
    r'(\w+)\s+packages\s+are\s+allowlisted\s+file\s+by\s+file\s+'
    r'\(`FILE_ALLOWLIST`:\s*([^)]*)\)')

def count_word(n: int) -> str:
    """Spell the allowlist count the way the README writes it.

    Bounded on purpose: a count past the table is a build failure that names
    the fix, never an IndexError from inside the drift check.
    """
    if n < len(COUNT_WORDS):
        return COUNT_WORDS[n]
    raise SystemExit(
        f'blob_collect.FILE_ALLOWLIST has {n} entries but COUNT_WORDS in '
        f'scripts/build-base-blob.py only spells 0..{len(COUNT_WORDS) - 1}; '
        'extend COUNT_WORDS so check_readme_allowlist() can spell the count '
        'the README must carry.')

def check_readme_allowlist() -> None:
    """Fail before the install if the README's allowlist sentence has drifted.

    `src/vendor/README.md` counts and names the packages in `FILE_ALLOWLIST`;
    the count went stale once already (six written, eight real), so the
    sentence is checked against the dict instead of trusted.
    """
    with open(VENDOR_README, encoding='utf-8') as fh:
        hit = README_ALLOWLIST.search(fh.read())
    if not hit:
        raise SystemExit(f'{VENDOR_README}: the regeneration steps no longer carry '
                         'the "<count> packages are allowlisted file by file '
                         '(`FILE_ALLOWLIST`: ...)" sentence this check reads')
    named = set(re.findall(r'`([^`]+)`', hit.group(2)))
    real = set(FILE_ALLOWLIST)
    want = count_word(len(real))
    if named != real or hit.group(1) != want:
        raise SystemExit(
            f'{VENDOR_README}: names {hit.group(1)} allowlisted packages '
            f'({", ".join(sorted(named))}); blob_collect.FILE_ALLOWLIST has '
            f'{want} ({", ".join(sorted(real))}). Fix the README.')

def zip_bytes(files: dict[str, bytes]) -> bytes:
    """Deterministic: sorted paths, fixed 1980 timestamps, deflate, 0644."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w') as z:
        for name in sorted(files):
            info = zipfile.ZipInfo(name, date_time=FIXED_DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, files[name])
    return buf.getvalue()

def entry_digests(data: bytes) -> dict[str, tuple[str, int] | None]:
    """Each entry's SHA-256 and size, from its decompressed bytes.

    The central directory's CRC and size are what a tampered entry keeps, so
    they are never read: the bytes are inflated (zipfile checks them against
    the stored CRC on the way) and hashed. `None` marks an entry whose bytes
    fail that check.
    """
    out: dict[str, tuple[str, int] | None] = {}
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for info in z.infolist():
            try:
                content = z.read(info.filename)
            except zipfile.BadZipFile:
                out[info.filename] = None
                continue
            out[info.filename] = (hashlib.sha256(content).hexdigest(), len(content))
    return out

def compare_with_committed(built: bytes, path: str) -> int:
    """Entry-by-entry diff of the built archive against the committed one.

    Same names with the same decompressed bytes is another zlib's deflate
    output, not drift: reported as such and accepted. Anything else - an
    entry added, gone, changed, or whose bytes no longer match the CRC the
    archive stores for them - is.
    """
    if not os.path.isfile(path):
        sys.stderr.write(f'{path} does not exist; run without --check to create it\n')
        return 1
    with open(path, 'rb') as fh:
        committed = fh.read()
    shown = os.path.relpath(path, REPO)
    if committed == built:
        log(f'\n--check: {shown} matches the tree ({len(built)} B)')
        return 0
    try:
        old = entry_digests(committed)
    except zipfile.BadZipFile as e:
        sys.stderr.write(f'{shown} is not a zip archive ({e}). '
                         'Rebuild it with scripts/build-base-blob.py and commit the result.\n')
        return 1
    new = entry_digests(built)
    if old == new:
        log(f'\n--check: {shown} matches the tree: same entries, '
            f'different compression ({len(committed)} -> {len(built)} B)')
        return 0
    lines = [f'  only committed: {n}' for n in sorted(old.keys() - new.keys())]
    lines += [f'  only built:     {n}' for n in sorted(new.keys() - old.keys())]
    both = sorted(old.keys() & new.keys())
    lines += [f'  corrupt:        {n} (its bytes fail the CRC the archive stores)'
              for n in both if old[n] is None]
    lines += [f'  changed:        {n} ({old[n][1]} -> {new[n][1]} B)'
              for n in both if old[n] is not None and old[n] != new[n]]
    sys.stderr.write(
        f'{shown} is stale against the tree '
        f'({len(committed)} -> {len(built)} B):\n' + '\n'.join(lines) + '\n'
        'Rebuild it with scripts/build-base-blob.py and commit the result.\n')
    return 1

def create_work(work: str) -> None:
    """An empty work directory marked as this script's.

    A marked directory is a previous run's and is replaced; one that exists
    without the marker is somebody else's and is refused, so `--work .` or a
    mistyped path never wipes a directory the script did not create.
    """
    marker = os.path.join(work, WORK_MARKER)
    if os.path.lexists(work):
        if os.path.islink(work) or not os.path.isfile(marker):
            raise SystemExit(
                f'{work} exists and was not created by this script (no {WORK_MARKER} '
                'in it); pass --work with a new path or remove the directory yourself')
        shutil.rmtree(work)
    os.makedirs(work)
    with open(marker, 'w', encoding='utf-8') as fh:
        fh.write('work directory of apps/dcl-scene-cli/scripts/build-base-blob.py\n')

def remove_work(work: str) -> None:
    """Remove the work directory, and only one `create_work()` marked."""
    if os.path.isfile(os.path.join(work, WORK_MARKER)):
        shutil.rmtree(work, ignore_errors=True)

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--work', default=DEFAULT_WORK,
                    help='work directory; created and marked by this script, which '
                         'refuses one it did not create (default: %(default)s)')
    ap.add_argument('--keep-work', action='store_true',
                    help='keep the work directory at the end')
    ap.add_argument('--reuse-install', action='store_true',
                    help='skip both installs and reuse --work as-is (implies --keep-work)')
    ap.add_argument('--cli-bin',
                    help='dcl-scene-cli binary that builds the prebuilt chunks '
                         '(default: the newest build under the cargo target dir)')
    ap.add_argument('--out', default=OUT, help='where to write the archive')
    ap.add_argument('--check', action='store_true',
                    help='build in memory and compare with the committed archive')
    args = ap.parse_args()
    check_readme_allowlist()
    version, ts_pin = scaffold_pins()
    check_tree_built()
    versions = {pkg: version for pkg in TREE_PACKAGES}
    cli_bin = find_cli_bin(args.cli_bin)

    nm = os.path.join(args.work, 'node_modules')
    if args.reuse_install:
        if not os.path.isdir(nm):
            raise SystemExit(f'--reuse-install: {nm} does not exist; run without it first')
    else:
        create_work(args.work)
        install_registry(args.work, ts_pin)
        pack_tree_packages(args.work, versions)

    present = set(list_packages(nm))
    keep, why = reachable(nm, present)
    dropped = sorted(present - keep)

    log(f'\n{len(present)} packages in the install tree -> keeping {len(keep)}')
    for p in sorted(keep):
        log(f'    keep  {p:28s} {why[p]}')
    log(f'  dropping {len(dropped)}: {", ".join(dropped)}')

    files, kept_bytes = collect(nm, keep)
    check_ecs7_tsconfig(files)

    build_chunks(args.work, cli_bin, files, kept_bytes)
    check_chunk_peer_trust(files)
    rollup = build_types_rollup(nm)
    files[TYPES_ROLLUP] = rollup
    kept_bytes['@dcl/js-runtime'] = kept_bytes.get('@dcl/js-runtime', 0) + len(rollup)

    runtime_specs, declared_specs = specifiers_in_files(files)
    ambient = ambient_modules(rollup)
    unresolved = sorted(s for s in runtime_specs if not resolvable(s, files))
    unresolved += sorted(f'{s} (from a declaration)' for s in declared_specs - runtime_specs
                         if not resolvable(s, files) and s not in ambient)
    if unresolved:
        sys.stderr.write(
            'these imports would not resolve in the extracted tree:\n  '
            + '\n  '.join(unresolved) + '\n')
        return 1
    log(f'\nresolver check: all {len(runtime_specs | declared_specs)} static imports resolve')
    check_chunk_registry(files)

    for pkg, want in sorted(versions.items()):
        manifest = files.get(f'node_modules/{pkg}/package.json')
        if manifest is None:
            continue
        shipped = json.loads(manifest)['version']
        if shipped != want:
            raise SystemExit(f'{pkg} ships version {shipped}, the tree says {want}')

    built = zip_bytes(files)
    raw = sum(len(v) for v in files.values())
    log('\nper-package unpacked size:')
    for p, n in sorted(kept_bytes.items(), key=lambda kv: -kv[1]):
        log(f'  {n/1048576:8.2f} MB  {p}')
    log(f'\n{len(files)} files, {raw/1048576:.1f} MB unpacked, {len(built)/1048576:.1f} MB zipped')

    if args.check:
        status = compare_with_committed(built, args.out)
    else:
        with open(args.out, 'wb') as fh:
            fh.write(built)
        log(f'{args.out} written')
        status = 0

    if not (args.keep_work or args.reuse_install):
        remove_work(args.work)
    return status

if __name__ == '__main__':
    raise SystemExit(main())
