# Visual scenes

The 17 SDK7 scenes of explorer-automation's Visual suite (`explorer/scenes/packages` at
`955b338`, 2026-10-22), copied once: each exercises one rendering path (primitives and
transforms, glTF models, materials, lights, text, UI, tweens, raycasts, animations, the avatar)
and ends in `setupVisualTest`, which hides avatars and locks a virtual camera, so a frame of it
is deterministic. Every reference is recorded by
dcl-one's CI (`../references`).

The scenes were authored for one host scene that swapped bundles. Here every scene is a member of
`dcl-workspace.json` at its own parcels, `12k,0` for its index `k` in that list (the widest scene
spans three parcels, so none can see another), with the host's `skyboxConfig.fixedTime` of 43200
(noon). Positions in a scene's source are scene-relative, so nothing else changed but each scene's
`tsconfig.json`, which extends the vendored SDK's `@dcl/sdk/types/tsconfig.ecs7.json` as
`dcl-scene-cli init` writes it (the host's shared base file did not come along), and
`gltf-materials`' grid and camera, centred on its two parcels (x 16 instead of 8): the 18 m grid
crossed the scene's west edge, where the engine dissolves geometry with a noise that moves with
time. The visual spec (`../specs/visual`) teleports to each scene's base parcel.

`smart-wearable` is the last member and no visual scene: a smart-wearable project
(`wearable.json`, a `hat` whose `model.glb` is `swap_material`'s cube, and `scene.json` with
`"isPortableExperience": true` and no `requiredPermissions`, which the engine ignores) whose code logs
`smart wearable fixture started` and shows the screen-space label `Smart wearable running`. The
`smart-wearables` spec serves it from a loopback realm's `/preview-wearables`, and once, renamed
and at `0,0`, as the parcel scene that disables portable experiences; its own parcel `204,0`
(index 17) only keeps a workspace preview clear of the others. It
carries dcl-scene-cli's scene `tsconfig.json` and no `scripts.build`, so `build-scenes` bundles it
with the CLI's built-in build into `bin/` (a loader, `index.js`, and the chunks it reads).

Build them with `apps/dcl-scene-cli` (the vendored SDK toolchain, no npm):

```sh
DCL_SCENE_CLI=/path/to/dcl-scene-cli pnpm run build-scenes        # every scene
DCL_SCENE_CLI=/path/to/dcl-scene-cli pnpm run build-scenes core   # one
```

## The pointer and raycast scenes

Two scenes outside explorer-automation, in the workspace but not in `SCENES`, so without a
reference:

- `pointer-targets`, parcels `0,20` to `9,20`, spawn at `(8, 0, 8)`: the targets of
  `specs/visual/pointer.test.ts`, which hovers and clicks them through `/hover_entity` and
  `/click_entity`. A near box with a Down and an Up (`Near`), a wall and the box it hides
  (`Behind`), a box 50 m out reachable by camera distance (`Far`, `maxCameraDistance: 60`) and
  the avocado glTF (`Model`, its pointer events on the container). The layout holds whether the
  player faces the spawn's `cameraTarget` (+X) or keeps facing +Z after the teleport, as the
  explorer does. It logs `pointer-targets ready` once the entities exist, then
  `pointer down entity=<id> tick=<tick>` and `pointer up entity=<id> tick=<tick>` once per new
  input command on a target.
- `raycast-storm`, parcel `0,22`: 200 invisible entities in a 10 by 20 grid, each with a
  continuous raycast down onto one floor collider, the load of `specs/visual/raycast-budget.test.ts`.
  It logs `raycast entity=<id>` the first time an entity's raycast answers and
  `raycast-storm complete` once all 200 have.

`bin/`, `node_modules/` and the other build outputs are ignored. The `.glb`, `.png` and `.jpg`
assets are in Git LFS.

## Environment components

[`environment`](environment/README.md), parcel `0,24`, authors the root Skybox, Sun and
EnvironmentFog components through the SDK. It is a workspace fixture without a visual
reference. After building it, `DCL_E2E_NATIVE_BIN=/path/to/decentra-bevy pnpm run check:environment`
serves an isolated loopback realm and checks its optional values, sky image, component removal
and scene-departure reset through the native console. Captures are saved in `report/environment`.

## Material fidelity charts

`material-fidelity`, parcels `0,26` to `9,26`, is a workspace fixture without a visual reference:
charts of the SDK material for viewing in two clients side by side. Eight boards stand in a row,
20 m apart, each tilted 45 degrees towards the sky and holding a 6 by 6 grid of spheres. A row
varies one value across its six columns and holds the rest, and the line under it names the
values left to right. Boards 7 and 8 also use boxes, planes, cylinders, cones and glTF cubes.

| Board | Rows |
| --- | --- |
| 1 | metallic by roughness, both 0 to 1, white albedo |
| 2 | the same chart, red albedo |
| 3 | `specularIntensity`, `reflectivityColor` and `directIntensity`, each at metallic 0 and 1 |
| 4 | `emissiveIntensity` 0 to 512, `emissiveColor` hues on black and white albedo, `emissiveTexture` |
| 5 | albedo alpha under blend, unset and opaque modes; `alphaTest` thresholds; every transparency mode on a texture with alpha; blend over other values; the deprecated PBR `alphaTexture`. A black and a white tile sit behind every sphere |
| 6 | albedo texture and tint, `bumpTexture`, wrap, tiling, offset and filter modes, unlit colour, texture and `alphaTexture`, `castShadows`, a sphere without a Material and one with an empty PBR material |
| 7 | which way up a texture sits on each shape; wrap and filter modes on images used by one cell each; `bumpTexture` on each shape; the emissive rules (texture without a colour, a black colour, a colour over an albedo texture, an empty `src`); lit and unlit grey ramps |
| 8 | `bumpTexture` on planes, a sphere and a box rolled about the board's normal, which brings each axis of the normal map under the sun; a glTF cube as it is and with SDK materials put on it through `GltfNodeModifiers`; glTF cubes with emissive materials of their own |

Each board has a virtual camera on its normal, 11.5 m out, so two clients at the same board
render the same frame. The scene starts at board 1; E goes to the next board and F to the
previous one, and after board 8 comes the free player camera. Avatars are hidden. The scene adds
no light, so the sun and sky of each client at the fixed noon light the boards.

`assets/lava.jpg` and `assets/smoke.png` are the `materials` scene's textures;
`assets/bumps-normal.png` is a generated tangent-space normal map (+Y up) of round bumps,
`assets/orientation.png` has a colour for each corner and an arrow to its top, and the
`assets/checker-*.png` files are 16 px checkers with one dark corner cell. The
`assets/cube-*.glb` files are generated unit cubes with normals and uvs and no tangents: one
plain, and six black ones whose material emits (a factor of 0.25, 0.5 and 1, orange, the lava
texture, and `KHR_materials_emissive_strength` 4).

To compare clients, build it (`pnpm run build-scenes material-fidelity`) and deploy the folder
to a world, adding the world's name to a copy of `scene.json`.
