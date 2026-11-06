# Porting from Unity

The Unity explorer is the reference client. A port carries its observable behaviour and its
numbers over verbatim, reshapes the code to this engine's rules, and names every divergence.
Read [decisions](decisions.md), [code style](code-style.md), [scheduling](scheduling.md) and
[platforms](platforms.md) before designing, and read the Unity assets, not only its C#.

Unity paths below are relative to the root of the `unity-explorer` repository. That checkout is
a reference only: never add a dependency on it, and never copy a Unity asset in without
checking its licence and provenance.

## Where things are in Unity

| | Path |
|---|---|
| Systems and components | Under `Explorer/Assets/DCL/<Area>/`, mostly in a `Systems/` folder: `*System.cs` holds `[Query]` methods on a `BaseUnityLoopSystem`; `*Component.cs` the Arch components |
| Wiring | `Explorer/Assets/DCL/PluginSystem/Global/<Area>Plugin.cs`: which systems run and with what assets |
| Gating | `Explorer/Assets/DCL/FeatureFlags/FeatureFlagsStrings.cs` and `FeaturesRegistry.cs`; `Explorer/Assets/DCL/Infrastructure/Global/AppArgs/AppArgsFlags.cs` |
| Assets | `.prefab`, `.mat`, `.shader`, `.hlsl` and `.shadergraph` files next to the code; the shared avatar shaders under `unity-shared-dependencies/Runtime/Shaders/` |
| Tests | `Explorer/Assets/DCL/<Area>/Tests/`, PlayMode tests with their `TestAssets/` |

## Procedure

1. **Check [decisions](decisions.md) first.** It lists the deliberate divergences from Unity.
   If the feature is there, parity is not wanted: say so and stop.
2. **Inventory the feature from every file, not just the C#.** The scripts give the state
   machine and the constants; the prefab, material and shader give what actually renders
   (static or skinned, which passes, which material each renderer carries). Write the
   inventory as a checklist: what it shows, what it skips, every constant with its unit, what
   is animated, what is gated, what cleans it up, and which other systems branch on it
   (nametags, visibility, tests).
