> Shrink the 3D models in your scene from the Creator Hub, with a one-click revert.

# Optimize Models

Heavy 3D models are the most common reason a scene is slow to load. The Creator Hub's **Optimize** tool rewrites the `.glb` files in your project to make them smaller: it cleans up the meshes and compresses, shares and shrinks the textures. Your originals are backed up first, so you can undo the whole thing at any time.

## Run it

1. Open your scene and click the **Optimize** button (the speedometer icon) in the title bar.
2. The first time you run it, the Creator Hub asks permission to download the tools it needs. Click **Continue**.
3. Review the options, then start the run.


> [!NOTE]
> **💡 Tip**: You can also start it from the **Local Assets** panel at the bottom of the editor. Open the **File System** tab and click the broom icon, labeled **Optimize models**.


The tools run entirely on your machine. They are downloaded from npm the first time you continue, pinned to fixed versions, and kept in the Creator Hub's data folder. About 20 MB is downloaded once. Your models are never uploaded anywhere.

When the run finishes you get a per-model list showing how much each file shrank. Models that couldn't be processed are marked **failed** and listed first, with the reason. A failure on one model doesn't stop the rest of the run.

## What it changes

### Meshes

**Optimize meshes (weld, reorder, clean up)** is on by default. This pass is lossless: it merges duplicate vertices, reorders indices so the GPU can cache them better, drops unused and duplicated data, and folds flat-color textures into material colors. The result loads in every Decentraland client.

### Textures

Three switches, all on by default:

- **Compress and resize textures**: resizes each texture down to the maximum size set for its type and re-encodes it.
- **Remove duplicate textures**: finds textures with identical pixels and points every model at one shared copy. This is the biggest win when the same texture is reused across many models.
- **Extract embedded textures to files**: pulls textures out of the `.glb` files into shared files under `assets/optimized-textures/`, so a shared texture downloads once instead of once per model. This is required for sharing textures between models.

When **Compress and resize textures** is on, you can also set:

- **Format**: `PNG` is the default and is lossless. `JPEG` and `WEBP` produce smaller files but lose some quality.
- **Quality**: a 1 to 100 slider, 85 by default. It only appears for `JPEG` and `WEBP`, since `PNG` ignores it.
- **Max texture sizes (px)**: the largest height each kind of texture is allowed to have. Anything taller is scaled down proportionally.

| Texture type                         | Default max height |
| ------------------------------------ | ------------------ |
| Base color                           | 1024               |
| Normal                               | 1024               |
| ORM (occlusion, roughness, metallic) | 512                |
| Emissive                             | 512                |
| Other                                | 512                |

Each can be set to any of 32, 64, 128, 256, 512, 1024, 2048 or 4096. Base color and normal maps usually need more resolution than the rest, which is why they start higher.


> [!WARNING]
> **📔 Note**: Only `.glb` files are processed. `.gltf` files are left alone. See [3D models](/docs/creator/3d-modeling/3d-models/) for how to export to `.glb`.


## Undo Optimizations

Your original models are copied to a `.optimize/backup/` folder inside the project before anything is changed. To put them back, open the dialog again and click **Revert to originals**.

Reverting restores every model it changed, deletes the extracted texture files it created, and removes the backup folder and the `.dclignore` block it added. Files you added yourself under `assets/optimized-textures/` are left in place.


> [!NOTE]
> **💡 Tip**: Run the optimizer, then open a [preview](/docs/creator/sdk7/getting-started/preview-scene/) and look at your scene closely. If a model doesn't look right, revert and run again with a higher max size for that texture type, or with **Compress and resize textures** off.


## Related pages

- [3D models](/docs/creator/3d-modeling/3d-models/): exporting and inspecting `.glb` files.
- [Performance optimization](/docs/creator/sdk7/optimizing/performance-optimization/): what else affects how your scene runs.
- [Scene limitations](/docs/creator/sdk7/optimizing/scene-limitations/): the limits your scene has to fit in.
- [Import custom assets](/docs/creator/scene-editor/build/import-items/): adding models to your project.
