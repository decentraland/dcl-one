> 입력 동작을 직접 UI 요소에 연결하여 플레이어 입력을 처리하게 합니다.

# UI 입력 바인딩

`UiInputBinding` 임의의 UI 요소를 조작 버튼으로 바꿉니다. 요소를 하나 이상의 [`InputAction`](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons)s를 바인딩하고, 터치나 포인터로 눌려 있는 동안에는 해당 동작이 유지되어 로컬 플레이어의 입력(이동, 점프)과 씬의 모든 [`InputAction`](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 리스너를 구동합니다. 이는 기본 온스크린 버튼과 정확히 같습니다.

자신만의 터치 컨트롤을 만드는 데 사용하세요. 보통 다음과 함께 사용됩니다. [온스크린 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/): 기본 버튼을 숨기고 그 자리에 직접 만든 버튼을 배치하세요.

<figure><img src="/docs/_remote/19dc4254d88476f6abb9fbcf.webp" alt="A retro-style gamepad assembled from custom UI elements, each wired to an input action"><figcaption><p>UI 요소로 만든 완전한 커스텀 게임패드 — 각 버튼은 입력 동작에 바인딩된 UI 엔티티입니다</p></figcaption></figure>

## UI 요소에 동작 바인딩하기

추가하세요 `uiInputBinding` 속성을 당신의 `.tsx` UI에 추가하고, 눌려 있는 동안 유지할 동작을 나열하세요. 이 prop은 모든 UI 요소([`UiEntity`](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#ui-entities), [`버튼`](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#ui-entities), [`Label`](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#ui-entities), …)에서 사용할 수 있으며, 이는 `uiTransform` 및 `uiBackground`.

**누르고 있는 동안 플레이어를 앞으로 이동시키는 버튼:**

```tsx
import { InputAction } from '@dcl/sdk/ecs'
import { Button } from '@dcl/sdk/react-ecs'

export const forwardButton = () => (
	<Button
		value="▲"
		uiTransform={{ width: 100, height: 100 }}
		uiInputBinding={{ actions: [InputAction.IA_FORWARD] }}
	/>
)
```

## 커스텀 컨트롤 클러스터를 만들기

여러 바인딩된 요소를 결합해 완전한 조작 체계를 구성하세요.

**이동 버튼과 발동되는 동작 버튼: `IA_PRIMARY`:**

```tsx
import { InputAction } from '@dcl/sdk/ecs'
import { Button, UiEntity } from '@dcl/sdk/react-ecs'

export const customControls = () => (
	<UiEntity uiTransform={{ width: 240, height: 120 }}>
		<Button
			value="▲"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_FORWARD] }}
		/>
		<Button
			value="Use"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_PRIMARY] }}
		/>
	</UiEntity>
)
```

바인딩된 동작은 기본 버튼과 똑같이 작동합니다: `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` 아바타를 이동시키며, 모든 동작은 씬의 [`InputAction`](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 리스너에서 읽을 수 있습니다. prop(또는 컴포넌트)을 제거하면 바인딩이 해제됩니다.

## 속성

| 속성        | 유형                                                                                                               | 설명                                                                                                                                                                                                                                                                |
| --------- | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `actions` | 배열 [`InputAction`](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) | 이 요소를 누르고 있는 동안 유지되는 입력 동작입니다. 다음을 참조하세요. [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/) 의 전체 목록은 [`InputAction`](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 값을 참조하세요. |


> [!NOTE]
> 이를 다음과 함께 사용하세요. [`TouchScreenControls`](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/) 기본 컨트롤을 숨기고 직접 만든 터치 UI로 대체하세요 — 완전히 커스텀된 조작 체계를 제공하는 권장 방식입니다.


## 관련

* [온스크린 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/)
* [UI 버튼 이벤트](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_button_events/)
* [모바일에서의 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/)
* [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/ui_input_binding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
