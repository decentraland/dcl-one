> 씬의 특정 지역 안에 있는 플레이어에 대해 아바타를 숨기거나, 이름표를 숨기거나, 패스포트를 비활성화합니다

# 아바타 수정 영역

아바타는 장면을 이동하는 동안 Decentraland 전반에서 일관되게 행동하고 보입니다. 하지만 다음을 추가할 수 있습니다. `AvatarModifierArea` 장면의 한 영역에 이를 추가하여 플레이어 아바타가 그 영역에 들어왔을 때 어떻게 행동하는지에 영향을 줄 수 있습니다.


> [!CAUTION]
> **❗경고**\
> 다음의 수를 제한해 주세요 `AvatarModifierAreas` 장면에서 사용하는 수를 몇 개로만 제한하세요. 너무 많이 사용하면 성능에 큰 영향을 미칠 수 있습니다.


### 아바타 수정자 영역 배치

다음이 포함된 엔티티를 추가하세요: `AvatarModifierArea` 컴포넌트를 추가하고, 다음을 사용하여 이 엔티티의 위치를 지정하세요: `Transform` 컴포넌트.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

다음과 같은 `AvatarModifierArea` 컴포넌트를 사용하려면 다음을 제공해야 합니다:

* `area`: 수정 영역의 크기
* `수정자`: 영역에 적용할 수정자 목록이 들어 있는 배열입니다. 이 속성은 다음 값들을 사용합니다: `AvatarModifierType` 열거형.

지원되는 수정자는 다음과 같습니다:

* `AvatarModifierType.AMT_HIDE_AVATARS`
* `AvatarModifierType.AMT_DISABLE_PASSPORTS`
* `AvatarModifierType.AMT_HIDE_NAMETAGS`

다음의 모든 효과는 `AvatarModifierArea` 해당 영역의 범위 내에서만 적용됩니다. 플레이어는 영역 밖으로 나가면 정상으로 돌아갑니다.

하나의 `AvatarModifierArea` 영역 안에 있는 플레이어에게만 영향을 줍니다. 영역에 들어간다고 해서 영역 밖의 다른 플레이어가 어떻게 인식되는지는 바뀌지 않습니다.

다음의 효과는 `AvatarModifierArea` 각 플레이어에 대해 로컬에서 계산됩니다. 다음을 가질 수 있습니다: `AvatarModifierArea` 일부 플레이어에게만 장면에 존재하고 다른 플레이어에게는 존재하지 않는 영역입니다. 예를 들어, 장면의 한 플레이어에게만 다른 모든 플레이어를 숨기는 수정자 영역이 있는 "마르코 폴로" 게임을 만들 수 있습니다. 로컬 버전의 장면에 이 수정자 영역이 없는 다른 플레이어들은 서로를 정상적으로 볼 수 있습니다.

영역이 아바타를 숨긴다면, 장면의 로컬 버전에 해당 영역이 없는 플레이어는 자신이 숨겨져 있다고 인식하는 아바타까지 포함해 모든 아바타를 정상적으로 보게 됩니다. 해당 영역이 있는 플레이어는 그 영역에 들어갔을 때 자신과 다른 모든 아바타가 그 영역의 영향을 받는 것으로 인식하게 됩니다.


> [!WARNING]
> **📔 참고**: 아바타 수정자 영역은 다음의 영향을 받습니다: *위치* 및 *회전* 의 영향을 받지만, *크기*.



