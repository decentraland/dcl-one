> NPC 아바타 표시 및 제어

# NPC 아바타

장면에서 아바타를 엔티티로 표시합니다.


> [!NOTE]
> **💡 팁**: 다음을 사용해 보세요 [NPC Toolkit 라이브러리](https://github.com/decentraland-scenes/dcl-npc-toolkit) NPC를 다루는 과정을 더 쉽게 만들어 주며, 특히 대화 트리를 통해 상호작용해야 할 때 유용합니다.


## 아바타 만들기

다음 코드 조각은 무작위 착용 아이템과 바디 셰이프, 그리고 이름이 "NPC"인 아바타를 만듭니다.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar)

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

아바타 생성을 위해 데이터를 전달할 때 `AvatarShape`, 다음 필드가 필수입니다:

* `id`: (필수) 아바타의 내부 식별자입니다. 값의 마지막 4자는 `id` NPC 이름 뒤에 희미한 텍스트로 표시되며, 플레이어 지갑 접미사처럼 보입니다.  `id` 4자 미만이면 `0000`. 각 NPC에 `id` 이름과 함께 봤을 때 어울리는 끝맺음을 가진 `'guide-0001'`.

다음의 선택적 필드도 사용할 수 있습니다:

* `이름`: 아바타 머리 위에 표시할 이름입니다. 기본값: "NPC". 아무 이름도 표시하지 않으려면 빈 문자열로 설정하세요. 이는 오직 하나만 원할 때 유용합니다. [아바타 이름표](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-nametags/) NPC 위의 명패.
* `bodyShape`: 어떤 바디 셰이프를 사용할지 정의하는 문자열입니다. 유효한 옵션은 'urn:decentraland:off-chain:base-avatars:BaseMale' 및 'urn:decentraland:off-chain:base-avatars:BaseFemale'입니다.
* `wearables`: 아바타가 현재 착용 중인 착용 아이템의 URN 목록 배열입니다. 착용 아이템이 서로 충돌하는 경우(예: 두 개가 모자인 경우), 목록의 마지막 항목이 다른 항목을 대체합니다.
* `emotes`: 아바타가 재생할 수 있는 NFT 이모트의 URN 목록 배열입니다.
* `eyeColor`: *Color3* 눈 색상용(모든 색상 가능)
* `skinColor`: *Color3* 피부 색상용(모든 색상 가능)
* `hairColor`: *Color3* 머리 색상용(모든 색상 가능)
* `talking`: 만약 *true*, 이름 옆에 초록색 막대가 표시됩니다. 이는 플레이어가 월드 내 음성 채팅을 사용할 때와 같습니다.
* <div data-gb-custom-block data-tag="hint" data-style="info" class="hint hint-info"><p><strong>💡 팁</strong>: 다음을 참조하세요 <a href="/docs/creator/content-creator-ko/sdk7/3d/color-types/">색상 유형</a> 색상 설정 방법에 대한 자세한 내용은</p></div>


