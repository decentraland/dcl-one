> 플레이어의 마우스 움직임에 실시간으로 반응하여 드래그 제스처를 구현하거나, 오브젝트를 회전시키거나, 커스텀 카메라를 제어하세요.

# 마우스 움직임

장면은 매 프레임마다 플레이어의 원시 마우스 움직임을 읽어 실시간 상호작용을 구동할 수 있습니다: 드래그와 스와이프 제스처, 플레이어가 끄는 대로 오브젝트를 회전시키거나 미끄러지게 하는 것, 또는 1인칭 슈터처럼 사용자 정의 카메라를 조작하는 것입니다.

이를 위해서는 다음을 읽으세요 `screenDelta` 컴포넌트의 `PrimaryPointerInfo` 의 컴포넌트 `engine.RootEntity`. 이 속성은 `Vector2` 마지막 프레임 이후 마우스가 몇 픽셀 움직였는지 보고합니다. 양수 `x` 값은 마우스가 오른쪽으로 움직였음을 의미하고, 양수 `y` 값은 아래로 움직였음을 의미하며, 화면의 원점이 왼쪽 위 모서리에 있는 것과 일치합니다 — 다음과 같은 동일한 좌표 공간입니다 `UiTransform` 및 `UiCanvasInformation`. 마우스가 움직이지 않는 프레임에서는 두 값 모두 0입니다.

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'

engine.addSystem(() => {
	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta) return
	console.log(`mouse moved: ${delta.x}, ${delta.y}`)
})
```

왜냐하면 `screenDelta` 한 프레임의 움직임만 담고 있으므로, 항상 [system](/docs/creator/content-creator-ko/sdk7/architecture/systems/), 그래서 프레임 사이에 어떤 움직임도 놓치지 않게 됩니다.


> [!WARNING]
> **📔 참고**: 다음을 참조하지 마세요 `engine.RootEntity` 초기 장면 로딩 시에는 엔티티가 아직 초기화되지 않았다면 오류가 발생할 수 있으므로, 이를 참조하지 마세요. 이 문제를 피하려면 항상 시스템 안에서 엔티티를 참조하세요. 시스템의 첫 실행은 장면이 이미 올바르게 초기화된 후에 호출되므로, 엔티티는 항상 사용할 수 있습니다.



> [!WARNING]
> **📔 참고**: 이 문서에 설명된 내용은 데스크톱 플레이어에게만 해당됩니다. 모바일 앱에서는 [모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/), 입력은 터치 기반이며 자유롭게 움직이는 커서가 없으므로 `screenDelta` 항상 0을 반환합니다. 다음을 참조하세요 [모바일에서의 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/) 터치 기기에서 입력이 작동하는 방식은 다음을 참고하고, [플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 모바일 플레이어에게 대체 조작 방식을 제공하기 위해.


## 커서가 잠겨 있을 때의 마우스 움직임

`screenDelta` 는 다음의 다른 속성과 다르게 작동합니다 `PrimaryPointerInfo` 커서가 [잠겨 있을 때](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor): `screenCoordinates` 화면 중앙에 고정되고 `worldRayDirection` 항상 화면 중앙의 레이를 보고하지만, `screenDelta` 매 프레임마다 원시 마우스 움직임을 계속 보고합니다. 이 덕분에 커서가 잠겨 있을 때 마우스 움직임을 읽는 유일한 방법이 되며, 바로 사용자 정의 카메라나 조준 조작에 필요한 것입니다.

## 드래그하여 오브젝트 회전하기

다음 예제에서는 포인터 버튼을 누른 채 마우스를 옆으로 드래그하여 플레이어가 큐브를 회전시킬 수 있습니다:

```ts
import {
	engine,
	InputAction,
	inputSystem,
	MeshCollider,
	MeshRenderer,
	PrimaryPointerInfo,
	Transform,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const DRAG_SENSITIVITY = 0.5

export function main() {
	const cube = engine.addEntity()
	Transform.create(cube, { position: Vector3.create(8, 1, 8) })
	MeshRenderer.setBox(cube)
	MeshCollider.setBox(cube)

	engine.addSystem(() => {
		// 포인터 버튼이 눌려 있는 동안에만 회전
		if (!inputSystem.isPressed(InputAction.IA_POINTER)) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta || delta.x === 0) return

		const transform = Transform.getMutable(cube)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromEulerDegrees(0, delta.x * DRAG_SENSITIVITY, 0)
		)
	})
}
```

같은 패턴은 모든 드래그 상호작용에 적용됩니다: 다음을 사용해 엔티티를 레일을 따라 미끄러뜨리거나 `delta.x` 를 오프셋으로 사용하거나, 애니메이션을 스크럽하거나, 큰 delta 값이 있는지 확인해 빠른 스와이프를 감지할 수 있습니다.

## 마우스룩 카메라 조작

왜냐하면 `screenDelta` 커서가 잠겨 있는 동안에도 계속 작동하므로, 다음을 위한 마우스룩 조작을 만들 때 사용할 수 있습니다 [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/#using-virtual-cameras). 이를 위해 마우스 움직임을 yaw와 pitch 각도로 누적한 다음, 매 프레임 카메라의 회전에 적용하세요.

다음 예제에서는 플레이어가 상자를 클릭해 마우스로 조작할 수 있는 가상 카메라로 전환하고, 보조 버튼(*F* 또는 오른쪽 클릭)으로 기본 카메라로 돌아갑니다. 가상 카메라가 활성화되어 있는 동안, 장면은 커서도 잠그고 아바타도 다음과 같은 방식으로 고정합니다 [입력 수정자](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#freeze-the-player), 그래서 플레이어는 카메라를 조작하는 동안 무작정 돌아다니지 않습니다.

```ts
import {
	engine,
	Entity,
	InputAction,
	InputModifier,
	inputSystem,
	MainCamera,
	MeshCollider,
	MeshRenderer,
	PointerEventType,
	pointerEventsSystem,
	PointerLock,
	PrimaryPointerInfo,
	Transform,
	VirtualCamera,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

// 마우스 움직임 1픽셀당 카메라 회전 각도
const SENSITIVITY = 0.15

let cameraEntity: Entity
let cameraActive = false
let yaw = 0
let pitch = 0

export function main() {
	// 카메라가 따라가는 엔티티
	cameraEntity = engine.addEntity()
	Transform.create(cameraEntity, { position: Vector3.create(8, 3, 8) })
	VirtualCamera.create(cameraEntity, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0.5) },
	})

	// 클릭하여 마우스룩 모드로 전환
	const box = engine.addEntity()
	Transform.create(box, { position: Vector3.create(8, 1, 4) })
	MeshRenderer.setBox(box)
	MeshCollider.setBox(box)
	pointerEventsSystem.onPointerDown(
		{
			entity: box,
			opts: { button: InputAction.IA_POINTER, hoverText: '카메라 조작' },
		},
		() => activateCamera(true)
	)

	// 마우스로 카메라를 조작
	engine.addSystem(() => {
		if (!cameraActive) return
		if (!PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta) return

		yaw += delta.x * SENSITIVITY
		// 카메라가 뒤집히지 않도록 pitch를 제한
		pitch = Math.max(-85, Math.min(85, pitch + delta.y * SENSITIVITY))

		Transform.getMutable(cameraEntity).rotation = Quaternion.fromEulerDegrees(pitch, yaw, 0)
	})

	// 보조 버튼으로 마우스룩 모드 종료
	engine.addSystem(() => {
		if (!cameraActive) return
		if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN)) {
			activateCamera(false)
		}
	})
}

