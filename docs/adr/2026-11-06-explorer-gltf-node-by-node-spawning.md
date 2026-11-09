# ADR: glTF models are spawned and prepared node by node, up to a per-frame deadline

- Status: Accepted
- Date: 2026-11-06

## Context

The Explorer instantiates a glTF model in two steps, both in the `SpawnScene` schedule:

1. **Spawning.** Bevy's `SceneSpawner` writes the model's `Scene` into the world. Bevy 0.16 writes
   a queued instance in one call (`Scene::write_to_world_with`): it reflects, clones and inserts
   each component of each entity, one archetype move per component, and `scene_spawner_system`
   writes every queued instance in the same frame. Avatar bodies and wearables, emote props, the
   ghost and NFT frames spawn the same way.
2. **Processing.** `update_ready_gltfs` prepares each spawned model for the scene: per node it
   hashes the mesh and reuses a cached one, builds the collision shape, swaps Bevy's material for
   a scene material, hides collider meshes, adds colliders and triggers and records node names.
   It processed every ready model completely in the frame it became ready.

So a large model, or many models arriving together, cost one long frame, and nothing the Explorer
schedules could split that work. A Genesis Plaza arrival on `main` spent 57 ms in the spawner and
406 ms in processing over seven frames, with one frame at 328 ms of processing.

unity-explorer bounds its instantiation with a frame-time cap checked before each asset
(`FrameTimeCapBudget`, 33 ms, bypassed behind the loading screen). An earlier dcl-one engine, a
Bevy 0.20 fork, found that a budget per instance is not enough for large models and patched Bevy
to spawn a model a node at a time (`WorldSpawnerThrottle`, a per-node resumable cursor). That
source is no longer available; Bevy 0.20 itself still writes an instance in one call. Since PR
#274 the Explorer's Bevy is vendored in this repository, so the spawner can be changed here.

## Decision

Both steps run a node at a time and stop at one deadline per frame; what is left resumes on the
next frame, and a model stays hidden until both steps are done.

- **Resumable spawning.** `SceneSpawner` writes a `Scene` instance one source entity at a time
  (`Scene::write_entity_to_world`, split out of `write_to_world_with` with the same behaviour).
  `set_spawn_deadline(Some(instant))` stops the writing once the instant has passed and resumes
  it on the next run; each run writes at least one entity, so every instance completes.
  Instances are written one after another in request order. `None`, the default, spawns every
  queued instance whole, as stock Bevy does. Dynamic scenes and `spawn_sync` always spawn whole.
- **A part-written instance is hidden.** Its top-level entities are attached to the parent from
  the first run, in the order a whole spawn attaches them, and carry `Visibility::Hidden` until the
  instance is complete; then each gets back the visibility its scene gives it (none, if it had
  none). A root's own visibility written by a later slice is hidden again at once. Children
  written before their parents, skinned meshes without their joints and live lights therefore
  never show. `instance_is_ready` and `SceneInstanceReady` mean complete, as before, so every
  caller that waits for them is unchanged.
- **Cancelling mid-spawn leaves nothing behind.** Entities are allocated when an instance starts
  and attached only as they are written, so a despawned parent does not reach them all. Its parent
  despawned, `despawn_instance` and `unregister_instance` despawn every entity the instance
  allocated. A scene replaced (another `World`) or removed mid-spawn starts its instance over;
  a removed one waits for its scene to load again.
- **Hidden until revealed.** `spawn_as_child_hidden` keeps an instance hidden after it has
  spawned, until `reveal_instance_sync`. The Explorer spawns glTF containers that way and reveals
  them when processing is done, so Bevy's own materials and a model's collider meshes never show,
  even while processing waits for textures or for later frames. Hiding covers rendering only:
  processing adds colliders and triggers node by node, so for those frames a hidden model already
  blocks the player and takes pointer events where its processed nodes are.
- **Resumable processing.** `update_ready_gltfs` processes a model's nodes until the same
  deadline (`SceneSpawnerPlus::spawn_deadline`) and keeps its progress on the container
  (`GltfProcessing`: the node list and a cursor, the collider counter, the animation flag and the
  named nodes). Models already in progress go first; at least one node is processed a frame.
  Resetting or removing the container drops the progress.
