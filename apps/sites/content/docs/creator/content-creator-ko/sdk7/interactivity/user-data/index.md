> 플레이어가 씬과 상호작용할 때 데이터를 가져오세요.

# 사용자 데이터

## 플레이어 위치와 회전

다음을 사용하세요 `PlayerEntity` 과 `CameraEntity` 플레이어의 위치와 회전을 알아보려면 해당 `Transform` 컴포넌트.

```ts
function getPlayerPosition() {
	if (!Transform.has(engine.PlayerEntity)) return
	if (!Transform.has(engine.CameraEntity)) return

	//플레이어 위치
	const playerPos = Transform.get(engine.PlayerEntity).position

	//플레이어 회전
	const playerRot = Transform.get(engine.PlayerEntity).rotation

	//카메라 위치
	const CameraPos = Transform.get(engine.CameraEntity).position

	//카메라 회전
	const CameraRot = Transform.get(engine.CameraEntity).rotation

	console.log('플레이어 위치: ', playerPos)
	console.log('플레이어 회전: ', playerRot)
	console.log('카메라 위치: ', CameraPos)
	console.log('카메라 회전: ', CameraRot)
}

engine.addSystem(getPlayerPosition)
```

* **PlayerEntity의 위치**: 아바타의 위치로, 가슴 높이입니다. 지면에서 약 0.88미터 위입니다.
* **PlayerEntity의 회전**: 아바타가 바라보는 방향이며, 쿼터니언으로 표현됩니다.
* **CameraEntity의 위치**:
  * 1인칭에서는: 아바타의 위치와 같지만 눈높이입니다. 지면에서 약 1.75미터 위입니다.
  * 3인칭에서는: 카메라 움직임에 따라 달라질 수 있습니다.
* **PlayerEntity의 회전**:
  * 1인칭에서는: 아바타가 바라보는 방향과 유사하며, 쿼터니언으로 표현됩니다. 플레이어의 회전과 약간 다르게 반올림될 수 있습니다.
  * 3인칭에서는: 카메라 움직임에 따라 달라질 수 있습니다.


