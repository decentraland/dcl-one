> 씬에서 사용자 클릭을 처리하는 방법을 알아봅니다.

# 콜백 등록

버튼 이벤트를 처리하는 가장 쉬운 방법은 특정 엔티티에 콜백 함수를 등록하는 것입니다. 해당 엔티티가 특정 버튼으로 상호작용될 때마다 콜백 함수가 호출됩니다.

여러 비슷한 엔티티에 동일한 동작을 추가해야 한다면, [시스템 기반](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/) 방식을 사용하는 것을 고려해 보세요. 유사한 엔티티 목록을 순회할 때 시스템 기반 방식이 더 효율적일 수 있습니다.

Register callback 방식은 하나의 엔티티에만 영향을 주는 동작을 설명하고 싶을 때 특히 유용합니다. 더 직관적이기 때문입니다.


> [!WARNING]
> **📔 참고**:\
> 엔티티가 상호작용 가능하려면, 그것은 **반드시** 다음을 가져야 합니다: [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/)로 설정하는 것입니다. 참조: [장애물](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#obstacles) 자세한 내용은.


## 포인터 누르기

다음을 사용하세요 `pointerEventsSystem.onPointerDown()` 특정 버튼의 눌림을 감지합니다.

이 문은 두 개의 매개변수를 필요로 합니다:

* `데이터를`: 다음 항목을 포함하는 객체:
  * `엔티티`: 처리할 엔티티
  * `opts`: 선택적 추가 데이터를 포함하는 객체:
    * `버튼`: 어떤 버튼을 감지할지 지정합니다. 참조: [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 지원되는 옵션은 위를 참조하세요. 버튼을 지정하지 않으면 전진 및 점프와 같은 이동 버튼을 포함한 모든 버튼을 감지합니다.
    * `maxDistance`: 엔티티와 플레이어의 **아바타** 사이의 최대 거리(미터)입니다. 기본값은 10입니다. 참조: [거리 제한](#distance-limits).
    * `maxCameraDistance`: 엔티티와 **활성 카메라** 사이의 최대 거리(미터)입니다. 기본값은 설정되어 있지 않습니다. 참조: [거리 제한](#distance-limits).
    * `호버 텍스트`: 호버 피드백 힌트에 표시할 문자열입니다. 기본값은 "Interact"입니다.
    * `피드백 표시`: false이면 이 엔티티의 호버 힌트와 가장자리 하이라이트를 모두 숨깁니다. *true* 기본값입니다.
    * `하이라이트 표시`: true이면 플레이어가 커서를 엔티티 위에 올렸을 때 가장자리 하이라이트를 보게 됩니다. *true* 기본값입니다. 이 값은 다음 경우에만 고려됩니다: `피드백 표시` 가 *true*.
* `cb`: 엔티티를 가리키는 동안 버튼 누름 이벤트가 발생할 때마다 실행할 콜백 함수

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Click' },
	},
	function () {
		console.log('clicked entity')
	}
)
```

위 명령은 콜백 함수를 등록된 상태로 유지하며, 관련 버튼 이벤트가 발생할 때마다 호출됩니다. 콜백은 동기 함수여야 한다는 점에 유의하세요; `비동기` 함수는 여기서 지원되지 않으며 오류가 발생합니다.


> [!WARNING]
> **📔 참고**:\
> 하나만 `pointerEventsSystem.onPointerDown` 엔티티마다 등록할 수 있습니다. 추가되면 리스너가 제거될 때까지 이벤트를 계속 수신합니다. 시스템 안에서 이를 반복적으로 실행하지 마세요. 포인터 이벤트 동작이 계속 덮어써지게 됩니다.


## 호버 피드백

엔티티와 상호작용할 수 있다는 것을 플레이어에게 어떤 식으로든 알려 주는 것이 매우 중요합니다.

와 함께 입력 동작을 등록하면 `EventsSystem`, 기본적으로 플레이어에게 다음이 표시됩니다:

* 엔티티의 가장자리 하이라이트
* 커서 근처에, 눌러야 할 버튼 아이콘과 "Interact"라는 문자열이 있는 호버 힌트

이 요소들은 켜고 끌 수 있으며 사용자 지정할 수 있습니다.

UI의 호버 피드백은 다음에서 선택한 입력에 따라 다른 아이콘을 표시합니다: `버튼` 필드입니다. PC에서는 다음에 대한 아이콘을 표시합니다: `E` 용 `InputAction.IA_PRIMARY`, `F` 용 `InputAction.IA_SECONDARY`, 그리고 다음에는 마우스 아이콘이 표시됩니다: `InputAction.IA_POINTER`.

문자열을 다음을 변경하여 바꾸세요: `호버 텍스트` 값. 이 문자열은 짧게 유지하세요. 그래야 빠르게 읽을 수 있고 화면을 너무 방해하지 않습니다.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open door' },
	},
	function () {
		// 문 열기
	}
)
```

호버 힌트는 숨기되 가장자리 하이라이트는 남기려면, 다음 값에 `호버 텍스트` 를 ""로 설정하세요.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, hoverText: ''}},
  function () {
    console.log("clicked on surprise interactive item")
  }
)
```

가장자리 하이라이트는 숨기되 호버 힌트는 남기려면, 다음을 설정하세요: `하이라이트 표시` 을(를) *false*.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: {
			button: InputAction.IA_PRIMARY,
			hoverText: 'Open door',
			showHighlight: false,
		},
	},
	function () {
		console.log('opened secret door')
	}
)
```

