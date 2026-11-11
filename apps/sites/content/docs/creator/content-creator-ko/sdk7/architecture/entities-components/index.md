> Decentraland 씬의 엔티티와 컴포넌트에 대한 필수 사항을 알아봅니다

# 엔티티 및 컴포넌트

디센트럴랜드 씬은 다음을 중심으로 구성됩니다 [*엔티티*, *구성 요소* 및 *시스템*](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system). 이는 여러 게임 엔진의 아키텍처에서 사용되는 일반적인 패턴으로, 쉽게 조합하고 확장할 수 있게 해줍니다.

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

## 개요

*엔티티* 은 디센트럴랜드 씬의 모든 것을 구축하는 기본 단위입니다. 씬에 있는 모든 보이는 3D 오브젝트와 보이지 않는 3D 오브젝트, 그리고 오디오 플레이어는 각각 하나의 엔티티가 됩니다. 엔티티는 컴포넌트가 참조할 수 있는 ID일 뿐입니다. 엔티티 자체에는 고유한 속성이나 메서드가 없으며, 단지 여러 컴포넌트를 함께 묶는 역할만 합니다.

*컴포넌트* 은 엔티티의 특성을 정의합니다. 예를 들어, `Transform` 컴포넌트는 엔티티의 좌표, 회전, 크기를 저장합니다.  `MeshRenderer` 컴포넌트는 씬에서 렌더링될 때 엔티티에 보이는 형태(예: 큐브나 구)를 부여하고,  `머티리얼` 컴포넌트는 엔티티에 색상이나 텍스처를 부여합니다. 또한 씬에 필요한 데이터를 담기 위해 사용자 정의 컴포넌트를 만들 수도 있습니다. 예를 들어 사용자 정의  `체력` 은 엔티티의 남은 체력 값을 저장하고, 게임에서 플레이어가 아닌 적을 나타내는 엔티티에 추가할 수 있습니다.

웹 개발에 익숙하다면, 엔티티를 다음에 해당하는 것으로 생각하면 됩니다. *Elements* 을(를) *DOM* 트리, 그리고 컴포넌트를 그 요소들의  *속성* 으로 생각하면 됩니다.

다음에서 [Creator Hub의 씬 에디터에 추가한 뒤](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/)엔티티를 선택하면 해당 엔티티에 속한 컴포넌트를 볼 수 있습니다.

![](/docs/_remote/f49a93029e97b3d8ef464fb3.webp)


> [!WARNING]
> **📔 참고**: 이전 버전의 SDK에서는 엔티티가  *객체* 로서 인스턴스화되었고, 함수 추가를 위해 확장할 수 있었습니다. SDK 7.0부터는 엔티티가 ID일 뿐입니다. 이 구조는 다음의 원칙에 더 잘 맞습니다. [데이터 지향 프로그래밍](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/) 그리고 씬 성능 향상에 도움이 됩니다.


