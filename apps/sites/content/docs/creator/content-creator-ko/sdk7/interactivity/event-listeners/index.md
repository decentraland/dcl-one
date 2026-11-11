> 플레이어 행동 및 씬 변경과 관련되어 씬이 추적할 수 있는 이벤트.

# 이벤트 리스너

장면이 구독할 수 있는 여러 이벤트가 있으며, 장면 안에 있거나 가까이 있을 때 플레이어의 동작을 알 수 있습니다.

플레이어가 수행하는 버튼 및 클릭 이벤트는 다음을 참조하세요 [버튼 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/).

## 플레이어가 장면에 들어오거나 나감

아바타가 장면을 구성하는 토지 구획 안팎으로 들어오거나 나가거나, 또는 텔레포트로 들어오거나 나갈 때마다 이를 들을 수 있는 이벤트가 생성됩니다.

이 이벤트는 플레이어 자신의 아바타를 포함한 모든 아바타에 의해 발생합니다.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('장면에 들어옴', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('장면에서 나감', userId)
	})
}
```

에서 `onEnterScene` 이벤트에서는 함수가 다음에서 반환된 모든 데이터에 접근할 수 있습니다: [플레이어 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data) 를 통해 `player` 속성.  `onLeaveScene` 이벤트에서는 함수가 플레이어 ID에만 접근할 수 있습니다.

### 현재 플레이어만

다른 아바타가 주변에 있더라도, 트리거된 이벤트를 필터링해서 플레이어의 아바타에만 반응하도록 할 수 있습니다.

```ts
import { getPlayer, onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	let myPlayer = getPlayer()

	onEnterScene((player) => {
		if (!player) return
		console.log('장면에 들어옴', player)

		if (myPlayer && player.userId == myPlayer.userId) {
			console.log('내가 들어옴')
		}
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('장면에서 나감', userId)

		if (myPlayer && userId == myPlayer.userId) {
			console.log('내가 나감')
		}
	})
}
```

이 예제는 먼저 플레이어의 ID를 가져온 다음, 이벤트를 구독하고 이벤트에서 반환된  `userId` 를 플레이어의 ID와 비교합니다.

### 장면 내 모든 플레이어 조회

다음과 같이 `PlayerIdentityData` 컴포넌트를 가진 모든 엔티티를 순회하여 현재 장면에 있는 전체 플레이어 목록을 살펴보세요.

```ts
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'

export function main() {
	for (const [entity, data, transform] of engine.getEntitiesWith(
		PlayerIdentityData,
		Transform
	)) {
		console.log('플레이어: ', { entity, data, transform })
	}
}
```

## 플레이어가 카메라 모드를 변경함

카메라 모드를 알면 이 모드에서 더 편안한 방식에 맞춰 장면의 메커니즘을 미세 조정하는 데 매우 유용할 수 있습니다. 예를 들어, 3인칭에서는 작은 목표물을 클릭하기가 더 어렵습니다.

다음 코드 조각은 `onChange` 함수를 사용해 카메라가 바뀔 때마다 이벤트를 발생시킵니다. 또한 장면이 로드될 때 플레이어의 초기 카메라 모드와 함께 이벤트를 발생시킵니다.

```ts
import { engine, CameraMode } from '@dcl/sdk/ecs'

