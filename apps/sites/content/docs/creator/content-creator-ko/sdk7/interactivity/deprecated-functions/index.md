> 레거시 함수

# 더 이상 사용되지 않는 함수

다음 함수들은 모두 레거시이므로 사용을 피해야 합니다. 아직은 작동하지만, 앞으로는 지원이 중단될 수 있습니다. 이 모든 예제에는 동일한 정보를 얻거나 같은 결과를 달성하는 대체 방법에 대한 링크가 포함되어 있습니다.

## 플레이어가 씬에 들어오거나 나감


> [!WARNING]
> **📔 참고**: `onEnterSceneObservable` 및 `onLeaveSceneObservable` 이벤트는 SDK 7.x에서 지원 중단되었습니다. 대신 `onEnterScene` 를 사용하세요. 자세한 내용은 [플레이어가 씬에 들어오거나 나감](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-enters-or-leaves-scene).


아바타가 씬을 구성하는 토지 구획 안으로 들어오거나 나가거나, 또는 텔레포트로 들어오거나 나갈 때마다, 이를 감지할 수 있는 이벤트가 생성됩니다. 이 이벤트는 플레이어의 아바타를 포함한 모든 아바타에 의해 트리거됩니다.

```ts
import {
	onEnterSceneObservable,
	onLeaveSceneObservable,
} from '@dcl/sdk/observables'

onEnterSceneObservable.add((player) => {
	console.log('player entered scene: ', player.userId)
})

onLeaveSceneObservable.add((player) => {
	console.log('player left scene: ', player.userId)
})
```


> [!WARNING]
> **📔 참고**: 이 이벤트는 현재 로컬에서 렌더링 중인 플레이어에게만 반응합니다. 씬 크기가 시야 범위를 초과하는 큰 씬에서는 반대편 구석으로 들어오는 플레이어가 등록되지 않을 수 있습니다. 지역의 플레이어 수가 Decentraland 서버의 섬(island) 처리 한도를 초과하면, 같은 섬을 공유하지 않는 플레이어는 보이지 않으며 이 이벤트에서도 추적되지 않습니다.


해당 옵저버블 `onEnterScene` 및 `onLeaveScene` 가 `'@dcl/sdk/observables'` 에서 가져온 경우도 지원 중단되었습니다. 올바른 함수는 이름은 같지만, 다음에서 가져와야 합니다. `'@dcl/sdk/src/players'` 대신 사용합니다. 아래 예제에서는 지원 중단된 버전과 올바른 버전을 차례로 모두 볼 수 있습니다.

```ts
// 지원 중단됨 - observables에서 가져옴
import { onEnterScene, onLeaveScene } from '@dcl/sdk/observables'

onEnterScene.add((player) => {
	console.log('player entered scene: ', player.userId)
})

onLeaveScene.add((player) => {
	console.log('player left scene: ', player.userId)
})

// 현재 버전 - 플레이어 함수로 가져옴
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('ENTERED SCENE', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('LEFT SCENE', userId)
	})
}
```

## 플레이어가 연결되거나 연결 해제됨


> [!WARNING]
> **📔 참고**: `getConnectedPlayers` 함수와 `onPlayerConnectedObservable` 및 `onPlayerDisconnectedObservable` 이벤트는 SDK 7.x에서 지원 중단되었습니다. 대신 `onEnterScene` 를 사용하세요. 자세한 내용은 [플레이어가 씬에 들어오거나 나감](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-enters-or-leaves-scene). 이제 각 씬은 별도의 통신 섬이므로, 연결되어 있는 것과 같은 씬에 있는 것은 동일하게 취급됩니다.


다음에서 현재 연결된 모든 플레이어의 전체 목록을 가져오세요. `getConnectedPlayers`.

```ts
import { getConnectedPlayers } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getConnectedPlayers({})
	connectedPlayers.players.forEach((player) => {
		console.log('player was already here: ', player.userId)
	})
})
```

다른 플레이어가 로컬 엔진에 의해 렌더링되기 시작하거나 중지될 때마다, 이를 감지할 수 있는 이벤트가 생성됩니다. 플레이어는 당신과 같은 씬 위에 있을 수도 있고 아닐 수도 있지만, 시야 범위 안에는 있어야 합니다(반드시 눈에 보여야 하는 것은 아님). `onPlayerConnectedObservable` 플레이어가 근처에 새로 연결되거나 시야 범위 안으로 들어올 때를 감지하고, 마찬가지로 `onPlayerDisconnectedObservable` 플레이어가 세션을 종료하거나 멀리 걸어 나갈 때를 감지합니다.

