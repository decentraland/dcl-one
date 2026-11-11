> 씬의 기본 화면 터치 컨트롤을 설정합니다.

# 화면 내 컨트롤

모바일 클라이언트에서 플레이어는 네이티브 화면 조작 세트 — 가상 조이스틱, 조준선, 그리고 버튼형 게임패드 — 를 통해 당신의 씬과 상호작용합니다. 이 `TouchScreenControls` 컴포넌트는 씬이 해당 HUD를 재구성할 수 있게 해줍니다: 요소를 정리하고, 조이스틱이나 조준선을 숨기고, 큰 중앙 버튼의 동작을 바꾸고, 버튼의 심볼을 자신만의 아이콘으로 바꾸거나, 버튼을 완전히 숨기고 자신만의 UI로 대체할 수 있습니다.

<figure><img src="/docs/_remote/8414c098957fe0e8df7af46b.webp" alt="The default mobile HUD: virtual joystick on the left, gamepad buttons on the right"><figcaption><p>커스터마이즈하기 전의 기본 화면 조작</p></figcaption></figure>


> [!NOTE]
> 이 컴포넌트는 플레이어가 씬 안에 있는 동안 자동으로 적용되고, 떠나는 순간 기본값으로 되돌아가므로 사용하지 않는 씬에는 영향을 주지 않습니다. 네이티브 화면 조작이 있는 플랫폼에만 영향을 주며, 데스크톱에서는 아무 동작도 하지 않고 VR에서도 효과가 없습니다.


## 기본적으로 표시되는 항목

SDK에서 `7.32.0`각 씬은 시작할 때 세 개의 게임패드 버튼이 표시된 상태로 시작합니다:

* `IA_JUMP`큰 중앙 버튼
* `IA_POINTER`상호작용 버튼
* `IA_PRIMARY`E 버튼

`IA_SECONDARY` 및 `IA_ACTION_3` 에서 `IA_ACTION_6` 은 기본적으로 숨겨져 있습니다. 다음이 필요할 때만 `TouchScreenControls` 사용하면 됩니다.


> [!WARNING]
> **📔 참고**: 세 버튼 기본값과 `showDefault()` 헬퍼에는 SDK가 필요 `7.32.0` 또는 이후 버전이 필요합니다. 더 이른 버전에서는 씬이 일부 버튼을 숨기지 않는 한 8개 버튼이 모두 표시되며, `showDefault()` 는 존재하지 않으므로 `showAll()` 버튼을 다시 표시하는 유일한 방법입니다.


SDK는 씬 시작 시, 첫 프레임 직전에 이 기본 세트를 한 번만 기록하며, 씬이 이미 자체 버튼을 선택한 경우에는 이를 건너뜁니다. 다음 중 하나라도 해당하면 씬이 이미 선택한 것으로 간주됩니다:

* 버튼 표시/숨김 헬퍼 중 하나가 이미 실행된 경우: `hideAll()`, `showAll()`, `showDefault()` 또는 `hide()`.
* 루트 엔터티의 컴포넌트에 이미 최소 하나의 버튼이 나열되어 있는 경우, 예를 들어 씬에서 다음으로 이를 작성했기 때문입니다: `createOrReplace` 의 최상위 수준에서 `index.ts`외부에서 `main()`.

`hideJoystick()`, `showJoystick()`, `hideCrosshair()`, `showCrosshair()` 및 `setMainAction()` 에는 해당하지 않습니다: 기본 버튼 세트는 여전히 함께 적용됩니다.

당신의 `main()` 함수는 그 기록 이후에 실행되므로, 거기서 하는 모든 작업은 기본 세트 위에 적용됩니다. 다음과 같은 헬퍼는 `hide()` 그것과 병합되고, 반면 `createOrReplace` 전체 목록을 당신의 것으로 교체합니다.

