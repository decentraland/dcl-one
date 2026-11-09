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
- `src/audio.rs`: plays the sounds the Explorer's emote code asks for (its `AudioEmitter`s),
  following the emote's pause and the host's sound toggle.
- `src/stage.rs`, `src/shadow_catcher.wgsl`: the light rig and the floor that shows only the
  avatar's shadow, so a transparent canvas still grounds it on the host page; the scene
  boundaries' cylinder and grid floor (`src/grid_01.png`, the Unity renderer's grid texture,
  copied unchanged from `decentraland/unity-explorer` (Apache-2.0) at `49c1644901`,
  `avatar-preview-renderer/Assets/Textures/`).
- `web/preview.js`: the `wearable-preview` iframe contract over the host surface.
- `web/compare.html`: wearable-preview's Unity renderer (`ref=<origin>`, default production) and
  this one side by side with the same preview options, with sliders that tune the light rig live.
- `web/harness.html`: embeds the page in an iframe from another origin, as the shop does, and
  lists the messages it sends (`?target=<origin>&page=<file>` plus the preview's own query).
  `web/blank.html` is a transparent page for checking the embedding itself.
- `tools/contract-check.mjs`: drives a page in headless Chrome like a host (waits for `load`,
  posts controller requests, saves the canvas, the screenshot and the page). `PROBE_JS`
  evaluates an expression in the page at the end, or from the start with `PROBE_EARLY=1`;
  `INIT_JS` runs before the page's own scripts (hiding `navigator.gpu` tests the WebGL2
  fallback); `TIMELINE` captures the page at set seconds after it opens.
- `web/ui/`: the Unity renderer's control icons, cursors and font (see its README).
- `web/index.html`: the page, with the Unity renderer's controls laid out over the canvas; `?debug=1` or the backquote key shows the console and the commands on screen.
- `build.sh`: `wasm-pack build --target web` with the Explorer's toolchain (`nightly-2026-04-15`),
  twice: `web/pkg` for WebGPU and `web/pkg-gl` for WebGL2 (`--features webgl2`), since the engine
  holds one backend per module. `./build.sh` is the shipping build (size-first release profile,
  wasm-opt, minutes); `./build.sh --fast` the iteration build (tuned dev profile, no wasm-opt, a
  larger module that never ships); `--webgpu-only` skips the WebGL2 module.

## Build and run

```
./build.sh
python3 tools/serve.py
```

Open `http://localhost:5300/`, or embed it in an iframe as a host embeds `wearable-preview`. The
page loads the WebGPU module where the browser offers a WebGPU adapter and the WebGL2 module
otherwise; `?backend=webgpu` or `?backend=webgl2` forces one. The WebGL2 module leaves out what
WebGL2 cannot do: the depth and normal prepasses (so the avatar has no outline), MSAA (FXAA
smooths the edges instead) and the scene-bounds cell table, which a preview never reads.

`?flags=` takes camera pipeline switches for diagnosis: `nobloom`, `nomsaa`, `sdr` (no HDR),
`clearsurface` (an explicit transparent surface clear).

Release profile: `opt-level = "z"`, fat LTO, `panic = "abort"`, `wasm-opt -Oz`. Measured: the
WebGPU module 24.9 MB raw, 8.3 MB gzip (2026-11-05); the WebGL2 module 25.2 MB raw, 8.4 MB gzip,
and an avatar on screen 3.2 s after the page opens, served locally (2026-11-06).

## The host surface (`src/host.rs`)

The module exports `start(canvas, baseDomain, onEvent)` and `preview_command(json)`.
Commands, as JSON objects with an `op`:

