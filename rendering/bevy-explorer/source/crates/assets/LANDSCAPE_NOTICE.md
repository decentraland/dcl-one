# Landscape assets

The files under `src/assets/landscape/` come from the Decentraland Unity explorer,
[decentraland/unity-explorer](https://github.com/decentraland/unity-explorer), and are embedded
into the build as `embedded://landscape/...`. They are stored in Git LFS (see the repository's
`.gitattributes`). Every CI job that builds the Explorer pulls only this folder from LFS after its
checkout (`git lfs pull --include="rendering/bevy-explorer/source/crates/assets/src/assets/landscape/**"`),
so the build embeds the files rather than their pointers; a local checkout needs `git lfs pull`.

That repository is licensed under the Apache License 2.0 (its `LICENSE` file). The satellite map
images are Decentraland's own map data, and the tree layouts Decentraland's own placement data,
published there.

Unity paths are relative to `Explorer/Assets/` in that repository.

| Files here | Unity repository and commit | Unity source path | Licence | Conversion | Tool and version |
| --- | --- | --- | --- | --- | --- |
| `src/assets/landscape/satellite/{x},{y}.jpg`, x and y each 0 to 7 (64 files, 512x512 JPEG) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/MapRenderer/MapLayers/Atlas/SatelliteAtlasFallbackImages/{x},{y}.jpg` (same names) | Apache-2.0 | none (copied byte for byte; the `.meta` files are not copied) | `cp` (GNU coreutils 8.32) |
| `src/assets/landscape/layout/genesis.trees` (2,215,004 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `StreamingAssets/GenesisTrees.bin` | Apache-2.0 | none (copied, renamed; the `.meta` file is not copied) | `cp` (GNU coreutils 8.32) |
| `src/assets/landscape/layout/worlds.trees` (2,215,004 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `StreamingAssets/WorldsTrees.bin` | Apache-2.0 | none (copied, renamed; the `.meta` file is not copied) | `cp` (GNU coreutils 8.32) |
