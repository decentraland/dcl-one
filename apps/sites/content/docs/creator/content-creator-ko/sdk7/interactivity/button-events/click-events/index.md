> 씬에서 사용자 클릭을 처리하는 방법을 알아봅니다.

# 클릭 이벤트

Decentraland 씬은 플레이어 아바타를 제어하는 데 사용되는 모든 버튼에서 입력 액션을 감지할 수 있습니다. 여기에는 포인터 클릭, 여러 액션 버튼, 그리고 아바타를 이동시키는 데 사용되는 키가 포함됩니다. 버튼 이벤트는 데스크톱 클라이언트의 마우스와 키보드, 화면상의 컨트롤에서 [모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/), VR 컨트롤러, 또는 다른 입력 장치에서 올 수 있으며 — 이들은 모두 SDK에서 동일하게 해석됩니다.

엔티티를 대상으로 하는 입력 액션을 감지할 수 있습니다. 이는 플레이어의 커서가 해당 엔티티의 콜라이더를 가리키고 있는 동안 버튼을 누르는 것을 포함합니다. 또한 *전역* 입력 이벤트도 감지할 수 있는데, 이는 포인터가 어디를 향하고 있는지와 상관없이 언제든지 입력을 누르거나 활성화하는 동작을 포함합니다.


> [!WARNING]
> **📔 참고**엔티티가 입력 액션에 응답하려면 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/) 가 있어야 합니다. `MeshRenderer` 모델에도 `MeshCollider` 컴포넌트를 추가해야 합니다. `GltfContainer` 의 모델은 자체적으로 내장된 충돌 지오메트리를 가질 수 있고, 보이는 지오메트리를 사용하도록 설정할 수도 있으며, 또한 `MeshCollider` 컴포넌트.


사용 사례에 따라 입력 액션을 처리하는 방법은 여러 가지가 있습니다.

* [**콜백 등록**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/): 단일 엔티티에 상호작용을 추가하는 가장 쉬운 방법입니다. 콜백 함수와 호버 피드백을 설정하는 단일 문장을 작성하세요.
* [**시스템 기반**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/): 비슷한 동작을 하는 여러 엔티티를 처리하는 데 이상적입니다. 시스템을 사용해 비슷한 엔티티들을 순회하면서 각 엔티티의 입력 액션을 조회하고, 모두 같은 로직으로 처리하세요. 호버 피드백은 별도로 설정해야 합니다. 이 접근 방식은 전역 입력 액션을 처리할 때도 필요합니다.
* [**고급**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/advanced-button-events/): 타임스탬프와 입력 이벤트의 이벤트 기록을 포함해 각 엔티티의 원시 응답 데이터를 읽습니다. 이는 사용자 정의 상호작용 패턴을 정의하는 데 유용할 수 있습니다.

## Creator Hub의 Scene Editor를 사용하세요

엔티티의 클릭 이벤트를 처리하는 가장 쉬운 방법은 Scene Editor를 사용하는 것입니다. 코드 없이 사용하는 **On Click** 또는 **On Input Action** 트리거를 사용해 클릭 시 액션을 호출하세요. 또는 **On Global Click**, **On Global Primary** 또는 **On Global Secondary** 트리거를 사용해 전역 버튼 이벤트에 반응하세요. 다음을 참조하세요 [모든 아이템을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).

## 간단한 예제