호버 힌트와 가장자리 하이라이트를 둘 다 숨기려면, 다음을 설정하세요: `피드백 표시` 을(를) *false*. 이렇게 하면 커서는 아이콘, 텍스트, 가장자리 하이라이트를 전혀 표시하지 않습니다.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, showFeedback: false}},
  function () {
    console.log("opened secret door")
  }
)
```

### 기존 피드백 변경

와 함께 입력 동작을 등록하면 `EventsSystem`, 이는 다음을 만드는 것입니다: `PointerEvents` 구성 요소를 백그라운드에서 생성하여 상호작용 가능한 엔티티에 추가합니다. 이 구성 요소는 UI 호버 힌트의 동작을 처리합니다. 호버 피드백의 동작을 바꾸려면 이 구성 요소를 수정하세요. 참조: [피드백 표시](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/#show-feedback) 이 구성 요소를 다루는 방법에 대한 자세한 내용은 위를 참조하세요.

```ts
const hoverFeedback = PointerEvents.getMutable(myEntity)

if (hoverFeedback.pointerEvents[0]?.eventInfo) {
	hoverFeedback.pointerEvents[0].eventInfo.hoverText = 'Close door'
}
```

## 거리 제한

기본적으로 플레이어는 자신의 **아바타** 가 그 엔티티로부터 10미터 이내에 있을 때만 엔티티와 상호작용할 수 있습니다. 플레이어가 너무 멀리 있으면 엔티티의 하이라이트 효과가 초록색이 아니라 빨간색으로 바뀌고 포인터 이벤트는 발생하지 않습니다.

다음을 사용하세요 `maxDistance` 그 범위를 바꾸려면:

```ts
// 플레이어의 아바타가 5미터 이내에 있을 때만 클릭 가능
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 5 } },
	function () {
		console.log('clicked entity')
	}
)
```

### 카메라 거리로 제한

`maxCameraDistance` 다음으로부터 측정합니다: **현재 활성 카메라** 아바타가 아니라. 이는 장면이 다음을 사용할 때 유용합니다: [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/#using-virtual-cameras) 플레이어와 떨어진 곳에 배치된 카메라이며, 카메라가 볼 수 있는 것에 따라 엔티티를 클릭 가능하게 만들고 싶을 때 유용합니다.

```ts
// 활성 카메라가 15미터 이내에 있을 때만 클릭 가능
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxCameraDistance: 15 } },
	function () {
		console.log('clicked entity')
	}
)
```

### 두 제한이 함께 적용되는 방식

두 옵션은 독립적인 검사입니다. 어떤 검사가 실행되는지는 무엇을 설정했는지에 따라 달라집니다:

| 설정한 항목                 | 플레이어에게 필요한 조건                  |
| ---------------------- | ------------------------------ |
| 둘 다 아님                 | 아바타가 10미터 이내(기본값)              |
| `maxDistance` 오직       | 아바타가 다음 이내 `maxDistance`       |
| `maxCameraDistance` 오직 | 카메라가 다음 이내 `maxCameraDistance` |
| 둘 다                    | **둘 중 하나** 검사 하나만 통과해도 충분      |

둘 다 설정하면 "걸어가서 닿을 만큼 가까우면서, **또는** 또렷하게 보일 만큼 가까움"이라고 말하는 방식입니다:

```ts
// 아바타가 3미터 이내이거나 카메라가 20미터 이내일 때 클릭 가능
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 3, maxCameraDistance: 20 } },
	function () {
		console.log('clicked entity')
	}
)
```


> [!WARNING]
> **📔 참고**: `maxCameraDistance` 필요 `@dcl/sdk` 버전 7.28.0 이상. Bevy 기반 탐색기에서는 이미 지원됩니다. 오늘날 장면이 모든 클라이언트에서 동일하게 동작해야 한다면, 다음에 의존하세요: `maxDistance` 그리고 다음을 `maxCameraDistance` 추가 기능으로 간주하세요.


### maxPlayerDistance는 사용 중단되었습니다

`maxPlayerDistance` 는 사용 중단된 별칭입니다: `maxDistance`: 둘 다 아바타로부터의 거리를 측정합니다. 새로운 장면에서는 다음을 사용하세요: `maxDistance` 새 장면에서.

장면이 둘 다 설정하면, **더 큰** 값이 아바타 거리 제한으로 사용됩니다.

## 포인터 떼기

다음을 사용하세요 `pointerEventsSystem.onPointerUp` 지정된 플레이어가 엔티티를 가리키는 동안 버튼을 뗄 때 호출되는 콜백 함수를 등록합니다.

```ts
pointerEventsSystem.onPointerUp(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Button up' },
	},
	function () {
		console.log('button up')
	}
)
```

이 문은 두 개의 매개변수를 필요로 합니다:

* `데이터를`: 다음 항목을 포함하는 객체:
  * `엔티티`: 처리할 엔티티
  * `opts`: 선택적 추가 데이터를 포함하는 객체:
    * `버튼`: 어떤 버튼을 감지할지 지정합니다. 참조: [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 지원되는 옵션은 위를 참조하세요. 버튼을 지정하지 않으면 전진 및 점프와 같은 이동 버튼을 포함한 모든 버튼을 감지합니다.
    * `호버 텍스트`: 호버 피드백 힌트에 표시할 문자열입니다. 기본값은 "Interact"입니다.
    * `피드백 표시`: false이면 이 엔티티의 호버 힌트를 숨깁니다. *true* 기본값입니다.
    * `maxDistance`: 플레이어의 아바타가 엔티티에서 얼마나 떨어져 있어도 상호작용할 수 있는지(미터)입니다. 기본값은 10입니다. 플레이어가 너무 멀리 있으면 호버 피드백이 표시되지 않고 포인터 이벤트도 작동하지 않습니다. 참조: [거리 제한](#distance-limits).
* `cb`: 엔티티를 가리키는 동안 버튼 뗌 이벤트가 발생할 때마다 실행할 콜백 함수

같은 엔티티에는 서로 다른 두 콜백을 등록할 수 있습니다. 하나는 `pointerEventsSystem.onPointerDown` 용이고, 하나는 `pointerEventsSystem.onPointerUp`. 엔티티에는 각 유형마다 하나의 콜백만 등록할 수 있습니다. [여러 버튼 처리](#handle-multiple-buttons) 한 콜백에서 서로 다른 버튼을 감지하려면.


> [!WARNING]
> **📔 참고**: 버튼 떼기 이벤트의 호버 피드백은 버튼이 현재 눌린 상태일 때만 표시됩니다. 플레이어가 버튼을 누르지 않은 채 엔티티를 가리키면 피드백이 없거나, 있다면 버튼 누름 이벤트의 피드백이 표시됩니다.


## 호버 진입 및 종료

다음을 사용하세요 `pointerEventsSystem.onPointerHoverEnter` 플레이어의 커서가 엔티티를 가리키기 시작할 때 콜백을 실행하려면 `pointerEventsSystem.onPointerHoverLeave` 커서가 그 엔티티를 가리키는 것을 멈출 때 실행하세요.

```ts
pointerEventsSystem.onPointerHoverEnter(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('Cursor started hovering over entity')
    }
)