```ts
import {
	onPlayerConnectedObservable,
	onPlayerDisconnectedObservable,
} from '@dcl/sdk/observables'

onPlayerConnectedObservable.add((player) => {
	console.log('player entered: ', player.userId)
})

onPlayerDisconnectedObservable.add((player) => {
	console.log('player left: ', player.userId)
})
```

다른 플레이어들이 플레이어가 씬을 로드하기 전에 이미 주변에서 렌더링되고 있었다면, 이 이벤트는 새로 로드된 씬에 이미 존재하는 플레이어들을 알려주지 않는다는 점에 유의하세요. 현재 모든 플레이어를 추적해야 한다면, 씬 로딩 시 기존 플레이어를 조회한 뒤 이 이벤트를 들어 업데이트를 받을 수 있습니다.

## 씬 내 모든 플레이어 조회


> [!WARNING]
> **📔 참고**: `getPlayersInScene` 함수는 SDK 7.x에서 지원 중단되었습니다. 대신 `PlayerIdentityData` 컴포넌트를 사용하여 모든 플레이어를 순회하세요. 자세한 내용은 [모든 플레이어 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#fetch-all-players).


다음 함수를 호출하여 현재 씬에 있고 렌더링 중인 모든 플레이어의 전체 목록을 가져올 수도 있습니다. `getPlayersInScene()`.

```ts
import { getPlayersInScene } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getPlayersInScene({})
	connectedPlayers.players.forEach((player) => {
		console.log('player was already here: ', player.userId)
	})
})
```

## 플레이어가 애니메이션을 재생함


> [!WARNING]
> **📔 참고**: `onPlayerExpressionObservable` 이벤트는 SDK 7.x에서 지원 중단되었습니다. 대신 `AvatarEmoteCommand` 컴포넌트를 사용하세요. 자세한 내용은 [플레이어가 애니메이션을 재생함](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-plays-animation).


플레이어가 이모트(댄스, 박수, 손 흔들기 등)를 할 때마다 이 이벤트를 감지할 수 있습니다.

```ts
import { onPlayerExpressionObservable } from '@dcl/sdk/observables'

onPlayerExpressionObservable.add(({ expressionId }) => {
	console.log('Expression: ', expressionId)
})
```

이 이벤트에는 다음 정보가 포함됩니다.

* expressionId: 수행된 이모트의 이름(예: *손 흔들기*, *박수*, *키스*)


> [!WARNING]
> **📔 참고**: 이 이벤트는 플레이어가 이모트를 할 때마다, 그리고 씬이 로드되어 있을 때 트리거됩니다. 이때 플레이어는 근처의 다른 씬에 서 있을 수도 있습니다.


## 플레이어가 프로필을 변경함


> [!WARNING]
> **📔 참고**: `onProfileChanged` 이벤트는 SDK 7.x에서 지원 중단되었습니다. 대신 `AvatarEquippedData` 컴포넌트를 사용하세요. 자세한 내용은 [플레이어가 프로필을 변경함](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-changes-profile).


플레이어가 프로필에 변경을 가할 때마다 `onProfileChanged` 이벤트가 호출됩니다. 이러한 변경에는 다른 착용 아이템 착용, 이름 변경, 설명 변경, 포터블 체험 활성화 등이 포함될 수 있습니다.

```ts
import { onProfileChanged } from '@dcl/sdk/observables'

onProfileChanged.add((profileData) => {
	console.log('Own profile data is ', profileData)
})
```

이벤트 데이터에는 catalyst 서버 기준으로 플레이어 ID와 해당 아바타 프로필의 버전 번호만 포함됩니다. 변경 사항이 전파될 때마다 버전 번호는 1씩 증가합니다.

이 이벤트가 트리거되면 다음 함수를 사용하여 [getUserData()](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data) 이 정보의 최신 버전(플레이어가 착용한 아이템 목록 포함)을 가져올 수 있습니다. 다음을 호출하기 전에 약간의 지연을 추가해야 할 수도 있습니다. `getUserData()` 함수가 반환하는 버전이 최신인지 확인하세요.


> [!NOTE]
> **💡 팁**: 레거시 웹 익스플로러로 미리보기에서 테스트할 때, 무작위 아바타를 사용하지 않으려면 Metamask 지갑이 연결된 브라우저에서 씬을 실행하세요.



> [!WARNING]
> **📔 참고**: 이 이벤트는 현재 플레이어의 변경 사항에 의해서만 트리거되며, 주변의 다른 플레이어 프로필 변경에 의해서는 트리거되지 않습니다.


## 씬 로딩 완료


> [!WARNING]
> **📔 참고**: `onSceneReadyObservable` 이벤트는 SDK v7.x에서 지원 중단되었습니다. 이 함수는 더 이상 관련이 없습니다. 씬 로딩이 끝난 뒤에 실행되도록 하려면 다음 안에서 실행하면 됩니다. `Main()` 함수. 자세한 내용은 [씬 생명주기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#scene-lifecycle)


씬 로딩이 완료되면 `onSceneReadyObservable` 가 호출됩니다. 이는 플레이어가 바로 씬 안으로 로드되든, 다른 곳에서 씬으로 걸어오든 모두 동작합니다. 무거운 모델 등을 포함한 씬의 모든 콘텐츠의 초기 로드가 완료되면 이 이벤트가 호출됩니다.

```ts
import { onSceneReadyObservable } from '@dcl/sdk/observables'

onSceneReadyObservable.add(() => {
	console.log('SCENE LOADED')
})
```

## 지원 중단된 플레이어 데이터 메서드


> [!WARNING]
> **📔 참고**: `getUserData()` 및 `getPlayerData()` 함수는 SDK v7.4.x에서 지원 중단되었습니다. 대신 `getPlayer()` 를 사용하세요. 자세한 내용은 [사용자 데이터](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data).


씬을 실행 중인 현재 플레이어의 정보를 얻으려면 `getUserData()`.

아래 예제는 다음을 가져옵니다. `~system/UserIdentity` 네임스페이스와 다음을 실행합니다. `getUserData()`.

```ts
import { getUserData } from '~system/UserIdentity'

executeTask(async () => {
	let userData = await getUserData({})
	console.log(userData.data)
})
```

다음 함수를 호출하면 근처에 있는 다른 플레이어의 데이터를 가져올 수도 있습니다. `getPlayerData()`에 Decentraland 계정 ID를 전달합니다.

```ts
import { getPlayerData } from '~system/Players'

executeTask(async () => {
	let userData = await getPlayerData({ userId: '0x….' })
	console.log(userData)
})
```

둘 다 `getUserData()` 및 `getPlayerData()` 콘텐츠 API를 통해 사용할 수 있는 것과 동일한 데이터 구조를 반환합니다. 자세한 내용은 [모든 플레이어의 데이터](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#data-from-any-player)

`getPlayerData()` 현재 근처에 있는 플레이어의 데이터만 가져올 수 있습니다. 반드시 같은 씬에 있어야 하는 것은 아니지만 시야 범위 안에 있어야 합니다. 이는 이 정보가 해당 아바타를 렌더링하는 로컬 엔진에서 가져오기 때문입니다. 미리보기에서 시험해 보려면 두 번째 탭을 열고 다른 계정으로 로그인하세요.


> [!WARNING]
> **📔 참고**: 사용자 ID는 항상 소문자여야 합니다. 지갑 주소를 복사할 경우 모든 문자가 소문자인지 확인하세요.


해당 `getUserPublicKey()` 및 `getUserAccount()` 함수도 또한 지원 중단되었습니다. 다음을 사용하세요. `getPlayer()` 를 사용하세요. 자세한 내용은 [사용자 데이터](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data).

## 하늘 상자 시간 가져오기


> [!WARNING]
> **📔 참고**: `getDecentralandTime()` 함수는 SDK v7.x.x에서 지원 중단되었습니다. 대신 `getWorldTime()` 를 사용하세요. 자세한 내용은 [Decentraland 시간 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/runtime-data/#get-decentraland-time).


```ts
import { getDecentralandTime } from '~system/EnvironmentApi'

executeTask(async () => {
	let time = await getDecentralandTime({})
	console.log(time)
})
```

## 영역 가져오기


> [!WARNING]
> **📔 참고**: `getCurrentRealm()` 함수는 SDK v7.x.x에서 지원 중단되었습니다. 대신 `getRealm()` 를 사용하세요. 자세한 내용은 [영역 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/runtime-data/#get-realm-data).


```ts
import { getCurrentRealm } from '~system/EnvironmentApi'

async function fetchPlayerData() {
	const playerRealm = await getCurrentRealm({})

	console.log(playerRealm.currentRealm?.domain)
}

fetchPlayerData()
```

## 미리보기 모드인지 여부


> [!WARNING]
> **📔 참고**: `isPreviewMode()` 함수는 SDK v7.x.x에서 지원 중단되었습니다. 대신 `getRealm()` 대신 다음을 사용하세요. 여기에는 `미리보기` 속성이 포함되어 있습니다. 자세한 내용은 [영역 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/runtime-data/#get-realm-data).


```ts
import { isPreviewMode } from '~system/EnvironmentApi'

executeTask(async () => {
	const { isPreview } = await isPreviewMode({})

	if (isPreview) {
		console.log('Running in preview')
	}
})
```

## 플레이어가 다른 플레이어를 클릭함

플레이어가 다른 플레이어를 클릭할 때마다 이벤트를 감지할 수 있습니다.

```ts
import { onPlayerClickedObservable } from '@dcl/sdk/observables'

onPlayerClickedObservable.add((clickEvent) => {
	console.log('Clicked ', clickEvent.userId, ' details: ', clickEvent.ray)
})
```


> [!WARNING]
> **📔 참고**: `onPlayerClickedObservable` 이벤트는 SDK v7.x에서 지원 중단되었습니다. 향후 버전에서는 보다 [데이터 지향적 접근 방식](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/)을 허용할 예정이며, 이벤트 대신 정기적인 데이터 조회를 기반으로 합니다.
> 
> 대안으로 플레이어에 보이지 않는 콜라이더를 붙이고 이에 대한 클릭을 감지할 수 있습니다.



> [!WARNING]
> **📔 참고**: 클릭하는 플레이어와 클릭당하는 플레이어 둘 다 씬의 토지 구획 안에 서 있어야 합니다. 이 리스너는 다른 플레이어가 수행한 클릭이 아니라 현재 플레이어가 다른 플레이어를 클릭하는 이벤트만 감지합니다.


이 이벤트에는 다음 데이터가 포함됩니다.

* `userId`: 클릭된 플레이어의 ID
* `ray`: 클릭으로 추적된 레이 정보
  * `direction`: *Vector3* 클릭의 시작점에서 히트 지점까지의 방향을 나타내는 정규화된 Vector3입니다.
  * `distance`: *number* 시작점에서 히트 지점까지의 거리(미터)입니다.
  * `origin`: *Vector3* 클릭의 시작점으로, 클릭한 플레이어의 위치이며 씬을 기준으로 합니다.


> [!NOTE]
> **💡 팁**: 다른 플레이어를 클릭할 때의 기본 동작은 플레이어 여권을 여는 것이며, 여기에서 해당 플레이어에 대한 추가 정보를 보거나 친구로 추가할 수 있습니다. 추가하려는 경험을 방해하지 않도록 이 UI가 열리지 않게 하려면 [아바타 수정 영역](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas/).


## 플레이어가 커서를 잠그거나 잠금을 해제함


> [!WARNING]
> **📔 참고**: `onPointerLockedStateChange` 이벤트는 SDK v7.x에서 제거되어 더 이상 사용할 수 없습니다. 자세한 내용은 [이벤트 리스너](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-locks-or-unlocks-cursor) 에서 현재 대체 방법을 확인하세요.


플레이어는 두 가지 커서 모드 사이를 전환할 수 있습니다. *잠긴 커서* 모드로 카메라를 제어하거나 *잠금 해제된 커서* 모드로 UI 위를 자유롭게 이동할 수 있습니다.

플레이어는 다음을 클릭하여 커서를 잠금 해제할 수 있습니다. *마우스 오른쪽 버튼* 또는 *Esc* 키를 누르고, 화면의 아무 곳이나 클릭하면 커서를 다시 잠글 수 있습니다.

플레이어가 이 두 모드 사이를 전환할 때마다 반응하려면 `onChange` 함수를 `PointerLock` 컴포넌트에서 사용하세요. 자세한 내용은 [이벤트 리스너](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-locks-or-unlocks-cursor).

## 플레이어가 영역이나 섬을 변경함

Decentraland의 플레이어는 서로 분리된 *영역*과 각 영역 내의 분리된 *섬* 에 존재합니다. 서로 다른 영역이나 섬의 플레이어는 같은 구획 위에 서 있어도 서로를 볼 수 없고, 상호작용하거나 채팅할 수도 없습니다.

플레이어가 영역이나 섬을 변경할 때마다 `onRealmChangedObservable` 이벤트가 호출됩니다.

```ts
import { onRealmChangedObservable } from '@dcl/sdk/observables'

onRealmChangedObservable.add((realmChange) => {
	console.log('PLAYER CHANGED ISLAND TO ', realmChange.room)
})
```


> [!WARNING]
> **📔 참고**: `onRealmChangedObservable` 이벤트는 SDK v7.x에서 지원 중단되었습니다. 향후 버전에서는 보다 [데이터 지향적 접근 방식](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/)을 허용할 예정이며, 이벤트 대신 정기적인 데이터 조회를 기반으로 합니다.


이 이벤트에는 다음 필드가 포함됩니다.

* **serverName**: *string*; catalyst 서버 이름입니다.
* **room**: *string*; 섬 이름입니다.
* **displayName**: *string*; catalyst 서버 이름 뒤에 *-* 와 섬 이름이 이어집니다. 예를 들어 `unicorn-x011`.
* **domain**: *string*; 사용 중인 catalyst 서버의 URL입니다.

플레이어가 지도를 이동할 때, 현재 가장 가까운 플레이어들과 함께 그룹화되도록 섬이 바뀔 수 있습니다. 섬은 각 섬에 관리 가능한 규모의 사람들을 맞추기 위해 경계도 동적으로 이동합니다. 따라서 플레이어가 가만히 서 있어도 주변 씬에 다른 플레이어가 들어오고 나가면 섬이 변경될 수 있습니다.

씬이 [3rd party server](/docs/creator/content-creator-ko/sdk7/networking/third-party-servers/) 에 의존하여 플레이어 간 변경 사항을 실시간으로 동기화한다면, 같은 realm+island로 묶인 플레이어끼리만 데이터를 공유하도록 할 수 있습니다. 따라서 플레이어가 섬을 바꿀 때마다 서드파티 서버의 방도 함께 변경하는 것이 좋습니다.

## 암호화 함수


> [!WARNING]
> **📔 참고**: 함수 `requirePayment()`, `signMessage()`, `convertMessageToObject()` 는 지원 중단되었습니다. 대신 `sendAsync()` 함수를 사용하세요. 자세한 내용은 [씬 블록체인 작업](/docs/creator/content-creator-ko/sdk7/blockchain/scene-blockchain-operations/)을 참고하세요. 이러한 함수로 흔히 쓰는 몇 가지 사용 사례를 단순화하는 데 도움이 되는 라이브러리도 있습니다.


## 비디오 이벤트


> [!WARNING]
> **📔 참고**: `onVideoEvent` 이벤트는 SDK v7.x에서 지원 중단되었습니다. 자세한 내용은 [이벤트 리스너](/docs/creator/content-creator-ko/sdk7/media/video-playing/#video-events) 에서 지원 중단되지 않은 대체 방법을 확인하세요.


비디오의 재생 상태가 변경되면 `onVideoEvent` 옵저버블이 이벤트를 수신합니다.

```ts
onVideoEvent.add((data) => {
	console.log('New Video Event ', data)
})
```

비디오 이벤트 입력에는 다음 속성이 포함됩니다.

* `videoClipId` ( *string*): 상태가 변경된 엔티티의 ID입니다.
* `componentId` (*string*): 상태가 변경된 엔티티의 ID입니다.
* `currentOffset` (*number*): 비디오의 `seek` 속성의 현재 값입니다. 이 값은 비디오의 원래 시작 시점 이후의 초를 나타냅니다. *-1* 기본값으로.
* `totalVideoLength` (*number* ): 전체 비디오의 길이(초)입니다. *-1* 길이를 알 수 없는 경우.
* `videoStatus`: 새 비디오 상태 값은 `VideoTexture`의 값이며, 다음 `VideoStatus` 열거형의 값으로 표현됩니다. 이 열거형에는 다음과 같은 가능한 값이 있습니다.
* `VideoStatus.NONE` = 0,
* `VideoStatus.ERROR` = 1,
* `VideoStatus.LOADING` = 2,
* `VideoStatus.READY` = 3,
* `VideoStatus.PLAYING` = 4,
* `VideoStatus.BUFFERING` = 5


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/deprecated-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
