> 엔티티에 3D 형태와 충돌을 부여하는 다양한 컴포넌트에 대해 알아봅니다.

# 형상 컴포넌트

Decentraland의 3차원 장면은 다음을 기반으로 합니다. [Entity-Component](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) 모델이며, 장면의 모든 것은 *entity*이고, 각 엔티티는 *컴포넌트* 를 포함하여 해당 특성과 기능을 형성할 수 있습니다.

엔티티의 렌더링되는 형태는 어떤 컴포넌트를 사용하는지에 따라 결정됩니다.

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

## Creator Hub의 Scene Editor를 사용하세요

엔티티에 형태를 부여하는 가장 쉬운 방법은 Scene Editor를 사용하는 것입니다. 다음을 추가할 수 있습니다. **메시 렌더러** 기본 도형을 제공하는 컴포넌트 또는 **GLTF** 파일에서 3D 모델을 참조하는 컴포넌트입니다. 다음을 참조하세요. [컴포넌트 추가](/docs/creator/content-creator-ko/scene-editor/build/components/#add-components).

## 기본 도형

흔히 *프리미티브*라고 불리는 여러 기본 도형은 엔티티에 다음을 부여하여 추가할 수 있습니다. `MeshRenderer` 컴포넌트가

다음 도형을 사용할 수 있습니다. 일부 도형에는 해당 도형에만 해당하는 선택적 추가 필드가 포함됩니다.

* **박스**:

  다음을 사용하세요 `MeshRenderer.setBox()`를 사용하면서 엔티티를 전달합니다. 추가 선택적 필드로 `uvs` 를 전달하면 텍스처 정렬을 매핑할 수 있습니다. 다음을 참조하세요. [머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/) 을 참고하세요.
* **평면**:

  다음을 사용하세요 `MeshRenderer.setPlane()`를 사용하면서 엔티티를 전달합니다. 추가 선택적 필드로 `uvs` 를 전달하면 텍스처 정렬을 매핑할 수 있습니다. 다음을 참조하세요. [머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/) 을 참고하세요.
* **구**:

  다음을 사용하세요 `MeshRenderer.setSphere()`를 사용하면서 엔티티를 전달합니다.
* **원기둥**:

  다음을 사용하세요 `MeshRenderer.setCylinder()`를 사용하면서 엔티티를 전달합니다. 추가 선택적 필드로 `radiusTop` 및 `radiusBottom` 를 추가 선택적 필드로 사용하여 원기둥을 수정할 수 있습니다.

  팁: `radiusTop` 또는 `radiusBottom` 중 하나를 0으로 설정하면 원뿔이 됩니다.

다음 예시는 큐브를 생성합니다.

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)
```

다음 예시는 다음을 가진 원기둥을 생성합니다. `radiusTop` 값을 0으로 설정하면 원뿔이 됩니다.

```ts
const myCone = engine.addEntity()

