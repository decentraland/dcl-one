# Implementing engine features

Define observable behavior and ownership before choosing systems or components. Read
[decisions](decisions.md), [code style](code-style.md), [scheduling](scheduling.md) and
[platforms](platforms.md), then inspect the owning crate, its plugin wiring and its assets.
Code alone does not establish what renders: materials, shaders, models and configuration can
override its defaults. Check the license before importing an external asset.
Keep external reference checkouts for inspection only; do not make builds, runtime paths or
tracked documentation depend on a sibling checkout.

## Procedure

1. **Check existing decisions.** Preserve deliberate behavior unless the task authorizes a
   change. Explain a conflict before implementing a different policy.
2. **Inventory the whole feature.** Record its states, visible output, constants and units,
   settings, loading, cleanup and consumers. Inspect static versus skinned geometry, material
   assignments, shader passes, visibility and nametags. Keep source inventories and comparison
   evidence in the ignored task report.
3. **Preserve the required constants.** Durations, offsets, thresholds and colors are behavior;
   keep their specified values and names unless the task authorizes a change. Check material
   values as well as shader defaults. Record each intentional difference and its reason in the
   task report rather than adjusting a value in passing.
4. **Use the owning crate's integration points** (below). Entity state belongs on its entity;
   reuse the existing schedules and lifecycle. A feature that hides or clips spawned content
   runs after its producer with deferred commands applied before rendering.
   Update persistent entities and assets in place; do not respawn them every frame.
5. **Decide what happens mid-transition.** Enumerate rebuilds, despawns, hiding, setting
   changes, late loads and failed loads while animation or staging is in progress. Give each
   required behavior a boundary test ([contributing](contributing.md#tests)).
6. **Test off the GPU where possible, then verify the built client.** A plain state machine
   can be driven frame by frame. Embedded asset tests use a headless app with `MinimalPlugins`,
   `AssetPlugin`, `assets::EmbedAssetsPlugin` and `GltfPlugin`. Shader changes need the
   [pipeline-compile test](recipes.md#a-pipeline-compile-test-for-a-shader) and a browser run;
   native execution does not verify web behavior. The workspace test command and required
   checks are in [running](running.md#tests).
7. **Document the resulting behavior and evidence.** The PR describes the problem, chosen
   constants, alternatives, verified platforms and rendering screenshots where applicable.
   Durable choices go in [decisions](decisions.md) or an ADR. Apply the
   [public-prose rule](contributing.md#public-prose).

## Engine integration points

| Concern | Integration |
|---|---|
| Per-frame behavior | An ordinary system in the existing `Update` sets. Order it after the producer whose state it reads; chain related systems |
| Entity state | A `Component` on the player, avatar or scene entity that owns it; avoid a parallel entity-keyed resource map |
| Cleanup | Parenting (`ChildOf`) or `DespawnWith(owner)`; despawn performs teardown ([code style](code-style.md#cleanup-is-structural)) |
| Material state | Update a unique material through `Assets<M>::get_mut`. Wearables and masks use the shared `crates/avatar/src/material_cache.rs`; put per-avatar data in `MeshTag` bits ([recipe](recipes.md#per-avatar-data-on-a-shared-material)) |
| Embedded visual assets | Files under `crates/assets/src/assets/<kind>/` are embedded at `embedded://<kind>/<file>`. Load shared handles in `Startup` and attach scenes with `SceneSpawner::spawn_as_child` |
| Graphics settings | A `GraphicsSettings` field in `crates/common/src/structs.rs`, a default and an `EnumAppSetting` registered in `crates/system_bridge/src/settings/mod.rs`; the HUD lists it |
| Launch options | A `LaunchOptions` field for a native switch; a page parameter for a web-only switch ([recipe](recipes.md#a-new-engine-parameter)) |
| Profile presentation | `comms::profile::UserProfile::name_color()` for name color; `ProfileManager::get_compact` for list data and `get_data` for the full profile |
| Profile publication | The `revert_to` field of `comms::profile::CurrentUserProfile` holds the confirmed snapshot that `deploy_finished()` restores when the current edit fails; `peer_profile()` exposes the version peers may hear while deployment is pending |
| Player-only avatar behavior | `Or<(With<ForeignPlayer>, With<PrimaryUser>)>` excludes scene `AvatarShape` NPCs and photo-booth previews |
| Avatar attachment and nametags | `AttachPoints` and `crates/avatar/src/dynamic_nametag.rs` |
| Tests | Plain state-machine tests, headless asset tests and explicitly ignored GPU tests where a renderer is required |

## Shaders

Materials are shared assets, lighting uses physical units, the fork's transparent pass has a
second output, and shared constants come from Rust. Start from the templates in
`crates/assets/src/assets/shaders/`: `bound_material.wgsl` and `bound_prepass.wgsl` for lit scene
materials, `toon.wgsl` for avatar lighting, and `mask_material.wgsl` for face masks. Load them
through `embedded://shaders/<file>.wgsl`.
For an unlit shader, avoid `pbr_input_from_vertex_output` and
`main_pass_post_lighting_processing`: they introduce UV and lighting requirements that the
unlit effect does not need.

| Concern | WGSL integration |
|---|---|
| Time | `globals.time` from `bevy_pbr::mesh_view_bindings` |
| Camera position | `view.world_position` |
| Alpha cutoff | `if x < 0.0 { discard; }` |
| Arithmetic | Translate HLSL `frac`, `lerp` and `half` to `fract`, `mix` and `f32`; `saturate`, `step`, `smoothstep` and `pow` keep their names |
| Instance and world transform | `vertex.instance_index` and `mesh_functions::get_world_from_local(instance_index)`; carry the instance index under `#ifdef VERTEX_OUTPUT_INSTANCE_INDEX` |
| World-to-clip position | `view_transformations::position_world_to_clip` |
| Skinning | Under `#ifdef SKINNED`, call `skinning::skin_model(joint_indices, joint_weights, instance_index)` and `skin_normals`, as `bevy_pbr`'s `mesh.wgsl` does. Without it, a custom vertex stage renders the bind pose |
| Height along a skinned avatar | Use world height above the mesh origin: `world_position.y - get_world_from_local(i)[3].y`. Skinned object space is the bind pose; avatar origins sit at the feet |
| Runtime parameters | An `AsBindGroup` struct with `#[uniform(0)]` fields for values Rust changes; WGSL `const` for fixed values |
| Base color | Multiply the lit term by `BRIGHTNESS * view.exposure`, as `toon.wgsl` does. An unscaled color can appear black |
| Emission | Multiply by `view.exposure * EMISSIVE_BOOST`, as `toon.wgsl` does |
| Camera dither | Use `discard_dither(...)` from `bound_material_effect.wgsl` with the avatar base opacity. Its `Dither.transparency` feeds `cap_brightness`, so bloom does not fill discarded regions |
| Transparent materials | Return `AlphaMode::Blend`, add `NotShadowCaster` where appropriate, and write `out.focus` under `#ifdef TRANSPARENT_FOCUS_OUTPUT` ([platforms](platforms.md#web)) |
| Cutoff and prepass | Repeat a lit pass's discard in its prepass, or depth and shadows show hidden pixels. `MaskMaterial` has no prepass shader; account for that limitation |
| Derivatives and texture sampling | Hoist `dpdx` and `textureSample` above divergent branches ([platforms](platforms.md#web)) |

Per-avatar values on shared materials, constants shared between Rust and WGSL, and the
pipeline-compile test each have a [recipe](recipes.md). Verify the rendered result: a unit test
can pass while a missing exposure factor, skinning branch or prepass makes the feature invisible
or incorrect in the scene.
