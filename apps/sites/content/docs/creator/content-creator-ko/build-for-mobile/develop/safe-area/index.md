> 씬 UI가 모바일에서 안전하게 배치될 수 있는 영역 — 기기의 하드웨어 여백과 클라이언트 자체 컨트롤을 피한 곳.

# 모바일 안전 영역

휴대폰에서는 서로 관련 없는 두 가지가 UI가 안전하게 사용할 수 있는 화면 공간을 차지합니다:

* **기기의 하드웨어 여백** — 노치나 카메라 컷아웃, 상태 표시줄, 홈 인디케이터, 둥근 모서리입니다. 그 아래에 그려진 UI는 일부가 가려지거나 탭하기 어려워집니다.
* **Decentraland 클라이언트 자체의 컨트롤** — 조이스틱, 채팅, 프로필, 카메라 컨트롤, 액션 버튼입니다. 그 아래에 배치된 씬 UI는 시각적으로 충돌하고 같은 탭 입력을 두고 경쟁합니다.

둘 중 어느 것도 직접 측정할 필요는 없습니다. 클라이언트가 런타임에 두 영역을 모두 보고하고, SDK가 사용자가 선택한 영역 안에 UI를 배치하며, 다음 렌더러 옵션과 함께: [`screenInset`](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-inset-area) 렌더러 옵션:

| 원하는 것                       | 전달할 값                               |
| --------------------------- | ----------------------------------- |
| 하드웨어 여백을 피함                 | 없음 — `screenInset: 'device'` 기본값입니다 |
| 하드웨어 여백을 피함 **및** 클라이언트 컨트롤 | `screenInset: 'interactable'`       |
| 화면 전체, 여백 처리는 직접 맡음         | `screenInset: 'none'`               |

```ts
// 모바일 컨트롤과 충돌하면 안 되는 씬 UI에 권장
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```


