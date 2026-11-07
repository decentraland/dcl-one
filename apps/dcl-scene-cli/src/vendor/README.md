# Vendored node_modules

`node_modules.zip` is extracted by `dcl-scene-cli init`, so a scaffolded scene builds and previews
with no package manager and no network. It holds the scene toolchain the Rust CLI uses: `@dcl/sdk`
(with `@dcl/ecs`, `@dcl/react-ecs`, `@dcl/js-runtime`, `@dcl/ecs-math` and their runtime
dependencies) plus `typescript` for the type check. It is pure JavaScript, so one archive serves
Linux, macOS and Windows.

Other contents of this folder: `inspector-shim/` (the `@dcl/inspector` stand-in) and `registry/`
(the manifest and lockfile of the registry packages the archive ships or bundles).

## What the blob is

Every `@dcl/*` package is this tree's own build. The SDK (`@dcl/sdk` with the smart-item runtime
at `@dcl/sdk/smart-items`, `@dcl/ecs`, `@dcl/js-runtime`, `@dcl/react-ecs`) comes from
`library/js-sdk-toolchain`, whose components and scene APIs are generated from `library/protocol`;
`@dcl/ecs-math` comes from `library/ecs-math` and `@dcl/rpc` from `library/rpc`.
`scripts/build-base-blob.py` packs each the way npm would publish it (`pnpm pack`), stamps the SDK
packages with the scaffold's `@dcl/sdk` pin, keeps the other two at their manifest's version and
rewrites the dependencies between them to the shipped versions. Everything else comes from the
registry through `registry/pnpm-lock.yaml`: `typescript` (the scaffold's pin), `protobufjs` 7.2.4
(`library/protocol`'s pin; both are checked), `react`, `react-reconciler`, `ws`, `mitt`, `long`,
`@protobufjs/utf8` and the other `@protobufjs/*` packages `protobufjs/minimal` requires.
`scripts/blob_collect.py` prunes that install tree to what is reachable; its pruning removes files
and never edits one. `scripts/blob_overlays.py` holds the rewrites:

| Function | What the blob carries that the install tree did not |
| --- | --- |
| `add_shim()` | A hand-written `@dcl/inspector` from `inspector-shim/`. |
| `build_service_descriptor()` | `data-layer.gen.js`, transpiled at build time from the checked-in `data-layer.gen.ts`. |

Two behaviours are source in `library/js-sdk-toolchain` and only checked here:

| Check | What it proves |
| --- | --- |
| `check_ecs7_tsconfig()` | `@dcl/sdk/types/tsconfig.ecs7.json` names no option TypeScript 7 rejects (`downlevelIteration`, `suppressExcessPropertyErrors`) and resolves as `bundler`. Every scene extends it. |
| `check_chunk_peer_trust()` | The core chunk gates the client's server-only checks (`message-bus-sync.ts`, `events/implementation.ts`) on `globalThis.__dclOneAuthoritative`. A scene with `authoritativeMultiplayer` accepts CRDT, state responses and room events only from `authoritative-server`; other scenes keep peer trust. |

Two build steps add files without editing upstream ones: `build_chunks()` writes
`@dcl/sdk/prebuilt/{core,smart}.js` and `registry.json` (the SDK runtime), and
`build_types_rollup()` writes `@dcl/js-runtime/index.d.ts`, the rolled-up ambient declarations.
Keep the license files the build script retains.

## Pins and constraints

- The `@dcl/sdk` version is the scaffold's pin in `src/templates/init/*/package.json`
  (`@dcl/js-runtime` must match). It names the upstream base the tree's SDK derives from and a
  dcl-one iteration; it is not on npm, so a scene cannot reinstall it from the registry until the
  SDK is published from this tree. `init.rs::blob_tracks_the_scaffold_pin_and_carries_the_ecs_fixes`
  checks the shipped archive against the pin.
- The SDK provides `isServer`, `registerMessages`, `getRoom`, `@dcl/sdk/server` (`Storage`,
  `EnvVar`), authoritative component writes, `CreatedBy` and `AvatarNametag`. Keep
  `schema_crdt.rs` and the CRDT fixtures aligned with its wire format.
- Comms message ids: CRDT 7, REQ_CRDT_STATE 8, RES_CRDT_STATE 9, CRDT_SERVER 4,
  CRDT_AUTHORITATIVE 5, CUSTOM_EVENT 6. Both ends of a preview use this runtime.
- For authoritative previews, `split-loader.js` maps mini-comms' zero address to
  `authoritative-server`, and `host-runtime.mjs` frames inbound messages as
  `[senderLen][sender][payload]`.
- TypeScript 6.0.3 is pure JavaScript. TypeScript 7 uses platform-native packages, so changing
  the pin needs a packaging decision. The libraries are limited to ECMAScript and decorators. A
  scene that needs the DOM declarations must install them.
- Node `>=24` is declared by the scaffold's `package.json`.
- `protobufjs` ships as `protobufjs/minimal` (`Reader`, `Writer`, `util.Long`, `configure`), not
  the reflection library. A scene that imports the full library declares its own dependency.
- `ethers` is not vendored. A scene that uses it declares it.

## The `@dcl/inspector` stand-in

`templates/data-layer-host.mjs` needs `@dcl/inspector` for its protocol, not its UI.
`inspector-shim/` provides the protocol only (about 0.18 MB).

| File | Role |
| --- | --- |
| `engine-to-composite.js` | Ports of upstream `dumpEngineToCrdtCommands` and `dumpEngineToComposite` |
| `engine.js` | An `@dcl/ecs` engine with the editor's component set defined |
| `host.js` | The data-layer host: 4 live RPC methods, 18 inert stubs |
| `component-schemas.json` | 59 component schemas, a pinned snapshot of a real inspector |
| `minimal-composite.json` | The composite written at boot when a scene has none |
| `root-components.json` | The `smart-items` entries seeded on entity 0 after every load |

- The live methods are `crdtStream`, `save`, `getInspectorPreferences` and
  `setInspectorPreferences`. That is enough for a live editing session: the editor connects,
  receives the engine state, pushes edits, and the host writes `assets/scene/main.composite`.
  The preview's watcher turns that into `main.crdt`.
- The 18 stubs return well-formed empty responses, because `@dcl/rpc` binds every key of the
  descriptor. The asset panel stays empty and undo/redo is not implemented.
- Scene-metadata edits made against this host do not reach `scene.json`.
- Autosave is a trailing-edge debounce (100 ms floor) over engine changes. Preferences persist in
  `inspector-preferences.json`.
- The service descriptor has 22 methods. `build_service_descriptor()` fails the build if the
  transpiled file declares another count.
- `@dcl/rpc` and `mitt` are vendored in pruned form. `@well-known-components/pushable-channel` is
  not, because `@dcl/rpc/dist/push-channel` exports a compatible `AsyncQueue`.
- The editor UI is not in the archive. `/inspector/*` answers 404 until
  `pnpm add -D @dcl/inspector` or `DCL_ONE_INSPECTOR_DIR` supplies one.

### Refresh the pinned snapshots

`component-schemas.json`, `minimal-composite.json` and `root-components.json` come from an
installed `@dcl/inspector` (currently 7.43.2). They cannot be derived from the blob. Re-run
`scripts/dump-inspector-tables.cjs` on every inspector bump, against the install that
`scripts/install-inspector-host.sh` produces (`~/.cache/dcl-scene-cli/inspector-host`); the dump
renames the smart-item components the registry inspector still calls `asset-packs::*` to the
`smart-items::` namespace and recomputes their component numbers.

A stale table is a data-loss bug. The `@dcl/ecs` receive loop only re-broadcasts a component it
does not know, so an edit to an undefined component is accepted, echoed and then dropped on the
next save.

## Regenerate the blob

From the repository root, inside the Nix shell:

```sh
SDK_PROTOC=protoc make -C library/js-sdk-toolchain install build
(cd services/catalyst && cargo build -p dcl-scene-cli)
python3 apps/dcl-scene-cli/scripts/build-base-blob.py [--work DIR] [--cli-bin PATH]
                                                      [--reuse-install] [--keep-work]
(cd services/catalyst && cargo build -p dcl-scene-cli && UPDATE_GOLDEN=1 cargo test -p dcl-scene-cli --test golden)
```

The first `cargo build` only has to contain rolldown and `src/split.rs`; the second embeds the new
archive, and the goldens record its hash. `--check` builds the archive in memory and compares it
entry by entry with the committed one; the `Scene CLI journeys` workflow runs it, so the archive
cannot drift from `library/js-sdk-toolchain`, `library/protocol` or `registry/`. The steps, in
order:

1. **Install.** `pnpm install --frozen-lockfile` of `registry/` (hoisted, no scripts, no peer
   auto-install) into the work tree, after checking its `typescript` pin against the scaffold's
   and its `protobufjs` pin against `library/protocol`'s.
2. **Pack the tree.** `pnpm pack` of the four SDK packages from `library/js-sdk-toolchain` and of
   `library/ecs-math` and `library/rpc`, each extracted into the install tree; the SDK packages
   are stamped with the scaffold pin, the other two keep their version, and the dependencies
   between them are rewritten to the shipped versions. The script refuses to run on an unbuilt
   tree.
3. **Prune by code reachability.** `reachable()` in `scripts/blob_collect.py` walks the bare
   specifiers of the `.js` and `.d.ts` files that will ship, from `ENTRY_PACKAGES`.
   Declarations count, because `build --production` type-checks.
4. **Prune inside kept packages.** Symlinks, nested `node_modules`, source maps, `.md` files and
   test or docs directories go everywhere; eight packages are allowlisted file by file
   (`FILE_ALLOWLIST`: `@dcl/ecs`, `@dcl/js-runtime`, `@dcl/rpc`, `@dcl/sdk`, `long`, `mitt`,
   `protobufjs`, `typescript`). `check_readme_allowlist()` fails the build if that sentence stops
   matching the dict. `protobufjs` keeps `minimal.js` and its require closure
   (`PROTOBUFJS_MINIMAL`). `typescript` keeps only `lib/tsc.js`, `lib/_tsc.js`, `package.json`
   and the ES `lib.*.d.ts` files that `tsc -p tsconfig.json --noEmit` reads.
5. **Rewrites and checks.** The functions in the two tables above, in the order `main()` runs them.
6. **Build products.** `build_chunks()` (via `dcl-scene-cli vendor-chunks`), the two chunk checks,
   then `build_types_rollup()`.
7. **Resolver check.** Every bare specifier of every kept file must resolve. The scan must come
   back empty, or the script exits non-zero.
8. **Zip.** Deterministic: sorted paths, fixed 1980 timestamps, deflate per entry. Two builds of
   the same tree are byte-identical.
9. **Prove it.** In an empty directory run `dcl-scene-cli init`, `build --production`, then
   `start` and probe `/about`, with a scene that imports `@dcl/sdk/players`, `isServer`,
   `registerMessages` and `Storage` from `@dcl/sdk/server`. A composite scene must still reach
   `[4/5] main.crdt regenerated`.

`--reuse-install` keeps the previous `--work` tree (default `.cache/scene-cli-blob`), skipping
steps 1 and 2.

To bump the `@dcl/sdk` or `typescript` pin, edit `src/templates/init/*/package.json` first; a
`typescript` bump also goes into `registry/package.json`, followed by `pnpm install` there to
refresh the lock. To change a registry extra, edit `registry/package.json` the same way.

## Smart items (`@dcl/sdk/smart-items`)

`@dcl/sdk/smart-items` resolves offline, and no file of it ships.

- The runtime is inside `prebuilt/smart.js`, bundled out of the packed `@dcl/sdk` with the rest
  of `@dcl/sdk/*` external (`split::smart_bundled`). `build.rs` installs that chunk only for a
  scene that is an editor scene or whose bundle names the module. The chunk publishes
  `@dcl/sdk/smart-items`, `@dcl/sdk/smart-items/scripts` and the deep paths the creator docs
  import (`SMART_REGISTRY_KEYS` in `src/split.rs`).
- The declarations are inside the ambient rollup at `@dcl/js-runtime/index.d.ts`
  (`ROLLUP_FULL_SUBPATH_PKGS` in `scripts/build-base-blob.py` walks every `.d.ts` of
  `@dcl/sdk`).
- Composites written before the rename carry `asset-packs::` component names. Every reader
  renames that prefix to `smart-items::` on load: `dcl_scene_authoring::composite_norm` for the
  CLI and `library/dcl-scene-build`, `inspector-shim/host.js` for the data-layer host and
  `templates/data-layer-host.mjs` for the crdt dumper.
- Do not add `@dcl/sdk`'s `.d.ts` files to the blob. A `node_modules/@dcl/sdk` whose `types`
  resolve makes tsc prefer the on-disk tree over the rollup, and it drags in declarations nothing
  public re-exports.
