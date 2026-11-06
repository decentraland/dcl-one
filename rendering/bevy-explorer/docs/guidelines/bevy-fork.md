# The bevy fork

bevy-explorer builds bevy from `rendering/bevy-explorer/bevy`: upstream bevy 0.16.1 plus our
changes, vendored from [`robtfm/bevy`, branch `release-0.16-dcl`](https://github.com/robtfm/bevy/tree/release-0.16-dcl)
at `dac1ac9` (102 commits on top of `v0.16.1`; see the
[ADR](../../../../docs/adr/2026-11-05-explorer-vendored-bevy.md)). Bugfix-only forks of other
crates exist too (see the workspace `Cargo.toml`); this page is only about bevy, because that is
where behaviour differs from stock and where a reader who knows bevy will otherwise be wrong.

If a change belongs in bevy, make it in the vendored tree. Changes marked **throwaway** are 0.16
adaptations of work that is already upstream in a later bevy and are dropped, not ported,
on any migration.

## Web workers and threading (`web-worker` feature)

Stock bevy on wasm is single-threaded on the page's main thread. The fork runs the engine
across workers:

- `bevy::web_worker::start` takes the page's canvas and spawns every worker from one page
  call: the render worker (OffscreenCanvas, wgpu device, render world), the compute workers
  (`ComputeTaskPool`), the async-compute workers (`AsyncComputeTaskPool`) and the engine worker
  (app world, winit event loop). One script serves every role; each worker instantiates the
  module over the page's shared memory and calls bevy's export for its role. `start_render`
  brings the render worker up first so a pipeline cache can warm while the launch screen shows.
- Page functions: JS hooks the engine looks up on `self` are declared in an extern block with
  `#[page_functions]` (`bevy_derive`) and relayed from every worker to the page. Worker console
  output is mirrored to the page and printed only there.
- The render world runs on its own worker (`bevy_render::web_worker`,
  `RenderCreation::WebWorker`, `App::set_finisher`, `PrecreatedSurface`). The app is paced from
  the render worker's animation frames, rendering happens inside the rAF callback, and at most
  two frames are in flight, fenced with mappable buffers.
- `RenderCapabilities`: a main-world resource for limits, features and adapter info. With the
  feature on, no wgpu handle is ever inserted into the main world.
- `WgpuWrapper` on wasm records its creating thread; a value dropped off-thread is parked and
  released on the render thread at `RenderSet::PostCleanup` instead of panicking.
- `bevy_tasks`: the multi-threaded `TaskPool` on the web. Workers join through `run_worker`;
  `Task::split` separates handle from future. `TaskPool::spawn` keeps the single-threaded
  contract (static, non-`Send`, driven by the JS event loop). `AsyncComputeTaskPool::spawn`
  requires `Send` on the web and runs on the async-compute workers. With WASM atomics,
  `wasm-bindgen-futures` can re-poll a self-waking task inline. A CPU loop that awaits
  `yield_now()` can therefore occupy the engine worker for the whole loop. Run CPU-heavy
  work on an async-compute worker or in the scene worker that owns the input.
- `bevy_ecs`: parallel iteration and the multi-threaded executor on the web under the feature.
  The render world's Extract and Render schedules stay single-threaded, since WebGPU objects
  are thread-bound.
- `PipelinedRenderingDiagnosticsPlugin`: per-stage timings of a pipelined frame under
  `pipelined/`, including GPU backlog and animation-frame waits on the web.
- winit comes from `robtfm/winit` `v0.30.x` for OffscreenCanvas worker windows.
- `wasm_threaded_loader` feature (`bevy_asset`): asset loading on a worker; in-flight loads
  cancel when the last handle drops; `futures-channel` mpsc instead of `async-channel`, whose
  lock lowers to `memory.atomic.wait` and traps on the main thread.

## Depth of field and transparency

- **Transparent focus target.** When a view has depth of field, the main transparent pass gets
  a second `Rg16Float` attachment. Fragments blend `(view_depth, 1, 0, alpha)` into it, so it
  accumulates alpha-weighted depth and coverage, and the DoF shader derives focus depth from
  it. `FragmentOutput` gains the output behind `TRANSPARENT_FOCUS_OUTPUT`. A custom material
  shader that omits it fails pipeline creation on web. Gizmos declare the target with an empty
  write mask. Transmission-reading blend pipelines are included.
- `DepthOfFieldMode::Disabled` keeps the component active with no blur, so the focus target is
  still rendered for passes that read it (the explorer's stars).
- The DoF gather weights each sample by the ratio of its circle of confusion to the gathering
  pixel's, so sharp neighbours don't smear into a blurred pixel. Earlier: distant-only blur and
  alpha blending in the DoF shader.
- Per-asset write ticks in `RenderAssets` so entities respecialize when a prepared asset is
  swapped render-world-only (fallback material to real, deferred mesh re-upload).
- `Transparent3d` sorts by euclidean distance from the camera rather than view-space z.
- Superseded: an "alpha depth write" change (blend materials wrote depth in the main pass,
  `MeshPipelineKey::IN_PREPASS`) that the focus target replaced. The key flag remains.

## GPU upload and render assets

- Byte-budgeted, prioritised per-frame uploads: `RenderAssetTransferPriority`,
  `RenderAssetBytesPerFrame::new_with_priorities`, `request_bytes`. `RenderAssets` keeps the
  previous GPU copy while a re-upload is pending (`get_latest`, `set_stale`), and GPU textures
  are reused across re-uploads.
- Render assets are not removed when the main-world asset changes:
  `Assets::get_mut_bypass_change_detection`, `Mesh::take_gpu_copy`.
- `immediate_upload` on `Image`, `Mesh` and font atlases, and as a glTF loader setting, to
  bypass the byte budget.
- **Fallback materials**: `ExtendedMaterial` renders its base while the extension's assets
  aren't ready; `ForwardDecalMaterial<B>` is generic over the base; a material extension's
  `alpha_mode` receives the base as input.
- Pipeline cache: `PipelineCompilationMode::async_with_handler` with a
  `PipelineCompilationHandler` that is notified as pipelines compile.
- **Throwaway**: `RenderMeshInstances` and `RenderMaterialInstances` mirrored into render-entity
  components with component-first lookups (`MeshInstanceLookup`, `MaterialInstanceLookup`,
  `validate_instance_mirror` feature); upstream #22966 removes the need.
- **Throwaway**: `ViewVisibility` carries previous-frame visibility instead of per-frame hash
  sets (`SetViewVisibility::set_visible`); upstream #22226.

## Shader and uniform hooks

- `UserGlobal(f32)` resource surfaces as `globals.user_global`; `ViewUserValue(f32)` on a
  camera surfaces as `view.user_value`.
- Light textures (upstream #18031): `PointLightTexture`, `SpotLightTexture`,
  `DirectionalLightTexture`, extended to multicolour cookies.
- Unclipped-depth emulation is forced on WebGPU, where the feature may be declared but not work.
- `MAX_MORPH_WEIGHTS` raised from 64 to 256.
- `specular_multiscatter` matches the Filament reference (upstream #23514); inverse tonemapping
  fix.

## glTF and images

- `GltfLoader::uri_resolver` (`GltfPlugin::with_uri_resolver`): the host resolves every URI a
  glTF references. This is how content paths are contained; see the security page.
- Accessors with no `bufferView` are accepted (zero-filled per spec; Blender morph targets).
- Texture decode is skipped when `load_materials` is empty, with a 1x1 placeholder under the
  normal label: headless servers keep geometry without pixels.
- `Rgba16Unorm` textures are converted to 8-bit, preserving sampler and sRGB (the web can't
  sample 16-bit).
- `reduce_image_sizes` feature: textures over a per-dimension cap are resized at load
  (1024 on wasm, 8192 on native).
- Smaller: glTF textures load sequentially rather than in parallel; non-embedded textures honour
  `render_asset_usages`; samplers survive downsizing; unique mesh names; default image format
  guessing.

## ECS, app and scene

- `HierarchyPropagatePlugin`, `Propagate<C>`, `PropagateStop<C>`, `PropagateOver<C>` (upstream
  #17575): propagate a component down the hierarchy.
- `children!` accepts more than twelve children (upstream #18865, #21099).
- `SceneSpawner` forgets child instances when their parent is gone, and `unregister_instance`
  cancels a spawn that hasn't happened yet.
- `SceneSpawner::set_spawn_deadline`: queued `Scene` instances are written an entity at a time,
  one instance after another in request order, until the deadline. The rest waits for the next
  run, and each run writes at least one entity. `None`, the default, spawns them whole as stock
  bevy does; dynamic scenes and `spawn_sync` always do.
  - A part-written instance's top-level entities are attached to the parent from the first run
    and kept `Visibility::Hidden` until it is complete. Their own visibility is restored then.
  - Its parent despawned, `despawn_instance` or `unregister_instance` despawn what was written.
  - A scene replaced or removed mid-spawn starts the instance over.
  - `spawn_as_child_hidden` keeps an instance hidden after it has spawned, until
    `reveal_instance_sync`.
  - The explorer sets the deadline each frame (`scene_runner::scene_spawn_slice`) and spawns glTF
    containers hidden until `update_ready_gltfs` has processed them.
- `mark_dirty_trees` keys its early-out on this run's tick, so transform propagation can run
  twice per frame (the explorer's early pass plus bevy's own).
- `GlobalTransform::from(Transform)` and `mul_transform` sanitise the affine: a zero scale
  component becomes `f32::EPSILON` and non-finite parts fall back to identity. A singular
  affine decomposes to a NaN rotation, which reached colliders as NaN AABBs and crashed parry's
  BVH build on zero-thickness glTF walls. Every derived `GlobalTransform` is safe by
  construction; the local `Transform` keeps its exact values, so scene read-back of what a scene
  wrote comes from locals (`TransformHelperPub::compute_relative_transform`), never from the
  global.
- `bevy_animation`: newly added graphs are threaded the same frame (no one-frame T-pose);
  evaluator hash maps replaced with dense index lists; the evaluator stack is cleared.
- `bevy_text`: faces of removed `Font` assets are evicted from the cosmic font system;
  `TextFont` is `PartialEq`.
- `bevy_winit`: modifier transitions reported only via `ModifiersChanged` are synthesised as key
  events, so a modifier swallowed by an OS chord doesn't stick; `UpdateMode::Reactive` timing
  fix.

## UI

- `BorderColor { left, top, right, bottom }` with `BorderColor::all` (upstream #18682).
- `CameraCursorPosition` on a camera gives UI picking a per-camera cursor, for render-to-texture
  UI; cursor positions are in physical coordinates.
- `Interaction::Pressed` is dropped when the pointer leaves the node.

## Visibility and small changes

- `ComputedCameraValues` fields, `RenderMaterialInstance` fields public; `Projection` is
  `PartialEq`.
- Web screenshots blit through the sRGB swapchain view (upstream #22090).
- Fixes with no API surface: unicode asset paths, skin buffer growth, missing cascades panic,
  bad vertices for a frame on second mesh load, sparse index warning.

## Changing bevy

- Edit `rendering/bevy-explorer/bevy` in place, in an ordinary PR, and update this page in the
  same PR. This repository is the source of truth; the fork branch stays frozen at `dac1ac9`.
- The import left out `assets/` (example assets), the symlink `examples/wasm/assets` into it, and
  the fork's `.gitattributes`. The root `.gitattributes` sets `-text` for the tree, so its bytes
  stay exact on every platform.
- `source/Cargo.toml` points both patch tables at the tree. `[patch.crates-io]` lists each bevy
  subcrate an ecosystem crate names by crates.io version (18 today);
  `[patch."https://github.com/robtfm/bevy"]` catches `bevy_dui`, `bevy_ecss` and `boimp`, which
  name the former fork by git. With `Cargo.lock` nothing fetches the fork. A resolve without it
  reads the branch once, so the branch has to stay fetchable while those crates name it.
- The tree is not a workspace member, so the explorer's Clippy and Rustfmt gates never reach it.
  Cargo doesn't cap a path dependency's rustc warnings as it did the git dependency's: the
  vendored root `Cargo.toml` allows `dead_code` (code our feature set leaves unused) and
  `mismatched_lifetime_syntaxes` (a lint newer than bevy 0.16); other warnings stay visible.
  Don't run Clippy inside bevy's own workspace: its code predates this toolchain's lints.
- Backport an upstream change: `git fetch https://github.com/bevyengine/bevy <sha>`, then
  `git format-patch -1 --stdout <sha> | git apply -3 --directory=rendering/bevy-explorer/bevy`.
  Name the upstream PR on this page, as the entries above do.
- Our delta over upstream, for a migration: `git fetch https://github.com/bevyengine/bevy tag v0.16.1`,
  then `git diff v0.16.1^{tree} HEAD:rendering/bevy-explorer/bevy` (the import's omissions show
  as deletions).
- CI reuses a cached bevy build until its files change: Cargo would otherwise rebuild a path
  dependency after every checkout, since a checkout makes every file new.
  `tooling/bevy-mtimes.py` records the files' blob ids and times with the cache and gives
  unchanged files their times back; every job that caches the explorer's target runs it.
- Never share a `target/` directory with a checkout that builds a different bevy.
- The lockfile is edited by hand, never with `cargo update`: see the dependencies section of
  [contributing](contributing.md).