> [!WARNING]
> **📔 참고**: 다음을 참조하는 것은 피하세요 `engine.PlayerEntity` 또는 다음: `engine.CameraEntity` 장면 초기 로딩 시에는, 엔티티가 아직 초기화되지 않았으면 오류가 발생할 수 있으므로 사용하지 마세요. 이 문제를 피하려면 다음 안에서 사용하세요 `main()` 함수, 또는 `main()`. 또한 동작을 async [`executeTask` 블록](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/#the-executetask-function).
> 
> 또 다른 방법은 시스템 안에서 이러한 엔티티를 참조하는 것입니다. 시스템의 첫 실행은 장면이 이미 제대로 초기화된 뒤에 호출되므로, 그곳에서는 항상 사용할 수 있습니다.


## 모든 플레이어 가져오기

장면의 모든 플레이어는 `Transform` 컴포넌트를 가집니다. 이 컴포넌트는 아바타에서 읽기 전용입니다. 모든 플레이어의 위치를 가져오려면, [다음이 있는 모든 엔티티를 순회하세요:](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/) 하나의 `PlayerIdentityData` 컴포넌트.

```ts
import { PlayerIdentityData } from '@dcl/sdk/ecs'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	console.log('플레이어 데이터: ', { entity, data, transform })
}
```

위 코드는 다음을 가진 모든 엔티티를 순회합니다: `Transform` 와 `PlayerIdentityData` 컴포넌트이며, 해당 데이터를 로그에 출력합니다. 이 동일한 방법을 사용해 모든 플레이어의 사용 가능한 데이터를 가져올 수 있습니다.

다음을 참조하세요 [이벤트 리스너](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-enters-or-leaves-scene) 새 플레이어가 장면에 들어올 때 이를 감지하고 반응하는 방법을 알아보세요.

## 플레이어 데이터 가져오기

다음을 사용하세요 `getPlayer()` 현재 플레이어 또는 장면의 다른 플레이어에 대한 데이터를 가져오려면

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('이름 : ', myPlayer.name)
		console.log('사용자 ID : ', myPlayer.userId)
	}
}
```

`getPlayer()` 다음과 같은 값을 반환합니다:

* `name`: *(문자열)* 다른 사람들이 월드에서 보는 플레이어의 사용자 이름
* `userId`: *(문자열)* 플레이어를 식별하는 문자열입니다. 지갑에 연결된 플레이어의 경우 지갑 주소를 소문자로 표기한 것입니다. 게스트 계정의 경우 로컬에서 생성된 식별자입니다.
* `isGuest`: *(불리언)* 플레이어가 공개 키를 가지고 있는지 나타냅니다. *참* 플레이어가 공개 키가 없는 게스트 계정인 경우.
* `위치`: *(Vector3)* 장면에서 아바타의 위치입니다.
* `avatar`: 플레이어의 기본 아바타와 외형에 대한 데이터를 담은 중첩 객체입니다.
* `wearables`: 플레이어가 현재 착용 중인 각 웨어러블의 식별자 배열입니다. 예를 들어 `urn:decentraland:off-chain:base-avatars:green_hoodie`. 모든 웨어러블은 NFT이더라도 비슷한 식별자를 가집니다.
* `emotes`: 플레이어가 현재 퀵 액세스 휠에 장착한 각 이모트의 식별자 배열입니다.
* `엔티티에`: 플레이어 엔티티에 대한 참조입니다. 다른 함수에 전달하거나 사용자 지정 컴포넌트를 추가할 때 유용합니다.

다음 `avatar` 객체에는 다음 중첩 정보가 있습니다:

* `bodyShapeUrn`: 아바타의 전반적인 몸체 형태를 나타내는 식별자입니다. 다음 중 하나입니다: `urn:decentraland:off-chain:base-avatars:BaseFemale` 여성용 또는 `urn:decentraland:off-chain:base-avatars:BaseMale` 남성용.
* `skinColor`: 플레이어의 피부색을 나타내는 `Color3`
* `eyesColor`: 플레이어의 눈 색상을 나타내는 `Color3`
* `hairColor`: 플레이어의 머리색을 나타내는 `Color3`
* `name`: 플레이어의 이름입니다.


> [!WARNING]
> **📔 참고**: 로드 시간에 따라 플레이어 데이터가 장면의 첫 프레임에 उपलब्ध하지 않을 수 있습니다. 데이터가 반환되었는지 확인하고, 그렇지 않다면 몇 밀리초 후에 다시 시도해야 합니다.


```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('게스트 여부: ', myPlayer.isGuest)
		console.log('이름 : ', myPlayer.name)
		console.log('사용자 ID : ', myPlayer.userId)
		console.log('아바타 형태 : ', myPlayer.position)
		console.log('아바타 형태 : ', myPlayer.avatar?.bodyShapeUrn)
		console.log('아바타 눈 색상 : ', myPlayer.avatar?.eyesColor)
		console.log('아바타 머리 색상 : ', myPlayer.avatar?.hairColor)
		console.log('착용 중인 웨어러블 : ', myPlayer.wearables)
		console.log('사용 가능한 이모트 : ', myPlayer.emotes)
	}
}
```

현재 플레이어와 다른 장면 내 특정 플레이어의 데이터를 가져오려면 다음을 실행하세요 `getPlayer()` 다음 속성을 가진 객체를 사용하여 `userId` 속성.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	let player = getPlayer({ userId: data.address })
	console.log('플레이어 : ', player?.name)
}
```

위 코드 조각은 다음을 가진 모든 엔티티를 순회합니다: `PlayerIdentityData` 컴포넌트를 대상으로 하며, 이는 장면의 모든 아바타 엔티티를 의미합니다. 그런 다음 해당 엔티티에 대해 `getPlayer()` 를 실행합니다.