export function main() {
	CameraMode.onChange(engine.CameraEntity, (cameraComponent) => {
		if (!cameraComponent) return
		console.log('카메라 모드가 변경됨', cameraComponent?.mode)
		// 0 = 1인칭
		// 1 = 3인칭
	})
}
```

다음을 참조하세요 [플레이어의 카메라 모드 확인](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#check-the-players-camera-mode).

## 플레이어가 애니메이션을 재생함

다음을 사용하세요 `onChange` 함수를 `AvatarEmoteCommand` 컴포넌트에서 플레이어가 이모트를 재생할 때마다 이벤트를 발생시킵니다. 여기에는 기본 이모트(춤, 박수, 손 흔들기 등)와 토큰 이모트가 모두 포함됩니다.

```ts
import { AvatarEmoteCommand } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return
		console.log('재생된 이모트: ', emote.emoteUrn)
	})
}
```

이 이벤트에는 다음 정보가 포함됩니다:

* `emoteUrn`: 수행된 이모트의 이름(예: *wave*, *clap*, *kiss*)
* `loop`: 이모트가 반복 재생 중인지 또는 한 번만 재생되는지
* `timestamp`: 이모트가 트리거된 시점.
* `state`: 값은  `EmoteState` 열거형의 라이프사이클 이벤트 설명 값입니다: `ES_STARTED` (이모트가 시작됨, 그리고 필드가 없을 때의 값이기도 합니다), `ES_FINISHED` (반복 재생하지 않는 이모트가 끝까지 재생됨), 또는 `ES_INTERRUPTED` (이모트가 이동, 텔레포트, 다른 이모트, 명시적 중지, 또는 장면 이탈로 인해 중간에 끊김). 다음을 참조하세요 [이모트가 끝나는 시점 감지](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#detect-when-an-emote-finishes).

다른 플레이어 대신 `engine.PlayerEntity`.


> [!NOTE]
> **💡 팁**: 이모트 재생 이벤트의 작동 예시는 다음의 [`4,23-emote-finish`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/4,23-emote-finish) 테스트 장면을 참조하세요. 이 장면은 플레이어 엔티티에 추가된 모든 항목을 다음과 같이 기록합니다: `AvatarEmoteCommand` STARTED `STARTED`, `FINISHED` 또는 `INTERRUPTED` — 이모트를 끝까지 재생하여 `FINISHED`를 확인하거나, 재생 중간에 걸어 나가서 `INTERRUPTED`.


## 플레이어가 프로필을 변경함

다음을 사용하세요 `onChange` 함수를 `AvatarEquippedData` 컴포넌트를 사용해 플레이어가 착용 아이템 중 하나를 변경할 때마다, 또는 퀵 액세스 휠에 나열된 이모트를 변경할 때마다 이벤트를 발생시킵니다. 마찬가지로 `onChange` 함수를 `AvatarBase` 를 사용해 플레이어가 머리색, 피부색, 아바타 형태, 이름 등 기본 아바타 속성을 변경할 때마다 이벤트를 발생시킵니다.

```ts
import { AvatarEquippedData, AvatarBase } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('새 착용 아이템 목록: ', equipped.wearableUrns)
		console.log('새 이모트 목록 : ', equipped.emoteUrns)
	})

	AvatarBase.onChange(engine.PlayerEntity, (body) => {
		if (!body) return
		console.log('새 눈 색상: ', body.eyesColor)
		console.log('새 피부색: ', body.skinColor)
		console.log('새 머리색: ', body.hairColor)
		console.log('새 바디 URN: ', body.bodyShapeUrn)
	})
}
```

이 이벤트 `AvatarEquippedData` 에는 다음 정보가 포함됩니다:

* `wearableUrns`: 플레이어가 현재 장착 중인 착용 아이템 목록.
* `emoteUrns`: 플레이어가 현재 퀵 액세스 휠에 장착한 이모트 목록.

이 이벤트 `AvatarBase` 에는 다음 정보가 포함됩니다:

* `name`: 플레이어의 이름.
* `bodyShapeUrn`: 남성 또는 여성 바디 유형에 해당하는 ID.
* `skinColor`: 플레이어 피부색,  `Color3`
* `eyesColor`: 플레이어 눈 색상,  `Color3`
* `hairColor`: 플레이어 머리색,  `Color3`
* `talking`: *true* 플레이어가 음성 채팅으로 말하고 있을 때 *false* 조용할 때 `undefined` 클라이언트가 음성 활동을 보고하지 않을 때.

왜냐하면 `talking` 에 존재하기 때문에 `AvatarBase`이 콜백은 플레이어가 말하기를 시작하거나 멈출 때마다도 발생합니다. 외형만 중요하다면 모든 변경에 반응하기보다 필요한 필드를 비교하세요. 다음을 참조하세요 [플레이어가 말하고 있는지 감지](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#detect-when-a-player-is-speaking).

다른 플레이어의 착용 아이템이나 아바타 변경도 감지할 수 있으며, 다음 대신 다른 플레이어에 대한 참조를 간단히 전달하면 됩니다 `engine.PlayerEntity`.

다른 플레이어의 프로필 변경도 감지할 수 있으며, 다음 대신 다른 플레이어에 대한 참조를 간단히 전달하면 됩니다 `engine.PlayerEntity`.

## 플레이어가 커서를 잠그거나 잠금 해제함

플레이어는 두 가지 커서 모드 사이를 전환할 수 있습니다: *잠긴 커서* 모드로 카메라를 제어하거나 *잠금 해제된 커서* 모드로 커서를 UI 위에서 자유롭게 이동할 수 있습니다.

플레이어는 *오른쪽 마우스 버튼* 을 클릭하거나 *Esc* 키를 눌러 커서를 잠금 해제하고, 화면의 아무 곳이나 클릭하여 다시 잠급니다.

다음을 추가하세요 `PointerLock` 컴포넌트를 `engine.CameraEntity` 엔티티에 추가하고, `onChange` 함수를 `PointerLock` 컴포넌트를 사용해 플레이어가 두 커서 모드 사이를 변경할 때마다 이벤트를 발생시킵니다.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    // 커서 모드에 대한 힌트 표시
		   }
	})
}
```