- **The Explorer's slice.** `scene_runner::scene_spawn_slice` sets the deadline 4 ms after it runs
  each frame (`SpawnScene`, before `scene_spawner_system`), shared by spawning and processing, and
  none while the player is `OutOfWorld` (the loading backdrop covers the arrival, as
  unity-explorer's loading screen does) or on headless (`NoRenderApp`), so arrival and servers
  instantiate whole.
- **Instrumentation.** `spawn_timing` logs, under target `spawn_timing` at debug level, each frame
  that spends a millisecond or more in the scene spawner or in glTF processing, and the frame's
  slowest glTF slice: its time, how many nodes and which model.

## Rejected

- **A budget per instance only** (the earlier engine's first step, unity-explorer's granularity):
  one large model still costs its whole spawn and processing in one frame.
- **Slicing spawning alone**: processing cost seven times more than spawning on the arrival
  measured, so bounding the spawner alone would leave the hitch.
- **Spawning synchronously under the Explorer's own clock** (`SceneSpawner::spawn_sync` in an
  Explorer system): it skips the spawner's parent bookkeeping (`instance_parents`), and a single
  instance still runs to completion.
- **An Explorer-only spawner for glTF containers**: it would duplicate the spawner's instance
  bookkeeping and parent handling, and leave avatars, emote props, the ghost and NFT frames
  unbounded.
- **Writing each entity as one bundle** (Bevy 0.20's `BundleScratch`): one archetype move per
  entity instead of one per component would make each entity cheaper, but Bevy 0.16 has no safe
  equivalent; it needs unsafe component copies. Slicing bounds the frame without it.
- **Hiding by holding render components back, a hidden staging parent, or `RenderLayers`**: the
  first needs the spawner to know which components render, the second moves the hierarchy twice
  and breaks the Explorer's roots-under-the-container assumption, and the third is the same work
  as `Visibility` with lights and gizmos filtered differently.
- **Reading asset events to notice a replaced scene**: asset events reach readers after
  `PostUpdate`, so the spawner would write one more slice from the new scene with the old cursor;
  the scene world's `WorldId` changes when the asset is replaced and is checked before every slice.
- **Separate deadlines for spawning and processing**: one deadline bounds the frame's whole
  instantiation, which is what a hitch is made of; at least one entity and one node a frame keep
  both moving.
- **Slicing behind the loading backdrop and on headless**: the backdrop hides the hitches, and the
  arrival and the server want the content as soon as possible.

## Consequences

**Measured.** Native release builds were compared with `main` plus the same timing log, Windows,
60 fps, median of three interleaved autopilot runs per build:
- **Walking in.** The run arrives on the empty parcel −12,2 and walks 192 m east into Genesis
  Plaza at 6 m/s while measuring (`--autopilot-walk-to`, added for this). The plaza loads during
  play:

  | | `main` | this change |
  |---|---|---|
  | longest frame | 539 ms | 115 ms |
  | longest glTF processing in a frame | 412 ms | 41 ms |
  | longest scene spawner run in a frame | 20 ms | 7 ms |
  | frames over 50 ms | 10 | 5 |
  | worst 1 % of frames (mean) | 62.4 ms | 43.8 ms |
  | frames in the 90 s | 3,701 | 4,160 |
  | glTF processing in total | 621 ms | 545 ms |

  The total work is about the same; it is spread over about 100 frames instead of 17. The slowest
  slice left is a single node of about 40 ms (`clocktower_main.glb`): one node is the smallest
  unit, so a heavy collision shape still costs its build in one frame. One run had a 366 ms frame
  that glTF instantiation did not cause (its longest processing was 45 ms).
- **Standing at the arrival point** (arriving at 0,0, the arrival behind the loading backdrop):
  no difference within noise. The heavy instantiation happens behind the backdrop, which both
  builds run whole by design, before the measured 90 s begin.

**What else follows:**

- A model appears after about (its spawning and processing time) / 4 ms frames once it is
  requested, instead of in the frame it is requested; scenes that block their first ticks on their
  glTFs (`check_gltfs_ready`) start that much later.
- Avatars, emote props, the ghost and NFT frames are spawned in slices too, and appear
  correspondingly later.
- One node is the smallest unit: a single mesh whose collision shape takes longer than the slice
  still costs that time in one frame. The `spawn_timing` slowest-slice line shows when it happens.
- The cost per entity and per node is unchanged: the same work, spread out.

## Follow-ups

- The frame-time budget (E2): replace the fixed 4 ms with a deadline after the scene loop, order
  requests nearest first, and gate avatar builds.
- Collision shapes and mesh hashing off the main thread, as the earlier engine did: the walk-in
  shows single nodes of about 40 ms.
- One archetype move per entity (a `BundleScratch`-like writer), if a measurement shows the
  per-entity cost matters.
