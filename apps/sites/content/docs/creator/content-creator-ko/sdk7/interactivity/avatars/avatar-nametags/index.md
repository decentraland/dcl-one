> 아바타 이름표 위에 사용자 지정 라벨을 표시하여 순위, 역할 또는 팀을 보여줍니다.

# 아바타 이름표

해당 `아바타 이름표` 이 컴포넌트는 아바타의 일반 이름표 위에 작은 판을 추가해, 씬에서 선택한 텍스트를 표시합니다. 순위, 역할, 팀 등 플레이어에 대해 씬이 추적하는 어떤 정보든 표시할 수 있으며, 예를 들어 "VIP", "Team Red", "Club Owner" 같은 텍스트를 넣을 수 있습니다.

<figure><img src="/docs/_remote/bb915fa6311eac2059c6e8e8.webp" alt="An avatar with a Club Owner plate floating above its regular nametag"><figcaption><p>플레이어의 일반 이름표 위에 씬이 제공하는 판</p></figcaption></figure>

이 페이지에서는 판을 추가하고, 스타일을 지정하고, 플레이어가 들어오고 나갈 때 이를 동기화하는 방법을 다룹니다.


> [!WARNING]
> **📔 참고**: `아바타 이름표` 필요 `@dcl/sdk` 버전 7.28.0 이상.


## 플레이어에게 판 추가하기

필수 필드는 `label`, 표시할 텍스트입니다.

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'

// 현재 플레이어의 이름표 위에 "클럽 소유자" 표시
AvatarNametag.create(engine.PlayerEntity, {
	label: '클럽 소유자',
})
```

이 판은 기본적으로 클라이언트 자체의 이름표 색상을 사용하므로, 별도의 작업 없이 인터페이스의 나머지 부분과 어울립니다.


> [!NOTE]
> **💡 팁**: 아바타가 로드되기를 기다린 다음 컴포넌트를 작성할 필요는 없습니다. 아바타가 아직 로드 중이라면 준비되는 즉시 판이 적용됩니다.


## 컴포넌트가 작동하는 위치

`아바타 이름표` 아바타가 있는 엔티티에서만 작동합니다:

* `engine.PlayerEntity`, 씬을 실행 중인 플레이어.
* 다른 플레이어의 엔티티로, 그 플레이어의 [`PlayerIdentityData`](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/) 컴포넌트도 부여해야 합니다.
* 하나의 [NPC 아바타](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/), 즉 다음이 있는 모든 엔티티: `AvatarShape` 컴포넌트도 부여해야 합니다.

다른 어떤 엔티티에서는 이 컴포넌트가 무시됩니다.


> [!WARNING]
> **📔 참고**: 이 판은 이를 그리는 클라이언트에 로컬입니다. 다른 플레이어에게는 전송되지 않습니다. 모든 사람이 같은 판을 보게 하려면, 각 클라이언트가 모두가 읽을 수 있는 데이터를 바탕으로 동일한 로직을 실행해야 합니다. 자세한 내용은 [멀티플레이어 씬의 판](#plates-in-multiplayer-scenes).


## 색상 변경하기

세 개의 선택적 필드는 다음 타입을 사용합니다: [Color3](/docs/creator/content-creator-ko/sdk7/3d/color-types/):

* `labelColor`: 텍스트 색상입니다. 기본값은 클라이언트의 기본 이름표 텍스트 색상입니다.
* `backgroundColor`: 판의 채우기 색상입니다. 기본값은 클라이언트의 기본 이름표 배경색입니다.
* `borderColor`: 판의 테두리입니다. 기본값은 `backgroundColor`, 따라서 설정하지 않으면 보이는 테두리가 없습니다.

`Color3` 알파 채널이 없고, 컴포넌트에도 불투명도 필드가 없으므로 판은 항상 완전히 불투명합니다.

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'
import { Color3 } from '@dcl/sdk/math'

// 짙은 파란색 판 위의 흰색 레이블, 연한 파란색 테두리
AvatarNametag.create(engine.PlayerEntity, {
	label: '학생',
	labelColor: Color3.White(),
	backgroundColor: Color3.create(0.1, 0.2, 0.6),
	borderColor: Color3.create(0.78, 0.85, 1),
})
```

필드의 기본 색상으로 돌아가려면, 그 필드 없이 컴포넌트를 다시 작성하세요. 다음을 사용하세요 `createOrReplace` 대신 `getMutable`, 전체 컴포넌트를 교체하는 것이 빠뜨린 필드를 지우는 방법이기 때문입니다:

```ts
// 레이블은 유지하고, 사용자 지정 색상은 제거하여 기본 모양으로 돌아가기
AvatarNametag.createOrReplace(engine.PlayerEntity, { label: '학생' })
```

## 판 제거하기

다음을 사용하세요 `deleteFrom` 를 사용해 판을 제거하세요. 아바타의 일반 이름표에는 영향을 주지 않습니다:

```ts
AvatarNametag.deleteFrom(engine.PlayerEntity)
```

## 레이블 동작

레이블 텍스트를 정하기 전에 알아두면 좋은 몇 가지 사항이 있습니다:

* **레이블은 한 줄입니다.** 절대 줄바꿈되지 않습니다.
* **긴 레이블은 잘립니다** 말줄임표와 함께. 레이블은 한두 단어로 유지하세요.
* **빈 레이블은** (`''`) 텍스트 없이 판을 그립니다.
* **공백은 유지됩니다**, 따라서 공백만으로 이루어진 레이블은 텍스트가 없는 판의 너비를 넓힙니다. 이는 단색 판의 크기를 조절하는 한 가지 방법입니다.
* **단어를 숨기고 너비는 유지하려면**, 다음을 설정하세요 `labelColor` 와 같은 값으로 `backgroundColor`.