> [!WARNING]
> **📔 참고**: `AvatarShape`컴포넌트는 다음을 통해 임포트해야 합니다.
> 
> > `import { AvatarShape } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요 [Imports](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 처리하는 방법을 확인하세요.



> [!WARNING]
> **📔 참고**: URN 필드는 다음과 같은 형식을 따라야 합니다: [NFTShapes](/docs/creator/content-creator-ko/sdk7/media/display-a-certified-nft/): `urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`


## NPC 이동하기

NPC를 이동하려면, 해당 항목의 `위치` 의 `Transform`을 변경하세요. 아바타는 새 위치로 바로 순간이동하지 않습니다. 현재 위치에서 그곳까지 걸어가며, 초당 약 4미터 속도로 이동하고, 대상이 1.5미터 이상 떨어져 있으면 더 빠르게 이동합니다. 이동하는 동안에는 걷는 방향을 향하도록 회전합니다.  `회전` 에서 설정한 `Transform` 은 도착한 뒤에만 적용됩니다.

```ts
// NPC가 현재 위치에서 새 위치로 걸어갑니다
Transform.getMutable(myAvatar).position = Vector3.create(10, 0.25, 5)
```

이동을 직접 제어하려면 [Tween](/docs/creator/content-creator-ko/sdk7/3d/move-entities/). 위치에 Tween을 적용하면 이 자동 걷기가 대체되고, 회전에 Tween을 적용하면 NPC가 걷는 방향을 향하도록 도는 동작이 중지됩니다. 엔티티는 한 번에 하나의 Tween만 가질 수 있으므로, 같은 엔티티의 위치와 회전을 동시에 tween할 수는 없습니다.

## 애니메이션

아바타는 가만히 있을 때 기본 대기 애니메이션을 재생합니다.

아바타에서 애니메이션을 재생하려면 `expressionTriggerId` 문자열을 재생하려는 애니메이션 이름으로 설정하세요.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'dancer-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'robot',
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

다음의 `expressionTriggerId` 필드는 다음을 모두 지원합니다: [기본 애니메이션](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#default-animations), 그리고 사용자 지정 애니메이션도 [장면 파일의](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#custom-animations), 심지어 마켓플레이스에 게시된 이모트의 URN도 가능합니다.

사용자 지정 애니메이션을 사용하려면 다음의 경로를 지정하세요 `_emote.glb` 프로젝트 루트 폴더의 파일, 예를 들어 `'assets/animations/Cheer_emote.glb'`. 마찬가지로 [`triggerSceneEmote()`](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#custom-animations), 잘못된 경로를 지정하면 아무 것도 재생되지 않습니다.


> [!WARNING]
> **📔 참고**: `AvatarShape` 에는 `마스크` 필드가 없으므로 `expressionTriggerId` 애니메이션은 항상 전신에 재생됩니다. [상체만 애니메이션하기](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#animate-only-the-upper-body) 는 NPC가 아니라 플레이어 자신의 아바타에서만 사용할 수 있습니다.


### 이모트를 재생하는 동안 NPC 이동하기

아바타의 `Transform.position`, 또는 [Tween](/docs/creator/content-creator-ko/sdk7/3d/move-entities/) 을 실행하면 재생 중인 이모트가 취소됩니다. 아바타는 다시 걷기 사이클로 돌아가는데, 이는 Decentraland가 마지막 프레임 이후 아바타가 얼마나 이동했는지에 따라 어떤 이동 애니메이션을 블렌딩할지 계산하기 때문입니다.

이모트 중에 NPC를 이동하려면, 이동 범위가 작은 경우 움직임 자체를 애니메이션에 베이크하는 것이 좋습니다. 즉, 다음의 내부에 있는 루트 또는 힙 동작처럼 `_emote.glb` 파일 안에

### 루프 애니메이션

에 적용된 애니메이션은 `AvatarShape` 한 번만 재생됩니다. 아바타가 애니메이션을 계속 루프하도록 하려면, 몇 초마다 애니메이션을 다시 재생하도록 지시하는 시스템을 만들어야 합니다.

예외는 루프되는 기본 이모트인 `sittingChair1`, `sittingChair2`, `sittingGround1` 그리고 `sittingGround2`, 이것들은 `AvatarShape` 에서 다시 재생할 필요 없이 자세를 유지합니다. 파일의 사용자 지정 애니메이션은 `AvatarShape`, 클립 자체가 루프로 표시되어 있어도 절대 루프되지 않으므로, 지속적인 사용자 지정 포즈에는 아래의 재생 패턴이 필요합니다.

다음을 사용하세요 `expressionTriggerTimestamp` 같은 이모트를 다시 재생합니다. 이 필드의 값은 [램포트 타임스탬프](https://en.wikipedia.org/wiki/Lamport_timestamp), 즉 시간이 아니라 이모트를 반복할 때마다 1씩 증가하는 인덱스입니다.

따라서 이모트를 처음 재생할 때는 `expressionTriggerTimestamp` 로 설정하여 *0*. 이모트를 다시 재생하려면 이 값을 1로 업데이트해야 합니다. 그래야 엔진이 이것이 이미 처리한 지시가 아니라 새로운 지시임을 알 수 있습니다.

다음 코드 조각은 2초마다 같은 이모트를 실행하는 시스템을 만듭니다:

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'fan-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'clap',
    expressionTriggerTimestamp: 0
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})

let clapTimer = 0
let emoteDuration = 2  // 2 seconds

// system
engine.addSystem((dt: number) => {
    clapTimer += dt
      
    if (clapTimer >= emoteDuration) {
        // Trigger the clap emote
        AvatarShape.getMutable(myAvatar).expressionTriggerTimestamp += 1
        
        clapTimer = 0 // Reset timer
    }
})
```


> [!NOTE]
> **💡 팁**: 이모트의 지속 시간을 알아야 하며, 이를 시스템의 지속 시간으로 설정해야 합니다. 아바타를 같은 자세로 고정하는 이모트를 만든다면, 이모트의 지속 시간을 시스템보다 더 길게 하는 것이 좋습니다. 그래야 애니메이션을 마무리하고 재설정할 때 아티팩트가 생기지 않음을 확실히 할 수 있습니다.


## 이름 위에 레이블 추가하기

다음을 사용하세요 `AvatarNametag` 컴포넌트는 NPC 이름 위에 사용자 지정 텍스트가 있는 명패를 표시합니다. 예를 들어 "Shopkeeper"나 "Boss" 같은 역할을 표시할 수 있습니다. 이것을 다음이 있는 동일한 엔티티에 연결하세요 `AvatarShape`:

```ts
AvatarNametag.create(myAvatar, { label: 'Shopkeeper' })
```

만약 `AvatarShape` 에 비어 있는 `이름`, 명패만 표시되고 아래에 빈 이름표 상자는 나타나지 않습니다. 이는 NPC에 직함만 붙이고자 할 때 유용합니다.


