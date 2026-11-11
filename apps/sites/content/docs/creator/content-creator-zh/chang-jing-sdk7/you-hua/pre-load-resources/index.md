> 预加载资源可让资源在场景启动时就被下载好，这样玩家首次与它们交互时就已准备就绪。

# 预加载资源

在某些情况下，资源会被添加到场景中，但不会立即使用。例如，音频文件可能只会在玩家按下按钮时播放。在这种情况下，玩家第一次按下按钮时，音频可能会延迟几秒播放，因为该文件仅在需要时才下载。

为避免此问题，请使用 `AssetLoad` 组件，以确保这些资源在需要前已下载完毕并可供使用。

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

列在 `AssetLoad` 组件中的资源会被下载并添加到内存中，确保场景需要加载它们时能够立即使用。

一些重要注意事项：

* 你可以将 `AssetLoad` 组件放在任何实体上（不只是 RootEntity），并且可根据需要在任意多个实体上使用它。这有助于处理场景中不同关卡或区域的独立加载状态。
* **请将每个 `AssetLoad` 实体控制在大约 40 个路径。** 将较长的列表拆分到多个实体中。请参阅 [将长列表拆分到多个实体](#split-long-lists-across-entities).
* 该 `AssetLoad` 组件用于将资源添加到内存中，而不是移除它们。在 `AssetLoad.create` 中从列表移除资源不会释放内存。
* 如果资源会在场景加载时立即使用（例如放置在场景中的 GLB 模型，或持续播放的背景音），则无需使用 `AssetLoad` 组件，因为它已经在下载中。
* 向其中添加资源时请谨慎 `AssetLoad.create`，并且仅预加载场景启动时不需要的资源，以避免不必要的性能开销。
* 你只能预加载作为场景文件一部分上传的资源。此功能无法预加载来自外部 URL 的图像

## 响应加载状态

如果场景能够响应每项资源何时完成下载，预加载资源会更有用。例如，你可能希望在所有资源准备就绪前保持加载画面显示、向玩家展示进度，或妥善处理下载失败的资源。

要跟踪此状态，请使用 `assetLoadLoadingStateSystem`。调用其 `registerAssetLoadLoadingStateEntity` 方法，传入包含 `AssetLoad` 组件的实体和一个回调函数。每当该实体的某项资源加载状态发生变化时，都会运行该回调。

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
      `资源 ${assetLoadState.asset} 当前状态为：${assetLoadState.currentState}`
    )
  }
)
```

回调会接收一个具有以下属性的对象：

* `asset`：状态发生变化的资源路径。这与您在 `AssetLoad` 组件中列出的字符串之一相匹配。
* `currentState`：来自 `LoadingState` 枚举的值，用于描述该资源的新状态。

该 `LoadingState` 枚举可以包含以下值：

* `LoadingState.LOADING`：该资源当前正在下载。
* `LoadingState.FINISHED`：该资源已成功下载完成并可供使用。
* `LoadingState.FINISHED_WITH_ERROR`：已找到该资源，但在下载时发生错误。
* `LoadingState.NOT_FOUND`：在提供的路径中未找到资源。
* `LoadingState.UNKNOWN`：该资源的状态未知。

以下示例使用加载状态，根据预加载的进展情况更改实体的颜色：

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
> **💡 提示**：创建后，你可以向现有 `AssetLoad` 组件中添加更多资源，例如响应玩家操作时。使用 `AssetLoad.getOrCreateMutable()` 来获取该组件，并将新路径推送到其 `assets` 数组中。已注册的回调也会在这些新添加的资源下载时触发。


### 将长列表拆分到多个实体

为每个 `AssetLoad` 实体 **最多指定 40 个路径**。当一个实体上的资源数量超过约 50 个时，其回调将停止触发，剩余资源永远不会被报告，且不会出现错误。

原因是加载状态的存储存在硬性限制。客户端会针对每项资源的每次状态变化追加一条记录，通常是 `LOADING` 然后是 `FINISHED`，追加到每个实体可容纳 100 条记录的集合中。该集合一旦满了就会保持在 100 条，而 SDK 仅在条目数量变化时调用你的回调，因此后续内容都无法传递。

如需预加载超过 40 项资源，请将列表拆分为批次，并为每个批次分配各自的实体和回调：

```ts
import { AssetLoad, assetLoadLoadingStateSystem, engine } from '@dcl/sdk/ecs'

const allAssets = [
	'assets/models/tree.glb',
	'assets/models/rock.glb',
	// ……较长的列表
]

// 每批 40 个路径对应一个 AssetLoad 实体
for (let i = 0; i < allAssets.length; i += 40) {
	const batchEntity = engine.addEntity()
	AssetLoad.create(batchEntity, { assets: allAssets.slice(i, i + 40) })

	assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(batchEntity, (assetLoadState) => {
		console.log(assetLoadState.asset, assetLoadState.currentState)
	})
}
```


> [!NOTE]
> **💡 提示**：如果你的场景在启动前等待所有资源，请为该等待设置超时，并记录那些从未收到响应的路径。具名列表比永远不消失的加载画面更容易调试。



> [!NOTE]
> **💡 提示**：有关该组件的可运行示例，请参阅 [`88,-12-asset-load`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load) 测试场景，该场景通过一个 `AssetLoad` 组件预加载一个 mp3、一个纹理、一个视频和一个 glb，并通过 `assetLoadLoadingStateSystem` 报告每项资源的状态——包括一个故意缺失、最终解析为 `NOT_FOUND`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/you-hua/pre-load-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
