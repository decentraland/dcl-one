# 콜라이더

엔티티에 3D 형태와 충돌을 부여하는 다양한 컴포넌트에 대해 알아봅니다.

콜라이더가 있는 엔티티는 공간을 차지하고 플레이어의 경로를 막지만, 콜라이더가 없는 엔티티는 플레이어의 아바타가 통과할 수 있습니다.

콜라이더는 엔티티를 클릭 가능하게 만드는 데도 필요합니다. 버튼 이벤트는 엔티티의 보이는 형태가 아니라 콜라이더 형태를 기준으로 합니다.

플레이어의 물리와 상호작용하는 것과 포인터 이벤트와 상호작용하는 것에는 별도의 충돌 레이어가 있으며, 콜라이더는 둘 중 하나와만 상호작용하도록 구성할 수 있습니다. 또한 사용자 지정 레이어와 상호작용하도록 구성할 수도 있으며, 이는 [레이캐스트](/docs/creator/content-creator-ko/sdk7/3d/colliders/) 를 사용해 씬에 맞는 방식으로 처리할 수 있습니다.


> [!WARNING]
> **📔 참고**: 콜라이더는 다른 엔티티들이 서로 상호작용하는 방식에는 영향을 주지 않으며, 엔티티는 항상 서로 겹칠 수 있습니다. 충돌 설정은 엔티티가 플레이어의 아바타 및 버튼 이벤트와 상호작용하는 방식에만 영향을 줍니다. Decentraland에는 기본 물리 엔진이 없으므로, 엔티티가 떨어지거나, 부딪히거나, 튕기게 하려면 이런 동작을 씬에 직접 코딩하거나, 이를 처리할 라이브러리를 가져와야 합니다.


## 씬 편집기 사용

엔티티의 콜라이더를 관리하는 가장 쉬운 방법은 [씬 편집기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/).

