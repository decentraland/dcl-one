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

`smart-wearable` is the last member and no visual scene: a smart-wearable project
(`wearable.json`, a `hat` whose `model.glb` is `swap_material`'s cube, and `scene.json` with
`"isPortableExperience": true` and no `requiredPermissions`) whose code logs
`smart wearable fixture started` and shows the screen-space label `Smart wearable running`. The
`smart-wearables` spec serves it from a loopback realm's `/preview-wearables`, never as a scene of
the realm; its parcel `204,0` (index 17) only keeps a workspace preview clear of the others. It
carries dcl-scene-cli's scene `tsconfig.json` and no `scripts.build`, so `build-scenes` bundles it
with the CLI's built-in build into `bin/` (a loader, `index.js`, and the chunks it reads).

Build them with `apps/dcl-scene-cli` (the vendored SDK toolchain, no npm):

```sh
DCL_SCENE_CLI=/path/to/dcl-scene-cli npm run build-scenes          # every scene
DCL_SCENE_CLI=/path/to/dcl-scene-cli npm run build-scenes -- core   # one
```

## The video scene

`video` is the one scene not from explorer-automation: after unity-explorer's UUAV PlayMode suite
(`Explorer/Assets/Plugins/UUAV/Packages/UUAV/Tests`), it plays a committed fixture through the
engine's player, UUAV on the standalone and the browser's video element on the web, at parcel
`204,0`. `assets/video/red_tone.mp4` is 6 s of solid red at 320x240 with a 440 Hz tone, written
by `video/generate-fixture.sh` (ffmpeg, as the Unity fixtures are), so a presented frame is red
whenever it is sampled and the audio path runs too. `specs/visual/video.test.ts` asserts the
player's events and the frame's centre pixel instead of comparing a reference; the scene's label
shows the last event for whoever reads the report's frame.

`bin/`, `node_modules/` and the other build outputs are ignored. The `.glb`, `.png`, `.jpg` and
`.mp4` assets are in Git LFS.
