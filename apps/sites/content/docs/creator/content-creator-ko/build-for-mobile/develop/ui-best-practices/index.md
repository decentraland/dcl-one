> 모바일에서 잘 작동하는 씬 UI를 디자인하기 위한 권장 사항과 금지 사항입니다.

# UI 모범 사례

아직 Decentraland 모바일 UI에 대한 단 하나의 검증된 정답은 없습니다 — 플랫폼은 새롭고 우리는 아직 반복 개선 중입니다. 이 페이지의 권장 사항은 실제 씬 작업에서 추려낸 현재의 모범 사례입니다. 출발점으로 삼고 실제 기기에서 테스트하세요.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/5OmTTzpPdDc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=5OmTTzpPdDc>">Watch on YouTube</a></p>



## 해야 할 일

* **모바일 전용 UI를 디자인하세요**, 또는 화면 크기와 플랫폼에 따라 UI를 다르게 하세요. 다음을 사용하세요 [`isMobile()`](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 로 분기하세요.
* **중요한 UI는** [**안전 영역**](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/)**.** 기기의 하드웨어 여백(노치, 상태 표시줄, 홈 인디케이터)은 기본적으로 자동으로 처리됩니다 — 단, 다음으로 이를 끄지 마세요 `screenInset: 'none'`. 클라이언트 자체의 컨트롤은 별도의 영역이며, `screenInset: 'interactable'` 가 이 역할의 일부를 담당합니다: 탐색기가 씬 UI용으로 지정한 직사각형 안에 UI를 배치합니다. 모바일에서는 왼쪽 컨트롤을 비우고, 오른쪽 아래의 액션 버튼은 설계상 그 위에 그려지므로 [그 모서리](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/#where-the-client-controls-live) 를 염두에 두세요. `'interactable'` 모바일 클라이언트가 필요합니다 `1.12.1` 또는 그 이상 — 이전 클라이언트는 여백을 전혀 보고하지 않으며 UI가 전체 화면을 덮습니다.
* **옵션을 최소화하세요.** 플레이어가 지금 당장 필요한 것만 보여 주고 나머지는 점진적으로 공개하세요.
* **행동이 필요한 대화상자는 화면 중앙에 배치하세요** — 플레이어가 읽고 응답해야 하는 곳이라면 어디든 좋습니다.
* **행동이 필요 없는 메시지는 상단 중앙에 배치하세요** — 상태, 알림, 주변 정보.
* **전체 화면 UI 뒤로 터치 컨트롤을 숨기세요.** 점수판, 상점, 결과 패널이 열려 있는 동안 플레이어는 점프하거나 E를 누를 필요가 없습니다. 다음을 참조하세요 [액션 버튼 위의 전체 화면 UI](#full-screen-ui-over-the-action-buttons).

## 하지 말아야 할 일

* **가상 화면을 염두에 두지 않고 UI 요소를 순수하게 픽셀 기준으로만 크기 지정하지 마세요.** 픽셀 값은 다음을 기준으로 스케일됩니다 `virtualWidth` / `virtualHeight` 참조 해상도는 다음에 설명되어 있습니다 [화면 내 UI](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) — `1600x720` 모바일에서는 기본적으로, `1920x1080` 데스크톱과 웹에서는 다르게 적용됩니다. 픽셀 값이 어떤 기준 해상도를 기준으로 작성되었는지 파악하고, 기본값이 아니라면 명시적으로 전달한 뒤 플랫폼을 고려한 크기 조정과 함께 사용하세요. 가상 화면은 (크기를 `0`)
* **안전 영역 밖에 요소를 배치하지 마세요.** 클라이언트의 컨트롤과 충돌하게 됩니다.
* **UI를 왼쪽 위나 왼쪽 가장자리에 고정하지 마세요.** 그곳에는 클라이언트가 채팅, 프로필, 가상 조이스틱을 모두 씬 UI 위에 그립니다. 대신 오른쪽이나 가운데에 고정하거나 `screenInset: 'interactable'`. 다음을 참조하세요: [UI를 절대로 왼쪽 위에 고정하지 마세요](/docs/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).
* **상호작용 가능한 요소를 오른쪽 아래 모서리에 두지 마세요.** 액션 버튼이 그 영역 위에 그려지며, 그곳을 탭하면 씬이 아니라 클라이언트의 버튼으로 입력됩니다.
* **작은 버튼에 의존하지 마세요.** 작은 대상은 터치스크린에서 누르기 어렵습니다.
* **키 동작을 `IA_SECONDARY` 또는 `IA_ACTION_3`–`IA_ACTION_6`** (데스크톱의 `F` 및 `1`/`2`/`3`/`4` 키보드의 키)에 바인딩하지 마세요. 단, 씬에 해당 버튼이 표시되는 경우는 예외입니다. 모바일에서는 기본적으로 숨겨집니다(SDK 7.32.0+). 다음을 참조하세요 [모바일 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/).

## 액션 버튼 위의 전체 화면 UI

`screenInset: 'interactable'` 왼쪽 컨트롤을 가려 주지만, 오른쪽 아래의 액션 버튼은 설계상 그 영역 위에 그려집니다. 해당 영역을 가득 채우는 패널도 그곳에서는 여전히 가려지며, 그 영역을 탭하면 UI 대신 클라이언트 버튼이 활성화됩니다.

다음의 경우 **게임플레이 HUD** 를 플레이어가 움직이는 동안에도 계속 사용할 수 있어야 한다면, 오른쪽 아래 모서리에서 벗어나게 하세요. 오른쪽 위나 상단 중앙에 고정하세요.

다음의 경우 **게임플레이를 방해하는 UI**, 예를 들어 점수판, 상점, 결과 화면처럼 패널이 열려 있으면, 열려 있는 동안에는 터치 컨트롤을 숨기고 닫을 때 다시 표시하세요:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

function openScoreboard() {
  TouchScreenControls.hideAll()
  TouchScreenControls.hideJoystick()
  // ... UI를 표시
}

function closeScoreboard() {
  TouchScreenControls.showDefault()
  TouchScreenControls.showJoystick()
  // ... UI를 숨김
}
```

다음을 사용하세요 `showDefault()` 대신 `showAll()` 버튼을 복원하려면: 씬은 시작할 때 점프, 포인터, 기본 버튼만 보이므로 `showAll()` 플레이어가 한 번도 갖지 않았던 다섯 개의 버튼을 다시 불러오게 됩니다. 세 버튼 기본값과 `showDefault()` 둘 다 SDK 7.32.0 이상이 필요합니다. 이전 SDK에서는 여덟 개의 버튼이 모두 표시되며 `showAll()` 이것이 그것들을 복원하는 유일한 방법입니다. `showDefault()` 게임패드 버튼만 복원하므로 `showJoystick()`. 두 도우미 모두 데스크톱에서는 아무것도 하지 않으므로 플랫폼에 따라 분기할 필요가 없습니다. 다음을 참조하세요 [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/) 전체 구성 요소는 다음을 참조하세요.

패널이 열려 있는 동안 아바타도 움직이지 않게 하려면, [입력 수정자](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#freeze-the-player) 를 추가하세요.

## 크기 조정

예전의 경험칙인 *데스크톱에서 디자인한 뒤 모바일에서는 UI 크기를 3배로 키운다* 는 두 번의 SDK 변경 이전 규칙이므로 이제는 무작정 적용해서는 안 됩니다. 그 3배 중 대부분은 이제 자동으로 적용됩니다:

* **`devicePixelRatio` 는 UI 레이아웃에 관여하지 않습니다.** 픽셀 크기의 UI는 이제 `devicePixelRatio` 배 더 크게 렌더링됩니다 — 일반적으로 **휴대폰에서는 2\~3배**이고, 레티나 노트북에서도 마찬가지입니다. 이를 끄는 옵션은 없습니다.
* **가상 화면은 플랫폼별 기본값을 사용합니다**, `1600x720` 모바일에서는 `1920x1080` 데스크톱과 웹에서는

데스크톱 크기에서 시작한 뒤, 실제 기기에서 측정하세요 — 다음을 참조하세요 [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/) — 그리고 탭 대상이나 텍스트가 실제로 부족한 부분에서만 확대하세요. 다음과 같이 분기하세요 [`isMobile()`](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 두 가지가 정말 다를 필요가 있을 때.

## 관련 항목

* [누락된 기능](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/missing-features/)
* [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/)
* [코드에서 플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/)
* [화면 내 UI](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/)
* [UX 및 UI 가이드](/docs/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