숨겨진 버튼을 다시 표시하려면 `showAll()` 을 8개 모두에 대해 호출하세요. `showDefault()` 기본 세트를 복원하면서 아이콘, 조이스틱, 조준선, 주요 동작은 그대로 유지합니다.

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// 1 / 2 / 3 / 4 버튼을 포함한 모든 화면 버튼을 표시
TouchScreenControls.showAll()
```

## 버튼 레이아웃이 작동하는 방식

게임패드 버튼은 하나의 **우선순위 스택**을 이룹니다. 순서는 고정되어 있습니다:

1. `IA_JUMP`
2. `IA_POINTER`
3. `IA_PRIMARY` (E)
4. `IA_SECONDARY` (F)
5. `IA_ACTION_3` (1)
6. `IA_ACTION_4` (2)
7. `IA_ACTION_5` (3)
8. `IA_ACTION_6` (4)

화면상의 위치도 고정되어 있습니다. 표시된 **버튼은** 스택의 맨 위부터 아래로 그 위치를 채우므로, 변경하는 것은 *어떤* 버튼이 표시되는지와 *어느 버튼이 앞에 오는지*이지, 순서가 아닙니다.

| 당신이… 하면                        | 조작은…                                                                          |
| ------------------------------ | ----------------------------------------------------------------------------- |
| **버튼 숨기기** (점프를 포함한 모든 버튼)     | 우선순위가 더 낮은 모든 버튼이 위로 올라와 빈자리를 채웁니다. 점프를 숨기면 `IA_POINTER` 가 중앙 자리를 차지합니다.      |
| **아무것도 변경하지 않음**               | 기본 세트가 적용됩니다: `IA_JUMP` 은 큰 중앙 버튼이고, `IA_POINTER` 및 `IA_PRIMARY` 주변 슬롯에 있습니다. |
| **주요 버튼 설정** 다음으로 `mainAction` | 해당 동작이 맨 앞으로 이동해 중앙 버튼이 됩니다. 다른 모든 버튼은 기존 순서를 유지합니다.                          |
| **숨겨진 상태이기도 한 주요 버튼 설정**       | 숨기기가 우선됩니다 — 버튼은 계속 숨겨진 상태입니다.                                                |
| **표시된 버튼을 5개 이하로 유지**          | 모두가 바로 표시됩니다(중앙 버튼과 그 주변 최대 4개); "+" 메뉴는 없습니다.                                |
| **표시된 버튼을 5개 초과로 유지**          | "+"가 마지막 슬롯을 차지하므로 4개가 직접 표시되고(중앙 버튼과 3개), 나머지는 "+" 오버플로 토글 뒤에 배치됩니다.         |

<figure><img src="/docs/_remote/ac9947bc2b985676d54e81a8.webp" alt="Three HUDs showing the gamepad reflowing as the number of visible buttons changes"><figcaption><p>표시된 개수에 따라 버튼이 재배치되는 방식. <strong>왼쪽(7개 버튼):</strong> 메인 버튼, 1–3의 호형 배열, 그리고 위로 올라가는 열에 4–7번 오버플로를 담는 "+". <strong>중앙(5개 버튼):</strong> 같은 호형 배열이지만, "+" 뒤에 더 짧은 오버플로 열(4–5)이 있습니다. <strong>오른쪽(4개 버튼):</strong> 4개 모두가 직접 표시되고 "+"는 사라집니다.</p></figcaption></figure>


> [!NOTE]
> 다음 `1`/`2`/`3`/`4` 버튼은 기본적으로 숨겨져 있습니다. `showAll()` 버튼을 다시 표시하지만, 그러면 8개 버튼이 보이게 되어 우선순위가 가장 낮은 버튼들은 "+" 뒤로 밀립니다. 번호가 매겨진 버튼을 화면에 직접 표시하려면 표시 개수를 5개 이하로 유지하세요: 씬이 실제로 사용하는 버튼만 표시하십시오.


## 자주 하는 작업

`TouchScreenControls` 편의 헬퍼 세트를 제공합니다. 각 헬퍼는 씬의 `RootEntity` (클라이언트가 이를 읽는 위치)에 컴포넌트를 기록하고 현재 값과 병합하므로, 어디서든 호출할 수 있습니다.

**주요 버튼 변경** — 큰 중앙 버튼이 다른 동작을 트리거하도록 만듭니다:

```ts
import { TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.setMainAction(InputAction.IA_PRIMARY)
}
```

**조이스틱 또는 조준선 숨기기** — 이동 스틱 및/또는 조준용 레티클을 제거하고, 대응하는 `show` 메서드로 다시 표시합니다:

```ts
TouchScreenControls.hideJoystick()
TouchScreenControls.hideCrosshair()

