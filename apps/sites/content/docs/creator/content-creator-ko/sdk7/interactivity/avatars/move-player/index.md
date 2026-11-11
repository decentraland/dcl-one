> 플레이어를 순간이동시키거나, 씬 내의 새 위치로 걸어가게 합니다

# 플레이어 이동

씬에서 플레이어의 위치를 변경하려면 `movePlayerTo()` 함수를 사용하세요. 플레이어를 리스폰시키거나, 체크포인트를 설정하거나, 컷신의 일부로 플레이어가 어떤 지점까지 걸어가게 할 때 사용합니다.


> [!NOTE]
> **💡 팁**: 플레이어를 이동시키는 가장 쉬운 방법은 [Creator Hub의 씬 편집기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/). 코드 없이 **플레이어 이동** 또는 다음: **플레이어를 여기로 이동** Actions를 보려면 [모든 아이템을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).


이 함수는 다음 속성을 가진 객체를 사용합니다.

* `newRelativePosition`: 플레이어를 배치할 위치이며, Vector3로 표현됩니다.
* `cameraTarget`: (선택 사항) 카메라가 바라볼 방향이며, 바라볼 공간의 한 점의 좌표를 나타내는 Vector3로 표현됩니다. 값을 제공하지 않으면 카메라는 이동 전과 동일한 회전을 유지합니다.
* `avatarTarget`: (선택 사항) 아바타가 바라볼 방향이며, 바라볼 공간의 한 점의 좌표를 나타내는 Vector3로 표현됩니다. 값을 제공하지 않으면 아바타는 이동 전과 동일한 회전을 유지합니다. 플레이어가 1인칭 카메라 모드인 경우 카메라와 아바타의 회전은 같습니다.
* `duration`: (선택 사항) 전환에 걸릴 시간(초)입니다. 값을 제공하지 않으면 전환은 즉시 발생합니다. 지속 시간이 제공되면 아바타가 이 새 위치로 걸어가거나 달려갑니다.


> [!WARNING]
> **📔 참고**: 전환 중에는 아바타가 충돌체의 영향을 받지 않으므로, 물체를 통과할 수 있습니다.


```ts
import { movePlayerTo } from '~system/RestrictedActions'

// 엔티티 생성
const myEntity = engine.addEntity()
MeshRenderer.setBox(myEntity)
MeshCollider.setBox(myEntity)

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

// 엔티티에 동작 부여
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '클릭' },
	},
	function () {
		// 플레이어 리스폰
		movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
		})
	}
)
```

플레이어의 이동은 확인 화면이나 카메라 전환 없이 즉시 이루어집니다.


> [!WARNING]
> **📔 참고**: 플레이어는 이미 씬의 경계 안에 서 있는 경우에만 이동할 수 있으며, 씬 경계의 한계 안에 있는 위치로만 이동할 수 있습니다. 다음은 사용할 수 없습니다 `movePlayerTo()` 플레이어를 다른 씬으로 이동시키는 데 사용합니다. 플레이어를 다른 씬으로 이동하려면 다음을 참조하세요 [텔레포트](/docs/creator/content-creator-ko/sdk7/interactivity/external-links/#teleports).


다음 `movePlayerTo()` 함수는 await할 수 있으므로, 이동에 지속 시간이 있다면 다음을 사용할 수 있습니다 `await` 플레이어가 목적지에 도착할 때까지 기다릴 수 있습니다.

```ts
import { movePlayerTo } from '~system/RestrictedActions'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '클릭' },
	},
	async function () {
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})
	}
)
```


> [!WARNING]
> **📔 참고**: 다음이 없으면 `duration`, promise는 플레이어가 도착했을 때가 아니라 텔레포트가 대기열에 들어가자마자 resolve됩니다. 다음 이후의 코드는 `await` 실행되는 동안 `Transform.get(engine.PlayerEntity)` 여전히 이전 위치를 보고하며, 잠시 후 도착하는 이동 때문에 그곳에서 시작한 이모트가 중단될 수 있습니다. 도착 시 어떤 일이 일어나야 한다면, 대신 플레이어의 위치를 폴링하세요:
> 
> ```ts
> import { engine, Transform } from '@dcl/sdk/ecs'
> import { Vector3 } from '@dcl/sdk/math'
> import { movePlayerTo } from '~system/RestrictedActions'
> 
> function teleportThen(target: Vector3, onArrive: () => void) {
> 	void movePlayerTo({ newRelativePosition: target })
> 
> 	let waited = 0
> 	function waitForArrival(dt: number) {
> 		waited += dt
> 		const pos = Transform.getOrNull(engine.PlayerEntity)?.position
> 		// 높이 차이가 대기를 멈추게 하지 않도록 X와 Z만 비교합니다
> 		const arrived = pos !== undefined && Math.hypot(pos.x - target.x, pos.z - target.z) < 0.2
> 		if (!arrived && waited < 1.5) return // 1.5초 후 포기
> 		engine.removeSystem(waitForArrival)
> 		onArrive()
> 	}
> 
> 	engine.addSystem(waitForArrival)
> }
> ```
> 


플레이어가 전환이 진행되는 동안 이동을 시도하면, 전환이 중단되고 `movePlayerTo` 함수는 절대 resolve되지 않습니다. 전환이 진행되는 동안 다음을 사용하여 모든 입력을 비활성화하면 이 동작을 방지할 수 있습니다 `InputModifier` : 구성 요소를 사용합니다. 다음을 참조하세요 [이동 제한](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#restrict-locomotion).

```ts
import { movePlayerTo } from '~system/RestrictedActions'
import {InputModifier, engine} from '@dcl/sdk/ecs'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '클릭' },
	},
	async function () {

		// 모든 입력 비활성화
		InputModifier.create(engine.PlayerEntity, {
			mode: InputModifier.Mode.Standard({
				disableAll: true,
			}),
		})

		// 플레이어 이동
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})

		// 모든 입력 활성화
		InputModifier.deleteFrom(engine.PlayerEntity)
	}
)
```


> [!WARNING]
> **📔 참고**: 또한 await의 결과를 저장하여 그 `성공` 속성을 평가하여 이동이 중단되었는지(예: 입력 이동으로 인해) 또는 성공적으로 완료되었는지 확인할 수 있습니다.



> [!NOTE]
> **💡 팁**: 플레이어를 다른 씬이나 Genesis City 맵의 다른 좌표로 이동하려면 다음을 참조하세요 [텔레포트](/docs/creator/content-creator-ko/sdk7/interactivity/external-links/#teleports).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars/move-player.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