> [!NOTE]
> **💡 팁**: 텍스트 없는 색상 구분 명패를 만들려면 `label` 을 공백 문자열로 설정하고 `backgroundColor`. 공백은 유지되므로 공백이 많을수록 명패가 더 넓어집니다.
> 
> ```ts
> AvatarNametag.create(myAvatar, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


다음을 참조하세요 [아바타 이름표](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-nametags/) 색상을 포함한 전체 필드 목록은 다음을 참조하세요.

## 플레이어의 착용 아이템 복사하기

다음 코드 조각은 NPC 아바타의 착용 아이템과 기타 특성을 플레이어가 현재 착용 중인 것과 일치하도록 변경합니다. 이는 장면에서 마네킹처럼 사용하여, 특정 착용 아이템이나 이모트를 플레이어의 현재 복장과 함께 보여줄 때 유용합니다.

```ts
import { getPlayer } from '@dcl/sdk/src/players'


export function swapAvatar(avatar: Entity) {

  let userData = getPlayer()
  console.log(userData)

  if (!userData || !userData.wearables) return

  const mutableAvatar = AvatarShape.getMutable(avatar)

  mutableAvatar.wearables = userData.wearables
  mutableAvatar.bodyShape = userData.avatar?.bodyShapeUrn
  mutableAvatar.eyeColor = userData.avatar?.eyesColor
  mutableAvatar.skinColor = userData.avatar?.skinColor
  mutableAvatar.hairColor = userData.avatar?.hairColor
  
}
```

## 착용 아이템만 표시하기

다음을 사용하세요 `showOnlyWearables` 필드는 아바타에 나열된 착용 아이템만 표시합니다. 아바타의 나머지 몸체는 보이지 않게 됩니다.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'mannequin-0001',
	emotes: [],
	wearables: [
    'urn:decentraland:matic:collections-v2:0x90e5cb2d673699be8f28d339c818a0b60144c494:0'
  ],
	showOnlyWearables: true,
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

이는 예를 들어 상점에서 착용 아이템을 전시할 때 유용합니다.


> [!NOTE]
> **💡 팁**: 착용 아이템이 꽤 작다면 `크기 조정` 의 `Transform` 를 더 큰 값으로 설정해 보세요.


## NPC에 엔티티를 부착하기

다음을 사용할 수 있습니다 `AvatarAttach` 기능을 사용하면 엔티티를 NPC 아바타의 뼈대 중 하나에 고정할 수 있습니다. 예를 들어 NPC가 손에 물건을 들고 있게 할 수 있습니다. 엔티티는 아바타가 애니메이션될 때 함께 움직입니다.

이 기능을 사용하려면 `id` 속성을 `AvatarShape` 에서 이 아바타에 임의의 id를 할당한 다음, 해당 id를 다음에서 참조하세요 `AvatarAttach`. id는 원하는 어떤 문자열이든 될 수 있습니다.

```ts
// Create NPC, with an ID
const myAvatar = engine.addEntity()
Transform.create(myAvatar, {
  position: Vector3.create(8, 0.25, 8),
})
AvatarShape.create(myAvatar, {
  id: "my-avatar-id", 
  wearables: [],
  emotes: []
})

// Create object to attach to NPC
const attachedEntity = engine.addEntity()
Transform.create(attachedEntity, {
    position: Vector3.create(4, 2, 4),
    scale: Vector3.create(0.15,0.15,0.15)
})
MeshRenderer.setBox(attachedEntity)
Material.setBasicMaterial(attachedEntity, { diffuseColor: Color4.Blue() })
AvatarAttach.create(attachedEntity, {
    avatarId: "my-avatar-id",
    anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND
})
```

다음에 대해 자세히 알아보세요 `AvatarAttach` 컴포넌트 [여기](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#attach-an-entity-to-an-avatar).

## 플레이어의 아바타를 교체하기

플레이어의 아바타가 착용한 아이템은 변경할 수 없지만, 대신 플레이어의 아바타를 완전히 사용자 지정할 수 있는 NPC 아바타로 교체할 수 있습니다.

이를 위해 플레이어 자신의 아바타를 다음으로 숨기세요 [Avatar Modifier Area](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas/#hide-avatars) 를 사용하는 `AMT_HIDE_AVATARS` 수정자를 적용한 뒤, 다음을 표시하세요 `AvatarShape` 플레이어 위치에 엔티티를 대신 표시합니다.


> [!WARNING]
> **📔 참고**: 플레이어가 해당 아바타를 완전히 제어할 수 있도록 하려면, 버튼 이벤트를 수신해 버튼을 누르는 시점을 감지한 다음 NPC 아바타에서 해당 애니메이션을 트리거해야 합니다. 다음을 참조하세요 [버튼 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/) 에서 자세한 내용을 확인하세요.
> 
> 이렇게 할 때 제어의 유연성이 완벽하지 않을 수 있으므로, 매우 특정한 경우에만 사용하는 것이 좋습니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