이 정보를 사용해 플레이어에게 은근히 알려줄 수 있습니다. 예를 들어, 이 게임은 잠금 해제된 커서로 플레이하는 것이 더 좋다는 UI 팝업을 보여주는 식입니다. 또는 `isPointerLocked` 를 컴포넌트에서 변경해 플레이어의 커서 모드를 강제로 바꿀 수도 있습니다. 다음 예시는 항상 커서 모드를 잠금 해제로 설정합니다:

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

## 탐색기 패널 열기

장면은 `openExplorerUi` 제한된 액션을 사용해 탐색기의 기본 제공 패널(지도, 설정, 배낭 등)을 열 수 있습니다. 이 중 대부분은 화면 전체를 차지하지만, 모두가 그런 것은 아닙니다. 이는 장면의 UI에서 플레이어를 탐색기 특정 기능으로 안내하는 데 유용합니다.

플레이어가 이를 요청해야 합니다.  `openExplorerUi` 은 클릭 핸들러 안에서 호출해야 하며, 단독으로 호출하면 안 됩니다:

```ts
// 플레이어가 큐브를 클릭하면 지도를 엽니다
import { openExplorerUi } from '~system/RestrictedActions'
import { engine, ExplorerUi, InputAction, MeshCollider, MeshRenderer, pointerEventsSystem, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const mapButton = engine.addEntity()
Transform.create(mapButton, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(mapButton)
MeshCollider.setBox(mapButton)

pointerEventsSystem.onPointerDown(
	{ entity: mapButton, opts: { button: InputAction.IA_POINTER, hoverText: '지도 열기' } },
	() => {
		void openExplorerUi({ ui: ExplorerUi.EU_MAP })
	}
)
```

다음 `ExplorerUi` 열거형에는 다음 값이 있습니다:

* `EU_SETTINGS` (0)
* `EU_MAP` (1)
* `EU_BACKPACK` (2)
* `EU_CAMERA_REEL` (3)
* `EU_COMMUNITIES` (4)
* `EU_PLACES` (5)
* `EU_EVENTS` (6)
* `EU_ITEM_PURCHASE` (7): 아이템 구매 확인 모달. 현재 탐색기는 이에 대해  `REJECTED_FEATURE_DISABLED` 를 반환합니다.


> [!WARNING]
> **참고:** 다른 제한된 액션(`movePlayerTo`, `triggerEmote`), `openExplorerUi` 와 마찬가지로, 플레이어가 장면의 구획 안에 서 있을 때만 작동합니다.


### 패널을 열 수 있는 것은 플레이어 동작뿐입니다

패널은 호출이 플레이어가 방금 한 일에 대한 직접적인 응답일 때만 열립니다. 그렇지 않은 호출은 거부됩니다.

**작동함:**

