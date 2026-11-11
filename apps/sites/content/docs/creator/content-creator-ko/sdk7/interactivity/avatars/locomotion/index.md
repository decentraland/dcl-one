> 플레이어의 속도와 점프 높이를 변경하거나, 이동 방식을 제한합니다

# 이동 설정 및 제한

장면 안에 있는 동안 플레이어의 아바타가 어떻게 움직이는지 바꿀 수 있습니다. 다음을 사용하세요 `AvatarLocomotionSettings` 컴포넌트로 속도와 점프 높이를 조정하고, `InputModifier` 컴포넌트로 플레이어를 고정하거나 특정 종류의 이동을 차단하세요. 두 컴포넌트 모두 `engine.PlayerEntity`에 배치하며, 둘 다 플레이어가 장면의 경계 안에 있을 때만 적용됩니다.

## 이동 설정

플레이어의 달리기 속도, 점프 높이 등 이동 방식을 조정할 수 있습니다. 이는 동적으로 변경할 수 있으며, 예를 들어 아이템과 상호작용해 일시적인 속도 증가를 얻도록 하거나 짧은 시간 동안 점프 기능을 비활성화하는 데 사용할 수 있습니다.


> [!NOTE]
> **💡 팁**: 코드를 작성하지 않고도 이 값들을 설정할 수 있습니다. 다음을 드래그하세요 **이동 설정** 스마트 아이템을 장면에 [Creator Hub의 씬 에디터에 추가한 뒤](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/) 슬라이더를 움직여 보세요. 참조 [이동 설정](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/#locomotion-settings).


이를 수행하려면, 다음을 추가하세요 `AvatarLocomotionSettings` 컴포넌트를 `engine.PlayerEntity`.

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	jumpHeight: 2,
})
```

다음 속성을 사용할 수 있습니다:

* `walkSpeed`: 플레이어가 걷는 속도(초당 미터). 데스크톱 클라이언트에서는 컨트롤 키를 눌러 걷습니다.
* `jogSpeed`: 플레이어가 조깅하는 속도(초당 미터). 이는 플레이어가 이동하는 기본 방식입니다.
* `runSpeed`: 플레이어가 달리는 속도(초당 미터). 데스크톱 클라이언트에서는 쉬프트 키를 눌러 달립니다.
* `jumpHeight`: 플레이어가 점프하는 높이(미터).
* `runJumpHeight`: 플레이어가 달린 후 점프하는 높이(미터).
* `doubleJumpHeight`: 더블 점프 시 두 번째 점프의 높이(미터).
* `glidingSpeed`: 플레이어가 글라이딩 중 이동하는 수평 속도(초당 미터).
* `glidingFallingSpeed`: 플레이어가 글라이딩 중 떨어지는 최대 속도(초당 미터). 이는 플레이어의 하강만 제한합니다. 장면의 지속 힘으로 인한 상승 등 위쪽으로의 움직임은 제한되지 않습니다.
* `hardLandingCooldown`: 강한 착지 후 재사용 대기시간(초). 높은 곳에서 떨어진 뒤 다시 움직일 수 있기 전에 플레이어가 기다려야 하는 시간입니다.

참고로, 이 속성들의 기본값은 다음과 같습니다:

* `walkSpeed`: 1.5 m/s
* `jogSpeed`: 8 m/s
* `runSpeed`: 10 m/s
* `glidingSpeed`: 6 m/s
* `glidingFallingSpeed`: 1 m/s
* `jumpHeight`: 1 m
* `runJumpHeight`: 1.5 m
* `doubleJumpHeight`: 2 m
* `hardLandingCooldown`: 0.75 s


> [!NOTE]
> **💡 팁**: 글라이딩 중에는 장면이 가하는 지속 힘이 1.5배 강해지고, 위쪽 힘으로 플레이어를 들어 올릴 수 있습니다. 다음을 참조하세요 [글라이딩 중 힘](/docs/creator/content-creator-ko/sdk7/interactivity/player-physics/#forces-while-gliding).



> [!NOTE]
> **💡 팁**: 이 속성들 중 어느 것도 0보다 작을 수 없습니다. 음수로 설정하면 0으로 고정됩니다. 이 값들을 0으로 설정하면 다음을 사용하는 것과 같은 효과가 납니다 `InputModifier` 특정 키의 사용을 차단합니다.
> 
> 플레이어가 장면의 경계 안에 있을 때만 플레이어의 이동 방식에 영향을 줄 수 있습니다. 다른 플레이어의 아바타에 영향을 주려면, 해당 플레이어 자신의 인스턴스에서 그들의 이동 방식에 영향을 주는 코드를 실행해야 합니다.


다음을 만들 수 있습니다 [스마트 웨어러블](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/smart-wearables/) 을 만들어 플레이어가 항상 더 빨리 달리거나 더 높이 점프하게 할 수 있습니다. 장면과 스마트 웨어러블이 이 매개변수에 대해 서로 다른 값을 정의하는 경우, 항상 장면의 값이 사용됩니다.

파쿠르 장면에서 누구도 불공정한 이점을 갖지 않도록 하려면, 장면에 기본값을 명시적으로 추가해 기본 매개변수를 강제할 수 있습니다:

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	walkSpeed: 1.5,
	jogSpeed: 8 ,
	jumpHeight: 1,
	runJumpHeight: 1.5,
	hardLandingCooldown: 0.75
})
```

