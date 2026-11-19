# Authentication platform

The disc the avatar stands on in `mode=authentication`, as the Unity renderer draws it.

From `decentraland/unity-explorer` at `49c1644901` (Apache-2.0),
`avatar-preview-renderer/Assets/Models/PreviewPlatform/`:

- `platform.glb`: `PreviewPlatform.fbx` converted with Blender 5.2, its node transform dropped
  (the Unity importer replaces it with the scene's: scale 0.7, 0.0813 m down, upright), no
  material, with tangents;
- `base_color.png`, `emission.png`, `normal.png`: `Platform_BaseColor.png`,
  `Platform_Emission.png` and `Platform_Normal.png`, unchanged.

`src/stage.rs` builds its material from `Platform_MAT`: no metal, smoothness 0, the emission map
times the material's HDR emission colour.