* 3D 엔티티를 클릭하기,  `pointerEventsSystem.onPointerDown` 또는 `pointerEventsSystem.onPointerUp`.
* 장면 UI의 요소를 클릭하기,  `onMouseDown`.
* 활성화 버튼 중 하나를 누르거나 놓기,  `E`, `F` 또는 `1` 부터 `4`다음으로 감지됨: `inputSystem.isTriggered` 및 `PointerEventType.PET_DOWN` 또는 `PointerEventType.PET_UP` (`IA_PRIMARY`, `IA_SECONDARY`, `IA_ACTION_3` 부터 `IA_ACTION_6`).

**작동하지 않음:**

* 에서 호출 `main()`이므로 패널은 로드 시 플레이어에게 인사할 수 없습니다.
* 타이머에서 호출하거나, 또는 플레이어 동작의 유효 시간이 지난 후에 호출하기.
* 다음 이후에 호출하기 `await` 와 같이 시간이 걸리는 작업에 대해 `fetch` 나 다른 제한된 액션을 수행한 뒤 호출하기. 플레이어 동작은 약 한 프레임 정도만 유효합니다. 먼저 작업을 수행한 다음 패널을 여세요.
* 호버링. `PET_HOVER_ENTER` 및 `PET_HOVER_LEAVE` 는 플레이어 동작이 아닙니다.
* 이동 키. 걷기, 달리기, 점프는 포함되지 않으며, 위에 나열된 활성화 버튼만 해당됩니다.

`openExplorerUi` 가 반환하는 `openResult` 를 통해 다음을 알려줍니다:

| 값                                | 의미                                                        |
| -------------------------------- | --------------------------------------------------------- |
| `UNSPECIFIED` (0)                | 결과가 보고되지 않았습니다.                                           |
| `OPENED` (1)                     | 패널이 열렸습니다.                                                |
| `WAS_ALREADY_OPEN` (2)           | 다른 패널이 이미 화면에 있었습니다. 패널은 하나의 슬롯을 공유하므로 한 번에 하나만 열 수 있습니다. |
| `REJECTED_NOT_CURRENT_SCENE` (3) | 플레이어가 당신의 장면에 서 있지 않습니다.                                  |
| `REJECTED_FEATURE_DISABLED` (4)  | 탐색기에서 해당 패널을 꺼 두었거나,  `ui` 값이 탐색기가 아는 패널이 아닙니다.           |
| `REJECTED_NO_USER_GESTURE` (5)   | 호출이 플레이어 동작에 대한 응답이 아니었습니다.                               |


> [!WARNING]
> **참고:** 아무것도 열리지 않고 오류도 보이지 않는다면,  `openResult`. `REJECTED_NO_USER_GESTURE` (5)는 호출이 플레이어 동작 밖에서 실행되었음을 뜻합니다. 보통 이는 `main()`타이머 안에 있거나, 또는 다음 이후에 있기 때문입니다 `await`.


`WAS_ALREADY_OPEN` 이것은 단지 장면이 두 번 호출했다는 뜻만은 아닙니다. 패널들은 모두 같은 슬롯을 차지하므로, 그중 하나가 화면에 떠 있으면 다른 어떤 것도 열 수 없고, 플레이어가 직전에 장면과 무관하게 직접 열었을 수도 있습니다. 이를 코드의 버그가 아니라 정상적인 결과로 취급하세요. 아래에 설명된 `openExplorerUiAndWait()`와 함께라면, 동일한 상황은 `notOpened` 결과로 나타납니다.

### 플레이어가 패널을 닫을 때까지 기다리기