// 그리고 다시 표시하려면:
TouchScreenControls.showJoystick()
TouchScreenControls.showCrosshair()
```

**특정 버튼 숨기기** — 제거하려는 동작을 전달하세요(나머지는 위로 밀려 올라갑니다):

```ts
TouchScreenControls.hide([InputAction.IA_SECONDARY, InputAction.IA_JUMP])
```

**모든 버튼을 숨기거나, 모든 버튼을 표시하거나, 기본 세트로 돌아가세요:**

```ts
TouchScreenControls.hideAll()     // 게임패드 버튼이 전혀 없음
TouchScreenControls.showAll()     // 8개 버튼 전체
TouchScreenControls.showDefault() // 점프, 포인터, 주요 버튼만 표시
```

이 세 가지는 게임패드 버튼에만 영향을 줍니다. 숨겨진 조이스틱이나 조준선을 복원하지 않으며, 사용자 지정 아이콘이나 주요 동작도 변경하지 않습니다. 그런 경우에는 `showJoystick()` / `showCrosshair()` 를 사용하세요.

**버튼의 아이콘 바꾸기** — 완전한 제어(사용자 지정 아이콘, 여러 변경을 한 번에)를 위해서는 원시 컴포넌트를 `engine.RootEntity`:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideCrosshair: true,
		mainAction: InputAction.IA_PRIMARY,
		touchInputs: [
			{
				inputAction: InputAction.IA_PRIMARY,
				icon: { tex: { $case: 'texture', texture: { src: 'images/grab.png' } } },
			},
		],
	})
}
```

헬퍼 한눈에 보기:

| 헬퍼                      | 기능                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------ |
| `setMainAction(action)` | 큰 중앙 버튼이 어떤 동작을 트리거할지 설정합니다.                                                               |
| `hideJoystick()`        | 네이티브 가상 조이스틱을 숨깁니다.                                                                        |
| `showJoystick()`        | 네이티브 가상 조이스틱을 다시 표시합니다.                                                                    |
| `hideCrosshair()`       | 화면상의 조준선 / 레티클을 숨깁니다.                                                                      |
| `showCrosshair()`       | 화면상의 조준선 / 레티클을 다시 표시합니다.                                                                  |
| `hide(actions)`         | 지정한 게임패드 버튼을 숨깁니다(현재 구성에 병합됨).                                                             |
| `hideAll()`             | 모든 게임패드 버튼을 숨깁니다.                                                                          |
| `showAll()`             | 8개의 게임패드 버튼을 모두 표시합니다(버튼 숨김 목록을 지웁니다). **저장되지** 조이스틱/조준선에 미칩니다.                            |
| `showDefault()`         | 기본 세트를 복원합니다: 점프, 포인터, 주요 버튼은 표시하고, 보조 및 3\~6번 동작은 숨깁니다. 아이콘, 조이스틱, 조준선, 주요 동작은 그대로 유지합니다. |

## 속성

다음을 사용할 때는 이를 직접 작성하세요 `createOrReplace`:

| 속성              | 유형                                                                                                            | 설명                                                                                                                                                                                          |
| --------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hideJoystick`  | *부울*                                                                                                          | 네이티브 가상 이동 조이스틱을 숨깁니다.                                                                                                                                                                      |
| `hideCrosshair` | *부울*                                                                                                          | 화면상의 조준선 / 레티클을 숨깁니다.                                                                                                                                                                       |
| `mainAction`    | [*InputAction*](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) | 이 동작을 스택의 맨 앞으로 이동시켜 큰 중앙 버튼으로 만듭니다. 다른 버튼들은 순서를 유지합니다. 유효한 것은 게임패드 동작뿐입니다(아래 참조). 설정하지 않으면 첫 번째로 보이는 버튼(기본값으로 )이 앞에 옵니다. 참조:`IA_JUMP` 으로 [버튼 레이아웃이 작동하는 방식](#how-the-button-layout-works). |
| `touchInputs`   | *배열*                                                                                                          | 버튼별 재정의입니다. 이 필드를 작성하면 전체 목록이 교체되므로, 생략한 버튼은 기본 심볼로 표시되며, 기본적으로 숨겨져 시작하는 버튼도 포함됩니다.                                                                                                         |

각 `touchInputs` 항목에는 다음이 있습니다:

| 필드            | 유형                                                                                                            | 설명                                                                                                                                                                                                                                                                                        |
| ------------- | ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `inputAction` | [*InputAction*](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) | 이 항목이 어떤 화면 버튼을 구성하는지 지정합니다.                                                                                                                                                                                                                                                              |
| `hide`        | *부울*                                                                                                          | 이 버튼을 숨깁니다. 기본값은 `false` (표시됨). 어떤 버튼이든 숨길 수 있습니다, **포함하여 `IA_JUMP`** — 나머지는 그 자리를 메우도록 위로 이동합니다.                                                                                                                                                                                         |
| `icon`        | [*TextureUnion*](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_background/#background) (선택 사항)                  | 버튼 심볼을 씬 이미지로 덮어씁니다. 콘텐츠에 매핑된 `texture` 변형(씬에 포함된 이미지)을 사용하세요 — `src` (이미지 포함) — `{ tex: { $case: 'texture', texture: { src: 'images/grab.png' } } }`. 씬 콘텐츠 경로만 지원됩니다(외부 URL, 아바타 또는 비디오 텍스처는 지원하지 않음). 점프 버튼의 경우, 이 설정은 모든 동적 상태(점프 / 더블 점프 / 글라이드)를 대체합니다. 경로를 확인할 수 없으면 내장 심볼이 사용됩니다. |

## 어떤 동작이 어떤 버튼에 매핑되는지

다음 [`InputAction`](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 여기의 값은 전반적으로 사용되는 것과 동일합니다 [모바일 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/) 및 [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/). 다음은 화면 버튼에 매핑되는 동작입니다:

| InputAction                                                   | 화면 버튼            |
| ------------------------------------------------------------- | ---------------- |
| `IA_JUMP`                                                     | 큰 중앙 버튼(기본값)     |
| `IA_POINTER`                                                  | 상호작용 버튼          |
| `IA_PRIMARY`                                                  | E 버튼             |
| `IA_SECONDARY`                                                | F 버튼             |
| `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` | 1 / 2 / 3 / 4 버튼 |


> [!WARNING]
> `IA_ANY` 및 `IA_MODIFIER` 는 메타 값입니다 — 버튼에 매핑되지 않으며 여기서는 사용할 수 없습니다.


## 예시

조이스틱을 숨기려면 F 버튼과 번호 버튼들을 숨긴 상태로 유지하고, 중앙 점프 버튼에 씬에 포함된 사용자 지정 아이콘을 적용하세요:

```ts
import { engine, TouchScreenControls, InputAction } from '@dcl/sdk/ecs'

export function main() {
	TouchScreenControls.createOrReplace(engine.RootEntity, {
		hideJoystick: true,
		touchInputs: [
			// 이 목록은 기본 목록을 대체하므로, 일반적으로
			// 숨겨져 시작하는 버튼도 여기에 함께 나열해야 합니다
			{ inputAction: InputAction.IA_SECONDARY, hide: true },
			{ inputAction: InputAction.IA_ACTION_3, hide: true },
			{ inputAction: InputAction.IA_ACTION_4, hide: true },
			{ inputAction: InputAction.IA_ACTION_5, hide: true },
			{ inputAction: InputAction.IA_ACTION_6, hide: true },
			{
				inputAction: InputAction.IA_JUMP,
				icon: { tex: { $case: 'texture', texture: { src: 'images/banana.png' } } },
			},
		],
	})
}
```

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="A mobile HUD with the joystick hidden and the central jump button showing a custom banana icon"><figcaption><p>결과: 조이스틱이 사라지고, 번호 버튼은 숨겨져 있으며(그래서 "+"가 사라짐), 점프 버튼의 아이콘이 바뀝니다</p></figcaption></figure>

네이티브 조작을 완전히 대체하려면 여기에서 이를 숨기고, 다음을 사용해 자신만의 터치 버튼을 만드세요 [UI 입력 바인딩](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_input_binding/).

## 관련 항목

* [UI 입력 바인딩](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_input_binding/)
* [모바일 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/)
* [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/)
* [코드에서 플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