pointerEventsSystem.onPointerHoverLeave(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('Cursor stopped hovering over entity')
    }
)
```

이 콜백은 플레이어가 엔티티를 조준할 때 소리를 재생하거나 엔티티를 애니메이션하는 등의 사용자 지정 호버 효과에 유용합니다.

## 콜백 제거

콜백 함수를 제거하려면 해당 remove 함수를 사용하세요:

```ts
pointerEventsSystem.removeOnPointerDown(myEntity)
pointerEventsSystem.removeOnPointerUp(myEntity)
pointerEventsSystem.removeOnPointerHoverEnter(myEntity)
pointerEventsSystem.removeOnPointerHoverLeave(myEntity)
```

제거하면 엔티티의 호버 피드백이 더 이상 표시되지 않아야 하며, 엔티티도 더 이상 상호작용 가능하지 않아야 합니다.

각 remove 함수는 대응하는 register 함수가 추가한 항목만 제거합니다. 같은 콜백을 반복해서 등록하고 제거해도 엔티티에 오래된 항목이 남지 않습니다.

## 근접 상호작용

커서 조준이 아니라 플레이어의 근접을 기준으로 트리거되는 상호작용은 다음을 참조하세요: [**근접 이벤트**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events/). `pointerEventsSystem` 에는 동등한 헬퍼(`onProximityDown`, `onProximityUp`, `onProximityEnter`, 그리고 `onProximityLeave`)가 포함되어 있으며, 이 페이지에 설명된 함수와 같은 패턴을 따릅니다.

엔티티는 같은 이벤트 유형에 대해 커서 핸들러와 근접 핸들러를 둘 다 가질 수 있습니다. 두 콜백 모두 실행되며, 각자의 조건이 충족될 때 실행됩니다.

### 입력 동작의 데이터

눌린 버튼, 맞은 엔티티, 레이의 방향과 길이 등 입력 동작의 데이터를 가져옵니다. 다음을 참조하세요([문서 보기](https://github.com/decentraland/docs/tree/main/README.md)) 사용 가능한 모든 데이터에 대한 설명이 있습니다.

이 데이터를 가져오려면 콜백 함수에 매개변수를 전달하세요. 이 매개변수에는 입력 이벤트에 대한 전체 데이터 구조가 들어 있습니다.

```ts
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { button: InputAction.IA_PRIMARY } },
	function (cmd) {
		console.log(cmd.hit?.entityId)
	}
)
```

### 여러 버튼 처리

하나 이상 등록할 수 없습니다 `onPointerDown` 하나의 엔티티에 대해. 이상적으로는 다음을 사용해야 합니다: [시스템 기반](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/) 방식입니다. 이렇게 하면 원하는 만큼 다양한 입력을 처리하고 각 버튼에 대한 UI 호버 피드백 힌트를 표시할 수 있습니다.

대안으로 Register callback 방식을 사용하고 다음 필드를 `버튼` 다음으로 설정할 수 있습니다: `InputAction.IA_ANY`.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_ANY}},
  function (cmd) {
      if(cmd.button === InputAction.IA_POINTER){
        // do X
      } else if (cmd.button === InputAction.IA_PRIMARY){
        // do Y
      }
  }
)
```

이 방식은 이상적이지 않습니다. 호버 힌트는 하나의 문자열만 표시하고 어떤 동작을 활성화할지 지정하지 않기 때문입니다. 또한 이렇게 하면 이동 키를 포함한 모든 입력 동작에 대해 콜백 함수가 실행되므로, 관심 있는 동작만 필터링해야 합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
