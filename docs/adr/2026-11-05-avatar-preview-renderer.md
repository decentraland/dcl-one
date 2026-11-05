# ADR: The avatar preview renderer is a separate workspace on the Explorer's crates

- Status: Accepted
- Date: 2026-11-05

## Context

The shop and the creator hub show avatars, wearables and emotes outside the world: item
pages, fitting rooms, outfit tiles, the wearable editor. Decentraland served that from
`wearable-preview`, a page embedded in an iframe, first on Babylon.js and then on a Unity
WebGL build of the avatar renderer, which never matched the explorer's look without manual
parity work and shipped a 56 MB module (20 MB compressed). With the Explorer on Bevy, the
same preview is to come from the Explorer's own avatar code: its wearable resolution,
materials, toon shading, outline, animation and emote playback, so that what the shop
shows is what the world renders.

The Explorer's web build is not a candidate for embedding: it is 119 MB of wasm, runs its
render world on a worker over shared memory (so the host page needs cross-origin isolation),
and boots a realm, comms and the system scene. A measured spike (single-threaded wasm linking
the Explorer's `assets`, `common`, `platform`, `console`, `ipfs`, `collectibles` and
`scene_material` crates by path) renders a base body with the Explorer's material and shaders
at 22 MB raw and 7.6 MB gzipped, needing no isolation headers.

## Decision

- `rendering/avatar-preview/` is the avatar preview renderer: its own Cargo workspace,
  lockfile, cargo configuration, build script, web page and release. It is not a member of
  the Explorer workspace and the Explorer never links it.
- It depends **by path** on named Explorer crates (today: `assets`, `common`, `platform`,
  `console`, `ipfs`, `collectibles`, `scene_material`; later the avatar build, animation
  and emote systems once they stand apart from the player, scene and comms glue) and repeats
  the Explorer's `[patch.crates-io]` table so both resolve the same engine fork, toolchain
  and shader pipeline. "Independent of the Explorer" means independent builds and releases,
  not independent code: the look is shared by construction, never copied.
- It builds single-threaded wasm (no shared memory, no atomics, no workers): the page that
  embeds it needs no cross-origin isolation, and one module serves any iframe host.
- Changes the preview needs in Explorer crates land in those crates behind features or as
  general fixes (for example the `assets` crate's `preview` feature, which embeds only the
  shaders, the ghost body and the idle clip); the preview carries no copies of Explorer code.
- The Explorer's CI does not gate on the preview. The preview's own workflow builds and
  publishes it, and runs on changes to the Explorer crates it links, so a change that breaks
  the preview is caught where it is made.
- The public contract is the one `wearable-preview` hosts already use (`@dcl/schemas`
  preview options, the iframe `postMessage` messages and the controller RPC), reached in
  stages: first the shop and creator hub surfaces in this repository, then the full Unity
  parity set.

This is an exception to the layout ADR's single-workspace expectation for Explorer code,
like `library/uuav`: a second workspace next to the Explorer's, consuming it by path.

## Consequences

- One avatar pipeline for the world and the previews; parity work happens once, in the
  Explorer crates.
- The Explorer crates gain a second consumer, so refactors there must keep the preview
  building (its workflow reports it) and crates the preview links stay free of player,
  scene and comms dependencies or gate them behind features.
- Upstream fixes the spike identified: the boimp bake plugin initialises its render-side
  resources before the render sub-app exists under asynchronous render creation; meshes the
  wearable loader loads for the main world only must be promoted by whoever spawns them, a
  step that belongs in `collectibles`; `ipfs` does not build with `ipfs_debug` off.
- The preview module must stay small: size-first release profile, the `assets` `preview`
  feature, and a size check in its workflow.
