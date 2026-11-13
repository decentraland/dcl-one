# Landscape assets

The files under `src/assets/landscape/`, the impostors excepted, come from the Decentraland Unity explorer,
[decentraland/unity-explorer](https://github.com/decentraland/unity-explorer), and are embedded
into the build as `embedded://landscape/...`. They are stored in Git LFS (see the repository's
`.gitattributes`). Every CI job that builds the Explorer pulls only this folder from LFS after its
checkout (`git lfs pull --include="rendering/bevy-explorer/source/crates/assets/src/assets/landscape/**"`),
so the build embeds the files rather than their pointers; a local checkout needs `git lfs pull`.

That repository is licensed under the Apache License 2.0 (its `LICENSE` file). The tree layouts are
Decentraland's own placement data, and the tree, bush and stone models and their textures
Decentraland's own art, published there.

Unity paths are relative to `Explorer/Assets/` in that repository.

The models and their textures are converted with `landscape_tools/fbx_to_glb.py` in Blender 4.5.9 LTS,
run as `blender --background --factory-startup --python-exit-code 1 --python landscape_tools/fbx_to_glb.py -- <arguments>`
from this crate's folder, with `$U` standing for the Unity repository's `Explorer/Assets` folder. The
`mesh` command keeps the first level of detail only and writes no textures; the `texture` command
writes the image at 512x512 with its stored values unchanged.

The impostors under `src/assets/landscape/impostors/` are generated here, not taken from Unity:
each is baked by the explorer's own impostor library, boimp (git revision
`dd1953bf0ccb1cbf692b75b000b940fbfeb34f72`, MIT or Apache-2.0), from the model of the same name
under `src/assets/landscape/trees/` dressed in its textures, so it carries that model's licence.
The bake photographs the model, held still, from 8 by 8 directions over the upper hemisphere,
64 by 64 texels each (a 512 by 512 atlas before boimp trims the empty edges), with 8 by 8 samples
per texel, framing a sphere around the model's bounds, and stores it with boimp's palette format
at an RGB error budget of 10. It runs as a test in a renderer with no window, from
`rendering/bevy-explorer/source` on a machine with a GPU:

```sh
LANDSCAPE_IMPOSTORS_OUT="$PWD/crates/assets/src/assets/landscape/impostors" cargo test -p visuals bake_landscape_impostors -- --ignored --nocapture
```

The client's `/bake_landscape_impostors <out_dir>` console command (native builds outside preview
mode) runs the same bake. Both delete the old files first and check each was written anew.
A bake is not byte-for-byte repeatable: the GPU's sampling and the palette's ordering vary the
bytes from run to run.