| `op` | fields | effect |
| --- | --- | --- |
| `outfit` | `bodyShape`, `wearables[]`, `skin`, `hair`, `eyes` (`#rrggbb`), `forceRender[]` (categories no other wearable hides), `hideFace` (no eyes, eyebrows or mouth) | the avatar; the first one spawns it |
| `item` | `urn` (or null), `bodyShape`, `skin`, `hair`, `eyes`, `keepHead` | the wearable shown on its own: a second avatar wearing only it, its body hidden (but the head, for a facial feature), framed once before `load` |
| `boundaries` | `shown` | the scene boundaries: a cylinder 1 m round and 3 m tall on a grid floor, framed with the avatar |
| `view` | `item` | the item on its own, or the avatar wearing it; a switch glides the camera across and starts the view over |
| `camera` | `alpha`, `beta` (radians), `radius` (m), `fov` (degrees), `orthographic`, `target[3]`, `autoRotate`, `autoRotateSpeed`, `interactive`, `lockAlpha`, `lockBeta` (the item view's vertical drag), `lockRadius`, `zoomMin`, `zoomMax`, `zoomStart` (factors over the framed view, closer is larger; a reset starts at `zoomStart`), `autoCenter` (bring a fitted subject's middle onto the view axis), `fitSubject` (fit the avatar whatever its size, the builder view) | the orbit camera; unset fields keep their value |
| `zoomBy` | `radius` (m) | a step in the Babylon preview's metres (its orbit stands 3.5 m off), positive pulls back, within the zoom range |
| `background` | `color` (`#rrggbb` or null for transparent) | the clear colour |
| `emote` | `urn` (a urn or a base emote name), `loop` | plays on the avatar |
| `stopEmote` | | |
| `pauseEmote`, `resumeEmote` | | hold the emote where it is, or let it run on |
| `seekEmote` | `seconds` | move the emote within its clip, keeping it paused or running |
| `sound` | `enabled` | emote sound on or off |
| `light` | `illuminance` (lux), `ambient`, `exposure` (stops), `yaw`, `pitch` (radians, relative to the camera), `shadow` (0..1), `shadowReach` (metres) | the light rig; unset fields keep their value. The toon shading ignores ambient light: `exposure` is what brightens or darkens the avatar |
| `collectible` | `kind` (`wearable` or `emote`), `urn`, `definition` (an entity definition json, by url), `files` (name to url) | an item not on the catalyst; send it before the outfit or emote naming the urn |
| `screenshot` | `id`, `width`, `height` (optional) | a PNG: with a size, rendered off screen at exactly that size (up to 4096 a side) by a camera copying the live one, its vertical framing kept and the requested aspect setting the width; without, the canvas as it is. Fails with `requestFailed` while a load is under way |
| `metrics` | `id` | triangle, material, texture and mesh counts |

Events reach `onEvent` as JSON strings with a `type`: `load` (the avatar is dressed and
visible), `error {message}`, `emote {event, urn, time, length, sound}` (`event` is `play`,
`end`, `interrupted`, `pause`, `resume`, `loop` or `playing`, the last about ten times a second;
`sound` is whether the emote has any, once its definition has loaded), `screenshot {id,
png}` (base64), `metrics {id, triangles, materials, textures, meshes, bodies, entities}`, and
`requestFailed {id, message}` for a request that could not be answered.

## The iframe contract (`web/preview.js`)

The page speaks the `@dcl/schemas` preview protocol: options from the URL (`contract` with
`item` or `token`, `profile` (an address, `default`, `defaultN` or a profile entity hash),
`bodyShape`, `urn`, `skin`, `hair`, `eyes`, `emote`, `camera`, `projection`, `zoom`,
`zoomScale`, `offsetX/Y/Z`, `cameraX/Y/Z`, `autoRotateSpeed`, `disableAutoRotate`,
`background`, `disableBackground`, `disableDefaultEmotes`, `peerUrl`, `marketplaceServerUrl`,
`env`, `type`, `mode`, `lockAlpha`, `lockBeta`, `lockRadius`, `disableSwitcher`, `disableFace`,
`disableDefaultWearables`, `showSceneBoundaries`, `showAnimationReference`) and from
`update` messages; `ready` at boot, `load {renderer: "bevy"}`, `error`, `emote_event`
(`animation_play`, `animation_end`); `controller_request` dispatched to `scene`
(`getScreenshot`, `getMetrics`, `changeZoom`, `panCamera`, `changeCameraPosition`,
`setUsername`, `cleanup`), `emote` (`play`, `stop`, `isPlaying`, sound getters) and
`physics`, with `controller_response` back. Profiles resolve on the page through
`/lambdas/profiles/{id}`, so the engine only receives urns.