다음을 추가할 수 있습니다. **Mesh Collider** 컴포넌트를 엔티티에 추가하여 기본 도형(큐브, 평면, 구, 원통 또는 원뿔)을 엔티티에 할당할 수 있습니다. 그런 다음 [충돌 레이어](#collision-layers) 를 드롭다운에서 선택할 수 있습니다.

또한 다음의 충돌 레이어를 구성할 수도 있습니다. **GLTF** 컴포넌트에서 기본값을 변경하려면 [충돌 레이어](#collision-layers) 를 사용합니다. 이는 콜라이더 지오메트리 또는 모델의 보이는 지오메트리에 적용됩니다. 참조 [컴포넌트 추가](/docs/creator/content-creator-ko/scene-editor/build/components/#add-components).

![](/docs/_remote/d21ae0005c271ae9c97a8144.webp)

## 기본 도형의 콜라이더

해당 `MeshCollider` 컴포넌트는 기본 도형(박스, 구, 평면, 원통 또는 원뿔)을 기반으로 한 단순한 콜라이더를 엔티티에 부여합니다.

다음을 가진 엔티티는 `MeshRenderer` 컴포넌트를 사용해 [기본 도형](/docs/creator/content-creator-ko/sdk7/3d/shape-components/#primitive-shapes) 을 부여해도 기본적으로는 콜라이더가 없습니다. 또한 엔티티에 `MeshCollider` 컴포넌트도 부여해야 합니다.

다음 콜라이더 형태를 사용할 수 있습니다. `MeshCollider`. 일부 형태에는 해당 형태에만 적용되는 선택적 추가 필드가 포함됩니다.

* **박스**:

  사용 `MeshCollider.setBox()`, 엔티티를 전달합니다.
* **평면**:

  사용 `MeshCollider.setPlane()`, 엔티티를 전달합니다.
* **구**:

  사용 `MeshCollider.setSphere()`, 엔티티를 전달합니다.
* **원통**:

  사용 `MeshCollider.setCylinder()`, 엔티티를 전달합니다. 추가 선택적 필드로 `radiusTop` 및 `radiusBottom` 를 전달해 원통을 수정할 수 있습니다.


> [!NOTE]
> **💡 팁**: 둘 중 하나를 `radiusTop` 또는 `radiusBottom` 0으로 설정하면 원뿔이 됩니다.


이 예시는 통과할 수 없는 박스 엔티티를 정의합니다.

```ts
// 엔티티 생성
const myCollider = engine.addEntity()

// 보이는 형태
MeshRenderer.setBox(myCollider)

// 콜라이더
MeshCollider.setBox(myCollider)
```

사용되는 형태는 `MeshCollider` 의 형태와 꼭 일치할 필요는 없습니다. `MeshRenderer`. 또한 3D 모델이 있는 엔티티에 `MeshCollider` GltfContainer `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도` 을 추가할 수 있습니다.


> [!WARNING]
> **📔 참고**: `MeshCollider` 컴포넌트와 `ColliderLayer` 는 다음을 통해 가져와야 합니다.
> 
> `import { MeshCollider, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


## 3D 모델의 콜라이더

3D 모델에는 두 가지 서로 다른 지오메트리 수준에서 콜라이더를 할당할 수 있습니다.

* `visibleMeshesCollisionMask`: 모델의 보이는 지오메트리를 가리킵니다. 기본적으로 이 지오메트리에는 콜라이더가 없습니다.
* `invisibleMeshesCollisionMask`: 이름이 `_collider`로 끝나는 콜라이더 메시를 가리킵니다. 기본적으로 이 지오메트리는 물리와 포인터 이벤트 모두의 콜라이더로 처리됩니다.

3D 모델의 일부로 포함된 모든 메시 중 이름이 `_collider` 로 끝나는 것은 `invisibleMeshesCollisionMask` 레이어의 일부로 처리되며, 기본적으로 콜라이더로 해석됩니다.

콜라이더 지오메트리를 별도의 보이지 않는 레이어로 정의하면 더 큰 제어가 가능하고, 일반적으로 원본 모델보다 훨씬 단순한(버텍스 수가 더 적은) 충돌 객체를 사용하므로 보이는 지오메트리를 사용하는 것보다 시스템 부담도 훨씬 적습니다.

모델에 콜라이더 지오메트리가 없고, 이를 물리 또는 포인터 이벤트 시스템에 영향을 주게 하고 싶다면, 다음 중 하나를 할 수 있습니다:

* 를 통해 보이는 지오메트리에 충돌 레이어를 직접 할당합니다. `visibleMeshesCollisionMask`.


> [!WARNING]
> **📔 참고**: 객체의 보이는 지오메트리에 버텍스가 많다면, 성능 비용이 더 클 수 있다는 점에 유의하세요.


* 엔티티에 `MeshCollider` 컴포넌트를 부여해 기본 도형 콜라이더를 만듭니다.
* 다음이 있는 보이지 않는 엔티티를 겹쳐 놓습니다. `MeshCollider` 컴포넌트도 부여해야 합니다.
* Blender 같은 외부 도구에서 모델을 편집해 *콜라이더 메시*를 포함시킵니다. 콜라이더는 *x\_collider*로 이름 지어야 하며, 여기서 *x* 는 모델의 이름입니다. 따라서 *house*라는 이름의 모델이라면 콜라이더 이름은 *house\_collider*.

로 지정해야 합니다. `visibleMeshesCollisionMask` 호버 힌트와 포인터 이벤트가 엔티티의 윤곽에 더 정확하게 반응하도록 하려면 포인터 이벤트 충돌 레이어를


> [!WARNING]
> **📔 참고**: 같은 레이어(물리, 포인터 이벤트 또는 사용자 지정 레이어)가 `visibleMeshesCollisionMask` 및 `invisibleMeshesCollisionMask`둘 다에 할당되지 않도록 하세요. 그렇게 하면 리소스를 매우 비효율적으로 사용하게 됩니다. 보이지 않는 레이어에는 물리를, 보이는 레이어에는 포인터 이벤트를 할당하는 식으로 각기 다른 레이어를 사용할 수 있습니다.


```ts
// 엔티티 생성
const myEntity = engine.addEntity()

// GLTF 형태 할당
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})
```

참조 [3D 모델](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) 에 콜라이더 보이지 않는 지오메트리를 추가하는 방법에 대한 자세한 내용.


> [!WARNING]
> **📔 참고**: `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도` 컴포넌트와 `ColliderLayer` 는 다음을 통해 가져와야 합니다.
> 
> > `import { GltfContainer, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


### 애니메이션 모델

팔레추 기반 애니메이션 [이 포함된 모델에서 보이는 지오메트리를 콜라이더로 사용하도록 설정할 때](/docs/creator/content-creator-ko/3d/animations/), 애니메이션은 콜라이더를 따라가지 않습니다. 콜라이더 메시들은 원래 형태를 유지합니다. 애니메이션이 메시의 지오메트리를 변형하는 경우, 애니메이션이 재생되는 동안 콜라이더 메시들은 애니메이션이 적용되지 않은 형태를 유지합니다.

전체 메시를 형태 변화 없이 이동시키는 애니메이션을 재생할 때는, 이런 변화가 콜라이더에 정확히 반영됩니다. 예를 들어 플랫폼이 애니메이션의 일부로 움직인다면, 플랫폼의 콜라이더도 애니메이션과 함께 움직입니다.

## 충돌 레이어

씬은 서로 다른 동작을 가진 별도의 충돌 레이어를 처리할 수 있습니다.

다음 중 하나를 구성할 수 있습니다. `MeshCollider` 컴포넌트 또는 `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도` 컴포넌트가 한 가지 상호작용에만, 여러 상호작용에만, 또는 아무 것도 반응하지 않도록 설정할 수 있습니다. 이를 위해 `MeshCollider` 에 `collisionMask` 속성을 설정하고, `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도` 에 `visibleMeshesCollisionMask` 또는 `invisibleMeshesCollisionMask` 속성에는 다음 값 중 하나 또는 여러 개를 지정합니다.

* `ColliderLayer.CL_PHYSICS`: 플레이어의 이동을 막습니다(씬의 벽, 바닥, 플랫폼). 포인터 이벤트에는 영향을 주지 않습니다.
* `ColliderLayer.CL_POINTER`: 포인터 이벤트에만 반응합니다. 플레이어의 이동은 막지 않습니다.
* `ColliderLayer.CL_PLAYER`: 콜라이더를 아바타 마커로 태그합니다. 이를 대상으로 하는 레이캐스트와 트리거 영역은 `CL_PLAYER` 를 감지하지만, 플레이어 캡슐은 이를 그대로 통과합니다(물리적 차단 없음). 씬 메시에서는 감지 전용으로 메시를 "아바타와 유사한 대상"으로 표시할 때 유용합니다.
* `ColliderLayer.CL_MAIN_PLAYER`: `CL_PLAYER`와 비슷하지만, 로컬 플레이어만을 대상으로 합니다. `CL_MAIN_PLAYER` 가 마스크에 있는 레이캐스트와 트리거 영역은 이를 감지하지만, 원격 아바타 레이캐스트/트리거는 감지하지 않습니다.
* `ColliderLayer.CL_CUSTOM1` 에서 `CL_CUSTOM8`까지: 레이캐스트와 트리거 영역과 함께 사용하여 특정 사용자 지정 레이어와의 충돌만 감지할 수 있습니다.
* `ColliderLayer.CL_NONE`: 어떤 종류의 충돌에도 반응하지 않습니다.


> [!NOTE]
> **💡 팁**: `CL_PLAYER` 및 `CL_MAIN_PLAYER` 의 `MeshCollider` / `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도` 은 **감지 전용** 레이어입니다. 플레이어 캡슐은 이를 통과합니다. 메시를 아바타로도 감지 가능하면서 동시에 플레이어를 물리적으로 막게 하려면, 아바타 레이어와 `CL_PHYSICS` 를 결합하세요(예: `CL_PHYSICS | CL_MAIN_PLAYER`).



> [!WARNING]
> **📔 참고**: a의 콜라이더를 비활성화하려면 `MeshCollider` 컴포넌트를 삭제하세요. 충돌 레이어를 `ColliderLayer.CL_NONE`로 설정하지 마세요. `MeshCollider` 컴포넌트에는 알려진 문제가 있습니다. 모든 충돌을 비활성화하는 대신, 이 값은 기본값(`ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`).


```ts
// 엔티티 생성
const myEntity = engine.addEntity()
// 보이는 형태
MeshRenderer.setBox(myEntity)

// 플레이어 물리에만 반응하는 MeshCollider 컴포넌트 생성
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)
```

하나의 충돌 마스크는 여러 충돌 레이어에 반응할 수 있습니다. `|` 문자열을 *또는*로 사용해 필요한 만큼의 레이어를 포함하세요. MeshCollider의 기본값은 `ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`.

```ts
MeshCollider.setBox(
	myEntity,
	ColliderLayer.CL_CUSTOM1 |
		ColliderLayer.CL_CUSTOM3 |
		ColliderLayer.CL_PHYSICS |
		ColliderLayer.CL_POINTER
)
```

8개의 서로 다른 사용자 지정 레이어는 씬에 가장 잘 맞는 용도로 사용할 수 있습니다. 예를 들어 하나는 NPC 시야 계산에, 또 다른 하나는 떨어지는 물체의 궤적 추정에 사용할 수 있습니다. 시스템마다 다른 레이어를 사용하면 각 경우에 관련 엔티티와의 충돌만 확인하면 되므로 더 적은 리소스를 사용할 수 있습니다.

참조 [레이캐스팅](/docs/creator/content-creator-ko/sdk7/interactivity/raycasting/) 을 참고해 사용자 지정 충돌 레이어를 사용하는 방법을 더 알아보세요.

### 카메라와 콜라이더

플레이어의 카메라가 3인칭 모드에서 움직일 때, 엔티티에 할당된 충돌 레이어에 따라 카메라가 콜라이더에 막힐 수도 있고 그렇지 않을 수도 있습니다. 씬을 설계할 때 이 점에 유의하세요. 카메라가 벽이나 다른 엔티티를 통과하지 않게 하고 싶을 수 있습니다.

카메라가 벽을 통과하지 않게 하려면, 카메라를 막고 싶은 엔티티에 `ColliderLayer.CL_PHYSICS` 과 `ColliderLayer.CL_POINTER` 레이어를 모두 할당해야 합니다. 두 레이어가 엔티티의 동일한 지오메트리에 할당되어야 한다는 점이 중요합니다. 따라서 `ColliderLayer.CL_PHYSICS` 레이어를 엔티티의 보이는 레이어에 할당했다면, `ColliderLayer.CL_POINTER` 레이어도 같은 지오메트리에 할당해야 합니다.

예를 들어 Creator Hub에서는 다음 설정 조합이 카메라가 벽을 통과하지 못하게 합니다.

![](/docs/_remote/1b4a254a3e56b4b32b988fb4.webp)

둘 다 `ColliderLayer.CL_PHYSICS` 과 `ColliderLayer.CL_POINTER` 레이어가 엔티티 지오메트리의 같은 보이지 않는 레이어에 할당됩니다. 둘 다 보이는 레이어에 할당되더라도 결과는 같습니다. 이는 Creator Hub를 통해 엔티티를 추가하든 코드로 추가하든 기본 동작입니다.

![](/docs/_remote/6d4572b10a1972901a7ea232.webp)

이 두 번째 예시에서는 `ColliderLayer.CL_PHYSICS` 레이어가 엔티티의 보이지 않는 레이어에 할당되고, `ColliderLayer.CL_POINTER` 레이어가 엔티티의 보이는 레이어에 할당되므로 카메라는 벽을 통과할 수 있습니다. 두 지오메트리의 전체적인 형태가 같더라도 마찬가지입니다.

```ts
// 카메라가 벽을 통과하지 않음
// 기본값(포인터와 물리 모두 보이지 않는 지오메트리를 사용)
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// 카메라가 벽을 통과하지 않음
// 둘 다 같은 보이지 않는 지오메트리를 사용
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// 카메라가 벽을 통과하지 않음
// 둘 다 같은 보이는 지오메트리를 사용
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	visibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// 네, 카메라가 벽을 통과함
// 물리와 포인터가 서로 다른 레이어에 있음
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// 네, 카메라가 벽을 통과함
// 물리와 포인터가 서로 다른 레이어에 있음
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
	visibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
})
```

### 포인터 차단

콜라이더가 있는 형태만 [포인터 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/)로 활성화할 수 있습니다. 엔티티가 포인터 이벤트가 그것을 통과하지 못하게 하고 뒤에 있는 엔티티를 맞히는 것을 막으려면 콜라이더도 필요합니다. 예를 들어, 플레이어는 상자 주위에 콜라이더가 있다면 상자 안에 잠겨 있는 물건을 집을 수 없습니다. 플레이어의 포인터 이벤트는 `ColliderLayer.CL_POINTER` 레이어에서 활성화된 메시의 영향을 받습니다.

기본적으로 MeshCollider는 물리와 포인터 레이어 모두에 영향을 주지만, 이 값을 바꿔 하나만, 또는 둘 다 아니게, 그리고 대신 사용자 지정 레이어에 영향을 주도록 할 수 있습니다.


> [!WARNING]
> **📔 참고**콜라이더 외에도 엔티티가 포인터 이벤트에 반응하려면 `PointerEvents` 컴포넌트도 필요합니다. `pointerEventsSystem` 헬퍼도 이 요구 사항을 처리해 줍니다.


```ts
// 플레이어 물리에만 반응
// 예: 통과할 수 없지만 클릭은 가능한 보이지 않는 벽
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)

// 플레이어의 포인터에만 반응
// 예: 클릭해서 집을 수 있지만 그대로 통과할 수 있는 아이템
MeshCollider.setBox(myEntity2, ColliderLayer.CL_POINTER)
```

기본적으로 `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도` 의 보이는 지오메트리는 어떤 충돌 레이어에도 매핑되지 않지만, 보이지 않는 지오메트리는 물리와 포인터 레이어 모두에 영향을 줍니다. 이 값을 바꿔 하나만, 또는 둘 다 아니게, 그리고 대신 사용자 지정 레이어에 영향을 주도록 할 수 있습니다. 같은 방식으로 보이는 지오메트리 레이어도 구성할 수 있습니다.

```ts
// 기본값(포인터와 물리 모두 보이지 않는 지오메트리를 사용)
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// 플레이어 물리는 더 단순한 보이지 않는 지오메트리를 사용
// 포인터 이벤트는 보이는 지오메트리의 전체 상세 윤곽을 사용
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// 플레이어 물리와 포인터 이벤트 모두 보이는 지오메트리의 전체 상세 윤곽을 사용
// 더 단순한 보이지 않는 지오메트리를 ColliderLayer.CL_NONE에 매핑해 둘 다 계산하지 않도록 함
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
	visibleMeshesCollisionMask:
		ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS,
})

// 보이는 지오메트리든 보이지 않는 지오메트리든 어떤 종류의 충돌에도 반응하지 않음:
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
})
```

## 고급 MeshCollider 구문

도우미 없이 단순화하지 않고 `MeshCollider` 컴포넌트를 생성하는 전체 구문은 다음과 같습니다:

```ts
MeshCollider.create(myBox, {
	mesh: {
		$case: 'box',
		box: {},
	},
})

MeshCollider.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: {},
	},
})

MeshCollider.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshCollider.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

이것이 기본 프로토콜이 MeshCollider 컴포넌트를 해석하는 방식입니다. 헬퍼 함수는 이를 추상화하여 더 친숙한 구문을 제공하지만, 내부적으로는 이 구문을 출력합니다.

해당 `$case` 필드를 사용하면 허용되는 유형 중 하나를 지정할 수 있습니다. 각 유형은 서로 다른 매개변수 집합을 지원합니다.

다음 값이 지원됩니다: `$case` 는 다음과 같습니다.

* `박스`
* `평면`
* `구`
* `원통`

다음 `$case`값에 따라, 해당 형태에 맞는 객체를 관련 속성을 전달해 정의하는 것이 유효합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
