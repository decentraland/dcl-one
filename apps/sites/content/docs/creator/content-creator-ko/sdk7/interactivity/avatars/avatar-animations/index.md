> 플레이어의 아바타가 기본 또는 사용자 지정 애니메이션을 재생하도록 하고, 이모트가 끝나는 시점을 감지합니다

# 아바타 애니메이션

씬 코드의 일부로 플레이어에게 애니메이션을 실행하게 할 수 있습니다. 이렇게 하면 더 몰입감 있게 만들 수 있고, 다른 플레이어들이 서로 무엇을 하고 있는지도 더 잘 전달할 수 있습니다. 아바타 애니메이션은 플레이어(3인칭 시점)와 주변의 다른 모든 플레이어가 모두 볼 수 있습니다.

전신에서 실행되는 플레이어 제어 애니메이션은 걷기나 점프 같은 기본 이동 애니메이션에 의해 덮어씌워집니다. 따라서 씬에서 재생되는 전신 애니메이션은 플레이어가 가만히 서 있을 때만 재생됩니다. 플레이어가 걷거나 점프하면 전신 애니메이션은 중단됩니다. 상체 애니메이션은 이동에 의해 중단되지 않습니다.


> [!WARNING]
> **📔 참고**: 플레이어가 애니메이션을 재생할 수 있는 것은 이미 씬 경계 안에 서 있는 경우뿐이며, 인접한 씬에 있는 경우는 아닙니다. 스마트 웨어러블은 어디서든 애니메이션을 재생할 수 있습니다.
> 
> 플레이어가 애니메이션을 수행하는 동안에는 충돌의 영향을 받지 않으며, 움직임도 씬의 물리 규칙에 의해 제약되지 않습니다. 또한 애니메이션이 플레이어를 원래 위치에서 이동시키는 경우(예를 들어 점프가 포함된 경우), 플레이어의 Transform 컴포넌트는 이 이동의 영향을 받지 않습니다.


### Scene Editor 사용

플레이어에게 애니메이션을 재생하게 하는 가장 쉬운 방법은 Scene Editor를 사용하는 것입니다. 노코드 **이모트 재생** 동작을 사용해 기본 애니메이션을 재생하거나, **사용자 지정 이모트 재생** 동작을 사용해 파일의 애니메이션을 재생하세요. 다음을 참조하세요. [아무 아이템이나 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).

### 기본 애니메이션

다음을 사용하세요. `triggerEmote()` 함수를 사용해 Decentraland 어디서든 플레이어가 재생할 수 있는 기본 애니메이션 중 하나를 실행합니다. 이 함수는 다음 속성을 가진 객체를 인수로 받습니다:

