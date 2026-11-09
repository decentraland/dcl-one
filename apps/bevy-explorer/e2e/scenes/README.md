# Visual scenes

The 17 SDK7 scenes of explorer-automation's Visual suite (`explorer/scenes/packages` at
`955b338`, 2026-10-22), copied once: each exercises one rendering path (primitives and
transforms, glTF models, materials, lights, text, UI, tweens, raycasts, animations, the avatar)
and ends in `setupVisualTest`, which hides avatars and locks a virtual camera, so a frame of it
is deterministic. Their Unity baselines were not copied: every reference is recorded anew by
dcl-one's CI (`../references`).

The scenes were authored for one host scene that swapped bundles. Here every scene is a member of
`dcl-workspace.json` at its own parcels, `12k,0` for its index `k` in that list (the widest scene
spans three parcels, so none can see another), with the host's `skyboxConfig.fixedTime` of 43200
(noon). Positions in a scene's source are scene-relative, so nothing else changed but each scene's
`tsconfig.json`, which extends the vendored SDK's `@dcl/sdk/types/tsconfig.ecs7.json` as
`dcl-scene-cli init` writes it (the host's shared base file did not come along). The visual spec
(`../specs/visual`) teleports to each scene's base parcel.

Build them with `apps/dcl-scene-cli` (the vendored SDK toolchain, no npm):

```sh
DCL_SCENE_CLI=/path/to/dcl-scene-cli npm run build-scenes          # every scene
DCL_SCENE_CLI=/path/to/dcl-scene-cli npm run build-scenes -- core   # one
```

## The pointer and raycast scenes

Two scenes outside explorer-automation, in the workspace but not in `SCENES`, so without a
reference:

- `pointer-targets`, parcels `0,20` to `9,20`, spawn at `(8, 0, 8)` facing +X: the targets of
  `specs/visual/pointer.test.ts`, which hovers and clicks them through `/hover_entity` and
  `/click_entity`. A near box with a Down and an Up (`Near`), a wall and the box it hides
  (`Behind`), a box 50 m out reachable by camera distance (`Far`, `maxCameraDistance: 60`) and
  the avocado glTF (`Model`, its pointer events on the container). It logs `pointer-targets ready` once the entities exist, then
  `pointer down entity=<id> tick=<tick>` and `pointer up entity=<id> tick=<tick>` once per new
  input command on a target.
- `raycast-storm`, parcel `0,22`: 200 invisible entities in a 10 by 20 grid, each with a
  continuous raycast down onto one floor collider, the load of `specs/visual/raycast-budget.test.ts`.
  It logs `raycast entity=<id>` the first time an entity's raycast answers and
  `raycast-storm complete` once all 200 have.

`bin/`, `node_modules/` and the other build outputs are ignored. The `.glb`, `.png` and `.jpg`
assets are in Git LFS.
