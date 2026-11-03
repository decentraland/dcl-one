# Architecture

Two views of the same functional / dataflow diagram. Boxes keep the same positions on both
pages, so flipping between them shows exactly what the platform changes.

- [Native](architecture-native.drawio.svg): the desktop client, with the `dcl_deno_ipc`
  sidecar running scene JS, the engine process, and the CEF processes hosting the HUD.
- [Web](architecture-web.drawio.svg): the Chrome build, with one sandbox worker per scene,
  the engine worker, the render worker, and the page main thread hosting the HUD.

The diagram is deliberately an overview: inaccurate and incomplete in places, but useful.

How to read it:

- Boxes are functional groups; the crates in each group are the small sub-boxes inside.
  Colours are by function: red = scene runtime and execution, orange = SDK component
  application, green = ECS and engine state, lilac = input and output, blue = backend services
  (ipfs, av, comms, wallet, social), yellow = the React HUD and its host.
- The dashed rounded outlines are process boundaries.
- The solid **core / platform independent** ring is the code that is the same on native and
  web. It runs through the middle of the backend boxes (the left half of each is
  platform-independent, the right half is platform-specific) and through the middle of output.
- The bevy ECS cloud is the bus. The "scene state" box inside it is the scene CRDT mirrored as
  bevy entities and components. Arrows into the cloud read and write world state; box-to-box
  arrows are flows that bypass the ECS (the scene ↔ engine ipc, the bridge-scene pipe, live
  media, the extracted render world).
- `dcl_types` appears on both sides of the sidecar boundary on purpose: its CRDT store and
  message types are used by both. `dcl_js`, the JS op surface, is scene-side only, so the
  engine never links `deno_core` or V8.
- Clouds and the stick figure are external systems and the person. Infrastructure crates are
  listed in one box with no arrows; `wallet` and `social` are drawn without dataflow arrows
  because they are capability providers used from many places.