`getPlayer()` 현재 같은 장면에 있는 플레이어의 데이터만 가져올 수 있습니다. 반드시 시야 범위 안에 있을 필요는 없지만, 같은 통신 아일랜드에 연결되어 있어야 합니다. 미리보기에서 시험해 보려면 두 번째 탭을 열어 다른 계정으로 로그인한 뒤, 두 플레이어가 모두 장면 안에 서 있게 하세요.


> [!WARNING]
> **📔 참고**: 사용자 ID는 항상 소문자여야 합니다. 지갑 주소를 복사할 때는 모든 문자가 소문자인지 확인하세요.


## 모든 플레이어의 데이터

어떤 플레이어의 정보든 얻으려면 [REST API 호출](/docs/creator/content-creator-ko/sdk7/networking/network-connections/#call-a-rest-api) 을 콘텐츠 서버로 보내세요.

이 정보는 다음 URL에서 제공되며, URL 매개변수에 플레이어의 사용자 ID를 덧붙입니다.

`https://peer.decentraland.org/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 팁**: 브라우저에서 URL을 열어 응답 구조를 확인해 보세요.


이 API에서 다음 정보를 사용할 수 있습니다:

* `displayName`: *(문자열)* 다른 사람들이 월드에서 보는 플레이어의 사용자 이름
* `userId`: *(문자열)* 플레이어를 식별하는 UUID 문자열입니다. 플레이어가 공개 키를 가지고 있다면 이 필드는 공개 키와 같은 값을 가집니다.
* `hasConnectedWeb3`: *(불리언)* 플레이어가 공개 키를 가지고 있는지 나타냅니다. *참* 플레이어가 공개 키를 가지고 있는 경우.
* `publicKey`: *(문자열)* 플레이어의 이더리움 지갑의 공개 키입니다. 플레이어가 연결된 지갑 없이 게스트로 로그인한 경우 이 필드는 `null`.
* `avatar`: 플레이어의 외형에 대한 데이터를 담은 중첩 객체입니다.
* `version`: *(숫자)* 플레이어가 설정을 변경할 때마다 1씩 증가하는 버전 번호입니다. 상충하는 데이터를 발견하면 어떤 버전이 더 최신인지 확인하는 데 이것을 사용하세요.


> [!WARNING]
> **📔 참고**: 플레이어와 관련된 모든 이더리움 거래에서는 항상 `publicKey` 필드 대신 `userId`, 존재하지 않는 지갑을 다루지 않도록 하세요.


다음 `avatar` 객체에는 다음 중첩 정보가 있습니다:

* `wearables`: `WearableId[]` 플레이어가 현재 착용 중인 각 웨어러블의 식별자 배열입니다. 예를 들어 `urn:decentraland:off-chain:base-avatars:green_hoodie`. 모든 웨어러블은 NFT이더라도 비슷한 식별자를 가집니다.
* `bodyShape`: 아바타의 전반적인 몸체 형태를 나타내는 식별자입니다. 다음 중 하나입니다: `urn:decentraland:off-chain:base-avatars:BaseFemale` 여성용 또는 `urn:decentraland:off-chain:base-avatars:BaseMale` 남성용.
* `skin`: 다음을 가진 객체입니다: `color` 필드로, 플레이어의 피부색을 다음 형식으로 담고 있습니다: `{ r, g, b, a }` 0부터 1까지의 숫자입니다.
* `hair`: 다음을 가진 객체입니다: `color` 필드로, 플레이어의 머리 색상을 담고 있습니다: `{ r, g, b, a }` 0부터 1까지의 숫자입니다.
* `eyes`: 다음을 가진 객체입니다: `color` 필드로, 플레이어의 눈 색상을 담고 있습니다: `{ r, g, b, a }` 0부터 1까지의 숫자입니다.
* `snapshots`: 다양한 해상도의 플레이어 .jpg 이미지를 base64로 표현한 중첩 객체입니다.
  * `face256`: *문자열* 플레이어의 얼굴을 256x256 픽셀 이미지로 나타낸 것입니다.
  * `body`: *문자열* 플레이어가 곧게 선 상태의 전체 해상도 이미지로, 512x1024 픽셀입니다.


> [!CAUTION]
> **❗경고** 아바타의 스냅샷은 향후 더 이상 사용되지 않으며 아바타 데이터의 일부로 반환되지 않습니다. 권장되는 방법은 `AvatarTexture` 를 대신 사용하는 것입니다. 자세한 내용은 [아바타 초상화](/docs/creator/content-creator-ko/sdk7/3d/materials/#avatar-portraits).


와 달리 `getPlayer()`이 옵션은 현재 같은 장면에 있는 플레이어뿐 아니라 같은 서버의 플레이어로도 제한되지 않습니다. 이 방법을 사용하면 과거에 서버에 로그인한 적이 있는 모든 플레이어의 데이터를 가져올 수 있습니다.

조회하려는 플레이어가 어느 서버에 연결되어 있는지 알고 있다면, 해당 특정 서버로 요청을 보내 더 최신 데이터를 얻을 수 있습니다. 예를 들어 플레이어가 옷을 갈아입으면 이 정보는 플레이어의 서버에서 즉시 उपलब्ध하지만, 다음으로 전파되기까지는 보통 몇 분 정도 걸립니다 `peer.decentraland.org` 서버.

`https://<player server>/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 팁**: 현재 플레이어의 서버는 다음을 호출하여 얻을 수 있습니다: `getRealm()` 그리고 다음을 읽으면 됩니다: `realmInfo.baseUrl` 응답의 필드입니다.


이 예제는 다음을 결합합니다: `myProfile.userId` 및 `getRealm()` 플레이어가 있는 서버에서 직접 플레이어 데이터를 가져옵니다:

```ts
import { getRealm } from '~system/Runtime'
import { myProfile } from '@dcl/sdk/network'

async function fetchPlayerData() {
	const { realmInfo } = await getRealm({})
	if (!realmInfo) return

	const url = `${realmInfo.baseUrl}/lambdas/profile/${myProfile.userId}`
	console.log('사용 중인 URL: ', url)

	try {
		const response = await fetch(url)
		const json = await response.json()

		console.log('전체 응답: ', json)
		console.log('플레이어가 착용 중인 것: ', json.avatars[0].avatar.wearables)
	} catch {
		console.log('플레이어 데이터를 가져오는 중 오류가 발생했습니다')
	}
}

fetchPlayerData()
```

## 플레이어 데이터 컴포넌트

다음을 사용하는 대신 `getPlayer()`각 플레이어 엔티티에 데이터를 저장하는 여러 컴포넌트에서 직접 데이터를 읽을 수 있습니다. 다음 컴포넌트가 있습니다:

* `PlayerIdentityData`: 플레이어 주소와 `isGuest` 게스트 계정을 표시하는 속성을 저장합니다.
* `AvatarBase`: 기본 아바타에 대한 데이터를 저장합니다. 다음이 포함됩니다:
  * `name`: 플레이어의 이름입니다.
  * `bodyShapeUrn`: 남성 또는 여성 바디 유형에 해당하는 ID입니다.
  * `skinColor`: 플레이어의 피부색을 나타내는 `Color3`
  * `eyesColor`: 플레이어의 눈 색상을 나타내는 `Color3`
  * `hairColor`: 플레이어의 머리색을 나타내는 `Color3`
  * `talking`: *true* 플레이어가 음성 채팅으로 말하고 있을 때, *false* 플레이어가 조용할 때, *undefined* 클라이언트가 해당 플레이어에 대한 음성 활동이 없다고 보고할 때 사용됩니다. 자세한 내용은 [플레이어가 말하고 있는지 감지하기](#detect-when-a-player-is-speaking).
* `AvatarEquippedData`: 장착된 웨어러블과 이모트 목록입니다.
  * `wearableUrns`: 플레이어가 현재 착용 중인 착용 아이템 목록입니다.
  * `emoteUrns`: 플레이어가 현재 퀵 액세스 휠에 장착한 이모트 목록입니다.
* `AvatarEmoteCommand`: 플레이어가 현재 재생 중인 이모트에 대한 정보입니다. 다음이 포함됩니다:
  * `emoteUrn`: 플레이어가 장면에 들어온 이후 마지막으로 재생한 이모트의 URN입니다
  * `loop`: 이모트가 반복 재생 중이면 참입니다
  * `timestamp`: 이 이모트가 트리거된 시각입니다

```ts
for (const [entity, data, base, attach, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	AvatarBase,
	AvatarEquippedData,
	Transform
)) {
	console.log('플레이어 데이터: ', { entity, data, transform, base, attach })
}
```


> [!WARNING]
> **📔 참고**: 이러한 컴포넌트는 모두 읽기 전용입니다. 장면에서 값을 변경할 수 없습니다.


## 플레이어가 말하고 있는지 감지하기

읽기 `talking` 다음의 `AvatarBase` 컴포넌트를 사용하여 지금 누군가 음성 채팅을 하고 있는지 확인하세요. 이는 장면을 실행 중인 플레이어와 주변의 모든 다른 플레이어에게 작동합니다.

```ts
import { AvatarBase, engine } from '@dcl/sdk/ecs'

// 로컬 플레이어가 말하기 시작하거나 멈출 때마다 로그를 출력
AvatarBase.onChange(engine.PlayerEntity, (base) => {
	console.log('발화 중: ', base?.talking)
})
```

`talking` 두 가지가 아니라 세 가지 상태가 있습니다:

| 값           | 의미                                                     |
| ----------- | ------------------------------------------------------ |
| `true`      | 플레이어가 지금 말하고 있습니다.                                     |
| `false`     | 플레이어가 말하고 있지 않습니다.                                     |
| `undefined` | 예를 들어 주변 음성 채팅이 꺼져 있어 클라이언트가 이 플레이어의 음성 활동을 보고하지 않습니다. |

세 번째 상태가 중요합니다: `undefined` "데이터 없음"을 의미하며, "무음"이 아닙니다. 어떤 비-`true` 값이든 조용한 것으로 취급하지 말고, 이를 명시적으로 확인하세요.

```ts
import { AvatarBase, engine, PlayerIdentityData } from '@dcl/sdk/ecs'

function whoIsSpeaking() {
	for (const [player, base] of engine.getEntitiesWith(AvatarBase)) {
		if (base.talking === undefined) continue // 이 플레이어에 대한 음성 데이터 없음
		if (base.talking) console.log(`${base.name} is speaking`)
	}
}

engine.addSystem(whoIsSpeaking)
```

말하는 플레이어의 머리 위에 마커를 표시하려면 다음과 함께 부착하세요 [`AvatarAttach`](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#attach-to-other-players) 다음의 주소를 사용하여 `PlayerIdentityData`. 엔티티를 플레이어 엔티티의 자식으로 만들지 마세요. 그렇게 하면 작동하지 않으며 마커가 장면 원점에 놓입니다.


> [!WARNING]
> **📔 참고**: `AvatarBase.talking` 실제 음성 채팅 활동을 보고하며, 여기에 쓸 수 없습니다. 이것은 다음과는 다른 것입니다: `AvatarShape.talking`, 장면이 다음 옆에 녹색 음성 막대를 그리기 위해 설정하는 플래그입니다 [NPC 아바타](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/) 를 생성한 경우입니다.


## 포터블 경험 가져오기

포터블 경험은 본질적으로 토지 구획에 제약되지 않는 장면입니다. 플레이어는 Decentraland 어디든 이를 가지고 다닐 수 있으며, 세계 위에 새로운 콘텐츠 층을 추가합니다. 스마트 웨어러블은 포터블 경험의 예입니다. 경쟁 게임에서 치트로 간주될 수 있는 능력을 플레이어에게 부여하는 스마트 웨어러블을 착용했는지 알고 싶을 수 있습니다. 예를 들어 플랫폼 게임에서 제트팩을 착용한 플레이어는 다른 사람들보다 매우 불공정한 이점을 가집니다.

장면 제작자로서, 포터블 경험을 착용한 플레이어가 장면에서 할 수 있는 일을 제한하고 싶을 수 있습니다. 사용하세요 `getPortableExperiencesLoaded()` 플레이어에게 현재 활성화된 포터블 경험이 있는지 확인합니다.

```ts
import { getPortableExperiencesLoaded } from '~system/PortableExperiences'

executeTask(async () => {
	let portableExperiences = await getPortableExperiencesLoaded({})
	console.log(portableExperiences.loaded)
})
```

`getPortableExperiencesLoaded()` 객체 배열을 반환하며, 각 객체에는 다음이 포함됩니다: `id` 속성이 있습니다. 웨어러블의 경우 id는 해당 웨어러블의 URN입니다.

## 플레이어의 웨어러블에 대한 상세 정보 가져오기

다음 `getPlayer()` 함수는 각 웨어러블의 정보 없이 웨어러블 ID 목록만 반환합니다. 특정 카테고리(예: 모자)나 특정 희귀도(예: Mythic)의 웨어러블이 있는지 확인하고 싶다면, 플레이어 웨어러블에 대한 더 자세한 정보를 가져와야 합니다.

다음을 수행하세요 [REST API 호출](/docs/creator/content-creator-ko/sdk7/networking/network-connections/#call-a-rest-api) 다음 URL로 요청을 보내 현재 사용할 수 있는 모든 웨어러블의 최신 전체 목록과 각 항목의 세부 정보를 가져옵니다.

`${playerRealm.realmInfo.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`


> [!WARNING]
> **📔 참고**: 이 URL을 구성하려면 realm을 얻어야 합니다(아마 다음을 사용하여 `getRealm()`)와 플레이어의 ID를 얻어야 합니다(아마 다음을 사용하여 `getPlayer()`)


이 기능은 플레이어 정보 가져오기와 함께 사용할 수 있습니다. 예를 들어 플레이어가 할로윈 컬렉션의 웨어러블을 착용했거나 다음 희귀도의 웨어러블을 착용한 경우에만 장소에 प्रवेश할 수 있게 할 수 있습니다 *전설* 희귀도.


> [!NOTE]
> **💡 팁**: 브라우저에서 URL을 열어 응답 구조를 확인해 보세요.


```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { getRealm } from '~system/Runtime'

async function fetchWearablesData() {
	try {
		let userData = getPlayer()
		const realm = await getRealm({})
		if (!userData || !realm.realmInfo) return

		const url =
			`${realm.realmInfo?.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`.toString()
		console.log('사용 중인 URL: ', url)

		let response = await fetch(url)
		let json = await response.json()

		console.log('전체 응답: ', json)
	} catch {
		console.log('웨어러블 데이터를 가져오는 중 오류가 발생했습니다')
	}
}

executeTask(fetchWearablesData)
```


> [!NOTE]
> **💡 팁**: 다음에서 특정 웨어러블에 대한 더 많은 정보를 가져올 수 있습니다 [다음 API](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/searchWearables).


## 플레이어의 카메라 모드 확인

플레이어는 Decentraland를 탐험할 때 1인칭 또는 3인칭 카메라를 사용할 수 있습니다. 다음 값의 정보를 확인하여 플레이어가 어떤 모드를 사용하는지 확인하세요 `CameraMode` 컴포넌트 `engine.CameraEntity` 엔티티에 사용하세요.

```ts
function checkCameraMode() {
	if (!CameraMode.has(engine.CameraEntity)) return

	let cameraEntity = CameraMode.get(engine.CameraEntity)

	if (cameraEntity.mode == CameraType.CT_THIRD_PERSON) {
		console.log('플레이어가 3인칭 카메라를 사용 중입니다')
	} else {
		console.log('플레이어가 1인칭 카메라를 사용 중입니다')
	}
}

engine.addSystem(checkCameraMode)
```


> [!WARNING]
> **📔 참고**: 카메라 정보는 장면을 실행 중인 현재 플레이어에 대해서만 사용할 수 있습니다. 다른 플레이어의 카메라 데이터는 조회할 수 없습니다.


카메라 모드는 다음의 값을 사용합니다: `CameraType` 열거형입니다. 가능한 값은 다음과 같습니다:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

다음 `CameraMode` 컴포넌트 `engine.CameraEntity` 는 읽기 전용이므로, 이를 통해 플레이어의 카메라 모드를 강제로 변경할 수는 없습니다.


> [!NOTE]
> **💡 팁**: 플레이어의 카메라 모드를 변경하려면 다음을 사용하세요 [카메라 수정 영역](/docs/creator/content-creator-ko/sdk7/3d/camera/#1st-and-3rd-person-camera-modes).


카메라 모드를 아는 것은 장면의 메커니즘을 미세 조정하여 이 모드에서 더 편안한 환경에 맞추는 데 매우 유용합니다. 예를 들어 3인칭에서는 작은 대상을 클릭하기가 더 어렵습니다.


> [!WARNING]
> **📔 참고**: 다음을 참조하는 것은 피하세요 `engine.CameraEntity` 장면 초기 로딩 시에는, 엔티티가 아직 초기화되지 않았으면 오류가 발생할 수 있으므로 사용하지 마세요. 이 문제를 피하려면 다음 안에서 사용하세요 `main()` 함수, 또는 `main()`. 또한 동작을 async [`executeTask` 블록](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/#the-executetask-function).
> 
> 또 다른 방법은 시스템 안에서 이 엔티티를 참조하는 것입니다. 시스템의 첫 실행은 장면이 이미 제대로 초기화된 뒤에 호출되므로, 이 엔티티는 항상 사용할 수 있습니다.


## 플레이어의 커서가 고정되어 있는지 확인

플레이어는 두 가지 커서 모드를 전환할 수 있습니다: *잠긴 커서* 카메라를 제어하는 모드 또는 *잠금 해제된 커서* UI 위에서 커서를 자유롭게 움직이기 위한 모드입니다.

플레이어는 다음을 클릭하거나 *마우스 오른쪽 버튼* 또는 다음을 눌러 커서를 잠금 해제합니다: *Esc* 키를 누른 뒤, 화면의 아무 곳이나 클릭하면 커서를 다시 잠급니다.

다음을 확인하세요: `PointerLock` 장면의 [카메라 엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#reserved-entities) 를 확인하여 현재 커서 모드가 무엇인지 알아보세요.

```ts
export function main() {
	const isLocked = PointerLock.get(engine.CameraEntity).isPointerLocked
	console.log(isLocked)
}
```

다음을 참조하세요 [이벤트 리스너](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-locks-or-unlocks-cursor) 를 통해 커서 상태 변화에 쉽게 반응하는 방법을 확인하세요.

다음을 작성하여 플레이어의 커서 상태를 강제로 설정할 수도 있습니다 `isPointerLocked` 의 필드 `PointerLock` 컴포넌트가 있는 `engine.CameraEntity`. 다음을 참조하세요: [커서 잠금 또는 잠금 해제](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor) 의 예시를 참고하세요.


> [!WARNING]
> **📔 참고**: 다음을 참조하는 것은 피하세요 `engine.CameraEntity` 장면 초기 로딩 시에는, 엔티티가 아직 초기화되지 않았으면 오류가 발생할 수 있으므로 사용하지 마세요. 이 문제를 피하려면 다음 안에서 사용하세요 `main()` 함수, 또는 `main()`. 또한 동작을 async [`executeTask` 블록](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/#the-executetask-function).
> 
> 또 다른 방법은 시스템 안에서 엔티티를 참조하는 것입니다. 시스템의 첫 실행은 장면이 이미 제대로 초기화된 뒤에 호출되므로, 이 엔티티는 항상 사용할 수 있습니다.


## 플레이어의 커서 위치 확인

다음을 사용하세요 `PrimaryPointerInfo` 컴포넌트를 `engine.RootEntity` 를 사용해 플레이어의 커서 위치를 가져옵니다. 이는 드래그 앤 드롭 상호작용, 스와이프 제스처 등의 메커니즘에 사용할 수 있습니다.

```ts
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

function CursorSystem() {
	const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
	console.log(pointerInfo)
}

engine.addSystem(CursorSystem)
```


> [!WARNING]
> **📔 참고**: 다음을 참조하는 것은 피하세요 `engine.RootEntity` 장면 초기 로딩 시에는, 엔티티가 아직 초기화되지 않았으면 오류가 발생할 수 있으므로 사용하지 마세요. 이 문제를 피하려면 항상 시스템 안에서 엔티티를 참조하세요. 시스템의 첫 실행은 장면이 이미 제대로 초기화된 뒤에 호출되므로, 이 엔티티는 항상 사용할 수 있습니다.


다음 `PrimaryPointerInfo` 컴포넌트는 다음 속성을 가진 객체를 반환합니다:

* `screenCoordinates`: *(Vector2)* 장면에서 커서의 위치를 픽셀 단위로 나타낸 것입니다. 원점은 화면 왼쪽 상단 모서리이며, `y` 아래 방향으로 증가합니다 — 다음과 같은 좌표 공간입니다: `UiTransform` 및 `UiCanvasInformation`. 커서가 고정되어 있으면 화면 중앙을 보고합니다.
* `screenDelta`: *(Vector2)* 마지막 프레임 이후 커서 위치의 변화량을 픽셀 단위로 나타낸 것입니다. 자세한 내용과 예시는 [마우스 이동](/docs/creator/content-creator-ko/sdk7/interactivity/mouse-movement/) 을 참고하세요.
* `worldRayDirection`: *(Vector3)* 카메라에서 커서로 향하는 레이의 방향을 나타내는 벡터입니다. 원점은 카메라 위치입니다. 이를 사용해 월드에서 커서의 위치를 계산하세요.
* `pointerType`: 0은 `없음`, 1은 `마우스`


> [!NOTE]
> **💡 팁**: 다른 속성과 달리, `screenDelta` 커서가 [고정되어 있어도 마우스 이동을 계속 보고합니다.](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor). 따라서 드래그 제스처나 사용자 지정 카메라 제어 같은 실시간 상호작용에 적합합니다. 자세한 내용은 [마우스 이동](/docs/creator/content-creator-ko/sdk7/interactivity/mouse-movement/).



> [!NOTE]
> **팁:** UI 요소에서 간단한 호버 이벤트에 반응하려면, 다음을 사용하는 것이 더 쉬울 수 있습니다. `onMouseEnter` 및 `onMouseLeave` 이벤트를 보려면 [UI 버튼 이벤트](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_button_events/#hover-feedback).


다음 `PrimaryPointerInfo` 구성 요소는 읽기 전용이므로 플레이어가 커서 위치를 변경하도록 강제할 수 없습니다.

다음 예제는 UI 요소에 커서 위치를 표시하는 방법을 보여줍니다.

***ui.tsx 파일:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import {cursorXpos, cursorYpos} from './index'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `커서 위치: `+  cursorXpos + `,` + cursorYpos, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***index.ts 파일:***

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
  ReactEcsRenderer.setUiRenderer(uiMenu)
}

export let cursorXpos: number | undefined = undefined
export let cursorYpos: number | undefined = undefined

function CursorSystem() {
  const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
  console.log(pointerInfo)

  cursorXpos = pointerInfo.screenCoordinates?.x
  cursorYpos = pointerInfo.screenCoordinates?.y
}

engine.addSystem(CursorSystem)
```

다음을 사용할 수 있습니다 `worldRayDirection` 를 설정하려면 `direction` 레이캐스트의 필드를 사용하여 엔티티가 커서의 시선 안에 있는지 확인합니다. 다음을 참조하세요 [레이캐스팅](/docs/creator/content-creator-ko/sdk7/interactivity/raycasting/) 에서 자세한 내용을 확인하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/user-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