## 이동 제한

장면에서 플레이어가 수행할 수 있는 동작을 제한할 수 있습니다. 이를 사용하여 플레이어를 고정하거나, 플레이어가 점프하거나 달리는 것을 막는 등 특정 이동 방식을 제한할 수 있습니다.

### 플레이어 고정

플레이어가 어떤 입력 키도 아바타를 움직일 수 없도록 고정할 수 있습니다. 이는 많은 게임 메커니즘에서 유용할 수 있습니다. 또한 이동으로 중단되면 안 되는 중요한 애니메이션을 재생하는 동안이나, 또는 [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/) 아바타에서 멀어지게 가리키고 있고 플레이어가 무턱대고 움직이지 않길 원할 때.

다음을 사용하세요 `InputModifier` 다음의 컴포넌트: `engine.PlayerEntity` 플레이어의 입력이 아바타의 이동에 영향을 주지 못하도록 합니다. 아바타는 그대로 있고, 플레이어는 카메라만 회전시킬 수 있습니다.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: true,
	}),
})
```

다음 사항을 염두에 두세요:

* 플레이어의 상호작용이 비활성화되어 있어도, 아바타는 중력이나 움직이는 플랫폼 같은 외부 힘의 영향을 받습니다.
* 해당 `InputModifier` 컴포넌트는 다음과 함께만 사용할 수 있습니다 `engine.PlayerEntity` 엔티티. 현재 플레이어에게만 영향을 줄 수 있으며, 다른 플레이어에게는 영향을 줄 수 없습니다.
* 이 컴포넌트는 아바타가 장면의 경계 안에 있을 때만 플레이어에게 영향을 줍니다. 플레이어가 장면을 벗어나는 즉시 이동 제한이 해제됩니다.
* 플레이어의 상호작용이 비활성화되어 있는 동안 플레이어는 이모트를 자유롭게 수행할 수 없지만, 장면은 아바타에서 애니메이션을 트리거할 수 있습니다.
* 플레이어 입력은 아바타에 영향을 주지 않지만, [전역 입력 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/#global-input-events) 장면에서 계속 감지할 수 있습니다. 이를 사용해 차량을 제어하거나, [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/) 다른 엔티티가 움직일 때 그 뒤를 따라가도록 하여 대체 아바타처럼 사용할 수 있습니다.

### 특정 종류의 이동 제한

플레이어를 완전히 고정하는 대신, 플레이어의 특정 이동 방식을 제한할 수 있습니다. 이는 게임플레이상의 이유로 사용할 수 있습니다. 예를 들어 더블 점프와 글라이딩을 막아 플랫포머의 난이도를 유지하는 데 쓸 수 있습니다. 이러한 능력은 게임 메커니즘으로 동적으로 전환할 수도 있습니다. 예를 들어 플레이어에게 스태미나 바를 주고 소진되면 달리지 못하게 할 수 있습니다. 또는 차분한 분위기를 의도한 장소에서 달리기나 점프를 막아 장면의 분위기를 설정하는 데도 사용할 수 있습니다. 다음 `InputModifier` 에는 다음 옵션이 포함됩니다:

* `disableWalk`: 플레이어는 천천히 걸을 수 없습니다(컨트롤 키 누르기). 플레이어가 걷기를 시도하면, 허용되는 경우 대신 조깅하거나 달리게 됩니다.
* `disableRun`: 플레이어는 달릴 수 없습니다(쉬프트 키 누르기). 플레이어가 달리기를 시도하면, 허용되는 경우 대신 조깅하게 됩니다.
* `disableJog`: 플레이어는 조깅할 수 없습니다(이것이 기본 이동 속도입니다). 플레이어가 조깅을 시도하면, 허용되는 경우 대신 달리거나 걷게 됩니다.
* `disableJump`: 플레이어는 점프할 수 없습니다.
* `disableEmote`: 플레이어는 자발적으로 이모트를 수행할 수 없습니다. 장면은 플레이어의 아바타에서 애니메이션을 트리거할 수 있습니다.
* `disableDoubleJump`: 플레이어는 더블 점프를 할 수 없습니다.
* `disableGliding`: 플레이어는 글라이딩할 수 없습니다.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: false,
		disableWalk: false,
		disableRun: true,
		disableJog: true,
		disableJump: true,
		disableEmote: true,
		disableDoubleJump: true,
		disableGliding: true
	}),
})
```

### 고급 구문

컴포넌트를 헬퍼 없이 사용하려면, 다음 구문을 사용할 수 있습니다:

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.createOrReplace(engine.PlayerEntity, {
	mode: {
		$case: 'standard',
		standard: {
			disableAll: false,
			disableWalk: false,
			disableRun: true,
			disableJog: true,
			disableJump: true,
			disableEmote: true,
		},
	},
})
```


> [!NOTE]
> **💡 팁**: 작동 예시는 다음을 참조하세요 [`0,1-input-modifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier) 테스트 장면. 이 장면은 각각의 `InputModifier` 이동 플래그를 개별적으로 전환하며, 이동이 차단되어 있어도 움직이는 플랫폼에 실린 아바타를 보여 줍니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
