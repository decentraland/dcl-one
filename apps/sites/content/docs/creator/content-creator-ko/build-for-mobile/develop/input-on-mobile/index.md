> 입력 동작이 Decentraland 모바일 클라이언트의 터치와 어떻게 매핑되는지.

# 모바일 입력

Decentraland의 입력 시스템은 기기와 무관하게 설계되었습니다. 동일한 `InputAction` 데스크톱에서 SDK가 노출하는 enum은 모바일의 화면 내 컨트롤에서도 전달되므로, 대부분의 씬은 변경 없이 작동합니다. 다만 터치용으로 빌드할 때 알아두면 좋은 몇 가지 규칙과 주의할 점이 있습니다.

전체 입력 모델은 [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/). 이 페이지는 모바일에만 해당하는 내용을 다룹니다.

<figure><img src="/docs/_remote/d29f6b96a1d1db9049d689d6.webp" alt="The on-screen controls with the + menu expanded, showing every input button"><figcaption><p>"+" 메뉴가 펼쳐진 화면 내 컨트롤 — 모든 입력 동작이 보임</p></figcaption></figure>

## 터치에 매핑되는 항목

모든 `InputAction` 데스크톱에 있는 것은 모바일 클라이언트에서도 사용할 수 있습니다. 화면 내 컨트롤은 단순히 터치를 동일한 SDK 입력으로 전달합니다. 즉, 표준 `InputAction` 값에 의존하는 모든 씬은 모바일에서도 별도 수정 없이 작동합니다.

화면 내 컨트롤은 다음과 같이 매핑됩니다:

* **화면 조이스틱** — 제어함 `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` 및 아바타 이동.
* **상호작용 버튼(오른쪽 아래)** — 실행함 `IA_POINTER` (데스크톱의 왼쪽 마우스 버튼) 플레이어가 조준하고 있는 대상에 대해.
* **E 버튼** — 실행함 `IA_PRIMARY` (데스크톱의 `E` 키).
* **F 버튼** — 실행함 `IA_SECONDARY` (데스크톱의 `F` 키). 기본적으로 숨겨져 있습니다(아래 참조).
* **점프 버튼** — 실행함 `IA_JUMP` (데스크톱의 `Space` 키). HUD에서 가장 큰 버튼이며 항상 쉽게 닿을 수 있습니다.
* **1 / 2 / 3 / 4 버튼** — 실행함 `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` 을 각각 수행합니다. 기본적으로 숨겨져 있습니다(아래 참조).
* **카메라 드래그** — 카메라를 회전합니다. 다음 항목으로는 노출되지 않습니다: `InputAction`.

## 컨트롤 사용자 지정

씬에서는 화면 내 컨트롤을 다시 구성할 수 있습니다. 조이스틱이나 십자선을 숨기고, 점프를 포함한 어떤 버튼이든 숨기거나, 중앙 버튼의 동작을 바꾸거나, 버튼 아이콘을 교체하거나, 기본 컨트롤을 완전히 사용자 지정 UI로 대체할 수 있습니다.  [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/) 및 [UI 입력 바인딩](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_input_binding/).

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="Customized on-screen controls"><figcaption><p>사용자 지정 HUD: 아이콘이 바뀐 중앙 버튼이 다른 동작에 매핑됨</p></figcaption></figure>

## 모바일에서 주요 동작에 사용을 피해야 하는 입력

모든 `InputAction` 은 화면 내 버튼으로 연결할 수 있지만, SDK 7.32.0 이상으로 만든 씬은 시작 시 상호작용 버튼, E 버튼, 점프 버튼의 세 개만 보이도록 되어 있습니다. `IA_SECONDARY` (F)와 `IA_ACTION_3`–`IA_ACTION_6` (데스크톱의 `1`/`2`/`3`/`4` 버튼)은 **기본적으로 숨겨져 있습니다**, 따라서 씬에서 표시하지 않으면 그중 하나에 바인딩된 주요 동작은 휴대폰에서 버튼이 없습니다. 이전 SDK에서는 8개 버튼이 모두 표시되지만, 우선순위가 낮은 버튼은 게임플레이 중에 쉽게 닿을 수 없는 "+" 오버플로 뒤에 있습니다.

씬에서 다음을 사용해 명시적으로 표시하지 않는 한 [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/), 주요 동작을 다음에 바인딩하지 마세요:

* `IA_SECONDARY` — 데스크톱의 `F` 키 / `F` 모바일의 버튼
* `IA_ACTION_3` — 데스크톱의 `1` 키 / `1` 모바일의 버튼
* `IA_ACTION_4` — 데스크톱의 `2` 키 / `2` 모바일의 버튼
* `IA_ACTION_5` — 데스크톱의 `3` 키 / `3` 모바일의 버튼
* `IA_ACTION_6` — 데스크톱의 `4` 키 / `4` 모바일의 버튼

대신 다음을 권장합니다:

* `IA_POINTER` 메인 탭 / 상호작용 버튼에는
* `IA_PRIMARY` E 버튼 동작에는
* `IA_JUMP` 동작이 점프와 자연스럽게 대응될 때(모바일 HUD에서 가장 크고 가장 쉽게 닿을 수 있는 버튼이기 때문)
* 근접 기반 트리거(참조: [Proximity Events](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events/)) 플레이어가 엔티티에 접근하면 동작이 자동으로 실행되어야 할 때

## 커서 잠금

이 [`PointerLock` 컴포넌트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#lock-or-unlock-the-cursor) 는 데스크톱 클라이언트의 개념입니다(잠긴 마우스 커서 vs. 잠금 해제된 마우스 커서). 모바일의 터치에는 적용되지 않으며 씬에 그대로 두어도 안전합니다. 모바일에서는 아무 영향이 없습니다.

## 관련 항목

* [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/)
* [UI 입력 바인딩](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_input_binding/)
* [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/)
* [Proximity Events](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events/)
* [코드에서 플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/)
* [모바일용 UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