엔티티의 클릭을 감지하려면 `pointerEventsSystem.onPointerDown`.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Click' },
	},
	function () {
		console.log('clicked entity')
	}
)
```

다음을 참조하세요 [**콜백 등록**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/) 를 참조하세요.

## 호버 피드백

플레이어가 엔티티와 상호작용할 수 있다는 것을 알아차리게 하는 것이 중요합니다. 그렇지 않으면, 여러분이 만든 경험을 완전히 놓칠 수 있습니다. 모든 오브젝트를 클릭해 보면서 하나쯤 반응하길 바라는 경험은 좋지 않습니다.

다음 [**콜백 등록**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/) 메서드를 사용할 때, 플레이어가 오브젝트 위로 커서를 옮길 때마다 두 종류의 피드백이 표시됩니다:

* 엔티티의 가장자리가 강조 표시됩니다(Decentraland 2.0 데스크톱 클라이언트에서만). 엔티티가 클릭 가능한 거리에 있으면 강조 색은 초록색이고, 너무 멀면 빨간색입니다.
* 커서 근처에 UI 텍스트가 있는 호버 힌트가 나타나, 클릭하면 어떤 일이 일어날지 알려 줍니다.

다음 [**시스템 기반**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/) 메서드를 사용할 때는 `PointerEvents` 컴포넌트를 클릭 가능한 엔티티에 추가하면 같은 결과를 얻을 수 있습니다.

이 메서드와 컴포넌트의 속성을 통해 엔티티 강조 표시와 호버 힌트 둘 다 비활성화할 수 있습니다.

가장자리 강조 표시는 커서가 올라가 있는 콜라이더를 가진 엔티티와 그 엔티티의 모든 자식을 둘러쌉니다. 엔티티의 부모나 형제는 둘러싸지 않습니다. 다음의 도형만 `MeshRenderer` 와 다음의 모델만 `GltfContainer` 윤곽선이 표시되며, `TextShape` 텍스트는 그렇지 않습니다. 보이지 않는 박스를 더 큰 클릭 영역으로 사용한다면, 보이는 부분들을 그 박스 엔티티의 자식으로 만들어야 합니다. 그렇지 않으면 아무것도 윤곽선이 표시되지 않습니다.

또한 [고급 사용자 정의 힌트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/#advanced-custom-hints)를 구현할 수도 있습니다. 예를 들어 소리를 재생하거나, 포인팅될 때 엔티티의 색을 바꾸고, 회전시키거나, 커지게 만들 수 있습니다. 무엇을 하든 그것이 분명한 신호임을 নিশ্চিত해야 합니다.

## 장애물

버튼 이벤트는 포인터 이벤트 충돌 레이어에 구독된 경로상의 첫 번째 엔티티와만 상호작용하는 레이를 발사합니다. 이는 엔티티가 거리 제한 내에 있는 한 성립합니다. 엔티티가 카메라와 아바타 모두에 대해 거리 제한을 정의한 경우, 둘 중 하나만 충분히 가깝다면 됩니다.

콜라이더가 있는 동일한 엔티티에 포인터 이벤트를 등록하세요. 부모 엔티티나 자체 콜라이더가 없는 자식 엔티티에 등록된 포인터 이벤트는 절대 실행되지 않습니다.

엔티티가 포인터 이벤트의 레이에 가로채지려면, 다음 중 하나여야 합니다:

* 모델에 [콜라이더 메쉬가 포함되어 있어야 합니다](/docs/creator/content-creator-ko/3d/colliders/).
* 다음 `GltfContainer` 다음 설정을 사용하도록 구성되어야 합니다 [충돌 마스크가 있는 보이는 지오메트리](/docs/creator/content-creator-ko/sdk7/3d/colliders/#colliders-on-3d-models).
* 엔티티에 [MeshCollider 컴포넌트가 있어야 합니다](/docs/creator/content-creator-ko/sdk7/3d/colliders/).

플레이어가 상호작용하려는 엔티티 앞에 다른 엔티티의 콜라이더가 놓여 있으면, 해당 엔티티에 콜라이더가 없거나 이 콜라이더가 포인터 이벤트 충돌 레이어에 응답하지 않도록 구성되어 있지 않는 한, 플레이어는 뒤에 있는 엔티티를 클릭할 수 없습니다.

```ts
// 클릭 가능한 엔티티
const clickableEntity = engine.addEntity()
MeshRenderer.setBox(clickableEntity)
MeshCollider.setBox(clickableEntity)
Transform.create(clickableEntity, { position: Vector3.create(8, 1, 8) })

pointerEventsSystem.onPointerDown(
	{
		entity: clickableEntity,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Click',
		},
	},
	function () {
		console.log('clicked entity')
		const t = Transform.getMutable(clickableEntity)
		t.scale.y += 0.2
	}
)

