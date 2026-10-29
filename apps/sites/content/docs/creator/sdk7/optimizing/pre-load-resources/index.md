> Pre-loading Resources allows assets to be downloaded at scene startup so they are ready when the player interacts with them for the first time.

# Pre-loading Resources

In some cases, an asset is added to the scene but is not used immediately. For example, a sound file may only be played when the player presses a button. In this scenario, the first time the player presses the button, the audio may play a couple of seconds too late, because the file is only downloaded when needed.

To avoid this issue, use the `AssetLoad` component to ensure these assets are downloaded and ready to use before they are needed.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

The assets listed in the `AssetLoad` component are downloaded and added to memory, ensuring they are instantly available when the scene needs to load them.

Some important considerations:

- You can place the `AssetLoad` component on any entity (not just the RootEntity), and you can use it on as many entities as needed. This can be helpful to handle separate load states for different levels or regions of your scene.
- The `AssetLoad` component is used to add assets to memory, not to remove them. Removing an asset from the list in `AssetLoad.create` will not free memory.
- If an asset is used immediately when the scene loads (for example, a GLB model placed in the scene, or a background sound that plays continuously), there is no need to use the `AssetLoad` component, since it is already being downloaded. 
- Be mindful when adding assets to `AssetLoad.create`, and only pre-load assets that are not required at scene startup to avoid unnecessary performance costs.
- You can only pre-load assets that are uploaded as part of the scene files. This feature won't work to pre-load images from an external URL

## Reacting to the loading state

Pre-loading assets is more useful if your scene can react to when each asset finishes downloading. For example, you may want to keep a loading screen up until every asset is ready, show progress to the player, or gracefully handle an asset that failed to download.

To track this, use the `assetLoadLoadingStateSystem`. Call its `registerAssetLoadLoadingStateEntity` method, passing the entity that holds the `AssetLoad` component and a callback function. The callback runs every time the loading state of one of that entity's assets changes.

```ts
import {
  AssetLoad,
  LoadingState,
  assetLoadLoadingStateSystem,
} from "@dcl/sdk/ecs"

const preloader = engine.addEntity()
AssetLoad.create(preloader, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})

assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(
  preloader,
  (assetLoadState) => {
    console.log(
      `Asset ${assetLoadState.asset} is now: ${assetLoadState.currentState}`
    )
  }
)
```

The callback receives an object with the following properties:

- `asset`: The path of the asset whose state changed. This matches one of the strings you listed in the `AssetLoad` component.
- `currentState`: A value from the `LoadingState` enum, describing the new state of that asset.

The `LoadingState` enum can hold the following values:

- `LoadingState.LOADING`: The asset is currently being downloaded.
- `LoadingState.FINISHED`: The asset finished downloading successfully and is ready to use.
- `LoadingState.FINISHED_WITH_ERROR`: The asset was found, but an error occurred while downloading it.
- `LoadingState.NOT_FOUND`: No asset was found at the provided path.
- `LoadingState.UNKNOWN`: The state of the asset is not known.

The following example uses the loading state to change the color of an entity depending on how its pre-load is progressing:

```ts
import {
  AssetLoad,
  LoadingState,
  Material,
  assetLoadLoadingStateSystem,
} from "@dcl/sdk/ecs"
import { Color4 } from "@dcl/sdk/math"

function getLoadingColor(state: LoadingState): Color4 {
  switch (state) {
    case LoadingState.FINISHED:
      return Color4.Green()
    case LoadingState.LOADING:
      return Color4.Yellow()
    case LoadingState.FINISHED_WITH_ERROR:
    case LoadingState.NOT_FOUND:
      return Color4.Red()
    default:
      return Color4.Gray()
  }
}

assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(
  preloader,
  (assetLoadState) => {
    Material.setPbrMaterial(myEntity, {
      albedoColor: getLoadingColor(assetLoadState.currentState),
    })
  }
)
```


> [!NOTE]
> **💡 Tip**: You can add more assets to an existing `AssetLoad` component after creating it, for example in response to a player action. Use `AssetLoad.getOrCreateMutable()` to fetch the component and push new paths onto its `assets` array. The registered callback will also fire for these newly added assets as they download.



> [!NOTE]
> **💡 Tip**: For a working example of this component, see the [`88,-12-asset-load`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load) test scene, which pre-loads an mp3, a texture, a video and a glb through one `AssetLoad` component and reports each asset's state through `assetLoadLoadingStateSystem` — including a deliberately missing path that resolves to `NOT_FOUND`.

