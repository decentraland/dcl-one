# Vendored node_modules

`node_modules.zip` is extracted by `dcl-scene-cli init`, so a scaffolded scene builds and previews
with no package manager and no network. It holds the scene toolchain the Rust CLI uses: `@dcl/sdk`
(with `@dcl/ecs`, `@dcl/react-ecs`, `@dcl/js-runtime`, `@dcl/ecs-math` and `@dcl/smart-items`
inside its prebuilt chunks and declaration rollup), `@dcl/inspector-service` for the editor's data
layer, self-contained, and `typescript` for the type check. It is pure JavaScript, so one archive
serves Linux, macOS and Windows.

The other content of this folder is `registry/`: the manifest, workspace file (one install setting)
and lockfile of the registry packages the build installs: `typescript`, which ships, and the tree
packages' build-time dependencies, which do not.

## What the blob is

Every `@dcl/*` package this tree builds is this tree's own build: `@dcl/sdk`, `@dcl/ecs`,
`@dcl/js-runtime`, `@dcl/react-ecs`, `@dcl/smart-items` (the smart-item runtime) and
`@dcl/inspector-service` (the editor's data-layer host and composite readers, without the UI) come
from `library/js-sdk-toolchain`, whose components and scene APIs are generated from
`library/protocol`. `scripts/build-base-blob.py` packs each the way npm would publish it
(`pnpm pack`), stamps it with the scaffold's `@dcl/sdk` pin, rewrites the dependencies between them
to that version and drops the dependencies on packages the archive does not carry
(`@dcl/sdk-commands`, which never ships, and `@dcl/react-ecs` and `@dcl/ecs-math`, whose code is
inside the prebuilt chunks and the declaration rollup), so a shipped manifest names only packages
the extracted tree resolves. The one registry package the archive ships is `typescript`, the
scaffold's pin, through `registry/pnpm-lock.yaml`. The registry's `devDependencies` are what the
chunk build and the declaration rollup resolve from the tree packages' own dependencies: `react`
and `react-reconciler` (`@dcl/react-ecs`), `@dcl/ecs-math` 2.1.0, `protobufjs` 7.6.5, `long` and
`@protobufjs/utf8` (`@dcl/ecs`) and `mitt` (`@dcl/smart-items`); their code ends inside the chunks
and the service bundle, their declarations inside the rollup, and no file of theirs ships. Every
registry pin a tree manifest owns is checked against it. `scripts/blob_collect.py` prunes that
install tree to what is reachable; its pruning removes files and never edits one, and nothing else
rewrites a shipped file.

Two behaviours are source in `library/js-sdk-toolchain` and only checked, by
`scripts/blob_overlays.py`:

| Check | What it proves |
| --- | --- |
| `check_ecs7_tsconfig()` | `@dcl/sdk/types/tsconfig.ecs7.json` names no option TypeScript 7 rejects (`downlevelIteration`, `suppressExcessPropertyErrors`) and resolves as `bundler`. Every scene extends it. |
| `check_chunk_peer_trust()` | The core chunk gates the client's server-only checks (`message-bus-sync.ts`, `events/implementation.ts`) on `globalThis.__dclOneAuthoritative`, which the bundle banner and the split loader set from scene.json's `authoritativeMultiplayer` before any SDK code runs; a missing flag counts as `true`. With `true` the scene accepts CRDT, state responses and room events only from `authoritative-server` and clients do not answer `REQ_CRDT_STATE`; with `false` every participant is trusted and answers it. |

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
  the pin needs a packaging decision; a scene that installs its own `typescript` (5.x, 6.x or 7.x)
  is checked with that one (`src/typescript.rs`). The libraries are limited to ECMAScript and
  decorators. A scene that needs the DOM declarations must install them.
- Node `>=24` is declared by the scaffold's `package.json`.
- No third-party runtime package ships. `protobufjs` (7.6.5, `@dcl/ecs`'s pin), `long`, `mitt`
  and the rest are inside the chunks and the service bundle; a scene that imports one itself
  declares its own dependency, as it does `ethers`, which is not vendored either.
- `@dcl/ecs` ships as a manifest: `@dcl/sdk` depends on it at the scaffold pin, which only the
  archive serves. Its runtime is inside the chunks and the service bundle (`ecs`), its types inside
  the rollup.

## The data-layer host (`@dcl/inspector-service`)

`templates/data-layer-host.mjs` needs the editor's protocol, not its UI. `@dcl/inspector-service`
is the tree's host (`library/js-sdk-toolchain/packages/@dcl/inspector-service`): one self-contained
esbuild bundle, `dist/index.js`, carrying `createDataLayerHost`, `serveDataLayer` (the WebSocket rpc
server over it), `DataServiceDefinition`, the engine context, `dumpEngineToCrdtCommands`,
`dumpEngineToComposite`, `renameLegacyComposite` and `ecs`, the `@dcl/ecs` it runs on. Its build
leaves nothing external but node's modules and three optional natives required in a try/catch
(`bufferutil`, `utf-8-validate`, `encoding`). Its `dist/lib/**/*.d.ts` describe the editor's API and
do not ship.

- The host serves a live editing session: the editor connects, receives the engine state, pushes
  edits, and the host writes `assets/scene/main.composite`. The preview's watcher turns that into
  `main.crdt`. The same bundle regenerates `main.crdt` for a build.
- `@dcl/rpc` (the wire protocol), `ws` (the transport) and the bundle's other dependencies are inside
  it; `code-to-composite.mjs` and the goldens' runtime take the CRDT codec from its `ecs`.
- The editor UI is not in the archive. `/inspector/*` answers 404 until `DCL_ONE_INSPECTOR_DIR`
  names an `@dcl/inspector` package directory (`scripts/install-ui-designer-runtime.sh` installs
  one). When it does, that package's host is loaded instead of the scene's, so the UI and its data
  layer always share a version; a host without `serveDataLayer` is served from the `@dcl/rpc` and
  `ws` beside it, and its engines are made of the `@dcl/ecs` beside it.

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
entry by entry with the committed one, on the decompressed bytes (another zlib's deflate output
passes, a swapped file does not); the `Development` workflow's scene CLI job runs it, so the
archive cannot drift from `library/js-sdk-toolchain`, `library/protocol` or `registry/`. The steps,
in order:

1. **Install.** `pnpm install --frozen-lockfile` of `registry/` (hoisted, no scripts, no peer
   auto-install, devDependencies included) into the work tree, after checking every pin a tree
   manifest owns (the exact `dependencies` of `@dcl/sdk`, `@dcl/ecs`, `@dcl/smart-items` and
   `@dcl/inspector-service`) and the `typescript` pin against the scaffold's.
2. **Pack the tree.** `pnpm pack` of the six packages from `library/js-sdk-toolchain`, each
   extracted into the install tree, stamped with the scaffold pin; the dependencies between them
   are rewritten to that version and the dependencies on packages the archive never carries
   (`UNSHIPPED` in the script) are dropped. The script refuses to run on an unbuilt tree.
3. **Prune by code reachability.** `reachable()` in `scripts/blob_collect.py` walks the bare
   specifiers of the `.js` and `.d.ts` files that will ship, from `ENTRY_PACKAGES`.
   Declarations count, because `build --production` type-checks.
4. **Prune inside kept packages.** Symlinks, nested `node_modules`, source maps, `.md` files and
   test or docs directories go everywhere; six packages are allowlisted file by file
   (`FILE_ALLOWLIST`: `@dcl/ecs`, `@dcl/inspector-service`, `@dcl/js-runtime`, `@dcl/sdk`,
   `@dcl/smart-items`, `typescript`). `check_readme_allowlist()` fails the build if that sentence
   stops matching the dict. `@dcl/ecs` and `@dcl/js-runtime` keep their manifests;
   `@dcl/smart-items` keeps its declarations and `@dcl/inspector-service` its bundle; `typescript`
   keeps only `lib/tsc.js`, `lib/_tsc.js`, `package.json` and the ES `lib.*.d.ts` files that
   `tsc -p tsconfig.json --noEmit` reads. The build-time packages (`DROP_PACKAGES`) never ship.
5. **Checks.** `check_ecs7_tsconfig()` on the pruned tree.
6. **Build products.** `build_chunks()` (via `dcl-scene-cli vendor-chunks`), the two chunk checks,
   then `build_types_rollup()`.
7. **Resolver check.** Every bare specifier of every kept `.js` file must resolve, through the
   package's `exports` map where it has one; a declaration's may also land on a module the rollup
   declares. The scan must come back empty, or the script exits non-zero.
8. **Zip.** Deterministic: sorted paths, fixed 1980 timestamps, deflate per entry. Two builds of
   the same tree are byte-identical.
9. **Prove it.** In an empty directory run `dcl-scene-cli init`, `build --production`, then
   `start` and probe `/about`, with a scene that imports `@dcl/sdk/players`, `isServer`,
   `registerMessages` and `Storage` from `@dcl/sdk/server`. A composite scene must still reach
   `[4/5] main.crdt regenerated`.

The script creates the `--work` directory (default `.cache/scene-cli-blob`) and marks it with
`.build-base-blob-work`; it refuses a directory it did not create and removes only a marked one
at the end. `--keep-work` keeps it. `--reuse-install` reuses the previous tree as it is, skipping
steps 1 and 2, and implies `--keep-work`. `--check` passes with "same entries, different
compression" when every entry matches by name, size and CRC and only the deflate bytes differ
(another zlib).

To bump the `@dcl/sdk` or `typescript` pin, edit `src/templates/init/*/package.json` first; a
`typescript` bump also goes into `registry/package.json`, followed by `pnpm install
--lockfile-only` there to refresh the lock. A pin a tree manifest owns changes there first and in
`registry/package.json` second; the build names the two when they differ.

## Smart items (`@dcl/smart-items`)

`@dcl/smart-items` resolves offline; only its manifest and declarations ship.

- The runtime is inside `prebuilt/smart.js`, bundled out of the packed `@dcl/smart-items` with the
  rest of the SDK external (`split::smart_externals`). `build.rs` installs that chunk only for a
  scene that is an editor scene or whose bundle names the package (or `@dcl/asset-packs`, its
  former name, which the loader maps onto it). The chunk publishes `@dcl/smart-items`,
  `@dcl/smart-items/scripts` and every module of the package (`SMART_REGISTRY_KEYS` in
  `src/split.rs`, held equal to the package's `src/` by a test).
- The declarations ship as `dist/**/*.d.ts`, so `tsconfig.ecs7.json`'s `paths` for
  `@dcl/asset-packs` and a scene's `@dcl/smart-items/<module>` imports resolve; their imports of
  `@dcl/ecs`, `@dcl/react-ecs` and `@dcl/sdk/math` fall to the ambient rollup.
- Composites written before the rename carry `asset-packs::` component names. Every reader
  renames that prefix to `smart-items::` on load, rewrites the `core-schema::Sync-Components`
  ids of the renamed components to the new names' numbers and merges a renamed block into an
  existing `smart-items::` block of the same name: `dcl_scene_authoring::composite_norm` for the
  CLI and `library/dcl-scene-build`, `renameLegacyComposite` from `@dcl/inspector-service` for
  the data-layer host and the crdt dumper.
- Do not add `@dcl/sdk`'s `.d.ts` files to the blob. A `node_modules/@dcl/sdk` whose `types`
  resolve makes tsc prefer the on-disk tree over the rollup, and it drags in declarations nothing
  public re-exports.
