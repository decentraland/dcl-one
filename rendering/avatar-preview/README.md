# Avatar preview renderer

The Explorer's avatar look as a standalone wasm module: wearable resolution, materials,
toon shading and outline from the Explorer crates, built single-threaded so any page can
embed it without cross-origin isolation. It is what the shop and the creator hub will show
for avatars, wearables and emotes, replacing the Unity renderer slot of
`decentraland/wearable-preview`. Architecture:
[ADR 2026-11-05](../../docs/adr/2026-11-05-avatar-preview-renderer.md).

## Layout

- `Cargo.toml`: its own workspace. Explorer crates come in by path from
  `../bevy-explorer/source/crates`; the `[patch.crates-io]` table repeats the Explorer's so
  both resolve the same engine fork. `Cargo.lock` is seeded from the Explorer's (a fresh
  resolution fails on the yanked `core2 0.4.0` that `multihash-codetable` needs).
- `.cargo/config.toml`: wasm flags without shared memory or atomics. `DCL_ONE_ROOT` comes
  from the repository root's cargo configuration.
- `src/lib.rs`: the module. `start(canvas, bodyShape, baseDomain, flags)` boots the shared
  plugins, points the collectible resolver at the catalyst, renders one body shape.
- `web/index.html`: a page that loads `web/pkg` and mirrors the console on screen.
- `build.sh`: `wasm-pack build --target web` into `web/pkg` with the Explorer's toolchain
  (`nightly-2026-04-15`); `./build.sh --no-opt` skips wasm-opt for iteration.

## Build and run

```
./build.sh
cd web && python3 -m http.server 5300
```

Open `http://localhost:5300/` in a WebGPU browser. Query parameters: `bodyShape` (a
base-avatars URN; default `basefemale`), `wearables` (comma-separated wearable URNs),
`baseDomain` (default `decentraland.org`), `flags` (`cube` adds a reference cube).

Release profile: `opt-level = "z"`, fat LTO, `panic = "abort"`, `wasm-opt -Oz`. Measured
on 2026-11-05: 22.9 MB raw, 7.6 MB gzip.

## What it renders today

One avatar through the Explorer's own renderer (`avatar::AvatarRenderPlugin`, the `avatar`
crate without its `explorer` feature): body shape and wearables resolved through
`collectibles` over the catalyst's `entities/active` and `contents` endpoints, hides, face
masks, skin and hair tints, the scene material with the toon and outline tags, and the idle
animation. No emote playback control, camera API or host API yet.

## Verifying without a browser window

Windows Chrome can be driven headless from WSL over the DevTools protocol
(`--headless=new --enable-unsafe-webgpu --remote-debugging-port`). Read the canvas with
`toDataURL` from the page; `Page.captureScreenshot` shows WebGPU canvases black.

## Next work

1. The host API: the `wearable-preview` iframe contract (`@dcl/schemas` options,
   `postMessage` messages, controller RPC) over this module.
2. Look parity against the Explorer's Backpack preview: light rig, ambient, bloom.
3. Emotes with audio and events; sized transparent captures; metrics; inline (base64,
   blob) items for unpublished wearables.
4. A workflow that builds, size-checks and publishes the module, and runs on changes to the
   Explorer crates it links.
