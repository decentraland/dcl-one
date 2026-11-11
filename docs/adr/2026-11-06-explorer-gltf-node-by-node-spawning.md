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

An earlier dcl-one engine, a Bevy 0.20 fork, found that a budget per instance is not enough
for large models and patched Bevy to spawn a model a node at a time (`WorldSpawnerThrottle`, a
per-node resumable cursor). That source is no longer available; Bevy 0.20 itself still writes an instance in one call. Since PR
#274 the Explorer's Bevy is vendored in this repository, so the spawner can be changed here.

## Decision

Both steps run a node at a time and stop at one deadline per frame; what is left resumes on the
next frame, and a model stays hidden until both steps are done.

- **Resumable spawning, parents first.** `SceneSpawner` writes a `Scene` instance one source
  entity at a time (`Scene::write_entity_to_world`, split out of `write_to_world_with` with the
  same behaviour). `set_spawn_deadline(Some(instant))` stops the writing once the instant has
  passed and resumes it on the next run; each run writes at least one entity, so every instance
  completes. Instances are written one after another in request order. `None`, the default,
  spawns every queued instance whole with stock Bevy's `write_to_world_with`, its roots attached
  after writing. Dynamic scenes and `spawn_sync` always spawn whole.
- **The progress is on the entities.** Every entity of an instance is allocated when it starts:
  Bevy's entity mapper turns a reference to an entity it has not seen into a dead id, so they must
  all exist before the first write. Each carries a sparse-set `UnwrittenSceneEntity` marker, with
  its scene entity, until it is written, and a second one, `SceneEntityReady`, once its parent is
  written: the top-level ones first, and writing an entity passes it on to the entity's children.
  A cached query finds the next ready entity, so the spawner keeps no list of entities and walks
  no archetypes after the allocation, which is stock's own loop. When the instance completes,
  entities despawned before they were written are dropped from their written parent's `Children`,
  which writes fill without relationship hooks, and a `Children` left empty is removed, as Bevy's
  hooks do. The instance is finished in the world it started in: continuing it, despawning it or
  running the spawner's system on another world meanwhile panics before anything changes, since
  the allocated ids mean nothing there.
- **A part-written instance is hidden.** When a run leaves an instance part-written, its
  top-level entities are attached to the parent, in the order a whole spawn attaches them, and
  carry `Visibility::Hidden` until the instance is complete, with a `HiddenSceneRoot` component
  holding the visibility their scene gives them. Then each gets that visibility back, unless
  something changed it while hidden; a root whose scene gives it none is left `Inherited`, since
  something may require the visibility components by then. A root's own visibility written by a
  later slice is hidden again at once. Hiding the top-level entities hides what inherits from
  them: a descendant with an explicit `Visibility::Visible` still shows (glTF nodes are all
  `Inherited`). Writing parents first is what makes the hiding hold. Bevy resolves an `Inherited` child whose parent has no visibility components yet as
  visible (`visibility_propagate_system`), so a child written before its parent, as archetype
  order does, would show at the origin; a test with Bevy's propagation checks that nothing
  written shows between slices. `instance_is_ready` and `SceneInstanceReady` mean complete, as
  before, so every caller that waits for them is unchanged.
- **Cancelling mid-spawn leaves nothing behind.** Entities are attached only as their instance is
  hidden or complete, so a despawned parent does not reach them all. Its parent despawned,
  `despawn_instance` and `unregister_instance` despawn every entity the instance allocated, and
  drop it if it is still queued. A scene replaced (another `World`) or removed mid-spawn starts
  its instance over; a removed one waits for its scene to load again. Every entity not written
  yet carries a marker, so what is left is known exactly. When none is ready although some are
  unwritten, an unwritten entity whose parent was despawned by others goes too, as it would with
  a whole spawn despawned afterwards, and the instance completes with what is left, so its ready
  event fires; one whose parent is written but does not list it among its children (a hierarchy
  that does not reach every entity) is made ready and written in slices; in a cycle, every
  unwritten entity is. Nothing starts over or is written whole. An instance that fails to write
  (an unregistered component), sliced or whole, is despawned and forgotten before the error is
  returned.
- **Hidden until revealed.** `spawn_as_child_hidden` keeps an instance hidden after it has
  spawned, until `reveal_instance_sync`. The Explorer spawns glTF containers that way and reveals
  them when processing is done, so Bevy's own materials and a model's collider meshes never show,
  even while processing waits for textures or for later frames. Colliders and triggers wait too:
  `update_colliders` skips the nodes of a container still processing, so a hidden model neither
  blocks the player nor takes pointer events. A collider is removed only by the entity that holds
  it: a model's replacement registers its colliders under the same ids (container, mesh name,
  index) while the old model is still shown, and despawning the old one must not remove them.
- **Resumable processing.** `update_ready_gltfs` processes a model's nodes until its deadline
  (`scene_spawn_slice::ProcessingDeadline`). A node is processed once it has a
  `ContainerEntity`, which processing gives it first; the model's running state stays on the
  container (`GltfProcessing`, which comes with the loaded model and is updated in place: where the
  next frame resumes in the instance's nodes, the collider counter, the animation flag and the
  named nodes). Models already in progress (resuming past their first node) go first; at least
  one node is processed a frame, and containers with nothing to process finish whatever the
  deadline. Resetting or removing the container drops the progress and despawns a model not
  processed yet. A container put again with the value last acted on keeps its model, or, if
  that value could not load, is not tried again; only a different value starts over.