function activateCamera(active: boolean) {
	cameraActive = active

	// 가상 카메라를 할당하거나 해제
	MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: active ? cameraEntity : undefined,
	})

	// 카메라를 조작하는 동안 아바타를 고정
	InputModifier.createOrReplace(engine.PlayerEntity, {
		mode: InputModifier.Mode.Standard({ disableAll: active }),
	})

	// 커서를 잠가 마우스가 즉시 카메라를 조작하도록 함
	PointerLock.createOrReplace(engine.CameraEntity, { isPointerLocked: active })
}
```

이 예제에서 주의할 점 몇 가지:

* 시스템은 다음을 더합니다 `delta.y` 를 pitch에 더합니다. 마우스를 위로 움직이면 음수 `delta.y`가 보고되며, 이는 pitch를 낮추고 카메라를 위로 기울입니다. 세로 조작을 반대로 선호한다면 그 부호를 바꾸세요.
* pitch는 -85도에서 85도 범위로 제한되므로, 카메라가 뒤로 뒤집힐 수 없습니다.
* 해당 `SENSITIVITY` 상수는 마우스 움직임 1픽셀당 회전 각도를 나타내며, 취향에 맞게 조정하세요.


> [!NOTE]
> **💡 팁**: 플레이어는 언제든지 다음을 눌러 커서를 잠금 해제할 수 있습니다 *Esc* 또는 오른쪽 클릭하여 카메라가 마우스에 반응하지 않게 할 수 있습니다. 위 예제의 보조 버튼처럼, 플레이어가 카메라 모드를 완전히 종료할 수 있는 명확한 방법을 항상 제공하세요. 코드로 커서를 다시 잠글 수도 있습니다. 다음을 참고하세요 [커서 잠금 또는 잠금 해제](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor).


## 관련 주제

커서의 움직임이 아니라 화면상의 절대 위치를 읽으려면 다음을 사용하세요 `screenCoordinates` 같은 컴포넌트의 속성입니다. 다음을 참조하세요 [플레이어의 커서 위치 확인](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#check-the-players-cursor-position).

커서 아래에 어떤 엔티티가 있는지 알아보려면 다음을 결합하세요 `worldRayDirection` 속성을 레이캐스트와 함께 사용하세요. 다음을 참조하세요 [레이캐스팅](/docs/creator/content-creator-ko/sdk7/interactivity/raycasting/).


> [!NOTE]
> **💡 팁**: 포인터 상태의 작동 예시는 다음을 참고하세요 [`0,5-primary-cursor-info`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,5-primary-cursor-info) 테스트 씬으로, 여기서는 다음을 읽고 `PrimaryPointerInfo` 매 프레임마다 읽어 다음 `worldRayDirection` 레이캐스트에; [`31,20-pointer-lock-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control), 여기서는 다음 값을 기록하여 커서 캡처를 요청하고 해제합니다 `PointerLock.isPointerLocked`; 그리고 [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), 여기서는 `screenDelta` 잠금 중에도 여전히 움직임을 보고하는 반면 `screenCoordinates` 은(는) 잠금 중 화면 중앙에 고정됩니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/mouse-movement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