| Files here | Unity repository and commit | Unity source path | Licence | Conversion | Tool and version |
| --- | --- | --- | --- | --- | --- |
| `src/assets/landscape/layout/genesis.trees` (2,215,004 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `StreamingAssets/GenesisTrees.bin` | Apache-2.0 | none (copied, renamed; the `.meta` file is not copied) | `cp` (GNU coreutils 8.32) |
| `src/assets/landscape/layout/worlds.trees` (2,215,004 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `StreamingAssets/WorldsTrees.bin` | Apache-2.0 | none (copied, renamed; the `.meta` file is not copied) | `cp` (GNU coreutils 8.32) |
| `src/assets/landscape/trees/tree01.glb` (143,696 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/TreeOptimized_v01.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/TreeOptimized_v01.fbx --object BirchOptimized_v01 --material LeafYellow --material Trunk --out src/assets/landscape/trees/tree01.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/tree02.glb` (176,320 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/TreeOptimized_v02.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/TreeOptimized_v02.fbx --object BirchOptimized_v02 --material LeafPink --material Trunk --out src/assets/landscape/trees/tree02.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/tree03.glb` (188,420 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/TreeOptimized_v03.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/TreeOptimized_v03.fbx --object BirchOptimized_v03 --material LeafGreen --material Trunk --out src/assets/landscape/trees/tree03.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/bush01.glb` (14,576 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/BushOptimized_v01.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/BushOptimized_v01.fbx --object BushOptimized_v01 --material LeafBushGreenDark --out src/assets/landscape/trees/bush01.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/bush02.glb` (70,404 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/BushOptimized_v02.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/BushOptimized_v02.fbx --object BushOptimized_v02 --material LeafBushGreen --material Trunk --out src/assets/landscape/trees/bush02.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone01.glb` (11,200 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Rocks/Rock01/Rock01.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/Rocks/Rock01/Rock01.fbx --object Rock01_LOD0 --material Rock01 --out src/assets/landscape/trees/stone01.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone02.glb` (6,060 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Rocks/Rock02/Rock02.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/Rocks/Rock02/Rock02.fbx --object Rocks02_LOD0 --material Rock02 --out src/assets/landscape/trees/stone02.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone03.glb` (6,640 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Rocks/Rock03/Rock03.fbx` | Apache-2.0 | `mesh --fbx $U/DCL/Landscape/Assets/Rocks/Rock03/Rock03.fbx --object Rock03_LOD0 --material Rock03 --out src/assets/landscape/trees/stone03.glb` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/leaf.png` (49,233 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/Leaf_Mask4_basecolor.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/Leaf_Mask4_basecolor.png --size 512 --alpha-from-gray --out src/assets/landscape/trees/leaf.png` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/bush_leaves.png` (47,207 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/Bushes_Leaves.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/TreeOptimisation/TreeOptimizationAssets/Bushes_Leaves.png --size 512 --alpha-from-gray --out src/assets/landscape/trees/bush_leaves.png` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/trunk_color.jpg` (75,296 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Trees/Trunk_Color.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Trees/Trunk_Color.png --size 512 --out src/assets/landscape/trees/trunk_color.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/trunk_normal.jpg` (17,331 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Trees/Trunk_Normal.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Trees/Trunk_Normal.png --size 512 --out src/assets/landscape/trees/trunk_normal.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone01_color.jpg` (15,344 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedRock01_baseColor.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Textures/stylizedRock01_baseColor.png --size 512 --out src/assets/landscape/trees/stone01_color.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone01_normal.jpg` (28,475 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedRock01_normal.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Textures/stylizedRock01_normal.png --size 512 --out src/assets/landscape/trees/stone01_normal.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone02_color.jpg` (16,239 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedRock02_baseColor.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Textures/stylizedRock02_baseColor.png --size 512 --out src/assets/landscape/trees/stone02_color.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone02_normal.jpg` (29,238 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedRock02_normal.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Textures/stylizedRock02_normal.png --size 512 --out src/assets/landscape/trees/stone02_normal.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone03_color.jpg` (17,278 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedRock03_baseColor.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Textures/stylizedRock03_baseColor.png --size 512 --out src/assets/landscape/trees/stone03_color.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/stone03_normal.jpg` (28,053 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Textures/stylizedRock03_normal.png` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Textures/stylizedRock03_normal.png --size 512 --out src/assets/landscape/trees/stone03_normal.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/trees/terrain_color.jpg` (18,326 bytes) | decentraland/unity-explorer `206efdac5b8d447899e70dd732f45af9482ed97b` | `DCL/Landscape/Assets/Terrain/TerrainBase_basecolor.tga` | Apache-2.0 | `texture --image $U/DCL/Landscape/Assets/Terrain/TerrainBase_basecolor.tga --size 512 --out src/assets/landscape/trees/terrain_color.jpg` | Blender 4.5.9 LTS |
| `src/assets/landscape/impostors/tree01.boimp` (124,341 bytes) | generated | baked from `src/assets/landscape/trees/tree01.glb` | as its model, Apache-2.0 | the bake above | boimp `dd1953b` |
| `src/assets/landscape/impostors/tree02.boimp` (131,314 bytes) | generated | baked from `src/assets/landscape/trees/tree02.glb` | as its model, Apache-2.0 | the bake above | boimp `dd1953b` |
| `src/assets/landscape/impostors/tree03.boimp` (115,425 bytes) | generated | baked from `src/assets/landscape/trees/tree03.glb` | as its model, Apache-2.0 | the bake above | boimp `dd1953b` |
| `src/assets/landscape/impostors/bush01.boimp` (43,708 bytes) | generated | baked from `src/assets/landscape/trees/bush01.glb` | as its model, Apache-2.0 | the bake above | boimp `dd1953b` |
| `src/assets/landscape/impostors/bush02.boimp` (121,007 bytes) | generated | baked from `src/assets/landscape/trees/bush02.glb` | as its model, Apache-2.0 | the bake above | boimp `dd1953b` |
