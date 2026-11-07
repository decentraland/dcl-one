"""Every rewrite `build-base-blob.py` applies on top of the install tree, and
the checks that prove the tree-sourced behaviour reached the shipped files.

Imported by that script, never run. A rewrite is anything the blob carries
that the SDK build plus the registry install would not produce, and every one
lives here so `src/vendor/README.md` can name them from a single place:

* `add_pbmin()` - `node_modules/protobufjs` is our dependency-free
  `protobufjs/minimal` (`src/vendor/pbmin`), not upstream's;
  `swap_pbmin_into_tree()` points the install tree at the same code so the
  prebuilt chunks bundle it, and `check_chunk_pbmin()` fails the build if they
  ever stop doing so.
* `add_shim()` - `node_modules/@dcl/inspector` is the hand-authored stand-in
  from `src/vendor/inspector-shim` (crdt dumper + minimal data-layer host);
  `build_service_descriptor()` transpiles its rpc service descriptor beside it.

Two behaviours used to be overlays on upstream's build and are now source in
`library/js-sdk-toolchain`; what remains here is the check that each one
reached the blob:

* `check_ecs7_tsconfig()` - `@dcl/sdk/types/tsconfig.ecs7.json` names no
  option TypeScript 7 rejects and resolves as `bundler`.
* `check_chunk_peer_trust()` - the sync transport's peer-trust gate
  (`globalThis.__dclOneAuthoritative`, set by the split loader from
  scene.json's `authoritativeMultiplayer`) is in `prebuilt/core.js`.

The two built products - the prebuilt chunks and the declaration rollup - are
build steps, not rewrites of a file, and stay in `build-base-blob.py`.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys

from blob_collect import CORE_CHUNK, SMART_CHUNK, log

CRATE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHIM = os.path.join(CRATE, 'src/vendor/inspector-shim')
PBMIN = os.path.join(CRATE, 'src/vendor/pbmin')

PBMIN_ENTRY = b'module.exports = require("./index.js");\n'
PBMIN_MANIFEST = {
    'name': 'protobufjs',
    'version': '7.2.4-dcl-one-sdk-pbmin.1',
    'description': 'protobufjs/minimal wire codec, reimplemented dependency-free '
                   'for dcl-scene-cli. NOT upstream protobufjs. See '
                   'src/vendor/pbmin in apps/dcl-scene-cli.',
    'main': 'index.js',
    'license': 'BSD-3-Clause',
}

PBMIN_TREE_ENTRY = b'''// Redirected by scripts/blob_overlays.py swap_pbmin_into_tree().
// NOT upstream's minimal entry point. See ../protobufjs/pbmin.js.
"use strict";
module.exports = require("./pbmin.js");
'''

def swap_pbmin_into_tree(work: str) -> None:
    """Point the INSTALL TREE's `protobufjs/minimal` at the replacement too.

    `add_pbmin()` decides what the blob *ships*; this decides what the prebuilt
    chunks are *built against*. Without it rolldown resolves
    `protobufjs/minimal` through this tree and bundles UPSTREAM into
    `prebuilt/core.js`, so an extracted scene would carry two protobuf codecs -
    ours in `node_modules`, upstream's inside the chunk - and the scene
    runtime, the one place a codec bug is visible in every scene rather than
    in one dev-machine process, would run the one with no owner.

    Mechanics: protobufjs 7.2.4 has NO `exports` map (checked, not assumed), so
    `protobufjs/minimal` resolves plainly to `minimal.js` and overwriting that
    file is the whole intervention. The replacement lands beside it as
    `pbmin.js` rather than clobbering `index.js`, which stays upstream's full
    reflection build:

      * `build_types_rollup()` still reads this package's `.d.ts` files, and
        `index.d.ts` is what `main` points at;
      * a `require('protobufjs')` for the reflection library still gets the real
        thing, so if something ever does reach for it the failure is a missing
        *dependency*, not a silently different library.

    Only `minimal.js` moves, and only in the throwaway `--work` tree, which is
    rebuilt from the registry install on every run. Nothing here touches a
    checkout. Verified by `check_chunk_pbmin()` below, which fails the build if
    the provenance marker is absent from `prebuilt/core.js`.
    """
    pkg = os.path.join(work, 'node_modules', 'protobufjs')
    entry = os.path.join(pkg, 'minimal.js')
    if not os.path.isfile(entry):
        raise SystemExit(
            f'{entry} does not exist - the install tree has no protobufjs to '
            'redirect. The package must stay in src/vendor/registry/package.json '
            'even though nothing of it ships; see PROTOBUFJS_SHIP_NOTHING in '
            'blob_collect.py.')
    with open(os.path.join(PBMIN, 'index.js'), 'rb') as fh:
        core = fh.read()
    with open(os.path.join(pkg, 'pbmin.js'), 'wb') as fh:
        fh.write(core)
    with open(entry, 'wb') as fh:
        fh.write(PBMIN_TREE_ENTRY)
    log(f'    install tree: protobufjs/minimal.js -> pbmin.js  {len(core)} B '
        '(prebuilt chunks now bundle the replacement)')

PBMIN_MARKER = b'dcl-one-sdk-pbmin.1'

def check_chunk_pbmin(files: dict[str, bytes]) -> None:
    """Fail the build if `prebuilt/core.js` does not contain the replacement.

    This exists because the failure it catches is silent. `swap_pbmin_into_tree()`
    works by file overwrite, so anything that reorders the pipeline, adds an
    `exports` map upstream, or re-installs between the swap and the chunk build
    puts upstream's codec back into the scene runtime with no error and no size
    change worth noticing.
    """
    chunk = files.get(CORE_CHUNK)
    if chunk is None:
        raise SystemExit(f'{CORE_CHUNK} missing from the blob')
    if PBMIN_MARKER not in chunk:
        raise SystemExit(
            f'{CORE_CHUNK} does not contain {PBMIN_MARKER.decode()} - the scene '
            'runtime is bundling a protobuf codec that is not the replacement.\n'
            'swap_pbmin_into_tree() must run against the same tree build_chunks() '
            'resolves against, and before it.')

    unmangled = [n for n in (b'__getOwnPropNames', b'__defProp', b'__hasOwnProp')
                 if n in chunk]
    if unmangled:
        raise SystemExit(
            f'{CORE_CHUNK} still has unmangled top-level names '
            f'({", ".join(n.decode() for n in unmangled)}), so rolldown gave up on '
            'renaming the whole bundle.\nAlmost always a direct eval() reintroduced '
            'into a bundled module - check inquire() in the pbmin source. Expect '
            'roughly +20% on this chunk while it lasts.')
    log(f'  chunk codec check: {CORE_CHUNK} contains the pbmin marker, '
        'top-level names mangled')

def add_pbmin(files: dict[str, bytes], kept_bytes: dict[str, int]) -> None:
    """Ship OUR `protobufjs/minimal` instead of upstream's.

    `node_modules/protobufjs/{package.json,index.js,minimal.js}`, where
    `index.js` is `src/vendor/pbmin/index.js` verbatim and `minimal.js` is a
    one-line re-export - the same shape as upstream's own `minimal.js`, so
    `require('protobufjs/minimal')` resolves to a file with the same name and
    no `exports` map is introduced.

    **What this replaces.** 13 files / 58,872 B of upstream `protobufjs` plus
    six whole `@protobufjs/*` micro-packages (`aspromise`, `base64`,
    `eventemitter`, `float`, `inquire`, `pool` - 25 files / 41,117 B) that only
    it imported. 38 files / 99,989 B out, 4 files / 48,340 B in.

    **Who reads it.** Three consumers in NODE:

    * `@dcl/ecs/dist-cjs` - the CRDT wire format `data_layer.rs` regenerates
      `main.crdt` through;
    * `@dcl/rpc` - the data-layer socket's framing;
    * `@dcl/inspector/data-layer.gen.js` - the 22-method service descriptor
      `build_service_descriptor()` emits.

    And, since `swap_pbmin_into_tree()`, the SCENE runtime as well: the same
    code is bundled into `prebuilt/core.js`, so QuickJS runs it too. The risk
    sits on the scene protocol's critical path in exchange for the toolchain
    having one codec instead of two.

    **Evidence.** `src/vendor/pbmin/tests`, run with `tests/setup.sh && bash
    tests/run-all.sh <seed>`. Per seed, against upstream 7.2.4 as the
    reference, asserting byte-identical encode in both directions, deep-equal
    decodes and throw-parity including error message:

    * all 336 `@dcl/ecs` message namespaces (142 unique types) x 300 instances
      x 4 environments (Buffer/no-Buffer x Long/no-Long) = 403,200 instances;
    * the ESM `dist/` build of the same corpus, 244 namespaces x 100;
    * all 41 namespaces of the two catalogues that are NOT `@dcl/ecs`
      (`@dcl/rpc/dist/protocol/index.js`, 11; the data-layer descriptor, 30)
      x 2000 x the same 4 environments = 328,000 instances, plus an assertion
      that all 22 descriptor methods' request and response types were among
      them;
    * 800,000 fuzz operations on random/truncated/corrupted input, where the
      two implementations must agree on WHICH inputs throw.

    Zero divergences, six seeds. Mutation-tested: 13 injected wire-format bugs,
    13 caught, and the rpc/descriptor phase catches two of them
    (`varint-length-boundary-off-by-one`, `fixed32-byte-order`) that the
    `@dcl/ecs` corpus phase does not.

    Two upstream inconsistencies are reproduced ON PURPOSE and must not be
    "fixed": `BufferWriter.string` and `Writer.string` encode lone surrogates
    differently, and `BufferReader.string` clamps a truncated length with
    `Math.min` where `Reader.string` throws.
    """
    kept_bytes.pop('protobufjs', None)
    total = 0
    src = os.path.join(PBMIN, 'index.js')
    with open(src, 'rb') as fh:
        core = fh.read()
    with open(os.path.join(PBMIN, 'LICENSE'), 'rb') as fh:
        licence = fh.read()
    out = {
        'node_modules/protobufjs/package.json':
            json.dumps(PBMIN_MANIFEST, indent=2).encode() + b'\n',
        'node_modules/protobufjs/LICENSE': licence,
        'node_modules/protobufjs/index.js': core,
        'node_modules/protobufjs/minimal.js': PBMIN_ENTRY,
    }
    for name, data in out.items():
        files[name] = data
        total += len(data)
    kept_bytes['protobufjs (pbmin)'] = total
    log(f'    node_modules/protobufjs <- {os.path.relpath(src, CRATE)}  '
        f'{len(core)} B, {len(out)} files')

SHIM_SOURCE_ONLY = ('.ts', '.proto')
SHIM_SOURCE_ONLY_NAMES = ('tsconfig.json',)
SHIM_GEN_TS = 'data-layer.gen.ts'
SHIM_GEN_JS = 'node_modules/@dcl/inspector/data-layer.gen.js'

def add_shim(files: dict[str, bytes], kept_bytes: dict[str, int]) -> None:
    """The `@dcl/inspector` stand-in: crdt dumper + minimal data-layer host.

    `dump-crdt` in `data-layer-host.mjs` needs `dumpEngineToCrdtCommands`, and
    its `serve` mode needs `createDataLayerHost` + `DataServiceDefinition`. The
    real package is 119 MB - ~90% browser editor UI - and ships separately as
    `inspector.zip`. Without this, every `.composite` scene fails build step 4
    with `Cannot find module '@dcl/inspector'`, and `start --data-layer` has no
    host to spawn.

    Three of the files copied here are generated, not written: see
    `scripts/dump-inspector-tables.cjs` for `component-schemas.json`,
    `minimal-composite.json` and `root-components.json`. All are version-pinned
    snapshots of a real `@dcl/inspector` and go stale silently when upstream
    adds a component or seeds new root state - re-run that script on every
    inspector bump.
    """
    total = 0
    for dirpath, _, filenames in os.walk(SHIM):
        for fn in sorted(filenames):
            src = os.path.join(dirpath, fn)
            rel = os.path.relpath(src, SHIM).replace(os.sep, '/')
            if rel.endswith(SHIM_SOURCE_ONLY) or rel in SHIM_SOURCE_ONLY_NAMES:
                continue
            with open(src, 'rb') as fh:
                data = fh.read()
            files[f'node_modules/@dcl/inspector/{rel}'] = data
            total += len(data)
    kept_bytes['@dcl/inspector'] = total

def build_service_descriptor(work: str, files: dict[str, bytes],
                             kept_bytes: dict[str, int]) -> None:
    """Transpile the data-layer service descriptor to CommonJS.

    `codegen.registerService` needs a descriptor: 22 methods, each with a
    `name`, `requestStream`/`responseStream` flags and a request/response type
    that can `encode`/`decode`. Upstream generates it from
    `packages/inspector/src/lib/data-layer/proto/data-layer.proto` with
    `protoc-gen-dcl_ts_proto` - a plugin from **`@dcl/ts-proto`, a DCL FORK**,
    not the `ts-proto` on npm - under
    `esModuleInterop=true,returnObservable=false,
    outputServices=generic-definitions,fileSuffix=.gen,oneof=unions,
    useMapType=true`.

    We vendor the OUTPUT (`src/vendor/inspector-shim/data-layer.gen.ts`,
    checked in verbatim beside the `.proto` it came from) and transpile it
    here, rather than reproducing the toolchain. Reproducing it would mean
    vendoring a ~5 MB Node codegen plugin plus `ts-proto-descriptors` /
    `case-anything` / `dprint-node` to regenerate a file that changes about
    once a year.

    The transpile has no dependencies of its own: the generated file's ONLY
    imports are `long` and `protobufjs/minimal`, both already in the blob.
    `--noCheck` is what makes this a *transpile* - the two imports resolve to
    packages whose `.d.ts` this blob deliberately drops, so a type check would
    report TS7016 on them and exit non-zero while emitting the same bytes.
    Checking them is not our job anyway; the file is upstream's generated
    artifact and was type-checked where it was generated.
    """
    src = os.path.join(SHIM, SHIM_GEN_TS)
    out = os.path.join(work, 'descriptor')
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    tsc = os.path.join(work, 'node_modules/typescript/lib/tsc.js')
    r = subprocess.run(
        ['node', tsc, '--noCheck', '--module', 'commonjs', '--target', 'es2020',
         '--esModuleInterop', '--skipLibCheck', '--outDir', out, src],
        capture_output=True, text=True)
    emitted = os.path.join(out, SHIM_GEN_TS[:-3] + '.js')
    if r.returncode != 0 or not os.path.isfile(emitted):
        sys.stderr.write(f'descriptor transpile failed:\n{r.stdout}\n{r.stderr}\n')
        raise SystemExit(1)
    with open(emitted, 'rb') as fh:
        data = fh.read()
    n = data.count(b'requestStream:')
    if n != 22:
        raise SystemExit(f'{SHIM_GEN_JS}: expected 22 methods, found {n}')
    files[SHIM_GEN_JS] = data
    kept_bytes['@dcl/inspector'] = kept_bytes.get('@dcl/inspector', 0) + len(data)
    log(f'    {SHIM_GEN_JS}  {len(data)} B, {n} methods')

ECS7_TSCONFIG = 'node_modules/@dcl/sdk/types/tsconfig.ecs7.json'
ECS7_REJECTED = ('downlevelIteration', 'suppressExcessPropertyErrors')
ECS7_SOURCE = 'library/js-sdk-toolchain/packages/@dcl/sdk/types/tsconfig.ecs7.json'

def check_ecs7_tsconfig(files: dict[str, bytes]) -> None:
    """The SDK's shared tsconfig must stay TypeScript 6/7 clean.

    EVERY scene extends this file - ours and third-party alike - so a
    deprecated option here lands in every scene's effective config. The tree's
    copy carries the fix (no `downlevelIteration`, which is emit-only and inert
    at `target: es2020`; no `suppressExcessPropertyErrors`, already the
    default; `moduleResolution: bundler`, the honest description of a
    rolldown-resolved toolchain and legal because `module` is `esnext`). TS 7
    rejects all three outright (TS5023) and `ignoreDeprecations` cannot
    silence them, so a catch-up with upstream that brings one back would stop
    every scene type-checking. This fails the build naming the option instead.
    """
    opts = json.loads(files[ECS7_TSCONFIG].decode('utf-8'))['compilerOptions']
    back = [o for o in ECS7_REJECTED if o in opts]
    if back:
        raise SystemExit(
            f'{ECS7_SOURCE} names {", ".join(back)}, which TypeScript 7 rejects '
            'in every scene that extends it. Drop the option(s) there.')
    if opts.get('module') != 'esnext' or opts.get('moduleResolution') != 'bundler':
        raise SystemExit(
            f'{ECS7_SOURCE} must keep module=esnext and moduleResolution=bundler '
            f'(found module={opts.get("module")!r}, '
            f'moduleResolution={opts.get("moduleResolution")!r}).')
    log(f'  tsconfig check: {ECS7_TSCONFIG} is TypeScript 7 clean')

PEER_TRUST_GLOBAL = '__dclOneAuthoritative'

def check_chunk_peer_trust(files: dict[str, bytes]) -> None:
    """Fail the build if the scene runtime lost the peer-trust gate.

    The auth-server line is where `isServer`, `registerMessages` and
    `@dcl/sdk/server` live, and upstream's client applies CRDT, state
    responses and room events ONLY from the sender named
    `AUTH_SERVER_PEER_ID` ('authoritative-server'). Every serverless
    multiplayer scene ever written (`syncEntity` with no server) depends on
    peers being trusted instead. So the tree's transport gates that check on
    `globalThis.__dclOneAuthoritative`, which the split loader sets from
    scene.json's `authoritativeMultiplayer` (`fromAuthority()` in
    `packages/@dcl/sdk/src/network/message-bus-sync.ts` and the room's
    CUSTOM_EVENT handler in `network/events/implementation.ts`;
    `CRDT_AUTHORITATIVE`, server corrections, stays server-only).

    The gate survives minification as the global's property name: once per
    gated handler, so at least two in `message-bus-sync.js` (three when
    rolldown inlines `fromAuthority` at both call sites). Removal condition:
    an upstream release whose client accepts peer CRDT without a server, or a
    decision that unflagged scenes need a server too (then drop the gate in
    the tree, this check, the loader's global and the README paragraph naming
    them).
    """
    core = files.get(CORE_CHUNK)
    if core is None:
        raise SystemExit(f'{CORE_CHUNK} missing from the blob')
    marker = PEER_TRUST_GLOBAL.encode()
    hits = core.count(marker)
    if hits < 2:
        raise SystemExit(
            f'{CORE_CHUNK} names {PEER_TRUST_GLOBAL} {hits} times, so the sync '
            'transport is the server-only build - the gate in '
            'library/js-sdk-toolchain/packages/@dcl/sdk/src/network is missing '
            'from the tree build_chunks() resolves.')
    if marker in files.get(SMART_CHUNK, b''):
        raise SystemExit(f'{SMART_CHUNK} bundles its own copy of @dcl/sdk/network')
    log(f'  peer-trust check: {CORE_CHUNK} gates {hits} handlers on {PEER_TRUST_GLOBAL}')
