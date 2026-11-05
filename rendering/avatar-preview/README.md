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
- `src/lib.rs`: the module. `start(canvas, baseDomain, onEvent)` boots the shared plugins,
  points the collectible resolver at the catalyst and waits for the first `outfit` command.
- `src/host.rs`: the host surface, below.
- `web/preview.js`: the `wearable-preview` iframe contract over the host surface.
- `web/index.html`: the page; `?debug=1` or the backquote key shows the console and the commands on screen.
- `build.sh`: `wasm-pack build --target web` into `web/pkg` with the Explorer's toolchain
  (`nightly-2026-04-15`); `./build.sh --no-opt` skips wasm-opt for iteration.

## Build and run

```
./build.sh
cd web && python3 -m http.server 5300
```

Open `http://localhost:5300/` in a WebGPU browser, or embed it in an iframe as a host embeds
`wearable-preview`. Release profile: `opt-level = "z"`, fat LTO, `panic = "abort"`,
`wasm-opt -Oz`. Measured on 2026-11-05: 24.9 MB raw, 8.3 MB gzip.

## The host surface (`src/host.rs`)

The module exports `start(canvas, baseDomain, onEvent)` and `preview_command(json)`.
Commands, as JSON objects with an `op`:

| `op` | fields | effect |
| --- | --- | --- |
| `outfit` | `bodyShape`, `wearables[]`, `skin`, `hair`, `eyes` (`#rrggbb`) | the avatar; the first one spawns it |
| `camera` | `alpha`, `beta` (radians), `radius` (m), `fov` (degrees), `orthographic`, `target[3]`, `autoRotate`, `autoRotateSpeed`, `interactive` | the orbit camera; unset fields keep their value |
| `background` | `color` (`#rrggbb` or null for transparent) | the clear colour |
| `emote` | `urn` (a urn or a base emote name), `loop` | plays on the avatar |
| `stopEmote` | | |
| `screenshot` | `id`, `width`, `height` | a PNG of the frame, scaled and cropped to fit |
| `metrics` | `id` | triangle, material, texture and mesh counts |

Events reach `onEvent` as JSON strings with a `type`: `load` (the avatar is dressed and
visible), `error {message}`, `emote {event: play|end|interrupted, urn}`, `screenshot {id,
png}` (base64), `metrics {id, triangles, materials, textures, meshes, bodies, entities}`.

## The iframe contract (`web/preview.js`)

The page speaks the `@dcl/schemas` preview protocol: options from the URL (`profile`,
`bodyShape`, `urn`, `skin`, `hair`, `eyes`, `emote`, `camera`, `projection`, `zoom`,
`zoomScale`, `offsetX/Y/Z`, `cameraX/Y/Z`, `autoRotateSpeed`, `disableAutoRotate`,
`background`, `disableBackground`, `disableDefaultEmotes`, `peerUrl`, `type`) and from
`update` messages; `ready` at boot, `load {renderer: "bevy"}`, `error`, `emote_event`
(`animation_play`, `animation_end`); `controller_request` dispatched to `scene`
(`getScreenshot`, `getMetrics`, `changeZoom`, `panCamera`, `changeCameraPosition`,
`setUsername`, `cleanup`), `emote` (`play`, `stop`, `isPlaying`, sound getters) and
`physics`, with `controller_response` back. Profiles resolve on the page through
`/lambdas/profiles/{id}`, so the engine only receives urns.

Not supported yet, answered with an error: `emote.pause`, `emote.goTo`, `emote.getLength`,
`physics.setSpringBonesParams`; `contract`/`item`/`token` lookups through the marketplace
API; `url`, `base64` and `blob` items; social emotes; the Unity `mode` flows.

## What it renders today

One avatar through the Explorer's own renderer (`avatar::AvatarRenderPlugin`, the `avatar`
crate without its `explorer` feature): body shape and wearables resolved through
`collectibles` over the catalyst's `entities/active` and `contents` endpoints, hides, face
masks, skin and hair tints, the scene material with the toon and outline tags, the idle
animation and emotes by urn or base name.

## Verifying without a browser window

Windows Chrome can be driven headless from WSL over the DevTools protocol
(`--headless=new --enable-unsafe-webgpu --remote-debugging-port`). Read the canvas with
`toDataURL` from the page; `Page.captureScreenshot` shows WebGPU canvases black. With
`?debug=1` the page keeps every host message in `window.__messages`, so a driver can post
`controller_request`s and read the responses.

## Next work

1. Look parity against the Explorer's Backpack preview: light rig, ambient, bloom.
2. Emote pause, seek and length; audio; inline (base64,
   blob) items for unpublished wearables.
3. A workflow that builds, size-checks and publishes the module, and runs on changes to the
   Explorer crates it links.