* `predefinedEmote`: 기존 이모트의 문자열 이름.
* `mask`: (선택 사항) 다음의 값을 사용해 아바타 몸의 일부에만 애니메이션을 재생합니다. `AvatarMask` 열거형. 예를 들어, `AvatarMask.AM_UPPER_BODY` 는 아바타의 상체에만 애니메이션을 적용합니다. 다음을 참조하세요. [상체만 애니메이션 적용](#animate-only-the-upper-body).

```ts
import { triggerEmote } from '~system/RestrictedActions'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: '춤추기' },
	},
	() => {
		triggerEmote({ predefinedEmote: 'robot' })
	}
)
```

다음 이모트는 씬에서 플레이어의 동작에 대한 피드백을 보여줍니다. 이들 모두 다음 필드의 유효한 값입니다. `predefinedEmote` 필드:

* `buttonDown`
* `buttonFront`
* `getHit`
* `knockOut`
* `lever`
* `openChest`
* `openDoor`
* `punch`
* `push`
* `swingWeaponOneHand`
* `swingWeaponTwoHands`
* `throw`
* `sittingChair1`
* `sittingChair2`
* `sittingGround1`
* `sittingGround2`

네 개의 `앉기` 이모트는 반복 재생됩니다. 아바타는 플레이어가 움직이거나 씬이 다음을 호출할 때까지 포즈를 유지합니다. `stopEmote({})`. `stopEmote` 다음 값을 반환합니다. `{ success: boolean }`이며, 이는 `false` 입니다. 예를 들어 플레이어가 더 이상 당신의 씬 안에 서 있지 않아서 Explorer가 호출을 거부한 경우입니다. 나머지 애니메이션은 한 번 재생된 뒤 아바타가 대기 상태로 돌아갑니다.

이 이모트는 모든 플레이어의 기본 이모트 휠에서 사용할 수 있으며, 어떤 씬에서도 사용할 수 있습니다.

* `wave`
* `fistpump`
* `robot`
* `raiseHand`
* `clap`
* `money`
* `kiss`
* `tik`
* `hammer`
* `tektonik`
* `dontsee`
* `handsair`
* `shrug`
* `disco`
* `dab`
* `headexplode`
* `crafting`
* `handsintheair`
* `Victory`
* `Waving`
* `fistpump_short`


> [!WARNING]
> **📔 참고**: 이 이름들을 목록에 나온 그대로 정확히 사용하세요. 이름이 틀리면 씬에서 조용히 실패합니다. 아무것도 재생되지 않고 씬 콘솔에도 아무것도 나타나지 않습니다. 다음에 주의하세요. `handsintheair` 구분자 없이, 그리고 `fistpump_short` 접미사까지.



> [!NOTE]
> **💡 팁**: 플레이어가 애니메이션 재생 중에 걷거나 점프하면 애니메이션이 중단됩니다. 그럴 가능성을 없애고 싶다면, 다음으로 아바타를 고정할 수 있습니다. [입력 수정자](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#freeze-the-player) 를 사용해 아바타 애니메이션이 끝날 때까지.


### 사용자 지정 애니메이션

다음을 사용하세요. `triggerSceneEmote()` 를 사용해 플레이어가 사용자 지정 애니메이션을 수행하게 하세요. 이 애니메이션은 씬 에셋의 일부로 .glb 파일에 저장됩니다.


> [!NOTE]
> **💡 팁**: 직접 만들 필요는 없습니다.  **아바타 애니메이션** 팩에는 앉는 자세, 낚시, 상자를 들고 있는 동작, 춤추기 등 바로 사용할 수 있는 45개의 애니메이션이 들어 있습니다. 하나를 3D 뷰로 끌어다 놓으면 에디터가 해당 파일을 당신의 씬에 복사하고, 그 다음 그 파일 경로를 다음에 전달하세요. `triggerSceneEmote()`. 파일은 다음 아래에 놓입니다. `assets/asset-packs/<animation-folder>/`, 예를 들어 `assets/asset-packs/whack/Whack_Avatar_emote.glb`. 정확한 경로는 다음에서 확인하세요. **로컬 에셋**. 다음을 참조하세요. [아이템 추가](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/#avatar-animations).



> [!WARNING]
> **📔 참고**: 파일 이름은 **반드시** 다음으로 끝나야 합니다. `_emote.glb` 아바타 애니메이션으로 작동하려면.


이 함수는 다음 속성을 가진 객체를 받습니다:

* `src`: 프로젝트 루트 폴더를 기준으로 계산한 이모트 파일의 전체 경로. 다음 위치의 파일은 `assets/animations/sit_emote.glb` 다음이 필요합니다. `'assets/animations/sit_emote.glb'`.
* `loop`: true이면 플레이어가 움직이거나 애니메이션이 중지될 때까지 애니메이션이 계속 반복됩니다. 기본값은 false입니다.
* `mask`: (선택 사항) 다음의 값을 사용해 아바타 몸의 일부에만 애니메이션을 재생합니다. `AvatarMask` 열거형. 예를 들어, `AvatarMask.AM_UPPER_BODY` 는 아바타의 상체에만 애니메이션을 적용합니다. 다음을 참조하세요. [상체만 애니메이션 적용](#animate-only-the-upper-body).


> [!WARNING]
> **📔 참고**: `src` 파일의 전체 경로여야 합니다. 파일 이름으로 검색하지 않으며, `_emote` 접미사도 자동으로 추가되지 않습니다. 경로가 잘못되면, `triggerSceneEmote()` 다음 값으로 반환됩니다. `{ success: false }` 그리고 아무것도 재생되지 않습니다. 프로젝트 폴더에서 경로를 복사해 그 안의 모든 폴더를 포함하세요.


```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import { AvatarMask } from '@dcl/sdk/ecs'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: '눈덩이 만들기' },
	},
	() => {
		triggerSceneEmote({ src: 'assets/animations/Snowball_Throw_emote.glb', loop: false, mask: AvatarMask.AM_UPPER_BODY })
	}
)
```


> [!NOTE]
> **💡 팁**: 플레이어가 애니메이션 재생 중에 걷거나 점프하면 애니메이션이 중단됩니다. 그럴 가능성을 없애고 싶다면, 다음으로 아바타를 고정할 수 있습니다. [입력 수정자](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#freeze-the-player) 를 사용해 아바타 애니메이션이 끝날 때까지.


#### 소품, 파티클, 소리

다음으로 재생한 이모트 파일은 `triggerSceneEmote()` 테니스 라켓이나 화염 분사처럼 아바타와 함께 애니메이션되는 소품을 포함할 수 있습니다. 다음의 명명 규칙을 따라 Emotes 2.0 이모트와 같은 방식으로 만드세요. [소품과 소리](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/props-and-sounds/):

* 이 파일에는 두 개의 애니메이션 클립이 들어 있습니다. 하나는 이름에 `_Avatar` 가 포함되어 아바타용이고, 다른 하나는 이름에 `_Prop` 가 포함되어 소품 뼈대용입니다.
* 두 클립의 길이는 같습니다.
* 파티클 효과는 애니메이션된 뼈대로 변환하면 작동합니다. 다음을 참조하세요. [이모트의 파티클](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/particles-in-emotes/).

주변의 다른 플레이어들도 아바타 애니메이션을 보는 것처럼 소품도 볼 수 있습니다.


> [!WARNING]
> **📔 참고**: `triggerSceneEmote()` 소리는 재생되지 않습니다. 오디오는 빌더에서 독립형 이모트로 업로드된 이모트에 대해서만 재생됩니다. 씬 이모트에 소리를 함께 붙이려면, 다음과 함께 재생하세요. [AudioSource](/docs/creator/content-creator-ko/sdk7/3d/sounds/) 를 동시에.


### 상체만 애니메이션 적용

둘 다 `triggerEmote()` 와 `triggerSceneEmote()` 는 선택적인 `mask` 속성을 받으며, 아바타 몸의 일부에만 애니메이션을 제한합니다.

보통 애니메이션은 플레이어가 걷거나 점프하는 즉시 중지됩니다. 기본 이동 애니메이션이 전신을 차지하기 때문입니다. 다음을 `mask` 로 설정하면 `AvatarMask.AM_UPPER_BODY`애니메이션은 아바타의 허리 위만 제어하고, 다리는 기본 이동 애니메이션의 제어를 유지합니다. 즉 플레이어는 상체가 당신의 애니메이션을 재생하는 동안에도 씬 안을 계속 걷거나 달릴 수 있습니다. 상자를 나르기, 횃불 들기, 저글링처럼 이동을 중단시키면 안 되는 동작에 이것을 사용하세요.

```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import {
	engine,
	AvatarMask,
	InputAction,
	MeshCollider,
	MeshRenderer,
	pointerEventsSystem,
	Transform,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const cheerButton = engine.addEntity()
Transform.create(cheerButton, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(cheerButton)
MeshCollider.setBox(cheerButton)
pointerEventsSystem.onPointerDown(
	{
		entity: cheerButton,
		opts: { button: InputAction.IA_POINTER, hoverText: '환호' },
	},
	() => {
		triggerSceneEmote({
			src: 'assets/animations/Cheer_emote.glb',
			loop: true,
			mask: AvatarMask.AM_UPPER_BODY,
		})
	}
)
```

이 예시에서는 플레이어가 버튼을 클릭하면 팔을 머리 위로 올린 채 환호를 시작하며, 반복되는 애니메이션이 재생됩니다. 상체만 애니메이션되므로, 예를 들어 경주나 축구 경기의 진행 상황을 따라가면서 환호하는 동안에도 씬을 계속 달릴 수 있습니다.

기억해 둘 점:

* `AvatarMask.AM_UPPER_BODY` 은 현재 `AvatarMask` 열거형의 유일한 값입니다. 전신에 애니메이션을 재생하려면 단순히 `mask` 속성을 설정하지 마세요.
* 다음 `loop` 속성은 전신 애니메이션과 동일하게 동작합니다: `loop: false` 마스크된 애니메이션이 한 번 재생된 뒤 상체는 다시 일반 이동 동작으로 돌아가고, `loop: true` 이면 중지될 때까지 반복됩니다.
* 코드에서 반복 재생되는 마스크 애니메이션을 중지하려면 다음을 호출하세요. `stopEmote({})`도 사용합니다. 다음에서 가져온 `~system/RestrictedActions`. 반환값은 다음과 같습니다. `{ success: boolean }`.

### 이모트가 끝났는지 감지하기

모든 이모트 생명주기 이벤트는 플레이어 엔터티의 `AvatarEmoteCommand` 컴포넌트를 통해 보고됩니다. 각 새 항목에는 다음이 포함됩니다. `state` 필드가 `EmoteState` 열거형의 값으로 함께 제공됩니다:

* `EmoteState.ES_STARTED`: 이모트 재생이 시작되었습니다. 이 값은 다음일 때도 보고됩니다. `state` 필드가 없을 때.
* `EmoteState.ES_FINISHED`: 반복되지 않는 이모트가 자연스럽게 끝까지 재생되었습니다.
* `EmoteState.ES_INTERRUPTED`: 이모트가 중간에 끊겼습니다. 플레이어가 움직이거나 점프했거나, 텔레포트했거나, 다른 이모트가 시작되었거나, 이모트가 명시적으로 중지되었거나, 플레이어가 씬을 떠났습니다.

이것은 씬에 의해 트리거된 이모트(`triggerEmote()` 와 `triggerSceneEmote()`)뿐 아니라, 플레이어가 이모트 휠을 통해 직접 재생한 이모트와 씬의 다른 플레이어가 재생한 이모트에도 적용됩니다.

다음을 사용하세요. `onChange` 의 `AvatarEmoteCommand` 함수로 각 새 항목에 반응하세요:

```ts
import { AvatarEmoteCommand, EmoteState } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return

		switch (emote.state ?? EmoteState.ES_STARTED) {
			case EmoteState.ES_STARTED:
				console.log('이모트 시작: ', emote.emoteUrn)
				break
			case EmoteState.ES_FINISHED:
				console.log('이모트가 자연스럽게 종료됨: ', emote.emoteUrn)
				break
			case EmoteState.ES_INTERRUPTED:
				console.log('이모트가 중단됨: ', emote.emoteUrn)
				break
		}
	})
}
```


> [!WARNING]
> **📔 참고**: 이 기능은 현재 데스크톱 클라이언트에서만 지원됩니다. 로컬 플레이어에게 `mask` 으로 재생된 이모트(부분 신체 이모트)는 현재 생명주기 이벤트를 보고하지 않습니다.



> [!NOTE]
> **💡 팁**: 플레이어가 아닌 아바타에 애니메이션을 재생하려면 다음을 참조하세요. [NPC 아바타](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/#animations).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