Transform.create(myCone, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setCylinder(myCone, 1, 0)
```

기본 도형에는 머티리얼이 포함되지 않습니다. 색상이나 텍스처를 지정하려면 [머티리얼 컴포넌트](/docs/creator/content-creator-ko/sdk7/3d/materials/) 를 같은 엔티티에 할당해야 합니다.

기본 도형을 클릭 가능하게 만들거나 플레이어가 이를 통과하지 못하게 하려면 엔티티에 다음을 부여해야 합니다. *콜라이더* 를 통해 [MeshCollider](/docs/creator/content-creator-ko/sdk7/3d/colliders/) 컴포넌트가

이미 다음을 가진 엔티티의 형태를 변경하려면 `MeshRenderer` 컴포넌트가 있으면 `MeshRenderer.setBox()` 또는 다른 헬퍼 함수를 실행하면 원래 형태를 덮어씁니다. 원래 `MeshRenderer` 를 제거하거나 고급 구문을 사용할 필요는 없습니다.

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)

// 형태 덮어쓰기
MeshRenderer.setSphere(myCube)
```


> [!WARNING]
> **📔 참고**: `MeshRenderer` 컴포넌트는 다음을 통해 가져와야 합니다.
> 
> > `import { MeshRenderer } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요. [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 이를 쉽게 처리하는 방법에 대해.


## 3D 모델

더 복잡한 형태의 경우 Blender와 같은 외부 도구에서 3D 모델을 만든 다음 *.glTF* 또는 *.glb* (바이너리 *.glTF*). [glTF](https://www.khronos.org/gltf) (GL Transmission Format)는 3D 자산을 위한 공통적이고 확장 가능한 형식을 제공하는 Khronos의 오픈 프로젝트로, 효율적이며 최신 웹 기술과의 상호 운용성이 매우 높습니다.

외부 모델을 장면에 추가하려면 엔티티에 `GltfContainer` 컴포넌트를 추가하고 그 `src` 를 모델이 들어 있는 glTF 파일 경로로 설정하세요.

```ts
const houseEntity = engine.addEntity()

GltfContainer.create(houseEntity, {
	src: 'models/House.gltf',
})
```

다음 `src` 필드는 필수이므로 컴포넌트를 생성할 때 값을 지정해야 합니다. 위 예시에서 모델은 장면 프로젝트 폴더의 루트 수준에 있는 `models` 폴더에 있습니다.


> [!NOTE]
> **💡 팁**: 모델은 `assets/scene/models` 폴더에 따로 분리해 두는 것을 권장합니다.


glTF 모델에는 자체적으로 포함된 텍스처, 머티리얼, 콜라이더, 애니메이션을 포함할 수 있습니다. 이에 대한 자세한 정보는 다음을 참조하세요. [3D 모델](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) 모델의 머티리얼을 재정의하려면 다음을 사용하세요. [GltfNodeModifiers](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 컴포넌트를 사용하여 모든 플레이어를 순회하세요. 자세한 내용은 [glTF 머티리얼 수정](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 을 참고하세요.

플레이어가 3D 모델을 통과하지 못하게 하거나 모델을 클릭 가능하게 만들려면 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/)가 필요하며, 이는 모델에 포함되어 있거나 다음을 통해 제공될 수 있습니다. `MeshCollider` 컴포넌트가

모든 모델, 셰이더, 텍스처는 [씬 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/).


> [!WARNING]
> **📔 참고**: `GltfContainer` 컴포넌트는 다음을 통해 가져와야 합니다.
> 
> `import { GltfContainer } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요. [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 이를 쉽게 처리하는 방법에 대해.


### 3D 모델 미리 로드하기

경우에 따라 커스텀 3D 모델이 장면에 추가되지만 즉시 사용되지는 않습니다. 예를 들어, 커스텀 의자 모델은 플레이어가 다른 객체와 상호작용할 때만 로드될 수 있습니다. 이 시나리오에서는 첫 상호작용 시 모델 다운로드와 로드가 완료될 때까지 시간이 걸릴 수 있습니다.

이를 방지하려면 `AssetLoad.create` 메서드를 사용하여 자산이 필요하기 전에 다운로드되도록 보장하세요.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

자세한 정보는 다음을 확인하세요. [리소스 미리 로드](/docs/creator/content-creator-ko/sdk7/optimizing/pre-load-resources/) 문서.

### 3D 모델용 무료 라이브러리

직접 3D 모델을 만드는 대신, 여러 무료 또는 유료 라이브러리에서 다운로드할 수도 있습니다.

시작을 돕기 위해, 아래에는 무료 또는 비교적 저렴한 콘텐츠를 제공하는 라이브러리 목록이 있습니다.

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [Builder의 에셋](https://github.com/decentraland/builder-assets/tree/master/assets)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/) (주로 3D 프린팅용으로 만들어진 3D 모델이지만, 가상 세계에 맞게 활용할 수 있음)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://www.cgtrader.com/)


> [!WARNING]
> **📔 참고**: 다운로드한 콘텐츠의 라이선스 제한에 주의하세요.


이들 사이트 중 몇 곳에서는 모델을 다운로드할 형식을 선택할 수 있습니다. 항상 *.glTF* 형식을 선택하세요. 사용할 수 없다면, 장면에서 사용하기 전에 *glTF* 로 변환해야 합니다. 이를 위해 Blender로 가져온 뒤 *.glTF* 으로 내보내는 것을 권장합니다.

### 3D 모델 최적화

장면의 3D 모델이 더 빨리 로드되고 메모리를 덜 차지하도록 하려면 다음 모범 사례를 따르세요.

* 모델을 *.glb* 형식으로 저장하세요. 이는 *.gltf*.
* 의 더 가벼운 버전입니다. 여러 모델이 같은 텍스처를 공유한다면, 텍스처를 별도 파일로 내보내세요. 그러면 여러 모델이 한 번만 로드하면 되는 단일 텍스처 파일을 참조할 수 있습니다.
* 장면에 나타났다 사라지는 엔티티가 있다면, 이 엔티티들을 풀링해서 지하에 두거나 크기를 0으로 유지하는 것이 좋을 수 있습니다. 이렇게 하면 더 빨리 나타나게 할 수 있지만, 대신 사용하지 않을 때도 메모리를 차지하게 됩니다.

## 도형 늘리기

기본 도형과 3D 모델은 기본 치수가 있으며, 엔티티의 `변환` 컴포넌트가

```ts
const primitiveEntity = engine.addEntity()

MeshRenderer.setBox(primitiveEntity)

Transform.create(primitiveEntity, {
	position: { x: 8, y: 1, z: 8 },
	scale: { x: 4, y: 0.5, z: 4 },
})
```

## 보이지 않게 만들기

엔티티에 다음을 부여하면 엔티티를 보이지 않게 만들 수 있습니다. `VisibilityComponent`를 `visible` 속성이 *false*.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(myEntity)

VisibilityComponent.create(myEntity, { visible: false })
```

다음 `VisibilityComponent` 는 기본 도형이 있는 엔티티와 `GLTFContainer` 컴포넌트가 있는 엔티티 모두에 동일하게 적용됩니다.

엔티티가 보이지 않더라도, 충돌 레이어에 따라 해당 콜라이더가 플레이어의 이동 경로를 막거나 뒤에 있는 엔티티를 클릭하지 못하게 할 수 있습니다.

### 가시성 전파

다음을 사용해 `propagateToChildren` 필드를 `VisibilityComponent` 에 설정하면 엔티티의 자식 트리 전체의 모든 자식에 구성을 적용할 수 있습니다. `propagateToChildren` 이 *true*로 설정되면, 이 설정은 아래의 모든 수준에 있는 모든 자식에게 영향을 줍니다. 이렇게 하면 각 자식 엔티티를 일일이 보이거나 보이지 않게 표시하는 번거로운 작업을 크게 줄일 수 있습니다.

```ts
// 부모 엔티티(명시적으로 보이지 않음)
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(parentEntity)
VisibilityComponent.create(parentEntity, { visible: false, propagateToChildren: true })

// 자식 엔티티(부모 때문에 암묵적으로 보이지 않음)
const child = engine.addEntity()
Transform.create(child, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity
})
MeshRenderer.setBox(child)
```


> [!WARNING]
> **📔 참고**: 엔티티가 자체적인 `VisibilityComponent`를 가지고 있다면, 이는 부모의 모든 구성을 재정의합니다.
> 
> 엔티티에 자체적인 `VisibilityComponent`가 없으면, 그 가시성은 가장 가까운 부모의 `VisibilityComponent` 및 `propagateToChildren` 가 *true*.


## 로 설정되어 있는지에 따라 결정됩니다.

3D 모델이 상당히 큰 경우 렌더링되는 데 눈에 띄는 시간이 걸릴 수 있으며, 이 시간은 플레이어의 하드웨어와 기타 여러 요인에 따라 달라질 수 있습니다. 때로는 다른 작업을 수행하기 전에 모델 로드가 완료되었는지 확인해야 합니다. 예를 들어, 플레이어를 하늘 위 플랫폼으로 순간이동시키려면 먼저 플랫폼이 완전히 렌더링되었는지 확인한 뒤 플레이어를 그곳으로 이동해야 하며, 그렇지 않으면 플레이어가 플랫폼을 그대로 통과해 떨어질 수 있습니다.

3D 모델 렌더링이 완료되었는지 확인하려면 엔티티의 `GltfContainerLoadingState` 컴포넌트를 확인하세요. 이 컴포넌트는 읽기 전용이며, 다음도 함께 가진 모든 엔티티에 존재합니다. `GltfContainer`컴포넌트가

이 컴포넌트에는 다음이라는 이름의 단일 속성이 있습니다. `currentState`로, `LoadingState` 열거형의 값을 보관합니다.

다음 예시는 시스템을 사용하여 엔티티의 3D 모델 로딩 상태를 주기적으로 확인합니다. 상태가 `LoadingState.FINISHED`이면, 그곳에서 사용자 정의 로직을 수행하고 시스템 실행을 종료할 수 있습니다.

```ts
export function main() {
	const meshEntity = engine.addEntity()
	GltfContainer.create(meshEntity, { src: 'models/Monster.glb' })
	engine.addSystem((deltaTime) => {
		const loadingState = GltfContainerLoadingState.getOrNull(meshEntity)
		if (!loadingState) return
		switch (loadingState.currentState) {
			case LoadingState.LOADING:
				console.log('mesh is LOADING')
				break
			case LoadingState.FINISHED:
				console.log('mesh is FINISHED')
				// 사용자 정의 로직 수행
				break
			case LoadingState.FINISHED_WITH_ERROR:
				console.log('mesh is FINISHED BUT MAY HAVE PROBLEMS')
				break
			case LoadingState.UNKNOWN:
				console.log('mesh is in an UNKNOWN STATE')
				break
		}
	})
}
```

## 고급 구문

다음을 생성하기 위한 전체 구문은 `MeshRenderer` 헬퍼 없이 단순화하면 다음과 같습니다.

```ts
MeshRenderer.create(myBox, {
	mesh: {
		$case: 'box',
		box: { uvs: [] },
	},
})

MeshRenderer.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: { uvs: [] },
	},
})