// 클릭을 막지 않는 객체
const nonBlocker = engine.addEntity()
MeshRenderer.setBox(nonBlocker)
MeshCollider.setBox(nonBlocker, ColliderLayer.CL_PHYSICS)
Transform.create(nonBlocker, { position: Vector3.create(10, 1, 8) })

// 클릭을 막는 객체
const blocker = engine.addEntity()
MeshRenderer.setBox(blocker)
MeshCollider.setBox(blocker, ColliderLayer.CL_POINTER)
Transform.create(blocker, { position: Vector3.create(8, 1, 10) })
```


> [!WARNING]
> **📔 참고**: 엔티티가 포인터 이벤트를 가로채는 것뿐만 아니라 데이터를 반환하려면, 해당 엔티티에도 `PointerEvents` 컴포넌트가 필요합니다.  `pointerEventsSystem` 헬퍼도 이 요구 사항을 처리해 줍니다.


## 포인터 버튼

다음 입력은 입력 이벤트를 감지하는 어떤 방식으로도 처리할 수 있습니다.

* `InputAction.IA_POINTER`: **왼쪽 마우스 버튼** 컴퓨터에서, **상호작용 버튼** 모바일에서.
* `InputAction.IA_PRIMARY`: **E** 컴퓨터의 키, **E 버튼** 모바일에서.
* `InputAction.IA_SECONDARY`: **F** 컴퓨터의 키, **F 버튼** 모바일에서.
* `InputAction.IA_ACTION_3`: **1** 컴퓨터의 키, **1번 버튼** 모바일에서. *모바일에서는 기본적으로 숨겨져 있음(SDK 7.32.0+) — 씬이 다음과 함께 이를 표시하는 경우에만 사용하세요 `TouchScreenControls`.*
* `InputAction.IA_ACTION_4`: **2** 컴퓨터의 키, **2번 버튼** 모바일에서. *모바일에서는 기본적으로 숨겨져 있음(SDK 7.32.0+) — 씬이 다음과 함께 이를 표시하는 경우에만 사용하세요 `TouchScreenControls`.*
* `InputAction.IA_ACTION_5`: **3** 컴퓨터의 키, **3번 버튼** 모바일에서. *모바일에서는 기본적으로 숨겨져 있음(SDK 7.32.0+) — 씬이 다음과 함께 이를 표시하는 경우에만 사용하세요 `TouchScreenControls`.*
* `InputAction.IA_ACTION_6`: **4** 컴퓨터의 키, **4번 버튼** 모바일에서. *모바일에서는 기본적으로 숨겨져 있음(SDK 7.32.0+) — 씬이 다음과 함께 이를 표시하는 경우에만 사용하세요 `TouchScreenControls`.*
* `InputAction.IA_JUMP`: **Space** 컴퓨터의 키, **점프 버튼** 모바일에서.
* `InputAction.IA_FORWARD`: **W** 키(컴퓨터에서).
* `InputAction.IA_LEFT`: **A** 키(컴퓨터에서).
* `InputAction.IA_RIGHT`: **D** 키(컴퓨터에서).
* `InputAction.IA_BACKWARD`: **S** 키(컴퓨터에서).
* `InputAction.IA_WALK`: **Control** 키(컴퓨터에서).
* `InputAction.IA_MODIFIER`: **Shift** 키(컴퓨터에서).

각 `InputAction` 키보드의 실제 입력에서 추상화되어 기기에 따라 다른 입력에 매핑될 수 있습니다. 같은 이유로, 키보드의 모든 버튼이 버튼 이벤트로 추적될 수 있는 것은 아니며, 이동과 상호작용에 사용되는 버튼만 추적할 수 있습니다. 이 의도적인 제한은 콘텐츠가 데스크톱, [모바일 클라이언트에서](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/)및 향후의 VR 컨트롤러와 기타 게임 컨트롤러 전반에서 호환되도록 보장합니다.


> [!WARNING]
> **📱 모바일**: [모바일 클라이언트에서](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/)에서는 모든 입력 액션을 화면상의 버튼에 연결할 수 있지만, 기본적으로는 세 개의 버튼만 표시됩니다: `IA_POINTER` (상호작용 버튼), `IA_PRIMARY` (E 버튼) 및 `IA_JUMP`. `IA_SECONDARY` (F)와 `IA_ACTION_3`–`IA_ACTION_6` (데스크톱의 `1`/`2`/`3`/`4` 버튼)은 기본적으로 숨겨진 상태로 시작합니다(SDK 7.32.0 이상; 이전 SDK는 8개 모두를 표시하며, 가장 낮은 것들은 "+" 오버플로 뒤에 있습니다). 모바일용으로 설계할 때는 핵심 액션에 `IA_POINTER`, `IA_PRIMARY` 및 `IA_JUMP` 를 우선 사용하세요. F나 숫자 버튼이 필요하면 [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/). 다음을 참조하세요: [모바일 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/).


## 포인터 이벤트의 종류

각 입력은 다음과 같은 종류의 포인터 이벤트를 생성할 수 있습니다. 다음은 각각 `PointerEventType` enum의 값입니다.

* `PET_DOWN`: 플레이어가 엔티티의 콜라이더를 향해 커서를 둔 상태에서 특정 버튼을 누릅니다.
* `PET_UP`: 플레이어가 엔티티의 콜라이더를 향해 커서를 둔 상태에서 특정 버튼을 놓습니다.
* `PET_HOVER_ENTER`: 플레이어의 커서가 엔티티의 콜라이더를 향하기 시작합니다.
* `PET_HOVER_LEAVE`: 플레이어의 커서가 엔티티의 콜라이더를 향하는 것을 멈춥니다.
* `PET_PROXIMITY_ENTER`: 플레이어가 어디를 보고 있든 상관없이 엔티티의 근접 범위 안으로 들어갑니다.
* `PET_PROXIMITY_LEAVE`: 플레이어가 엔티티의 근접 범위 밖으로 나갑니다.

다음을 참조하세요 [**Proximity Events**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events/) 근접 기반 상호작용을 사용하는 방법은 다음을 참조하세요.

## 입력 액션의 데이터

모든 입력 액션에는 활성화된 버튼이나 그 시점에 포인터가 어디를 가리키고 있었는지 같은 이벤트 데이터가 포함됩니다.

어떤 입력 이벤트에서든 다음 정보를 얻을 수 있습니다:

* `analog`: 선택적 숫자이며, 조이스틱 같은 아날로그 입력 이벤트에만 존재합니다. 입력의 아날로그 값을 저장합니다.
* `button`: 어떤 버튼 ID가 눌렸는지입니다. 이 숫자는 사용 가능한 모든 버튼을 나열한 `InputAction` enum에 해당합니다.
* `state`: 포인터 이벤트의 종류이며, enum `PointerEventType`. *0* 를 참조합니다 `PointerEventType.PET_UP`, *1* 에서 `PointerEventType.PET_DOWN`, *2* 에서 `PointerEventType.PET_HOVER_ENTER`, *3* 에서 `PointerEventType.PET_HOVER_LEAVE`, *4* 에서 `PointerEventType.PET_PROXIMITY_ENTER`, *5* 에서 `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`: [람포트 타임스탬프](https://en.wikipedia.org/wiki/Lamport_timestamp) 로 각 버튼 이벤트를 식별합니다.

  > 참고: 이 타임스탬프는 현재 시간을 기준으로 번호가 매겨지지 않습니다. 0에서 시작해 각 이벤트마다 1씩 증가하는 카운터라고 생각하세요.
* `hit`: 다음 데이터를 포함하는 객체입니다. hit 이벤트에 대한 정보는 다음과 같습니다:
  * `entityId`: 레이가 맞힌 엔티티의 ID 번호입니다.
  * `meshName`: *String* 맞은 3D 모델의 특정 메쉬 내부 이름이 들어 있습니다. 이는 3D 모델이 여러 메쉬로 구성되어 있을 때 유용합니다.
  * `globalOrigin`: *Vector3* 레이가 시작되는 위치(씬 기준)
  * `direction`: *Vector3* 레이의 방향 벡터(전역 좌표)
  * `위치`: *Vector3* 레이가 히트된 엔티티와 교차한 위치(씬 기준)
  * `length`: 레이의 시작점부터 엔티티와의 충돌이 발생한 위치까지의 길이입니다.
  * `normalHit`: *Vector3* 정규화된 방향 벡터로, 월드 공간에서 히트의 법선 각도를 설명합니다.

이 데이터는 입력 액션을 처리하는 데 어떤 접근 방식을 사용하느냐에 따라 다른 방법으로 접근합니다.

다음 [**콜백 등록**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/) 접근 방식을 사용하면, 콜백 함수에 전달되는 첫 번째 매개변수에 이 전체 데이터 구조가 들어 있습니다.

```ts
pointerEventsSystem.onPointerDown({ entity: myEntity }, function (cmd) {
	console.log(cmd.hit?.entityId)
})
```

다음 [**시스템 기반**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/) 접근 방식을 사용할 때는 `inputSystem.getInputCommand()` 를 사용해 이 데이터를 가져오세요.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```