> [!WARNING]
> **📔 참고** :  `AvatarModifierArea`컴포넌트는 다음을 통해 가져와야 합니다:
> 
> > `import { AvatarModifierArea } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 를 쉽게 다루는 방법을 확인하세요.


### 아바타 숨기기

플레이어가 다음에 들어가면 `AvatarModifierArea` 다음이 있는 `AvatarModifierType.AMT_HIDE_AVATARS` 수정자가 적용된 경우, 플레이어의 아바타가 더 이상 렌더링되지 않습니다. 이는 3인칭 시점의 플레이어에게도, 다른 플레이어가 그 영역에 들어올 때에도 적용됩니다.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

이를 통해 기본 Decentraland 아바타를 장면에서 보여주고 싶은 원하는 사용자 지정 아바타로 바꿀 수 있습니다. 다음을 참조하세요: [플레이어의 아바타 교체](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/#replace-the-players-avatar). 다른 플레이어를 사용자 지정 아바타로 보려면 플레이어 위치 동기화를 직접 처리해야 한다는 점에 유의하세요.

### 패스포트 팝업 비활성화

플레이어가 다음에 들어가면 `AvatarModifierArea` 다음이 있는 `AvatarModifierType.AMT_DISABLE_PASSPORTS` 수정자가 적용되면, 이를 클릭해도 플레이어 소개, 인벤토리 등을 보여주는 패스포트 UI가 더 이상 열리지 않습니다.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

이는 실수로 이 UI를 열어 게임 흐름이 끊길 수 있는 게임, 예를 들어 멀티플레이어 슈팅 게임에서 특히 유용합니다.

### 이름표 숨기기

플레이어가 다음에 들어가면 `AvatarModifierArea` 다음이 있는 `AvatarModifierType.AMT_HIDE_NAMETAGS` 수정자가 적용되면 아바타 자체는 보이는 상태로 유지되고 플레이어의 이름표는 숨겨집니다.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

이는 무대, 발표, 또는 스크립트된 장면에서 아바타 자체는 숨기지 않고 플레이어 이름표만 감추고 싶을 때 유용합니다. 예를 들어 공연 중에는 아바타는 보이되 떠다니는 이름은 관객의 주의를 분산시키지 않도록 깔끔한 시각적 경험을 원할 수 있습니다.

`AMT_HIDE_NAMETAGS` 또한 다음으로 추가된 사용자 지정 플레이트도 숨깁니다: [`AvatarNametag`](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-nametags/) 컴포넌트.


> [!NOTE]
> **💡 팁**: `AMT_HIDE_AVATARS` 는 아바타와 함께 이름표도 이미 숨기므로 다음을 추가할 필요가 없습니다: `AMT_HIDE_NAMETAGS` 다음을 사용할 `AMT_HIDE_AVATARS`. 다음을 사용하세요: `AMT_HIDE_NAMETAGS` 아바타는 보이게 유지하면서 이름표만 숨기고 싶을 때만 사용하세요.


다음을 결합할 수 있습니다 `AMT_HIDE_NAMETAGS` 다음과 같은 다른 수정자와 함께 `AMT_DISABLE_PASSPORTS` 같은 영역에 적용:

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!NOTE]
> **💡 팁**: 이름표는 플레이어의 머리나 몸통이 영역 안에 있을 때만 숨겨집니다. 영역이 너무 낮아서 플레이어가 그 위로 더블 점프하면 이름표가 잠시 다시 나타날 수 있습니다. 예상되는 이동 범위를 덮을 수 있도록 영역의 높이를 충분히 크게 만드세요.


### 아바타 제외

플레이어 ID를 다음 배열에 추가하면 수정자 영역의 영향을 받지 않을 플레이어를 제외할 수 있습니다: `excludeIds` 수정자 영역의 속성입니다.

이 예시는 특정 ID를 가진 플레이어를 제외한 영역 안의 모든 아바타를 숨깁니다. 예를 들어 라이브 이벤트에서 무대 위에는 진행자만 보이게 하고, 무대로 올라오는 다른 플레이어는 숨기는 데 사용할 수 있습니다.

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: ['0xx1...', '0xx2...'],
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!WARNING]
> **📔 참고**: 플레이어 ID는 모두 소문자로 작성해야 합니다. 필요하면 다음을 사용하세요: `.toLowerCase()` 필요한 경우.


수정자 영역은 각 플레이어의 인스턴스에서 로컬로 실행됩니다. 제외된 ID 목록은 플레이어마다 다를 수 있습니다. 아래 예시에서는 각 플레이어가 아바타를 숨기는 수정자에서 자신의 ID를 제외하여, 각자 자신의 아바타만 보고 다른 아바타는 보이지 않게 합니다.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	let userData = getPlayer()
	if (!userData) return

	const entity = engine.addEntity()

	AvatarModifierArea.create(entity, {
		area: Vector3.create(16, 5, 16),
		modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
		excludeIds: [userData.userId],
	})

	Transform.create(entity, {
		position: Vector3.create(8, 0, 8),
	})
}
```


> [!CAUTION]
> **❗경고**\
> 제외된 ID 목록이 주기적으로 변경될 예정이라면(예: 플레이어가 영역에 들어오거나 나가는 경우), 목록의 순서를 유지하세요. 다음을 수행하세요: `.sort()` 배열에 대해 정렬을 수행하여, 전달될 때마다 목록이 같은 순서를 유지하도록 하세요. 이렇게 하면 목록의 변경 사항만 계산됩니다. 그렇지 않으면 장면 성능에 큰 영향을 미칠 수 있습니다.
> 
> ```ts
> AvatarModifierArea.create(entity, {
> 	area: Vector3.create(16, 5, 16),
> 	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
> 	excludeIds: myAvatarList.sort(),
> })
> ```
> 


