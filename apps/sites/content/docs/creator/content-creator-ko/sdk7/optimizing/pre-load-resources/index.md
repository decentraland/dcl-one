> 리소스 미리 불러오기를 사용하면 에셋이 씬 시작 시 다운로드되어 플레이어가 처음 상호작용할 때 이미 준비되어 있습니다.

# 리소스 미리 불러오기

경우에 따라 에셋이 씬에 추가되지만 즉시 사용되지는 않습니다. 예를 들어, 사운드 파일은 플레이어가 버튼을 눌렀을 때만 재생될 수 있습니다. 이 경우 플레이어가 처음 버튼을 누를 때 파일이 필요할 때만 다운로드되므로 오디오가 몇 초 늦게 재생될 수 있습니다.

이 문제를 방지하려면 다음을 사용하세요. `AssetLoad` 컴포넌트를 사용하여 이러한 에셋이 필요해지기 전에 다운로드되어 사용할 준비가 되도록 하세요.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

다음에 나열된 에셋은 `AssetLoad` 컴포넌트에 의해 다운로드되어 메모리에 추가되므로, 씬에서 이를 로드해야 할 때 즉시 사용할 수 있습니다.

몇 가지 중요한 고려 사항:

* 다음을 배치할 수 있습니다. `AssetLoad` 컴포넌트는 RootEntity뿐만 아니라 모든 엔티티에 배치할 수 있으며, 필요한 만큼 많은 엔티티에서 사용할 수 있습니다. 이는 씬의 서로 다른 레벨이나 영역에 대한 별도의 로드 상태를 처리하는 데 유용할 수 있습니다.
* **각 `AssetLoad` 엔티티에는 약 40개의 경로만 유지하세요.** 더 긴 목록은 여러 엔티티에 나누어 배치하세요. 다음을 참조하세요. [긴 목록을 엔티티 간에 분할하기](#split-long-lists-across-entities).
* 다음 `AssetLoad` 컴포넌트는 에셋을 메모리에 추가하는 데 사용되며, 제거하는 데 사용되지 않습니다. 다음 목록에서 에셋을 제거해도 `AssetLoad.create` 메모리가 해제되지는 않습니다.
* 에셋이 씬 로드 시 즉시 사용되는 경우(예: 씬에 배치된 GLB 모델 또는 계속 재생되는 배경음)에는 다음을 사용할 필요가 없습니다. `AssetLoad` 컴포넌트는 이미 다운로드되고 있기 때문입니다.
* 에셋을 다음에 추가할 때 유의하세요. `AssetLoad.create`, 불필요한 성능 비용을 피하기 위해 씬 시작 시 필요하지 않은 에셋만 미리 로드하세요.
* 씬 파일의 일부로 업로드된 에셋만 미리 로드할 수 있습니다. 이 기능은 외부 URL의 이미지를 미리 로드하는 데는 작동하지 않습니다.

## 로딩 상태에 반응하기

씬이 각 에셋의 다운로드 완료 시점에 반응할 수 있다면 에셋 미리 로딩이 더 유용합니다. 예를 들어 모든 에셋이 준비될 때까지 로딩 화면을 유지하거나, 플레이어에게 진행 상황을 표시하거나, 다운로드에 실패한 에셋을 자연스럽게 처리할 수 있습니다.

이를 추적하려면 다음을 사용하세요. `assetLoadLoadingStateSystem`. 다음 메서드를 호출하세요. `registerAssetLoadLoadingStateEntity` 메서드에 다음을 보유한 엔티티를 전달하세요. `AssetLoad` 컴포넌트와 콜백 함수입니다. 해당 엔티티의 에셋 중 하나의 로딩 상태가 변경될 때마다 콜백이 실행됩니다.

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
      `에셋 ${assetLoadState.asset}의 현재 상태: ${assetLoadState.currentState}`
    )
  }
)
```

콜백은 다음 속성을 가진 객체를 받습니다:

* `asset`: 상태가 변경된 에셋의 경로입니다. 이는 다음에 나열한 문자열 중 하나와 일치합니다. `AssetLoad` 컴포넌트.
* `currentState`: 다음의 값입니다. `LoadingState` 열거형으로, 해당 에셋의 새 상태를 설명합니다.

다음 `LoadingState` 열거형은 다음 값을 가질 수 있습니다:

* `LoadingState.LOADING`: 에셋이 현재 다운로드되고 있습니다.
* `LoadingState.FINISHED`: 에셋 다운로드가 성공적으로 완료되어 사용할 준비가 되었습니다.
* `LoadingState.FINISHED_WITH_ERROR`: 에셋은 찾았지만 다운로드 중 오류가 발생했습니다.
* `LoadingState.NOT_FOUND`: 제공된 경로에서 에셋을 찾을 수 없습니다.
* `LoadingState.UNKNOWN`: 에셋의 상태를 알 수 없습니다.

다음 예제에서는 로딩 상태를 사용하여 미리 로드 진행 상황에 따라 엔티티의 색상을 변경합니다:

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
> **💡 팁**: 기존 `AssetLoad` 컴포넌트를 생성한 후, 예를 들어 플레이어 동작에 반응하여 더 많은 에셋을 추가할 수 있습니다. 다음을 사용하세요. `AssetLoad.getOrCreateMutable()` 를 사용하여 컴포넌트를 가져오고 해당 `assets` 배열에 새 경로를 추가하세요. 등록된 콜백은 새로 추가된 에셋이 다운로드될 때도 실행됩니다.


### 긴 목록을 엔티티 간에 분할하기

각 `AssetLoad` 엔티티에 **최대 40개의 경로를 지정하세요**. 하나의 엔티티에 에셋이 약 50개를 넘으면 콜백이 더 이상 실행되지 않고, 오류 없이 나머지 에셋은 보고되지 않습니다.

그 이유는 로딩 상태 저장 방식에 하드 제한이 있기 때문입니다. 클라이언트는 에셋당 상태가 변경될 때마다 항목 하나를 추가하며, 일반적으로 `LOADING` 그리고 나서 `FINISHED`를 엔티티당 100개 항목을 보유하는 세트에 추가합니다. 세트가 가득 차면 100개로 유지되며, SDK는 항목 수가 변경될 때만 콜백을 호출하므로 이후에는 아무것도 전달되지 않습니다.

40개가 넘는 에셋을 미리 로드하려면 목록을 배치로 나누고 각 배치에 자체 엔티티와 자체 콜백을 지정하세요:

```ts
import { AssetLoad, assetLoadLoadingStateSystem, engine } from '@dcl/sdk/ecs'