`openExplorerUiAndWait()` 패널을 열고, 플레이어가 닫을 때까지 한 번 해결되는 promise를 돌려줍니다. 장면이 플레이어가 끝낼 때까지 이어져야 할 때 사용하세요. 예를 들어, 플레이어를 배낭으로 보내 착용 아이템을 입히게 한 뒤처럼요.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// 배낭을 연 다음, 플레이어가 닫으면 계속 진행
async function openBackpack() {
	try {
		const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_BACKPACK })

		if (outcome.$case === 'closed') {
			console.log('플레이어가 배낭을 닫았습니다')
		}
	} catch (error) {
		console.log('대기 실패:', error)
	}
}
```

다음을 호출하세요 `openBackpack()` 를 클릭 핸들러에서, 동일하게 `openExplorerUi`. `await` 는 패널 요청 후 함수 안에서 발생하므로, 플레이어 동작은 여전히 유효합니다.

돌려받는 객체에는 `$case` 필드가 있으며, 대기가 어떻게 끝났는지 알려줍니다:

* `closed`: 플레이어가 장면이 연 패널을 닫았습니다. 이것이 정상적인 결과입니다.
* `notOpened`: 아무것도 열리지 않았습니다.  `openResult` 필드는 위 표의 값을 사용해 그 이유를 알려줍니다.
* `timedOut`: 설정한 시간 제한이 만료되었지만 패널은 아직 열려 있었습니다.
* `matched`:  `까지` 조건이 대기를 조기에 끝냈으므로, 패널이 아직 화면에 남아 있을 수 있습니다.

패널을 열 수 없을 때, promise는 즉시 다음으로 해결됩니다 `notOpened`. 장면은 존재하지 않는 패널을 기다리며 멈춰 있지 않습니다.

이 네 가지 결과가 모든 일반적인 종료를 포괄하지만, 프로미스는 여전히 거부될 수 있습니다. Explorer에 대한 요청이 완전히 실패할 수 있고, 두 채널이 같은 이름으로 등록될 수 있으며, 또는 도우미가 진행 중인 어떤 호출에도 귀속시킬 수 없는 이벤트가 도착할 수 있습니다. 그래서 위의 예시는 다음으로 감싸져 있습니다: `try`/`catch`. 하나의 `async` 포인터 핸들러에서 호출된 함수는 어디에도 보고되지 않으므로, 없이는 `catch` 기다림은 그냥 돌아오지 않고, 아무도 그 이유를 알려주지 않습니다.


> [!WARNING]
> **📔 참고**: `openExplorerUiAndWait()` 필요합니다 `@dcl/sdk` 버전 7.30.0 이상이 필요합니다. 이는 다음에서 제공됩니다: `@dcl/sdk/explorer-ui` 모듈이며, 다음도 다시 내보냅니다: `ExplorerUi`.


#### 시간 제한 설정

기본적으로 시간 제한은 없습니다. 플레이어가 패널을 몇 분 동안 열어 둔 채 떠날 수 있으며, 프로미스는 그 내내 대기 상태로 남아 있습니다. 다음을 전달하세요: `timeoutMs` 장면이 어쨌든 다음 단계로 넘어가야 할 때.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// 지도가 아직 열려 있든 아니든 30초 후 기다림을 중단
async function openMap() {
	const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_MAP }, { timeoutMs: 30000 })

	if (outcome.$case === 'timedOut') {
		console.log('플레이어는 아직 지도를 보고 있습니다')
	}
}
```

#### 대신 이벤트에서 중단

다음 `까지` 옵션은 채널에 이벤트가 도착하는 즉시 기다림을 끝내고, 다음으로 해결합니다: `matched` 대신 `closed`. 채널은 같은 모듈에서 내보내집니다: `ExplorerUiEvents` 패널 열기 및 닫기 이벤트용이며, `ItemPurchase` 구매 흐름의 결과용입니다. 같은 채널을 다음에 추가하세요: `collect` 옵션에 이벤트 자체도 원한다면, 그것들은 결과의 다음에 도착합니다: `events` 배열.

채널만 단독으로 전달하면 그 채널의 어떤 이벤트에서든 중단되고, 또는 다음 안에 감싸면 `variant()` 한 종류의 이벤트에서만 중단합니다:

```ts
import { ExplorerUi, ExplorerUiEvents, openExplorerUiAndWait, variant } from '@dcl/sdk/explorer-ui'

// 플레이어가 닫을 때까지 기다리지 않고, 배낭이 화면에 보이자마자 계속 진행
async function openBackpack() {
	const outcome = await openExplorerUiAndWait(
		{ ui: ExplorerUi.EU_BACKPACK },
		{ until: variant(ExplorerUiEvents, 'opened'), collect: [ExplorerUiEvents] }
	)

	if (outcome.$case === 'matched') {
		console.log('배낭이 열려 있습니다. 수집된', outcome.events.length, '개의 이벤트')
	}
}
```