> [!WARNING]
> **📔 참고**: 엔티티가 포인터 이벤트를 가로채는 것뿐만 아니라 데이터를 반환하려면, 해당 엔티티에도 `PointerEvents` 컴포넌트가 필요합니다.  `pointerEventsSystem` 헬퍼도 이 요구 사항을 처리해 줍니다.


다음 [**고급**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/advanced-button-events/) 접근 방식에서, `PointerEventsResult` 에는 해당 엔티티에 대한 모든 포인터 이벤트의 최근 기록 목록이 들어 있습니다.

```ts
engine.addSystem(() => {
	for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
		const pointerEvents = PointerEventsResult.get(entity)

		for (const event of pointerEvents) {
			console.log(event.hit?.entityId)
		}
	}
})
```

## 커서 잠금 또는 잠금 해제


> [!NOTE]
> **💡 참고**: `PointerLock` 는 데스크톱 클라이언트 개념입니다(잠긴 마우스 커서 vs 잠금 해제된 마우스 커서). 이는 [모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/)에는 영향을 주지 않으며, 그곳의 입력은 터치 기반입니다.


Decentraland의 데스크톱 클라이언트에서 플레이어는 커서를 사용해 두 가지 서로 다른 상호작용 모드를 전환할 수 있습니다:

* 잠긴 커서: 마우스를 움직이면 카메라가 이동하고, 클릭 이벤트는 화면 중앙의 조준선에서 발생합니다
* 잠금 해제된 커서: 커서를 카메라와 독립적으로 자유롭게 움직일 수 있으며, 플레이어는 화면 어디에서든 클릭하여 3D 공간이나 UI와 상호작용할 수 있습니다