Emote names map to the catalyst's base emotes (`clap`, `dab`, `dance`, `money`, `wave`,
`fist-pump`, `head-explode`) or to the poses in `web/emotes/` (`fashion`, `fashion-2..4`,
`love`, `walk`, `run`, `jump`). `url`, `base64` and `blob` items (unpublished wearables and
emotes, as the Builder sends them) register with the engine through the `collectible` command:
their files map to their urls, `data:` and `blob:` included, and an id that is not a urn maps to
`urn:decentraland:off-chain:base-avatars:preview-<id>` (or `base-emotes` for an emote).

A transparent background (`disableBackground`) presents the canvas with premultiplied alpha,
so the host page shows through around the avatar, with the avatar's shadow on it.

The look follows the Explorer's lobby preview (its noon sun, about 7000 lux, raised to 8000, and the ambient its
camera layer overrides to) and the Unity renderer's framing and light placement, measured with
`web/compare.html`: the camera and the key light stay put and the subject turns under them, and bloom
takes only what is brighter than white (emissive parts). Open differences from the Unity
renderer: its colours are deeper and more saturated, and it draws a soft light pool under the
feet.

## Parity with the Unity renderer

The target is the Unity renderer's behaviour, not only its look. In place so far:

- marketplace mode's controls, drawn over the canvas as its UI is (`web/ui/`, its 700x700
  reference scaled to the panel): the emote play/stop pill and the mute button for an emote
  item (mute only when it has sound), the loading spinner (`disableLoader`), the thumbnail frame
  (`showThumbnailBoundaries`), its cursors; `hideControls`, and the framing options the
  wrapper hid them for (`zoom`, `wheelZoom`, `showThumbnailBoundaries`, `camera=static`);
- its interaction: the camera stays put and the subject turns in front of it, under a light fixed
  in the world; drag turns on yaw with inertia, auto-rotation (0.2 rad/s) resumes a second
  after the last drag, in marketplace mode only and not while an emote item plays; right drag
  pans within 2 m; the wheel zooms between 1/1.5 and 2 times the framed distance; input in
  marketplace and builder modes only, four times the drag speed in builder;
- its zoom options, by its rule (`ZoomRange`) after the Babylon preview's: `zoom` (0..100) is the
  closest view, `wheelZoom` how far the wheel pulls back from it, `wheelStart` (100 the closest,
  50 by default) where the view starts, `zoomScale` extra zoom over the rest; `changeZoom` and
  `changeCameraPosition`'s radius step in Babylon's metres within that range;
  `disableAutoCenter` aims at the subject's own origin (an avatar's feet) instead of its middle,
  the offsets acting from there;
- its floor: the shadow catcher and, under it, the glow around the feet (the light rig's `glow`, 0 by
  default for now);
- its framing of avatars too big for the default view, set once before the avatar is seen: after the
  avatar or outfit changes, the page keeps its loading state while the preview waits for the outfit
  and the emote, steps the paused emote through its clip, measures the skinned surface at each pose
  and sets the camera; `load` follows and the camera then moves only for the user. The fit raises or
  lowers the view but the avatar always turns about its own vertical axis.