![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

다음과 같은 컴포넌트는 `Transform`, `머티리얼` 또는 다음의 어느 것과도 *형태* 컴포넌트는 씬의 렌더링과 밀접하게 연결되어 있습니다. 이 컴포넌트들의 값이 변경되면, 엔진은 다음 프레임에서 씬이 렌더링되는 방식을 바꾸기에 충분합니다.

엔진은 씬의 중앙에서 다른 모든 부분을 관리하는 부분입니다. 어떤 엔티티가 렌더링되는지, 플레이어가 그것들과 어떻게 상호작용하는지를 결정합니다. 또한 다음에서의 어떤 함수가 [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 실행되는지와 그 시점도 조정합니다.

컴포넌트는 참조하는 엔티티에 대한 데이터를 저장하도록 되어 있습니다. 이 데이터만 저장할 수 있으며, 스스로 이 데이터를 수정할 수는 없습니다. 컴포넌트 값에 대한 모든 변경은 다음에 의해 수행됩니다. [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/). 시스템은 컴포넌트와 엔티티 자체와 완전히 분리되어 있습니다. 엔티티와 컴포넌트는 어떤  *시스템* 가 자신에게 작용하는지에 대해 알지 못합니다.

## 엔티티와 컴포넌트의 문법

아래 예시는 기본 엔티티와 컴포넌트를 선언하고 설정하는 몇 가지 기본 작업을 보여줍니다.

```ts
export function main() {
	// 엔티티 생성
	const door = engine.addEntity()

	// 트랜스폼 컴포넌트를 통해 엔티티에 위치 부여
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// GltfContainer 컴포넌트를 통해 엔티티에 보이는 형태 부여
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```


> [!WARNING]
> **📔 참고**: 이전 버전의 SDK에서는 렌더링을 시작하려면 엔티티를 엔진에 수동으로 추가해야 했습니다. SDK 7부터는 컴포넌트가 할당되는 즉시 엔티티가 암묵적으로 엔진에 추가됩니다.


컴포넌트가 생성되면 항상 부모 엔티티에 할당됩니다. 그런 다음 컴포넌트의 값이 엔티티에 영향을 줍니다.


> [!NOTE]
> **💡 팁**: 엔티티를 하나씩 생성하는 대신,  [컴포지트](/docs/creator/content-creator-ko/sdk7/architecture/composites/) 파일을 추가할 수 있습니다.


## 엔티티 제거

엔진에서 엔티티를 제거하려면  `engine.removeEntity()`를 사용합니다. 이 함수는  `불리언`: `true` 엔티티가 제거되면 `false` 제거가 거부되면

```ts
export function main() {
	// 엔티티 생성
	const door = engine.addEntity()

	// GltfContainer 컴포넌트를 통해 엔티티에 보이는 형태 부여
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})

	// 엔티티 제거
	const removed = engine.removeEntity(door)
	console.log('Entity removed:', removed) // true
}
```

제거된 엔티티에 자식 엔티티가 있으면, 이들은 부모를 기본  `engine.RootEntity` 엔티티로 다시 변경하며, 이 엔티티는 씬의 기본 위치에 있고 크기는  *1*.

### 렌더러가 예약한 엔티티

일부 엔티티 ID는 원격 플레이어 아바타를 위해 렌더러에 의해 예약되어 있습니다. 이러한 엔티티는 제거할 수 없습니다. 만약  `engine.removeEntity()` 를 렌더러 예약 엔티티에 호출하면  `false` 를 반환하고 모든 컴포넌트를 그대로 둡니다.

해당 [이름이 지정된 예약 엔티티](#reserved-entities) (`engine.RootEntity`, `engine.PlayerEntity`, `engine.CameraEntity`)는 특별한 경우입니다: `engine.removeEntity()` 도 여전히  `false` 를 반환하지만(해당 ID는 절대 해제되지 않음), 컴포넌트는  **은** 정리됩니다. 즉, 예를 들어  `InputModifier` 다음에서 `engine.PlayerEntity`를 제거하는 등, 엔티티 ID 자체가 절대 해제되지 않더라도 자신이 추가한 컴포넌트는 지울 수 있습니다.

```ts
// 예약 엔티티를 제거하려고 시도
const result = engine.removeEntity(engine.PlayerEntity)
console.log(result) // false — 엔티티 ID가 해제되지 않음
// 하지만 PlayerEntity에 있는 씬 자체의 컴포넌트는 정리됨
```

예외 상황을 처리하려면 엔티티를 제거할 때 반환값을 확인할 수 있습니다:

```ts
if (!engine.removeEntity(someEntity)) {
	console.log('Entity could not be removed (reserved)')
}
```

### 자식이 있는 엔티티 제거

엔티티와 그 모든 자식(그리고 자식의 자식들까지 재귀적으로)도 함께 제거하려면  `removeEntityWithChildren()` 헬퍼를 사용합니다.

```ts
export function main() {
	// 부모 엔티티 생성
	const door = engine.addEntity()

	// 자식 엔티티 생성
	const doorKnob = engine.addEntity()

	// 엔티티들에 보이는 형태 부여
	GltfContainer.create(door, {
		src: 'models/door.glb',
	})
	GltfContainer.create(doorKnob, {
		src: 'models/doorKnob.glb',
	})

	// 부모
	Transform.create(doorKnob, {
		parent: door,
	})

	// 부모와 자식 모두 제거
	removeEntityWithChildren(engine, door)
}
```


> [!WARNING]
> **참고:** 렌더러 예약 엔티티가 트리의 어느 위치에 있든, `removeEntityWithChildren` 는 다른 모든 하위 항목은 제거하지만 예약 엔티티는 그대로 둡니다. 예약 엔티티의  `Transform.parent` 는 제거된 엔티티를 가리키게 됩니다. 이는 씬이 예약 엔티티를 씬 엔티티 아래에 부모로 두었을 때만 발생하며, 흔한 경우는 아닙니다.



> [!NOTE]
> **💡 팁**: 엔진에서 엔티티를 제거하는 대신, 나중에 지연 없이 다시 불러오고 싶다면 어떤 경우에는 엔티티를 보이지 않게 만드는 편이 더 좋을 수 있습니다. 다음을 참조하세요. [보이지 않게 만들기](/docs/creator/content-creator-ko/sdk7/3d/shape-components/#make-invisible)


### 씬 뒤에서 엔티티 제거하기

엔티티는 컴포넌트가 참조하는 ID일 뿐입니다. 따라서 엔티티를 제거한다는 것은 실제로는 이 엔티티를 참조하는 각 컴포넌트를 제거하는 것입니다. 엔티티의 모든 컴포넌트를 수동으로 제거하면, 플레이어 입장에서는  `engine.removeEntity()`를 수행하는 것과 동일하게 보입니다. 그러나 `engine.removeEntity()` 는 추가적인 내부 정리도 수행하여 엔티티 ID가 더 이상 사용되지 않도록 표시하므로, 항상 엔티티를 제거하는 데 권장되는 방법입니다.

## 중첩된 엔티티

엔티티는 다른 엔티티를 자식으로 가질 수 있습니다. 덕분에 웹페이지의 HTML처럼 엔티티를 트리 구조로 배치할 수 있습니다.

![](/docs/_remote/28a71287061591ece4e52409.webp)

엔티티를 다른 엔티티의 부모로 설정하려면 자식 엔티티에  `Transform` 컴포넌트가 있어야 합니다. 그런 다음  `parent` 필드에 부모 엔티티에 대한 참조를 설정할 수 있습니다.

```ts
export function main() {
	// 엔터티 생성
	const parentEntity = engine.addEntity()

	const childEntity = engine.addEntity()

	// 부모 설정
	Transform.create(childEntity, {
		parent: parentEntity,
	})
}
```

부모가 할당되면 자식 엔티티의  `parent` 필드에서 읽을 수 있습니다. `Transform` 컴포넌트도 부여해야 합니다.

```ts
// 엔티티에서 부모 가져오기
const parent = Transform.get(childEntity).parent
```

부모 엔티티에 위치, 크기 또는 회전에 영향을 주는  `Transform` 컴포넌트가 있으면, 그 자식 엔티티들도 영향을 받습니다. 위치 또는 회전 값은 더해지고, 크기 값은 곱해집니다.

부모나 자식 엔티티 중 어느 쪽이든  `Transform` 컴포넌트가 없으면 다음의 기본값이 사용됩니다.

* 의 경우 **위치**, 부모의 중심은  *0, 0, 0*
* 의 경우 **회전** 부모의 회전은 쿼터니언  *0, 0, 0, 1* (오일러 각도  *0, 0, 0*)
* 의 경우 **크기**과 동일), 부모는 크기가  *1*인 것으로 간주됩니다. 부모의 크기 조정은 크기와 위치에 비례하여 영향을 줍니다.

형태 컴포넌트가 없는 엔티티는 씬에서 보이지 않습니다. 이들은 여러 엔티티를 하나의 그룹으로 다루고 배치하기 위한 래퍼로 사용할 수 있습니다.

자식 엔티티를 부모로부터 분리하려면 엔티티의 부모를  `engine.RootEntity`.

```ts
const mutableChildTransform = Transform.getMutable(childEntity)
mutableChildTransform.parent = engine.RootEntity
```


> [!WARNING]
> **📔 참고**: 다른 플레이어와 동기화되는 중첩 엔티티를 다룰 때는  `parentEntity()` 함수를 사용하는 대신 `parent` 엔티티를 Transform에서 사용하세요. 다음을 참조하세요. [부모가 설정된 엔티티](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/#parented-entities)


씬 에디터에서는 왼쪽 패널에서 씬의 중첩 엔티티 전체 계층 구조를 볼 수 있습니다.

![](/docs/_remote/87863329d94332c21873d9dd.webp)

## ID로 엔티티 가져오기

씬의 모든 엔티티에는 고유한 번호  *id*가 있습니다. 이 ID를 기반으로 엔진에서 특정 엔티티를 참조하는 컴포넌트를 가져올 수 있습니다.

```typescript
// Transform 컴포넌트 가져오기
Transform.get(1000 as Entity)
```


> [!WARNING]
> **📔 참고**:  *0* 및 *511* 사이의 엔티티 ID는 플레이어 아바타, 기본 씬 등 고정된 엔티티를 위해 엔진이 예약합니다.


예를 들어, 플레이어의 클릭이나  [레이캐스트](/docs/creator/content-creator-ko/sdk7/interactivity/raycasting/) 이 엔티티를 맞히면, 이는 맞힌 엔티티의 ID를 반환하며, 위 명령을 사용해 그 ID와 일치하는 엔티티의 Transform 컴포넌트를 가져올 수 있습니다. 같은 방식으로 해당 엔티티의 다른 컴포넌트도 가져올 수 있습니다.

## 이름으로 엔티티 가져오기

씬 에디터에서 드래그 앤 드롭으로 엔티티를 추가하면 각 엔티티에는 고유한 이름이 있습니다. 코드에서 이들 엔티티 중 하나를 참조하려면  `engine.getEntityOrNullByName()` 함수를 사용하세요. 씬 에디터 UI의 왼쪽 트리 뷰에 표시된 그대로, 엔티티의 이름을 문자열로 전달합니다.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```


> [!WARNING]
> **📔 참고**: 반드시 다음만 사용하세요 `engine.getEntityOrNullByName()` 다음 내부에서만 `main()` 함수, 그리고 다음 이후에 실행되는 함수에서 `main()`, 또는 시스템에서 사용할 수 있습니다. 이러한 컨텍스트 밖에서 사용하면 씬 에디터 UI에서 생성된 엔티티가 아직 인스턴스화되지 않았을 수 있습니다.


이 메서드로 가져온 엔티티에는 컴포넌트 추가/제거, 기존 컴포넌트 값 수정, 엔진에서 엔티티 제거 등 어떤 작업이든 자유롭게 수행할 수 있습니다.

```ts
function main() {
	// 엔티티 가져오기
	const door = engine.getEntityOrNullByName('door-3')
	// 엔티티가 존재하는지 확인
	if (door) {
		// 포인터 이벤트 콜백 추가
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: '열기' },
			},
			function () {
				// 문 열기
			}
		)
	}
}
```

씬 에디터 UI를 통해 추가된 모든 엔티티에는  `이름` 컴포넌트가 있으며, 다음과 같이 모두 반복할 수 있습니다:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## 컴포넌트 추가 또는 교체

각 엔티티에는 같은 종류의 컴포넌트가 하나만 있을 수 있습니다. 예를 들어, 이미 Transform이 있는 엔티티에 또 다른 Transform을 할당하려고 하면 오류가 발생합니다.

이 오류를 방지하려면  `.createOrReplace` 대신 `.create`를 사용할 수 있습니다. 이 명령은 같은 종류의 기존 컴포넌트가 있으면 덮어쓰고, 없으면  `.create`.

```ts
Transform.createOrReplace(door, {
	position: Vector3.create(5, 1, 5),
})
```


> [!WARNING]
> **📔 참고**:  `.createOrReplace` 는 컴포넌트를 만들기 전에 추가 검사를 수행하므로, 항상  `.create`를 사용하는 편이 더 성능상 유리합니다. 추가하려는 것과 같은 컴포넌트가 엔티티에 이미 없다고 확신한다면  `.create`.


## 엔티티에서 컴포넌트 접근하기

엔티티의  `.get()` 또는 `getMutable()` 함수입니다.

```ts
export function main() {
	// 엔티티 생성
	const box = engine.addEntity()

	// 해당 엔티티에 컴포넌트 생성 및 추가
	Transform.create(box)

	// 컴포넌트의 읽기 전용 버전 가져오기
	let transform = Transform.get(box)

	// 컴포넌트의 변경 가능한 버전 가져오기
	let transform = Transform.getMutable(box)
}
```

해당 `get()` 함수는 컴포넌트에 대한 읽기 전용 참조를 가져옵니다. 이 컴포넌트 참조의 어떤 값도 변경할 수 없습니다.

컴포넌트 값을 변경하고 싶다면 대신  `getMutable()` 함수를 사용하세요. 컴포넌트의 변경 가능한 버전에서 값을 변경하면, 해당 컴포넌트가 속한 엔티티에 직접 영향을 줍니다.

참조 [변경 가능한 데이터](/docs/creator/content-creator-ko/sdk7/programming-patterns/mutable-data/) 자세한 내용은.


> [!WARNING]
> **📔 참고**: 실제로 컴포넌트 값을 변경할 경우에만  `getMutable()` 를 사용하세요. 그렇지 않다면 항상  `get()`를 사용하세요. 이 관행은  [데이터 지향 프로그래밍](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/)의 원칙을 따르며, 씬 성능 향상에 크게 도움이 됩니다.


```ts
// 컴포넌트의 변경 가능한 버전 가져오기
let transform = Transform.getMutable(box)

// 컴포넌트 값 변경
transform.scale.x = 5
```

위 예제는 Transform 컴포넌트의  *x* scale 값을 직접 수정합니다.

가져오려는 엔티티에 정말 그 컴포넌트가 있는지 확실하지 않다면  `getOrNull()` 또는 `getMutableOrNull()`.


> [!WARNING]
> **📔 참고**를 가능한 한 사용하지 마세요. 이 함수들은 추가 검사를 포함하므로  `getOrNull()` 또는 `getMutableOrNull()` 보다 효율이 떨어집니다. `.get()` 및 `getMutable()`.


```ts
//  getOrNull
const transformOrNull = Transform.getOrNull(myEntity)

//  getMutableOrNull
const mutableTransformOrNull = Transform.getMutableOrNull(myEntity)
```

가져오려는 컴포넌트가 엔티티에 존재하지 않으면:

* `get()` 및 `getMutable()` 는 오류를 반환합니다.
* `getOrNull()` 및 `getMutableOrNull()` 반환값은 `Null`.

## 엔티티에서 컴포넌트 제거

엔티티에서 컴포넌트를 제거하려면 엔티티의  `deleteFrom()` 메서드를 사용하세요.

```ts
Transform.deleteFrom(myEntity)
```

엔티티에 존재하지 않는 컴포넌트를 제거하려고 해도 오류가 발생하지 않습니다.


> [!WARNING]
> **📔 참고**: 엔티티의 모든 컴포넌트를 한 번에 제거하려면  [이 섹션](#remove-entities)


## 컴포넌트 확인

엔티티가 특정 컴포넌트 인스턴스를 소유하고 있는지 확인하려면  `has()` 함수를 사용할 수 있습니다. 이 함수는  *true* 컴포넌트가 있으면 *false* 없으면&#x20;

```ts
const hasTransform = Transform.has(myEntity)
```


> [!NOTE]
> **💡 팁**: 또한  [컴포넌트를 조회](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/) 를 사용해 특정 컴포넌트 또는 특정 컴포넌트 집합을 가진 컴포넌트의 전체 목록을 가져올 수 있습니다. 각 엔티티를 확인하기 위해 씬의 모든 엔티티를 수동으로 반복하며  `has()`를 호출하지 마세요. 그런 방식은 훨씬 비효율적입니다.


## 컴포넌트의 변경 사항 확인

다음을 사용하세요 `onChange` 함수를 사용하면 주어진 엔티티에 대해 컴포넌트 값이 바뀔 때마다 콜백 함수를 실행할 수 있습니다. 이 기능은 모든 컴포넌트에서 작동하며, 코드를 읽기 쉽게 유지하는 데 아주 유용한 지름길입니다.

콜백 함수에는 컴포넌트의 새 상태를 포함하는 입력 매개변수를 넣을 수 있습니다.

```ts
Transform.onChange(cubeEntity, (newTransform) => {
	if (!newTransform) return
	console.log(
		'큐브 위치 변경: ',
		newTransform.position,
		newTransform.rotation
	)
})

VisibilityComponent.onChange(cubeEntity, (newVisibilityComponent) => {
	if (!newVisibilityComponent) return
	console.log('Cube visibility changed: ', newVisibilityComponent.visible)
})
```

컴포넌트가 엔티티에서 제거되면, 함수는 입력으로  `undefined`.


> [!NOTE]
> **💡 팁**: `.onChange()` 함수는 SDK의 기본 컴포넌트뿐 아니라  [사용자 정의 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 에도 작동합니다.


## 자식 엔티티 가져오기

부모 엔티티의 직접 자식인 모든 엔티티에 접근하려면  `getEntitiesWithParent`를 사용하세요. 이 함수는 인자로  `엔진` 과 `parent` 엔티티를 받으며, 해당 엔티티를 부모로 가진 모든 엔티티 목록을 반환합니다. 직접 자식만 반환하며, 자식의 자식은 반환하지 않습니다.

```ts
import { getEntitiesWithParent } from '@dcl/sdk/ecs'

const children = getEntitiesWithParent(engine, myEntity)
for (const child of children) {
   // 각 자식 엔티티 처리
}
```

대신 엔티티의 모든 하위 항목에, 중첩 깊이에 상관없이 접근하려면 함수  `getComponentEntityTree()`를 사용하세요. 계층 구조를 레벨별로 수동으로 순회하는 대신, 이 함수는 반복하기 쉬운 모든 하위 항목의 평면 목록을 반환합니다. 또한 주어진 컴포넌트 또는 컴포넌트 목록을 가진 엔티티만 필터링합니다.

```ts
import { getComponentEntityTree } from '@dcl/sdk/ecs'

export function main() {
	// 중첩된 자식이 있는 부모 엔티티 생성
	const parentEntity = engine.addEntity()
	Transform.create(parentEntity, {
		position: Vector3.create(8, 0, 8),
	})

	// ... 부모에 여러 자식과 손자식이 있다고 가정

	// 부모 엔티티의 모든 하위 항목을 반복
	for (const descendantEntity of getComponentEntityTree(
		engine,
		parentEntity,
		Transform
	)) {
		// 각 하위 엔티티에 접근
		const transform = Transform.get(descendantEntity)
		console.log('Descendant position:', transform.position)
	}
}
```

해당 `getComponentEntityTree` 함수는 세 개의 매개변수를 받습니다:

* `엔진`: 엔티티를 실행하는 엔진 인스턴스
* `엔티티`: 시작할 루트 엔티티
* `컴포넌트`: 필터링할 컴포넌트(보통  `Transform` 공간 계층 구조에 사용됨)

이 함수는 트리 구조에서 각 하위 엔티티를 생성하는 제너레이터를 반환합니다. 지정된 컴포넌트를 가진 엔티티만 결과에 포함됩니다.

이를 다른 컴포넌트 검사와 함께 사용하여 계층 구조에서 특정 엔티티를 찾을 수 있습니다:

```ts
// 특정 이름을 가진 모든 하위 항목 찾기
for (const descendantEntity of getComponentEntityTree(
	engine,
	parentEntity,
	Transform
)) {
	const name = Name.getOrNull(descendantEntity)
	if (name && name.value === 'targetEntity') {
		console.log('대상 엔티티를 찾음:', descendantEntity)
	}
}
```

## 예약된 엔티티

일부 엔티티 ID는 모든 씬에 존재하는 특별한 엔티티를 위해 예약되어 있습니다. 다음 별칭으로 접근할 수 있습니다:

* `engine.RootEntity`
* `engine.PlayerEntity`
* `engine.CameraEntity`


> [!WARNING]
> **📔 참고**이 엔티티들이 초기화되기 전에 참조하지 마세요. 이 문제를 피하려면  `main()` 함수나 시스템에서 이 엔티티들을 참조하세요.


## 루트 엔티티

씬의 모든 엔티티는  `engine.RootEntity`의 자식이며, 직접적이든 간접적이든 마찬가지입니다.

이 엔티티에는 Transform 컴포넌트가 없지만, 다음과 같은 보다 전역적인 설정을 나타내는 여러 컴포넌트를 처리하는 데 사용됩니다. [하늘박스 제어](/docs/creator/content-creator-ko/sdk7/interactivity/skybox-control/), [커서 위치](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#check-the-players-cursor-position), 또는 [화면 크기](/docs/creator/content-creator-ko/sdk7/2d-ui/ui-positioning/#responsive-ui-size).

## 플레이어 엔티티

해당 `engine.PlayerEntity` 엔티티는 플레이어의 아바타를 나타냅니다.

플레이어의  `Transform` 컴포넌트를 가져와 플레이어의 현재 위치와 회전을 확인하세요. 다음을 참조하세요. [사용자 데이터](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/). 플레이어의 Transform은 읽기 전용이므로, 수정하려면  `movePlayerTo()` 함수를 사용하세요, [자세히 알아보기](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/move-player/).

이 엔티티의 자식으로 설정하여 플레이어에 오브젝트를 붙일 수도 있지만,  [플레이어에 부착](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#attach-an-entity-to-an-avatar) 가 그런 용도로는 종종 더 나은 선택입니다.

## 카메라 엔티티

해당 `engine.CameraEntity` 엔티티는 플레이어의 카메라를 나타냅니다.

카메라의  `Transform` 컴포넌트를 가져와 카메라의 위치와 회전을 확인하세요. 이 엔티티의 Transform 역시 읽기 전용입니다. 카메라 각도나 위치를 수정하려면  [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/#using-virtual-cameras).

카메라의  `CameraMode` 컴포넌트를 가져와 플레이어가 1인칭 또는 3인칭 카메라 모드를 사용 중인지 확인하세요. 다음을 참조하세요. [카메라 모드](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#check-the-players-camera-mode).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/entities-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