다음 `$case` 값 `variant()` 받아들일 수 있는 값은 채널 이벤트가 전달하는 값입니다: `opened` 및 `closed` 에서 `ExplorerUiEvents`.

### 패널이 열리거나 닫힐 때를 관찰하세요

다음을 사용하세요 `ExplorerUiEventsResult` 컴포넌트로, 자신의 장면이 연 패널을 추적합니다. 이는 다음 위의 증가 전용 컴포넌트입니다: `engine.RootEntity`, 다음과 비슷합니다 `PointerEventsResult`. 장면이 열어 둔 각 패널은 `openExplorerUi` 나타날 때 하나의 이벤트를 추가하고, 사라질 때 또 하나를 추가합니다. 플레이어가 다음으로 닫을 때도 포함됩니다: *Esc*.


> [!WARNING]
> **참고:** 이것은 Explorer UI에 대한 장면 전체 보기로서는 아닙니다. 플레이어가 Explorer의 자체 메뉴에서 직접 연 패널은 여기서 어떤 이벤트도 생성하지 않습니다. 다음을 한 번도 호출하지 않는 장면은 `openExplorerUi` 하나도 보지 못합니다.


`get()` 지금까지 누적된 전체 이벤트 집합을 넘겨주며, 최대 100개의 최신 항목까지 포함하지만, 마지막 프레임 이후 도착한 것들만 주는 것은 아닙니다. 이를 무작정 반복하는 시스템은 매 틱마다 과거의 모든 이벤트에 반응합니다. 각 이벤트의 다음을 비교하세요: `timestamp` 마지막으로 처리한 틱과:

```ts
import { engine, ExplorerUiEventsResult, ExplorerUi } from '@dcl/sdk/ecs'

let lastHandledTick = -1

engine.addSystem(() => {
	let newestTick = lastHandledTick

	for (const event of ExplorerUiEventsResult.get(engine.RootEntity)) {
		// 집합에는 모든 이벤트가 유지되므로 이미 처리한 틱은 건너뜁니다
		if (event.timestamp <= lastHandledTick) continue
		if (event.timestamp > newestTick) newestTick = event.timestamp

		if (event.event?.$case === 'opened') {
			console.log('패널이 열림:', event.ui)
		}
		if (event.event?.$case === 'closed') {
			console.log('패널이 닫힘:', event.ui)
		}

		// 어떤 특정 패널인지 확인
		if (event.ui === ExplorerUi.EU_MAP && event.event?.$case === 'closed') {
			console.log('플레이어가 지도를 닫았습니다')
		}
	}

	// 루프가 끝난 뒤에만 하므로, 같은 틱의 여러 이벤트가 모두 처리됩니다
	lastHandledTick = newestTick
})
```

`ExplorerUiEventsResult.onChange()` 에서 `engine.RootEntity` 는 스트림을 읽는 또 다른 방법입니다. 콜백이 새 이벤트마다 한 번씩 실행되므로, 별도 상태 관리를 맞게 유지할 필요가 없습니다.

각 이벤트에는 다음이 포함됩니다:

* `ui`: 이벤트가 어떤 패널에 관한 것인지(하나의 `ExplorerUi` 값).
* `timestamp`: 이벤트가 발생한 장면 틱입니다.
* `requestId`: 장면이 다음 호출에 전달한 ID입니다: `openExplorerUi` 이 패널을 연 호출의 ID이거나 `0` 호출에서 아무것도 전달하지 않았다면 그렇습니다. 진행 중인 여러 호출을 구분하는 데 사용하세요.
* `event`: 다음 중 하나입니다 `{ $case: 'opened' }` 또는 `{ $case: 'closed' }`.


> [!NOTE]
> **💡 팁**: 위의 모든 항목을 실제 예제로 보려면 다음을 보세요: [`80,-6-explorer-ui-events`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/80,-6-explorer-ui-events) 테스트 장면입니다. 모든 판정과 모든 이벤트를 자체 HUD에 출력하며, 제스처 게이트, 이미 열려 있는 패널, `requestId` 상관관계와 다음의 각 결과를 다룹니다 `openExplorerUiAndWait()`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/event-listeners.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
