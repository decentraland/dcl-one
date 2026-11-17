# ADR: Primitives and materials wait for the frame's instantiation slice

- Status: Accepted
- Date: 2026-11-10

## Context

The Explorer turns a scene's `MeshRenderer` and `Material` components into Bevy meshes and
materials in `Update`, `SceneSets::PostLoop`:

- `update_mesh` attaches a mesh (a shared default box, plane, cylinder or sphere, or one built for
  the entity) and, when the entity has no material yet, the scene's default material;
- `update_materials` reuses a material from the scene's hash-keyed cache or creates a
  `SceneMaterial`.

Both do every changed entity in the frame it arrives. A scene that adds many primitives or
materials in one tick costs one long frame: on a release build at 60 fps, 1,000 boxes with a new
material each made a 37 ms frame, and 5,000 a 128 ms frame (medians of three runs; one run had a
145 ms frame and then a 110 ms one; *Consequences*).

unity-explorer gates the same work with its frame-time budget, per item, and retries the rest
next frame:
- `InstantiatePrimitiveRenderingSystem` for new and changed renderers;
- `StartMaterialsLoadingSystem` and `CreateMaterialSystemBase` for materials, which also check
  the memory budget.

Since the [node-by-node spawning ADR](2026-11-06-explorer-gltf-node-by-node-spawning.md), the
Explorer has one instantiation slice a frame: `slice_scene_spawning`, in `SpawnScene`, gives the
scene spawner and glTF processing a quarter of the target frame together (4.2 ms at 60 fps).

A clock cannot bound this work where it is gated. In the `PostLoop` loops an item costs well under
a microsecond: a handle clone, a hash, queued commands. The cost lands later in the frame:
- the command flush: three archetype moves per primitive (the mesh and default material, the
  loading material, the material);
- `PostUpdate`: bounds for each new mesh;
- the render world: specialisation and binning per new entity, and per new `SceneMaterial` two
  uniform buffers and a bind group.

## Decision

An entity's first mesh and first material are charged an estimated cost against the frame's
instantiation slice; what does not fit waits, marked, and goes first next frame. The slice then
ends that much earlier for spawning and glTF processing.

- **One slice.** `update_mesh` and `update_materials` charge `InstantiationCharges` through the
  `InstantiationBudget` system param, which reads the same slice. `slice_scene_spawning` sets
  the slice's end to the slice − charges after it runs (the spawner's deadline a quarter of the
  slice before that end, processing's at it) and resets the charges. No system or ordering point
  is added: the charges are reset where they are read.