3. **Keep Unity's numbers and names.** Durations, offsets, thresholds and colours go in as
   Unity has them, with the Unity file named in the PR description, never in a code comment
   ([contributing](contributing.md#the-diff)). A changed number is a divergence (step 7), never a
   tweak made in passing. Read the `.mat` too: it can override the `.shader` defaults.
4. **Map each Unity piece to its shape here** (tables below), reading the crate that owns the
   area and its guideline page before choosing. No new schedule sets, no side maps, no
   per-frame respawns.
5. **List what can happen mid-transition and decide each.** For every animated or staged
   behaviour, enumerate the events that can land while it runs: the thing it replaces is
   rebuilt (a look change rebuilds an avatar), despawned, hidden, toggled by its setting, or
   loads late or fails. Decide the behaviour for each and test it. See
   [contributing](contributing.md#tests).
6. **Test at the boundary, off the GPU where possible.** Pull a state machine into a plain
   struct with a `tick(dt)` so a unit test drives it frame by frame. Pin an embedded asset with
   a headless app test (`MinimalPlugins`, `AssetPlugin`, `assets::EmbedAssetsPlugin`,
   `GltfPlugin`). A shader change gets the pipeline-compile test in
   [recipes](recipes.md#a-pipeline-compile-test-for-a-shader), and a run in Chrome: native does
   not prove the web build. `cargo test --all` is the CI run, not per-crate
   ([running](running.md#tests)).
7. **Write the PR description as the record** ([contributing](contributing.md)): the Unity
   files it follows, the constants carried over, every divergence with its reason, the
   platforms verified, and before and after screenshots for rendering. A divergence worth
   keeping also goes in [decisions](decisions.md) in the same PR.

## Mapping Unity to the engine

| Unity | Here |
|---|---|
| A `[Query]` method in a `BaseUnityLoopSystem`, `[UpdateAfter(X)]` | A system in `Update`, `.after(x)` on the system that produces what it reads; chain related systems. A system that hides or clips what another spawns runs `.after(spawner)` with a sync point in between, so nothing renders unhidden for a frame |
| An Arch component on the entity | A `Component` on the same entity (player, avatar, scene entity). Not a `HashMap<Entity, _>` in a resource |
| `*CleanupSystem` | Parenting (`ChildOf`) or `DespawnWith(owner)`; despawn does the teardown ([code style](code-style.md#cleanup-is-structural)) |
| `new Material(template)` per renderer plus `SetVector` each frame | One `Material` asset per entity updated through `Assets<M>::get_mut`, only for a material that is unique anyway. Wearables and masks come from `crates/avatar/src/material_cache.rs`, shared and batched: their per-avatar data goes in `MeshTag` bits ([recipes](recipes.md#per-avatar-data-on-a-shared-material)) |
| A prefab child `GameObject` with a serialised renderer | An embedded asset under `crates/assets/src/assets/<kind>/` (every file there is embedded; path `embedded://<kind>/<file>`), loaded in a `Startup` system into a resource and spawned under the entity with `SceneSpawner::spawn_as_child` |
| A feature flag plus an `--arg` | A `GraphicsSettings` field in `crates/common/src/structs.rs` with a default, an `EnumAppSetting` in `crates/system_bridge/src/settings/`, registered in `settings/mod.rs`; the HUD lists it on its own. A launch-time switch is a `LaunchOptions` field; a web-only one is a page param ([recipes](recipes.md#a-new-engine-parameter)) |
| `Profile.UserNameColor` | `comms::profile::UserProfile::name_color()` |
| Skipping `CharacterPreviewComponent` and SDK avatars | `Or<(With<ForeignPlayer>, With<PrimaryUser>)>`: excludes scene `AvatarShape` NPCs and photo-booth previews |
| `AvatarBase` anchor points, `GetAdaptiveNametagPosition` | `AttachPoints`; `crates/avatar/src/dynamic_nametag.rs` |
| PlayMode tests | `#[test]` on the plain state struct; headless asset tests; `#[ignore]`d GPU tests |

## Porting a shader

The rendering model differs from Unity's: materials are shared assets, the scene is lit in
physical units, the fork's transparent pass has a second output, and constants come from Rust.
Copy a template, then translate. Templates in
`crates/assets/src/assets/shaders/`: `bound_material.wgsl` with `bound_prepass.wgsl` (the lit
scene material extension), `toon.wgsl` (avatar lighting), `mask_material.wgsl` (face masks).
Shaders load as `embedded://shaders/<file>.wgsl`.

| Unity HLSL / URP | WGSL here |
|---|---|
| `_Time.y` | `globals.time` from `bevy_pbr::mesh_view_bindings` |
| `GetCameraPositionWS()` | `view.world_position` |
| `clip(x)` | `if x < 0.0 { discard; }` |
| `frac`, `lerp`, `half` | `fract`, `mix`, `f32`; `saturate`, `step`, `smoothstep` and `pow` keep their names |
| `UNITY_SETUP_INSTANCE_ID`, `unity_ObjectToWorld` | `vertex.instance_index`; `mesh_functions::get_world_from_local(instance_index)`; carry it in `out.instance_index` under `#ifdef VERTEX_OUTPUT_INSTANCE_INDEX` |
| `TransformWorldToHClip` | `view_transformations::position_world_to_clip` |
| `SkinnedMeshRenderer` (skinning is implicit) | Explicit: under `#ifdef SKINNED`, `skinning::skin_model(joint_indices, joint_weights, instance_index)` and `skin_normals`, as `bevy_pbr`'s `mesh.wgsl` does. A custom vertex stage without it renders the bind pose |
| `positionOS.y` with a hip offset | Skinned object space is the bind pose here. Use world height above the mesh origin, `world_position.y - get_world_from_local(i)[3].y`; avatar origins sit at the feet, so Unity's hip offset disappears |
| `Properties` plus `Material.SetVector` per frame | An `AsBindGroup` struct with `#[uniform(0)]` fields for what Rust changes at runtime; everything else a WGSL `const` |
| `_Color`, a lit or unlit base colour | Multiply by `BRIGHTNESS * view.exposure` as `toon.wgsl` does for its lit term. Without the scale a colour renders black in this scene |
| `[HDR] _Color`, `_Emission_Intensity` | Multiply by `view.exposure * EMISSIVE_BOOST` as `toon.wgsl` does for emission. Unity's HDR intensity becomes the emission factor |
| A dither fade near the camera | `discard_dither(...)` from `bound_material_effect.wgsl` with the avatar base opacity. It returns a `Dither` whose `transparency` is how faded the fragment is; `bound_material.wgsl` feeds it in as `cap_brightness` so bloom does not fill the discarded regions |
| `Blend SrcAlpha OneMinusSrcAlpha`, `ZWrite Off`, no ShadowCaster pass | `alpha_mode()` returning `AlphaMode::Blend`, `NotShadowCaster` on the entities, and `out.focus` written under `#ifdef TRANSPARENT_FOCUS_OUTPUT` ([platforms](platforms.md#web)) |
| `clip` in a lit pass | Repeat the discard in the prepass shader, or depth and shadows show what colour hides. `MaskMaterial` has no prepass shader: a known gap, say so in the PR |
| `dpdx`, `textureSample` inside a branch | Hoist above the `if` ([platforms](platforms.md#web)) |

Per-avatar values on a shared material, constants shared between Rust and WGSL, and the
pipeline-compile test each have a recipe in [recipes](recipes.md).

## Common mistakes

- Designing from the C# alone and getting the rendered thing wrong: static where Unity is
  skinned, a pass the shader does not have, a material the renderer does not carry.
- Rounding Unity's constants or widening a range without naming it as a divergence.
- Citing a Unity file, type or constant in a code comment. Name it in the PR description, and
  give the code's own reason in the comment.
- Testing only the transition's happy path, not the rebuilds, despawns and setting toggles
  that can land mid-way.
- Per-avatar material properties on a shared, cached material: breaks the cache and the
  batching that `material_cache.rs` exists for.
- An unscaled colour (no `view.exposure` factor): invisible in-world though it looks
  right in a unit test.
- `pbr_input_from_vertex_output` and `main_pass_post_lighting_processing` in an unlit shader:
  they add UV and lighting requirements the shader does not use.
- The discard only in the main pass; the prepass still writes depth and shadows.
- Verifying only on native. The fork's transparent pass and derivative rules fail on web only.
