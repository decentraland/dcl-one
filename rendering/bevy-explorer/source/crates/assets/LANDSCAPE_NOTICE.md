# Landscape assets

The files under `src/assets/landscape/` come from the Decentraland Unity explorer,
[decentraland/unity-explorer](https://github.com/decentraland/unity-explorer), and are embedded
into the build as `embedded://landscape/...`. They are stored in Git LFS (see the repository's
`.gitattributes`). Every CI job that builds the Explorer pulls only this folder from LFS after its
checkout (`git lfs pull --include="rendering/bevy-explorer/source/crates/assets/src/assets/landscape/**"`),
so the build embeds the files rather than their pointers; a local checkout needs `git lfs pull`.

That repository is licensed under the Apache License 2.0 (its `LICENSE` file).

The ground, sand and cliff textures are Decentraland's own assets, published there (confirmed by
the project on 2026-11-12). They are the textures Unity's runtime binds: the splat, ground and sand
colour of its ground material (`DCL/Landscape/Assets/Terrain/TerrainGround.mat`; that material's
ground and sand normal maps never reach its lighting and are not brought), and the colour and
normal map of its cliff material (`DCL/Landscape/Assets/Materials/Cliff.mat`). KTX2 is the intended
format for them; until its encoder is available they are JPEG (colour) and PNG (data and normals),
converted with FFmpeg, and the engine builds their mip chains as they load.

Unity paths are relative to `Explorer/Assets/` in that repository.

| Files here | Unity repository and commit | Unity source path | Licence | Conversion | Tool and version |
| --- | --- | --- | --- | --- | --- |
| `src/assets/landscape/ground/splat.png` (1024x1024 RGBA PNG) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/SplatAlpha.png` | Apache-2.0 | none (copied byte for byte: linear blend weights, kept lossless) | `cp` (GNU coreutils 8.32) |
| `src/assets/landscape/ground/ground_basecolor.jpg` (1024x1024 JPEG) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/TerrainBase_basecolor.tga` (2048x2048) | Apache-2.0 | scaled to 1024x1024 (area filter), JPEG at `-q:v 2` (high quality): `ffmpeg -i TerrainBase_basecolor.tga -vf scale=1024:1024:flags=area -q:v 2 ground_basecolor.jpg` | FFmpeg 8.1.2 (gyan.dev full build) |
| `src/assets/landscape/ground/sand_basecolor.jpg` (1024x1024 JPEG) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/StylizedGround_v01_basecolor.png` (1024x1024) | Apache-2.0 | JPEG at `-q:v 2` (high quality): `ffmpeg -i StylizedGround_v01_basecolor.png -q:v 2 sand_basecolor.jpg` | FFmpeg 8.1.2 (gyan.dev full build) |
| `src/assets/landscape/cliff/cliff_basecolor.jpg` (1024x1024 JPEG) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedCliff02_basecolor.png` (1024x1024, 16-bit RGBA, alpha opaque) | Apache-2.0 | JPEG at `-q:v 2` (high quality): `ffmpeg -i stylizedCliff02_basecolor.png -q:v 2 cliff_basecolor.jpg` | FFmpeg 8.1.2 (gyan.dev full build) |
| `src/assets/landscape/cliff/cliff_normal.png` (1024x1024 RGB PNG) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedCliff02_normal.png` (1024x1024, 16-bit RGBA) | Apache-2.0 | 8-bit RGB PNG: `ffmpeg -i stylizedCliff02_normal.png -pix_fmt rgb24 cliff_normal.png` | FFmpeg 8.1.2 (gyan.dev full build) |