- **What waits.** Only instantiation:
  - a primitive is an entity with a `MeshDefinition` and no `Mesh3d` yet;
  - a material is a `PbMaterialComponent` added since `update_materials` last ran (Bevy keeps a
    component's added tick when it is replaced, and the CRDT bridge writes with `try_insert`), or
    one still marked `PendingMaterial`.

  A changed mesh or material is attached at once, uncharged. Texture tweens rewrite their
  entity's `PbMaterialComponent` every frame, and each rewrite creates a material: gated, they
  would either animate less often under load or spend the slice every frame.
- **The gate.** A system run's first item is always admitted, so every consumer moves each frame,
  as the spawner writes at least one entity and processing at least one node. Each other item is
  admitted while it fits in what is left of the slice. Behind the loading backdrop
  (`OutOfWorld`) and on headless (`NoRenderApp`) everything is admitted uncharged, as spawning
  and processing run whole there.
- **The costs.** Four constants, measured with the stress scene in the appendix (1,000-entity
  waves; each wave's frame time above the run's median + 3 ms over the 2 s after it, differences
  between waves, per item, rounded up):

  | Constant | Charged for | Value |
  |---|---|---|
  | `PRIMITIVE_COST` | a first mesh, a shared default one, and the default material | 7 µs |
  | `NEW_MESH_COST` | on top: a mesh built for the entity, or a glTF mesh sent to the GPU | 10 µs |
  | `MATERIAL_COST` | a first material found in the scene's cache | 1 µs |
  | `NEW_MATERIAL_COST` | a first material created | 8 µs |

  The gate sits just before the work: after the cache lookup and texture resolution, before the
  mesh handle is made or the material added. A glTF mesh is charged once the resolver returns it,
  and a texture source that is not ready yet costs nothing.
- **Waiting is a marker on the entity.** `PendingPrimitive` and `PendingMaterial` are sparse-set
  components, so marking moves no table rows. Each system queries its pending entities first,
  then its usual changed ones without the marker, in one loop: what waited goes before what is
  new, so new arrivals cannot starve it. Among the waiting there is no arrival order; the query
  returns them in storage order. Once a run has refused an item, the loop leaves the rest of the
  waiting (`take_while` on the gate): they wait as they are, untouched and unvisited, with no
  resolving and no marker written again. The tick's arrivals are still all gone through, to mark
  the new ones and apply changes, so a frame costs what it admits plus that tick's arrivals,
  whatever the backlog's size. Removals are handled before that loop and drop the marker,
  unless the component was put back in the same tick: the entity then keeps waiting. Every path
  that gives up on an item (no scene context, a glTF mesh the resolver rejects) drops it too.
  A refused material leaves only its resolved glTF base (`BaseMaterial`), so a retry does not
  resolve it again; `MaterialSource` and the texture camera's cursor are inserted once it is
  admitted.
- **A new primitive brings its material.** When `update_mesh` admits a first mesh for an entity
  that has a `PbMaterialComponent` and no material yet, it charges a new material too and marks
  the entity `MaterialPrepaid`; `update_materials` creates a prepaid material without asking the
  gate. A first material whose primitive still waits waits with it, uncharged, and outside the
  materials' waiting query (`Without<PendingPrimitive>`), so a flood of waiting primitives is not
  walked there either: it comes back `MaterialPrepaid` once `update_mesh` admits the primitive.
  Prepaid materials have a query of their own, gone through first, so the walk's stop never holds
  one back. So a new primitive appears with its own material, never with the scene's white
  default first. A textured material
  still shows the default while its textures load. A prepaid material that has to wait for a
  texture source or its glTF base loses `MaterialPrepaid`, so the slice decides it again when it
  is ready, instead of all such materials arriving in one uncharged frame.
- **Instrumentation.** The debug panel's Instantiation section (`/instantiation_state`) shows
  what primitives and materials charged the last frame and how many first meshes and first
  materials wait. A consumer that defers also logs one line under target `instantiation_budget` at
  debug level: admitted, deferred, and the frame's charges.

## Rejected

- **The clock only** (unity-explorer's predicate): the gated loops spend well under a
  microsecond per item, so it would admit thousands of items a frame and bound nothing.
- **A separate slice for primitives and materials:** two budgets that do not know about each
  other; the node-by-node ADR rejected separate deadlines for the same reason.
- **Starting the slice in `PostLoop` and measuring real time from there:** the unrelated
  `PostLoop` work between the two points would eat the spawner's slice.
- **A learned cost per item** (a moving average of measured cost): the cost lands in the command
  flush, `PostUpdate` and the pipelined render world, so there is no clean window to measure it in.
- **Gating changes too** (unity-explorer's scope): texture tweens, above.
- **Separate gates for a primitive and its material** (unity-explorer's look): a new primitive
  whose material waits shows the white default for those frames.
- **Hiding a primitive until its material is ready** (`Visibility::Hidden`): it fights the SDK's
  own visibility component.
- **Running `update_materials` before `update_mesh`:** an entity would lose the default material
  while its textures load.
- **A list of waiting entities in a resource:** the progress stays on the entities, found with
  filtered queries.
- **Count caps per frame** (like text's 64 builds a frame): a second budget beside the slice.
- **A share of the slice kept for the spawner:** a second budget in disguise. During a flood of
  primitives the spawner writes one entity a frame (below).

## Consequences

**Measured.** Native release builds on Windows (RTX 5080 laptop, Vulkan, 60 fps), the base
`3e3e40526` against this change at `3bb15259c`, three runs each, interleaved, on an idle
machine; medians. The slice was then a fixed 4 ms; it is now a quarter of the target frame,
4.2 ms at 60 fps. The
stress scene (appendix) adds one wave every 9 s, each created in one tick. *Excess* is a wave's
frame time above the run's median + 3 ms over the 2 s after it.

| Wave | Longest frame, base | This change | Excess, base | This change | Frames deferring |
|---|---|---|---|---|---|
| 1,000 transforms only | 22.4 ms | 21.8 ms | 5.3 ms | 4.2 ms | 0 |
| 1,000 boxes, no material | 22.5 ms | 23.1 ms | 5.3 ms | 8.8 ms | 1 |
| 1,000 boxes, one shared material | 23.5 ms | 22.5 ms | 7.6 ms | 3.5 ms | 3 |
| 1,000 boxes, a new material each | 36.9 ms | 22.0 ms | 22.6 ms | 4.5 ms | 3 |
| 1,000 spheres, a new material each | 39.0 ms | 23.3 ms | 25.2 ms | 5.2 ms | 3 |
| 1,000 boxes, custom texture coordinates | 28.2 ms | 21.9 ms | 13.1 ms | 5.2 ms | 6 |
| 1,000 tapered cylinders | 32.9 ms | 22.0 ms | 16.9 ms | 4.1 ms | 6 |
| 5,000 boxes, a new material each | 127.7 ms | 34.3 ms | 325.9 ms | 262.3 ms | 18 |

Per run, frames over 50 ms went from 3 to 0 and the worst 1 % mean from 29.6 to 26.8 ms.

- Every 1,000 wave now peaks at the transforms-only wave, which is the scene's own CRDT work and
  outside this change. For the waves with materials or meshes of their own the excess falls too,
  not only spreads: smaller batches cost less per item. The no-material wave's rose from 5.3 to
  8.8 ms, within the spread between runs.
- The 5,000 wave: in one base run it made a 145 ms frame and a 110 ms one; this change spreads it
  over about 20 frames of 22–34 ms. After the wave both builds settle at about 20 ms a frame
  (12,000 entities drawn), so those frames are 2–14 ms over it, against the 4 ms charged. The
  cost per item grows with the number of entities in the scene, and the constants, measured with
  1,000, underestimate it there by about two times.
- Only primitives deferred: every material of the scene came with its primitive.
- The walk's stop came after the measurements above, from review: until then, once the slice was
  spent, each frame still visited every waiting primitive and every material waiting for its
  primitive. `per_frame_cost_with_a_large_backlog` (`scene_runner`'s material tests, run with
  `--ignored --nocapture`) times both systems in frames with the slice spent and boxes with a new
  material each waiting, one admitted a frame. Release test build, same machine, medians of 60
  frames:

  | Waiting | `update_mesh` before | After | `update_materials` before | After |
  |---|---|---|---|---|
  | 100 | 1.1 µs | 1.0 µs | 1.6 µs | 1.4 µs |
  | 10,000 | 18 µs | 1.0 µs | 54 µs | 1.2 µs |
  | 95,000 | 189 µs | 1.1 µs | 562 µs | 1.4 µs |

  At 95,000 the slowest frames went from 0.7 and 1.5 ms to under 0.02 ms. The 5,000 wave above
  left at most about 4,700 waiting, under 0.05 ms a frame then, so the table stands.

**What else follows:**

- A scene that adds many primitives at once fills in over a few frames: at these costs and
  60 fps, about 590 primitives a frame with the default material, or about 275 each with a new
  material.
- A primitive's first material is charged as a new one even when the scene's cache holds it:
  `update_mesh` cannot tell, since the cache fills in `update_materials`, later in the frame. The
  shared-material wave above is charged that way: it fills in over four frames where two would
  do, at the floor's frame time.
- A waiting primitive's collider (`MeshCollider` is not gated) can work a few frames before its
  mesh shows; unity-explorer does the same.
- During a flood of primitives the spawner and glTF processing run at their minimum, one entity
  and one node a frame: avatars and glTFs pause for the flood, about 20 frames for 5,000 boxes.
- The constants are one machine's. A machine three times slower spends about 12 ms of real time
  on a frame's charges: still bounded, and far below the hitch it replaces. The web build runs the
  same code without pipelined rendering, so its frames carry the render world's share too.

## Follow-ups

- A cost that scales with the scene, or one learned on the running machine, if floods of
  thousands of primitives turn out to matter. Short of that, doubled costs (half as much a
  frame) would give the web build and weaker machines headroom; measuring there would say
  whether they need it.
- Charging glTF spawning and processing for the work they defer. The spawner and
  `update_ready_gltfs` stop at a clock deadline, and the clock sees only their own work, not the
  command flush, `PostUpdate` or the render world's work for the entities they reveal. Charging
  each entity they write with a mesh an estimated cost to `InstantiationCharges` would count both
  halves of the slice the same way; until then a frame heavy on glTFs can overrun the slice
  without the debug panel showing it.
- Nearest-first ordering for every instantiation consumer, as the node-by-node ADR proposes
  for the spawner.
- Texture tweens create a new `SceneMaterial` for each tweened entity every frame; reusing or
  updating the material in place is separate work.
- Charging a primitive's first material at the cache's price when the cache holds it: hash
  `PbMaterialComponent` in `update_mesh`, or have `update_materials` refund the difference to the
  frame's charges. A refund frees the rest of the slice for spawning and processing, not for more
  primitives that frame.

## Appendix: the stress scene

An SDK7 scene on one parcel (`0,0`, spawn at `8,0,1` facing `8,2,8`), built with `@dcl/sdk`
7.33.0 (`sdk-commands build`) and served with
`sdk-commands start --port 8765 --no-browser --no-watch --skip-build --no-client`. Each run:
`decentra-bevy --autopilot --csv <file> --log-file <file> --realm http://127.0.0.1:8765
--position 0,0 --fps 60 --scene_log_to_console`, with `RUST_LOG=info,instantiation_budget=debug`;
the scene logs `stress wave <n> <name>` when a wave starts, which aligns it with the CSV.

```ts
import { engine, Transform, MeshRenderer, Material, Entity } from '@dcl/sdk/ecs'
import { Vector3, Color4 } from '@dcl/sdk/math'

const FIRST = 12
const GAP = 9

type Wave = { name: string; count: number; make: (e: Entity, i: number, wave: number) => void }

const shared = Color4.create(0.8, 0.3, 0.2, 1)
const distinct = (i: number) =>
  Color4.create((i % 20) / 20, (Math.floor(i / 20) % 20) / 20, Math.floor(i / 400) / 20, 1)
// every distinct-material wave gets its own materials, so none hits an earlier wave's cache
const pbr = (i: number, wave: number) => ({ albedoColor: distinct(i), metallic: wave / 10 })
const boxUvs: number[] = []
for (let f = 0; f < 6; f++) boxUvs.push(0, 0, 1, 0, 1, 1, 0, 1)

const waves: Wave[] = [
  { name: 'transform-only', count: 1000, make: () => {} },
  { name: 'box-no-material', count: 1000, make: (e) => MeshRenderer.setBox(e) },
  { name: 'box-shared-material', count: 1000, make: (e) => { MeshRenderer.setBox(e); Material.setPbrMaterial(e, { albedoColor: shared }) } },
  { name: 'box-distinct-materials', count: 1000, make: (e, i, w) => { MeshRenderer.setBox(e); Material.setPbrMaterial(e, pbr(i, w)) } },
  { name: 'sphere-distinct-materials', count: 1000, make: (e, i, w) => { MeshRenderer.setSphere(e); Material.setPbrMaterial(e, pbr(i, w)) } },
  { name: 'box-custom-uvs-shared-material', count: 1000, make: (e) => { MeshRenderer.setBox(e, boxUvs); Material.setPbrMaterial(e, { albedoColor: shared }) } },
  { name: 'cone-shared-material', count: 1000, make: (e) => { MeshRenderer.setCylinder(e, 0.5, 0.1); Material.setPbrMaterial(e, { albedoColor: shared }) } },
  { name: 'box-distinct-materials-5000', count: 5000, make: (e, i, w) => { MeshRenderer.setBox(e); Material.setPbrMaterial(e, pbr(i, w)) } },
]

let t = 0
let next = 0

export function main() {
  engine.addSystem((dt: number) => {
    t += dt
    if (next >= waves.length || t < FIRST + next * GAP) return
    const wave = waves[next]
    for (let i = 0; i < wave.count; i++) {
      const e = engine.addEntity()
      Transform.create(e, {
        position: Vector3.create(0.5 + (i % 32) * 0.47, 0.3 + next * 1.2 + Math.floor(i / 1024) * 0.3, 0.5 + (Math.floor(i / 32) % 32) * 0.47),
        scale: Vector3.create(0.2, 0.2, 0.2)
      })
      wave.make(e, i, next)
    }
    console.log(`stress wave ${next} ${wave.name} at t=${t.toFixed(2)}`)
    next++
  })
}
```