> [!WARNING]
> **클라이언트 지원**: `'interactable'` 모바일 클라이언트가 필요합니다 `1.12.1` 이상이어야 합니다. 더 오래된 클라이언트에서는 해당 영역이 0으로 보고되며 UI는 전체 화면을 덮는 방식으로 돌아갑니다. 같은 릴리스에서 다음도 정규화됩니다: `'device'` Android와 iOS 간의 영역.
> 
> **데스크톱에서는 아무 일도 없는 것이 아닙니다** — 데스크톱 클라이언트는 화면의 대략 왼쪽 25%를 자체 UI용으로 예약하므로 `'interactable'` 데스크톱 레이아웃도 바뀝니다. 다음을 조건으로 분기하세요: [`isMobile()`](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 휴대폰에서만 인셋을 적용하고 싶다면.


참조 [화면 인셋 영역](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-inset-area) 전체 참고 자료: 실제 기기에서 세 영역을 나란히 본 모습, 렌더러별 동작, 그리고 다음과의 관계: [가상 화면](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#default-virtual-size).

## 클라이언트 컨트롤이 있는 위치

`'interactable'` 영역을 제공합니다 **각 explorer가 씬 UI용으로 지정한** — 이는 모든 클라이언트 컨트롤이 그 밖에 있다는 보장은 의도적으로 하지 않습니다. 모바일 클라이언트에서는 기기 안전 영역에서 왼쪽 열(채팅, 프로필, 조이스틱, 이모트)을 뺀 영역이며, 오른쪽 아래의 액션 버튼은 그 영역 위에 그려집니다 **의도적으로**.

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the mobile client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> 모바일 클라이언트에서 <code>1.12.1</code>. 자주색 사각형은 씬 UI이고, 초록색 테두리는 상호작용 가능한 영역입니다. 왼쪽 열은 제외되며, 오른쪽 아래의 액션 버튼은 그 영역 위에 놓입니다.</p></figcaption></figure>

따라서 `'interactable'`다음 위치는 혼잡한 곳으로 간주하세요:

* **오른쪽 아래** — 액션 버튼과 상호작용 버튼이 위에 그려집니다. 여기에 배치한 요소도 여전히 사용할 수 있지만, 탭 입력을 두고 경쟁합니다.
* **오른쪽 위** — 프로필과 카메라 컨트롤이 영역 바로 바깥에 있으므로, 그 모서리에 붙은 UI는 클라이언트 HUD의 일부처럼 보입니다.

영역에 무엇이 포함되는지는 각 explorer의 결정 사항이며 버전마다 달라질 수 있습니다. 런타임에 읽고, 하드코딩하지 말며, 대상 플랫폼에서 확인하세요.

## 씬 UI를 배치할 위치

* **화면 중앙** — 사용자가 읽고 응답해야 하는 대화상자나 모든 요소.
* **상단 중앙** — 동작이 필요 없는 메시지, 상태, 알림.
* **하단 중앙, 상호작용 버튼 위** — 상황에 따라 달라지는 힌트.
* **오른쪽 아래 모서리는 아님** — 상호작용 가능한 영역 안쪽이라도, 그 자리는 액션 버튼의 몫입니다.

## 대신 UI의 일부를 감싸세요

다음으로 제외했다면 `screenInset: 'none'` 그리고 UI의 일부만 보호하고 싶다면, 다음으로 감싸세요: `ScreenInsetArea` 또는 `InteractableArea` 컴포넌트입니다. 각각은 대응하는 영역을 읽어 자식에 적용합니다. 다음에서 가져오세요: `@dcl/sdk/react-ecs`:

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity, ScreenInsetArea } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUi() {
  ReactEcsRenderer.setUiRenderer(() => (
    <ScreenInsetArea>
      {/* 100% × 100% 크기의 자식은 안전 인셋 영역을 정확히 채웁니다 */}
      <UiEntity
        uiTransform={{ width: '100%', height: '100%' }}
        uiBackground={{ color: Color4.create(0, 0, 0, 0.5) }}
      />
    </ScreenInsetArea>
  ), { screenInset: 'none' })
}
```

두 컴포넌트는 클라이언트가 보고하는 값을 사용해 절대 위치를 잡으므로 `positionType` 및 `위치` 그들의 필드 `uiTransform` 는 예약되어 있습니다 — 여기에 설정한 값은 무시됩니다. 그 외의 모든 `uiTransform` 속성(`padding`, `flexDirection`, `alignItems`, …)와 UI 컴포넌트(`uiBackground`, `onMouseDown`, …)는 평소처럼 작동합니다. 회전하거나 시스템 바가 나타나거나 사라져 영역이 바뀌면 자동으로 반응하며, 다음을 보정합니다: [UI 배율](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale), 따라서 가상 화면 크기와 상관없이 여백이 올바르게 적용됩니다.


> [!WARNING]
> **📔 인셋을 두 번 적용하지 마세요:** 위의 `screenInset: 'none'` 위 코드 조각의 이 부분이 중요합니다. 렌더러도 동일한 인셋을 적용하는데 UI를 컴포넌트로 감싸면, UI가 두 배의 여백만큼 안쪽으로 밀립니다. 둘 중 하나만 선택하세요.



> [!NOTE]
> **📱 모바일 전용:** 기기 인셋은 다음에서만 실제 값이 있습니다: **모바일 클라이언트**. 다음에서는 **데스크톱 클라이언트** 그 값은 `(0, 0, 0, 0)`0이므로 `screenInset: 'device'` 과 `ScreenInsetArea` 컴포넌트는 거기서 효과가 없으며 UI는 그것이 없는 것과 정확히 동일하게 렌더링됩니다. 상호작용 가능한 영역은 기기 인셋과 달리 *가* 데스크톱에서도 0이 아닙니다.


## 중요한 이유

클라이언트 컨트롤과 겹치는 씬 UI는 다음과 같은 문제가 생깁니다:

* 조이스틱, 상호작용 버튼, 또는 카메라 컨트롤 뒤에 일부가 가려집니다.
* 해당 컨트롤과 탭 입력을 두고 경쟁합니다 — 플레이어가 하나 또는 다른 하나를 실수로 누르게 됩니다.
* 모바일에서 씬이 깨진 것처럼 느껴지게 하며, 이는 노출과 유지율에 악영향을 줍니다.

항상 실제 기기에서 다음을 사용해 확인하세요: [미리보기 QR 코드](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/).

## 관련

* [화면 인셋 영역](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-inset-area) — 전체 `screenInset` 참고 자료.
* [모바일 UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/)
* [코드에서 플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) — 다음을 사용해 `isMobile()` 레이아웃을 전환하세요.
* [화면 UI](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/)
* [UX 및 UI 가이드](/docs/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop/safe-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