### 겹치는 영역과 런타임 변경

각 아바타는 자신이 서 있는 영역의 개수가 아니라 효과별로 하나의 켜짐/꺼짐 플래그를 가집니다. 모든 영역이 같은 플래그를 쓰며, 마지막에 쓴 값이 우선합니다. 이로 인해 두 가지 결과가 자주 혼동을 일으킵니다:

* **한 영역을 나가면 아바타가 다시 보이게 되며, 아직 다른 영역 안에 있더라도 마찬가지입니다.** 아바타를 숨기는 작은 영역에서 벗어나면, 장면 전체를 숨기는 영역이 여전히 그 자리를 덮고 있어도 아바타가 다시 보이게 됩니다.
* **플레이어가 포함된 영역은 `excludeIds` 목록에 있으면 진입 시 그들에게 "visible"을 기록합니다**, 그 영역이 자체적으로 나열한 각 효과에 대해 그렇게 합니다.

이것이 그들이 동시에 서 있는 모두 숨김 영역을 덮어씁니다.

**제자리에서 수정자를 제거해도 이전 효과가 취소되지는 않습니다.** 영역의 필드를 변경하면 클라이언트는 현재 나열된 수정자를 다시 적용합니다. 목록에서 제거한 수정자의 효과를 복원하는 것은 아무것도 없으므로, 다음을 비우거나 `수정자`, 또는 바꾸는 것은 `AMT_HIDE_AVATARS` 를 `AMT_HIDE_NAMETAGS`로 바꿔도 아바타는 숨겨진 상태로 남습니다. 제자리에서 수정자를 추가하는 것은 가능합니다.

영역을 다른 수정자 집합으로 바꾸려면 엔티티를 삭제하고 같은 프레임에서 대체 엔티티를 생성하세요:

```ts
import { AvatarModifierArea, AvatarModifierType, engine, Entity, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 영역을 제자리에서 수정하는 대신 교체합니다
function setModifiers(oldArea: Entity, modifiers: AvatarModifierType[]): Entity {
	engine.removeEntity(oldArea) // 내부의 모든 아바타도 다시 보이게 됩니다

	const area = engine.addEntity()
	Transform.create(area, { position: Vector3.create(8, 1, 8) })
	AvatarModifierArea.create(area, {
		area: Vector3.create(16, 5, 16),
		modifiers,
		excludeIds: [],
	})
	return area
}
```

### 수정자 영역 디버그

코드만 보고는 수정자 영역이 장면의 어느 부분을 덮는지 정확히 알기 어려울 수 있습니다. 시각적 피드백은 잘 배치되었는지 확인하는 데 큰 도움이 됩니다.

다음의 위치를 확인하려면 `AvatarModifierArea` 또는 `CameraModeArea`, 그것을 담고 있는 엔티티에 다음을 부여하세요: `MeshRenderer` 다음을 갖는 컴포넌트를: `박스` 모양으로 설정하고, 스케일을 다음과 같은 크기로 맞추세요: `area` 수정자 영역의


> [!WARNING]
> **📔 참고**: 수정자 영역은 다음의 영향을 받지 않습니다: `크기` transform의 속성, 크기는 다음을 기준으로 합니다: `area` 속성.


```ts
const entity = engine.addEntity()
const areaSize = Vector3.create(8, 3, 8)

AvatarModifierArea.create(entity, {
	area: areaSize,
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
  	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
	scale: areaSize,
})

MeshRenderer.setBox(entity)
Material.setPbrMaterial(entity, {
	albedoColor: Color4.create(0.5, 0.5, 0.5, 0.5),
})
```

수정자 영역의 효과를 활성화하려면 플레이어의 머리나 몸통이 영역에 들어와야 합니다. 플레이어의 발만 영역 안에 있어서는 효과가 적용되지 않습니다. 플레이어가 점프로 쉽게 영역을 피할 수 없도록 하세요.


> [!WARNING]
> **📔 참고**: 전체 영역은 장면의 범위 안에 들어가야 합니다.



> [!NOTE]
> **💡 팁**: 동작하는 예시는 다음을 참조하세요: [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas) 테스트 장면. 여기에는 다음이 결합되어 있습니다: `AvatarModifierArea` 런타임에 변경되는 `excludeIds`, 그리고 [`10,99-avatar-modifier-hide-nametags`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/10,99-avatar-modifier-hide-nametags), 이를 통해 다음을 분리합니다: `AMT_HIDE_NAMETAGS` 수정자.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