MeshRenderer.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshRenderer.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

이것이 베이스 프로토콜이 MeshRenderer 컴포넌트를 해석하는 방식입니다. 헬퍼 함수는 이를 추상화하여 더 편리한 구문을 제공하지만, 내부적으로는 이 구문을 출력합니다.

다음 `$case` 필드를 사용하면 허용된 유형 중 하나를 지정할 수 있습니다. 각 유형은 서로 다른 매개변수 집합을 지원합니다. 위 예시에서 `박스` 유형은 `uvs` 필드에 저장됩니다.

에 대해 지원되는 값은 다음과 같습니다. `$case` 입니다.

* `박스`
* `평면`
* `구`
* `원기둥`

의 값에 따라 `$case`해당 도형에 맞는 객체를 정의하고 관련 속성을 전달하는 것이 유효합니다.

다음을 추가하려면 `MeshRenderer` 컴포넌트가 이미 이 컴포넌트의 인스턴스를 가지고 있을 수도 있는 엔티티에 `MeshRenderer.createOrReplace()`을 사용하세요. `MeshRenderer.setBox()` 와 같은 헬퍼 함수는 기존 컴포넌트 인스턴스를 덮어쓰는 처리를 해주지만, `MeshRenderer.create()` 을 이미 이 컴포넌트를 가진 엔티티에서 실행하면 오류가 반환됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/shape-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
