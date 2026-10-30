> How many things can I put on my scene?

# Scene Limitations

In order to improve performance in the metaverse, we have established a set of limits that every scene must follow. These limits are per-parcel. So the larger the scene, the higher these limits are set.

When working with the [Creator Hub](/docs/creator/scene-editor/get-started/editor-installation/), you can see stats about the resources used by 3D models in your scene, together with the limits for your scene.

![](/docs/creator/images/editor/triangle-limit1.webp)

You can expand this menu to view details.

![](/docs/creator/images/editor/triangle-limit2.webp)


> [!NOTE]
> **💡 Tip**: For a reference table of all specific numbers per parcel count, see:
> 
> [Reference table](https://docs.google.com/spreadsheets/d/1BTm0C20PqdQDAN7vOQ6FpnkVncPecJt-EwTSNHzrsmg/edit#gid=0)


## Scene limitation rules

Below are the maximum number of elements that a scene is allowed to render at the same time:

> _n_ represents the number of parcels that a scene occupies.

* **Triangles:** `n x 10000` Total amount of triangles for all the models in the scene.
* **Entities:** `n x 200` Amount of entities in the scene.
* **Bodies:** `n x 300` Amount of meshes in the scene.
* **Materials:** `log2(n+1) x 20` Amount of materials in the scene. It includes materials imported as part of models.
* **Textures:** `log2(n+1) x 10` Amount of textures in the scene. It includes textures imported as part of models.
*   **Height:** `330 meters` Maximum height of the scene, regardless of how many parcels it occupies.

    > Warning: Entities placed higher than 200 meters may suffer multiplayer sync issues. Keep any content that players interact with or that is synced between players below that height.

    > Important: Only entities that are currently being rendered in the scene are counted for these limits. If your scene switches between 3D models, what matters is the rendered models at any point in time, not the total sum. Player avatars and any items brought by a player from outside the scene don't count for calculating these limits either.
* **Total file size:** In Genesis City -`15 MB per parcel - 300 MB max`. For Worlds, see [World size](/docs/creator/sdk7/optimizing/scene-limitations/#world-size). Total size of the files uploaded to the content server. Includes 3D models and audio. Doesn't include files that aren't uploaded, such as node.js packages. You can see the full list of files being published and their sizes before you confirm a deployment.
* **File count:** `200 files per parcel` Total count of the files uploaded. Includes 3D models and audio. Doesn't include files that aren't uploaded, such as node.js packages.
* **Max file size** `50 MB per file` No individual file of any type in the scene can exceed 50 MB. Small scenes are restricted further because the file mustn't exceed their Total File Size limit (For example, a single-parcel scene is limited to 15 MB total).


> [!WARNING]
> **📔 Note**: The file size and file count limits are enforced when deploying, exceeding them blocks the deployment. The other limits (triangles, entities, bodies, materials, textures, height) are soft limits: they're reported as warnings by the Creator Hub, but exceeding them mostly results in degraded performance rather than a hard failure. Treat them as strong recommendations.



> [!NOTE]
> **💡 Tip**: Not all files in your scene project folder count for the file size limit, only those that are uploaded to servers. All of the contents of the _node\_modules_ folder, which are very large, are dependencies that are not uploaded and therefore don't count. The same applies to any files in the `/src` folder, since the source code is not uploaded.
> 
> You can list any other files or folders you want to exclude from being uploaded in the `.dclignore` file in your project.


## Optimizing

See [Performance Optimization](/docs/creator/sdk7/optimizing/performance-optimization/) for tips about how you can keep your scene below these limits and make it run smoother for players.


> [!NOTE]
> **📱 Mobile**: The hard limits on this page apply to all clients, but mobile devices have less headroom than desktop. Aim to stay comfortably below these limits when targeting the [mobile app](/docs/creator/build-for-mobile/mobile-client/overview/), and prefer fewer triangles, fewer materials, and smaller textures.


## Scene boundaries

When running a preview, any content that is located outside the parcel boundaries is highlighted in red when rendered. If any content is outside these boundaries, that part of your content won't be rendered when players visit your scene.

If the tip of a large object leaves the boundaries, this tip will be sliced off the object.

A single parcel scene measures 16 meters x 16 meters. If the scene has multiple parcels, the dimensions vary depending on the arrangement of the parcels.

It's possible to position entities underground, to either hide them or to have only a portion of them emerge. A scene can't have tunnels that go below the default ground height, players can't travel below the `y = 0` height.

## Shader limitations

3D models used in decentraland must use supported shaders and materials. See [3D model materials](/docs/creator/3d-modeling/materials/) for a list of supported shaders.

## Mesh compression


> [!WARNING]
> **📱 Draco-compressed meshes on the [mobile client](/docs/creator/build-for-mobile/mobile-client/overview/) require asset optimization.** glTF/GLB models that use [Draco](https://google.github.io/draco/) mesh compression load on the Decentraland mobile app only after they've been processed through the asset optimization pipeline. If your Draco-compressed models fail to load on mobile, verify they've been optimized — or export your models **without** Draco compression to avoid the step entirely. Other forms of glTF optimization (quantization, texture compression, mesh decimation) are unaffected.


## Lighting

The scene's lighting conditions can't be changed for all players from the default setting, although each individual player is free to change their own skybox settings from the Explorer UI.

## Texture size constraints

Texture sizes must use width and height numbers (in pixels) that match the following numbers:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> This sequence is made up of powers of two: `f(x) = 2 ^ x` . 1024 is the maximum number we allow for a texture size, textures larger than that are scaled down by the asset bundle conversion process. This is a fairly common requirement among other rendering engines, it's there due internal optimizations of the graphics processors.

The width and height don't need to have the same number, but they both need to belong to this sequence.

**The recommended size for textures is 1024x1024**, we have found this to be the optimal size to be transported through domestic networks and to provide reasonable loading/quality experiences.

Examples of other valid sizes:

```
32x32
64x32
512x256
512x512
1024x1024
```


> [!WARNING]
> **📔 Note**: Although textures of arbitrary sizes sometimes work, they are also often rendered with bugs and are more unstable. We strongly advise that all your textures match these sizes.


## World Size

Decentraland [Worlds](/docs/creator/sdk7/publishing/publishing-options/#decentraland-worlds) have different limitations, since they are loaded as single scenes.

* Worlds published to Decentraland NAMEs draw from a storage budget that is shared across all the Worlds owned by the same wallet. Each NAME you own grants `100 MB`, each LAND parcel grants an additional `100 MB`, and every 2,000 MANA held grants another `100 MB`. You can distribute this budget between your Worlds however you like.
* Worlds published to ENS domains have a limit of `36 MB` that cannot be expanded.

You can check how much of your budget is used and how much remains in the **Manage** section of the Creator Hub, or in the **Worlds** tab of the [Builder](https://decentraland.org/builder/worlds). If you exceed your budget (for example after selling or transferring assets), you have 48 hours to free up space before your Worlds become inaccessible.

See [Worlds Size Limit](/docs/creator/sdk7/projects/kinds-of-project/#size-limits) for more details.

Total size of the files uploaded to the content server. Includes 3D models and audio. Doesn't include files that aren't uploaded, such as node.js packages. You can see the full list of files being published and their sizes before you confirm a deployment.

All other limits in worlds are per parcel, including triangles, materials, etc. Since adding more parcels to a world is free, you can add up to 300x300 parcels (90,000 total) to your scene, and have the corresponding limits to that parcel count.