- its item view: one wearable on its own (not an emote, nor several items), standing 5 m to the
  avatar's left as in the renderer, its body hidden and casting no shadow, with its own glow under
  it (each view shows its own floor); centred and fitted to its height and its widest turn with a 5%
  margin, and turning on both axes (the tilt eases back upright). A switch glides the camera from
  one subject to the other in 0.3 s, eased, as the renderer blends its cameras. The view switcher
  (`disableSwitcher` hides it) and the view it opens on: `type`, else the one last picked (kept in
  the page's storage, as the renderer keeps it), else the item. An item with no representation for
  the avatar's body shape locks the view to it, shown in the body shape it was made for, with the
  renderer's tooltip. The previewed items' categories are force-rendered on the avatar.

- its screenshots: `getScreenshot(width, height)` renders the view off screen at exactly that size
  with the live look and vertical framing, no controls; without a size, the canvas as it is; an
  invalid size, one over 4096, or a request while loading fails as the renderer's does.

- its builder mode (the publishing tool, which takes its thumbnails from it): no profile, but the
  page's own items and the requested urns (the page's win a category), dressed with the renderer's
  base wearables where nothing covers a category (the face's too unless `disableFace`, which also
  clears the face's features), a male body unless one is asked for, the builder's skin `#cc9b76` and
  black hair and eyes; only an emote of the page's own plays. Its view fits the avatar whatever its
  size, as the renderer's builder camera does;
- its scene boundaries (`showSceneBoundaries`, `showAnimationReference`): the translucent cylinder,
  1 m round and 3 m tall, on its 4 m grid floor, which stands in for the shadow and glow; the
  avatar's framing grows to take it in.
- its profile mode: the profile's avatar as it is (`default1` without a `profile`; urns and items
  are not worn), from the renderer's fixed profile camera: 7 m in front at a 20 degree lens, looking
  8.3 degrees down at 0.9 m, never fitted to the avatar. A drag still turns it; no zoom, pan or
  auto-rotation.
- its authentication mode (the auth dapp's sign-in and quick setup pages): profile mode on the
  renderer's platform, the gold disc with the glowing rim the avatar stands on and turns with
  (`src/platform/`), at its 0.7 scale 0.0813 m down. Its colour and glow are calibrated against
  the renderer's captures (`platformBase`, `platformEmission` on `web/compare.html`), as this
  light rig and tonemapping are the toon avatar's.
- its jesus mode (the download page): profile mode from a camera 0.32 m higher, the avatar
  floating in the renderer's own looping animation (`web/emotes/particles.glb`, whatever `emote`
  says), with glowing specks rising off random vertices of its posed surface (`src/effect.rs`, a
  preview feature with no Explorer counterpart; tuned on `web/compare.html`).

A named default emote plays once in the marketplace, then the avatar idles; it loops in the
builder.

Still to come: configurator mode.

Waiting on the Explorer: the preview renders what the Explorer's crates render, so a feature the
Explorer lacks waits for it there. Social emotes (the Explorer reads no `startAnimation` or
`outcomes` and plays no second avatar; its comms protocol reserves the fields) and spring bones (it
keeps a wearable's extra bones on the skeleton but simulates none) come once the Explorer has them.

The emote controller follows the Unity renderer: `goTo` on a stopped emote starts it and holds
it at that time, `play` resumes a paused one, and emote events wait for `load`.

Not supported yet, answered with an error: `physics.setSpringBonesParams`; social emotes.
`mode=configurator` renders as the marketplace does for now.

## Embedding it in the shop

The shop (`decentraland/shop`) embeds the preview through `decentraland-ui2`'s `WearablePreview`
and takes the iframe origin from `VITE_WEARABLE_PREVIEW_URL`. Pointing that at this page (no
trailing slash: the host filters messages by origin) switches every preview in the shop to this
renderer: item pages by contract and item or token id, outfits by profile and urns, promo tiles.
Items resolve through the marketplace API the host names (`marketplaceServerUrl`, else by
`env`), profiles through the host's `peerUrl`, whose domain also becomes the engine's catalyst.

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