const allAssets = [
	'assets/models/tree.glb',
	'assets/models/rock.glb',
	// ...긴 목록
]

// 경로 40개 배치당 AssetLoad 엔티티 하나
for (let i = 0; i < allAssets.length; i += 40) {
	const batchEntity = engine.addEntity()
	AssetLoad.create(batchEntity, { assets: allAssets.slice(i, i + 40) })

	assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(batchEntity, (assetLoadState) => {
		console.log(assetLoadState.asset, assetLoadState.currentState)
	})
}
```


> [!NOTE]
> **💡 팁**: 씬이 시작되기 전에 모든 에셋을 기다린다면, 해당 대기에 타임아웃을 설정하고 응답을 받지 못한 경로를 기록하세요. 이름이 지정된 목록은 사라지지 않는 로딩 화면보다 디버깅하기 훨씬 쉽습니다.



> [!NOTE]
> **💡 팁**: 이 컴포넌트의 작동 예제는 다음을 참조하세요. [`88,-12-asset-load`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load) 테스트 씬은 하나의 `AssetLoad` 컴포넌트를 통해 mp3, 텍스처, 비디오 및 glb를 미리 로드하고 각 에셋의 상태를 다음을 통해 보고합니다. `assetLoadLoadingStateSystem` — 의도적으로 누락된 경로가 다음으로 해석되는 경우도 포함합니다. `NOT_FOUND`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/optimizing/pre-load-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