> [!NOTE]
> **💡 팁**: 판에는 텍스트가 전혀 필요하지 않습니다. 예를 들어 팀 소속을 표시하기 위해 색상으로 구분된 판만 보여주려면, `label` 를 공백 문자열로 설정하고 `backgroundColor`. 공백이 많을수록 판은 더 넓어집니다.
> 
> ```ts
> AvatarNametag.createOrReplace(engine.PlayerEntity, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


## 플레이어가 도착할 때 태그 지정하기

다른 플레이어에게 태그를 지정하는 가장 간단한 방법은 `onEnterScene` 이벤트를 사용해, 플레이어가 씬에 들어올 때 컴포넌트를 붙이는 것입니다. 매번 새 엔티티를 넘겨주므로 조회하거나 캐시할 것이 없습니다:

```ts
import { onEnterScene } from '@dcl/sdk/players'
import { AvatarNametag } from '@dcl/sdk/ecs'

export function main() {
	onEnterScene((player) => {
		AvatarNametag.createOrReplace(player.entity, { label: 'VIP' })
	})
}
```

참조 [플레이어 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/) 플레이어가 들어오고 나갈 때 추적하는 방법에 대한 자세한 내용은

## 씬의 모든 플레이어에게 태그 지정하기

한 번에 모두에게 판을 표시하려면, 다음을 가진 엔티티들을 순회하세요 `PlayerIdentityData` 컴포넌트도 부여해야 합니다.

엔티티를 배열에 모으기 **하기 전에** 그 엔티티들에 쓰기 전에. 아직 `engine.getEntitiesWith()` 를 순회하는 동안 컴포넌트를 작성하면 쿼리가 도중에 무효화될 수 있습니다.

```ts
import { AvatarNametag, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

const LABELS = ['파란 팀', '빨간 팀']

function tagAllPlayers() {
	// 1. 먼저 모으기
	const players: { entity: Entity; address: string }[] = []
	for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
		players.push({ entity, address: identity.address.toLowerCase() })
	}

	// 주소로 정렬하면 모든 클라이언트가 동일한 결과에 도달합니다,
	// 플레이어가 어떤 순서로 들어왔는지와 무관하게
	players.sort((a, b) => a.address.localeCompare(b.address))

	// 2. 그런 다음 쓰되, 레이블이 실제로 바뀔 때만
	players.forEach(({ entity }, index) => {
		const label = LABELS[index % LABELS.length]
		if (AvatarNametag.getOrNull(entity)?.label === label) return
		AvatarNametag.createOrReplace(entity, { label })
	})
}
```

쓰기 전에 현재 값과 비교하면 매번 불필요한 컴포넌트 업데이트를 피할 수 있습니다.

### 플레이어의 입장과 퇴장을 따라가기

플레이어 엔티티 id는 **연결 해제 사이에 안정적이지 않습니다**. 플레이어가 나가면 엔티티 id가 재활용되어 다음에 들어오는 플레이어에게 할당될 수 있으므로, 오래된 엔티티에 쓴 판이 잘못된 아바타 위에 표시될 수 있습니다.

두 가지 규칙을 지키면 이 문제가 올바르게 처리됩니다:

* 쓰기 바로 직전에 엔티티를 다시 확인하세요 **매번** 엔티티를 캐시하지 말고.
* 플레이어가 나가면 컴포넌트를 제거하세요.

타이머로 태깅 함수를 다시 실행하면 나중에 들어온 플레이어를 잡아낼 수 있고, 오래된 엔티티를 붙잡고 있지 않게 됩니다. 1초 간격이면 충분합니다:

```ts
let timer = 0

engine.addSystem((dt: number) => {
	timer += dt
	if (timer < 1) return
	timer = 0
	tagAllPlayers()
})
```

## 멀티플레이어 씬의 판

판은 각 플레이어의 클라이언트에 로컬입니다. 판은 다른 플레이어에게 전송되지 않으며, 씬의 각 플레이어 복사본이 자체 판을 계산합니다. 모든 플레이어가 같은 태그를 보게 하려면, 태그를 할당하는 로직이 모두가 공유하는 데이터로부터 각 클라이언트에서 동일한 결과를 만들어야 합니다. 위 예시처럼 지갑 주소로 플레이어를 정렬하는 것은 그 방법 중 하나입니다. 그렇지 않다면 씬이 할당을 자체적으로 동기화해야 하며, 자세한 내용은 [서버 없는 멀티플레이어](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/).

## 숨겨진 이름표와의 상호작용

하나의 [`AvatarModifierArea`](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas/#hide-nametags) 와 함께 `AMT_HIDE_NAMETAGS` 모디파이어는 일반 이름표와 함께 이러한 판도 숨깁니다. 동일하게 `AMT_HIDE_AVATARS`, 아바타 전체를 숨깁니다.

아바타가 표시하도록 하려면 **오직** 판만 표시하고 아래에 이름은 표시하지 않게 하려면, `AvatarShape` 컴포넌트의 `이름` 를 빈 문자열로 설정하세요. 자세한 내용은 [NPC 아바타](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/#add-a-label-above-the-name).

## 관련 페이지

* [아바타 수정 영역](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas/): 씬의 한 영역 안에 있는 아바타와 이름표를 숨깁니다.
* [플레이어 이동하기](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/move-player/) 및 [아바타 애니메이션](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/): 플레이어의 아바타를 제어합니다.
* [NPC 아바타](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/): 플레이어가 아닌 아바타를 표시합니다.
* [플레이어 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/): 씬의 플레이어와 다른 모든 사람에 대한 데이터를 읽습니다.
* [색상 타입](/docs/creator/content-creator-ko/sdk7/3d/color-types/): 만드는 방법 `Color3` 값을 참조하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-nametags.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