- **The Explorer's slice.** `scene_runner::scene_spawn_slice` gives instantiation a quarter of
  the target frame (`SceneUpdates::frame_target`; 60 fps when uncapped), 4.2 ms at 60 fps, from
  when it runs (`SpawnScene`, before `scene_spawner_system`), as the raycast budget takes a share
  of the same target. The spawner stops a quarter of the slice before the end and processing at
  the end, so processing always has time, even while spawns are queued. There is no slice while
  the player is `OutOfWorld` (the loading backdrop covers the arrival) or on headless
  (`NoRenderApp`), so arrival and servers instantiate whole.
- **Diagnostics.** `spawn_timing` measures each frame's scene spawner and glTF processing into
  the Bevy diagnostics `instantiation/scene_spawner` and `instantiation/gltf_processing` (ms).
  The runtime debug panel's *Instantiation* section (`/instantiation_state`) shows the slice in
  effect, both measurements smoothed, the spawner's queue and the instance it is writing, and how
  many glTF models are spawning or being processed. Under target `spawn_timing` at debug level it
  also logs the frame's slowest model when it took a millisecond or more: its time, how many
  nodes and which model. One plugin, `SceneSpawnSlicePlugin`, owns all of it.

## Rejected

- **A budget per instance only** (the earlier engine's first step):
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
- **A list of the scene's entities and a cursor into it**: it walks
  every archetype of the scene world into a second collection of entities beside the instance's
  map, and writes in archetype order, which shows children written before their parents.
- **A cursor over the scene world's archetypes, without the list**: the same walk, done lazily,
  and the same archetype order.
- **Writing the unwritten entities in query order**: the query hands them out in archetype
  order, not parents first; hence the second, *ready* marker that a written parent passes on.
- **Reading asset events to notice a replaced scene**: asset events reach readers after
  `PostUpdate`, so the spawner would write one more slice from the new scene with the old
  entities; the scene world's `WorldId` changes when the asset is replaced and is checked before
  every slice.
- **Separate budgets for spawning and processing**: one slice bounds the frame's whole
  instantiation, which is what a hitch is made of. Within it the spawner stops early rather than
  sharing one deadline, which let a queue of spawns leave processing a single node a frame.
- **Slicing behind the loading backdrop and on headless**: the backdrop hides the hitches, and the
  arrival and the server want the content as soon as possible.

## Consequences

**Measured.** Native release builds were compared with `main` plus the same timing log, Windows,
60 fps, median of three interleaved autopilot runs per build:
- **Walking in.** The run arrives on the empty parcel −12,2 and walks 192 m east into Genesis
  Plaza at 6 m/s while measuring (`--autopilot-walk-to`). The plaza loads during play:

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
  that glTF instantiation did not cause (its longest processing was 45 ms). Across the three
  runs, frames over 50 ms went from 85 to 16, and over 200 ms from 5 to 1.

  The walk-in measures processing, which dominates: commit `a89eb9343` against `main`
  `a71ee5602`, on 2026-11-09. The spawner's own cost on this change is measured next. Re-run on
  2026-11-11 at `1e57af9ac` (merged with `main` `435bf3096`) against that `main`, it measured no
  instantiation in either build: two of the 60 scenes spawned started their sandbox, and Genesis
  Plaza's main JS file had not loaded when the 90 s ended. The table stays this change's
  measurement on a native build until the native walk-in loads content again.
- **The web build** (the PR's CI build of `1e57af9ac` merged with `main` `62712769f`, without
  wasm-opt, Chrome 152 driven by `local-harness/web-driver`): the same walk with a movement key
  held, about 24 s, then standing in the plaza; slowest frames from `/performance_state` per 1,000
  frames, three runs. Walking in, the slowest frame was about 170 to 200 ms, with 1 to 4 frames
  over 50 ms; standing while the plaza's models arrived, about 55 to 90 ms. The heaviest moment
  was the plaza's scene starting (about 90 to 250 ms), which shows its content at once. The
  spawner and processing stayed under 1 ms a frame each, smoothed: on the web, models wait for
  their meshes and textures, not the slice. Models appeared whole, avatars built and colliders
  held once models showed.
- **The spawner alone** (`scene_spawner/spawn_bench.rs`, an ignored test, release): a 5,051-entity
  hierarchy spawned seven times after a warm-up, medians:

  | | total | per entity | frames | longest frame |
  |---|---|---|---|---|
  | whole (stock `write_to_world_with`) | 9.96 ms | 1.97 µs | 1 | 9.96 ms |
  | 4 ms slices | 11.61 ms | 2.30 µs | 3 | 4.08 ms |

  Slicing caps the frame at the slice for about 17 % more spawner work in total: one marker added
  and removed per entity (sparse set, so no table move) and one query lookup per entity.
- **Standing at the arrival point** (arriving at 0,0, the arrival behind the loading backdrop):
  no difference within noise. The heavy instantiation happens behind the backdrop, which both
  builds run whole by design, before the measured 90 s begin.

**What else follows:**

- A model appears after about (its spawning and processing time) / (the slice) frames once it is
  requested, instead of in the frame it is requested; scenes that block their first ticks on their
  glTFs (`check_gltfs_ready`) start that much later.
- Avatars, emote props, the ghost and NFT frames are spawned in slices too, and appear
  correspondingly later.
- One node is the smallest unit: a single mesh whose collision shape takes longer than the slice
  still costs that time in one frame. The `spawn_timing` slowest-model line shows when it happens.
- The cost per entity and per node is unchanged: the same work, spread out.

## Follow-ups

- The frame-time budget (E2): replace the share of the target frame with a deadline after the
  scene loop, order
  requests nearest first, and gate avatar builds.
- Collision shapes and mesh hashing off the main thread, as the earlier engine did: the walk-in
  shows single nodes of about 40 ms.
- One archetype move per entity (a `BundleScratch`-like writer), if a measurement shows the
  per-entity cost matters.
