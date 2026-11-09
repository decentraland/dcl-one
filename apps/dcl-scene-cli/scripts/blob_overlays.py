"""The checks that prove the tree-sourced behaviours reached the shipped files.

Imported by `build-base-blob.py`, never run. Nothing here rewrites a file:
the blob is the tree's packages plus the registry install, pruned by
`blob_collect.py`, and its two built products (the prebuilt chunks and the
declaration rollup) are build steps in `build-base-blob.py`. Two behaviours
used to be overlays on upstream's build and are now source in
`library/js-sdk-toolchain`; what lives here is the check that each one
reached the blob, so a catch-up with upstream that loses one fails the build
naming it:

* `check_ecs7_tsconfig()` - `@dcl/sdk/types/tsconfig.ecs7.json` names no
  option TypeScript 7 rejects and resolves as `bundler`.
* `check_chunk_peer_trust()` - the sync transport's peer-trust gate
  (`globalThis.__dclOneAuthoritative`, set from scene.json's
  `authoritativeMultiplayer` by the bundle banner and the split loader; unset
  counts as authoritative) is in `prebuilt/core.js`.
"""
from __future__ import annotations

import json

from blob_collect import CORE_CHUNK, SMART_CHUNK, log, strip_comments

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
    # A tsconfig may carry comments; tsc and rolldown read them, json does not.
    opts = json.loads(strip_comments(files[ECS7_TSCONFIG].decode('utf-8')))['compilerOptions']
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
    `globalThis.__dclOneAuthoritative`, which the bundle's banner and the
    split loader set from scene.json's `authoritativeMultiplayer` before any
    SDK code runs; a missing flag counts as `true` (`sceneHasAuthoritativeServer()`
    in `packages/@dcl/sdk/src/network/message-bus-sync.ts`, read by
    `fromAuthority()`, the `REQ_CRDT_STATE` responder beside it and the room's
    CUSTOM_EVENT handler in `network/events/implementation.ts`;
    `CRDT_AUTHORITATIVE`, server corrections, stays server-only).

    The gate survives minification as the global's property name: once, in
    `sceneHasAuthoritativeServer`, or more when rolldown inlines it at the
    call sites. Removal condition:
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
    if hits < 1:
        raise SystemExit(
            f'{CORE_CHUNK} names {PEER_TRUST_GLOBAL} {hits} times, so the sync '
            'transport is the server-only build - the gate in '
            'library/js-sdk-toolchain/packages/@dcl/sdk/src/network is missing '
            'from the tree build_chunks() resolves.')
    if marker in files.get(SMART_CHUNK, b''):
        raise SystemExit(f'{SMART_CHUNK} bundles its own copy of @dcl/sdk/network')
    log(f'  peer-trust check: {CORE_CHUNK} reads {PEER_TRUST_GLOBAL} {hits} times')
