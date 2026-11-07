"""Every rewrite `build-base-blob.py` applies on top of the install tree, and
the checks that prove the tree-sourced behaviour reached the shipped files.

Imported by that script, never run. A rewrite is anything the blob carries
that the SDK build plus the registry install would not produce, and every one
lives here so `src/vendor/README.md` can name them from a single place:

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