이러한 상호작용 모드는 서로 다른 종류의 게임 메커니즘에 이상적이며, 씬에 가장 잘 맞는 쪽에 따라 플레이어가 둘 중 하나를 사용하도록 하고 싶을 수 있습니다. 또한 UI와 상호작용할 수 있도록 잠시 동안 플레이어의 커서가 잠금 해제되도록 보장하고 싶을 수도 있는데, 이는 이 제어 방식에 익숙하지 않은 신규 플레이어에게 특히 유용합니다.

플레이어의 커서 상태를 변경하려면 `PointerLock` 컴포넌트를 `engine.CameraEntity` 엔티티에 사용하세요.

```ts
import {PointerLock} from '@dcl/sdk/ecs'
    
PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false})
```


> [!WARNING]
> **📔 참고**: 엔진은 기본적으로 `PointerLock` 컴포넌트를 `engine.CameraEntity` 를 제공합니다. 다음을 사용하세요 `createOrReplace()` 또는 `getMutable()` 대신 `create()`는 컴포넌트가 이미 존재하면 오류를 발생시킵니다.


또한 `PointerLock` 컴포넌트의 상태를 읽어 플레이어의 커서 상태를 조회할 수도 있습니다.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

const isPointerLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked
```

다음을 사용하세요 `.onChange` 함수를 사용해 포인터 상태의 변화에 반응하세요. 다음 예제는 커서가 항상 잠금 해제 상태가 되도록 강제합니다.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {

    PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false});

    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    PointerLock.getMutable(engine.CameraEntity).isPointerLocked = false
		   }
	})
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/button-events/click-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
