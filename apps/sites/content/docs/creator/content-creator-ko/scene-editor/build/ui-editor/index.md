> 코드 대신 캔버스에서 Creator Hub를 사용해 씬의 화면 UI를 시각적으로 디자인하세요.

# UI 에디터

UI 편집기를 사용하면 직접 레이아웃 코드를 작성하는 대신 위젯을 캔버스로 끌어다 놓아 씬의 화면 인터페이스를 만들 수 있습니다. 실제 파일을 씬에 기록하므로, 여기서 만든 모든 것은 나중에 코드로 계속 편집할 수 있는 일반 SDK7 UI입니다.

## 열기

다음을 사용하세요 **2D** / **3D** 편집기 왼쪽 패널 맨 위의 전환 스위치입니다. **3D** 이미 알고 있는 씬 캔버스입니다. **2D** UI 편집기입니다.

![3D가 선택된 2D 및 3D 탭 전환입니다.](/docs/_remote/4e696bf6f0bb7532d974ec02.webp)


> [!WARNING]
> **📔 참고**: UI 편집기에는 `@dcl/sdk` 씬에 7.26.0 이상 버전이 필요합니다. 더 오래된 씬에서는 **2D** 탭에 **UI 편집기를 사용할 수 없음** 알림이 캔버스 대신 표시되며, **SDK 업데이트** 씬을 대신 업그레이드하는 버튼과 **나중에** 알림을 닫는 버튼이 있습니다.


Creator Hub는 씬을 2D나 3D 중 어느 상태로 두고 나갔는지 기억하고 같은 방식으로 다시 엽니다. 이 설정은 프로젝트별로 저장되며 씬과 함께 게시되지 않습니다.

Bevy 씬 렌더러를 사용하는 경우 **2D** 실행 중인 씬을 일시 중지하고, 다시 돌아가면 **3D** 이전부터 실행 중이었다면 다시 이어서 실행합니다.

## 첫 UI 만들기

1. 로 전환하세요 **2D** 탭을 여세요.
2. 클릭하세요 **새 GUI**, 빈 캔버스 중앙의 버튼 또는 **+** 옆의 **GUIs** 왼쪽 패널에 있는 버튼입니다. GUI는 하나의 UI 구성 요소로, 별도의 파일에 저장됩니다.
3. 아래쪽 팔레트에서 위젯을 캔버스로 끌어오세요. **전체 화면** 는 처음 시작하기 좋은 위젯입니다. 화면 전체를 덮는 루트를 제공합니다. 그런 다음 그 안에 다른 위젯을 놓으세요.
4. 위젯을 선택하고 오른쪽 패널에서 속성을 설정하세요.

저장 버튼은 없습니다. 편집기가 작업하면서 변경 사항을 디스크에 기록하며, 도구 모음 왼쪽의 배지에는 **모든 변경 사항 저장됨**.

![2D 모드의 UI 편집기: 왼쪽의 GUIs 및 Nodes 패널에는 Label과 Button을 포함한 Container가 있는 MainUI라는 GUI가 표시되고, 가운데 캔버스에는 Button이 선택된 상태로 크기 조절 핸들이 둘러싼 컨테이너가 보이며, 오른쪽의 Properties 탭에는 Button의 위치와 레이아웃이 표시되고, 아래쪽에는 위젯 팔레트가 있습니다.](/docs/_remote/3958b77d4c214591522f3386.webp)

## 패널

* **왼쪽**: **GUIs**, UI 구성 요소 목록이며, **Nodes**, 선택한 항목의 트리입니다. 검색 상자가 둘 다를 필터링합니다. 노드 위에 마우스를 올리면 잠금, 숨기기 또는 삭제할 수 있습니다.
* **오른쪽**: 하나의 **속성** 탭이 선택한 노드에 대해 있고, 하나의 **로직** 탭을 여세요.
* **아래쪽**: 위젯 팔레트입니다. 카드를 캔버스로 끌어다 놓아 추가하세요.
* **가운데**: 캔버스입니다. 노드를 끌어 이동하고, 핸들로 크기를 조절하며, 마우스 휠이나 드래그로 자유롭게 이동할 수 있습니다.

캔버스 오른쪽 아래 모서리에는 두 개의 떠 있는 제어 그룹이 있습니다:

* 다음 **미리보기** 그룹은 모바일 HUD 가이드와 안전 영역 가이드를 전환하며, 다음 사이를 전환합니다. **데스크톱 미리보기** 및 **모바일 미리보기**.
* 다음 **확대/축소** 그룹에는 빼기 및 더하기 버튼과 백분율 입력란이 있습니다. 백분율을 클릭해 10에서 200 사이의 정확한 확대/축소 수준을 입력한 다음 Enter를 누르거나 다른 곳을 클릭해 적용하고, Escape로 취소할 수 있습니다. 200%에 도달하면 더하기 버튼은 비활성화됩니다.

<div align="left"><img src="/docs/_remote/589a76a30cb6467ad0c864b3.webp" alt="UI 편집기의 왼쪽 패널에는 2D 및 3D 탭, 검색 상자, MainUI를 포함한 GUIs 목록, Container가 Label과 Button을 포함한 Nodes 트리가 있으며, Button 행에는 잠금, 숨기기, 삭제 아이콘이 표시됩니다." width="220"> <img src="/docs/_remote/e71a2f9dd042f80664595bb6.webp" alt="선택한 Button에 대한 UI 편집기의 Properties 탭에는 Visibility, Interaction States, Constraints, Position 및 Z-Index가 있는 Position 섹션과 Size, Min Size, Max Size, Padding 및 Margin이 있는 Layout 섹션이 있습니다." width="360"></div>

## 위젯

![UI 편집기 아래쪽의 위젯 팔레트에는 Full Screen, Container, Image, Label, Button, Input, Dropdown 카드가 있습니다.](/docs/_remote/353fd7589685b5f5d6abd5f2.webp)

팔레트는 위젯을 세 가지 범주로 나눕니다:

* **컨테이너**: **전체 화면**, **컨테이너**, **이미지**
* **텍스트**: **레이블**, **버튼**
* **입력**: **입력**, **드롭다운**

**전체 화면** 는 들어간 곳을 가득 채우는 컨테이너이므로 화면을 덮어야 하는 UI의 시작점으로 좋습니다.

## 이동 및 크기 조절

도구 모음에는 세 가지 도구 모드가 있습니다:

* **자유**: 이동 및 크기 조절을 위한 범용 모드입니다.
* **이동**: 드래그만 가능합니다.
* **크기 조절**: 핸들만 사용할 수 있습니다.

위치는 10픽셀 격자에 맞춰집니다. 드래그하는 동안 **Shift** 를 누르면 격자를 무시하고 자유롭게 이동할 수 있습니다. 도구 버튼 옆의 작은 화살표를 누르면 **스냅** 체크박스가 열리며, 스냅을 완전히 끌 수 있습니다.

![2D 도구 모음에는 모든 변경 사항 저장됨 배지, 실행 취소 및 다시 실행, 자유, 이동, 크기 조절 도구, 그리고 스냅 드롭다운이 열려 체크된 스냅 옵션이 보이며, 그 뒤에 재생 및 정지 버튼이 있습니다.](/docs/_remote/5ab6e1170c733fa0011bba2b.webp)

## 레이아웃: 흐름 및 자유 배치

노드가 어디에 배치될지는 두 개의 별도 속성이 결정합니다. 이를 명확히 이해하면 많은 혼란을 줄일 수 있습니다.

**흐름** 는 하나의 **컨테이너** 에 설정되며 자식들을 어떻게 배치할지 결정합니다:

* **자유**: 자식은 배치한 위치에 그대로 있습니다.
* **행** / **행 반전**: 자식은 왼쪽에서 오른쪽으로, 또는 오른쪽에서 왼쪽으로 배치됩니다.
* **열** / **열 반전**: 자식은 위에서 아래로, 또는 아래에서 위로 쌓입니다.
* **자식 줄바꿈**: 자식이 맞지 않을 때 여러 줄로 넘어가게 합니다.

**레이아웃 흐름 무시** 는 하나의 **자식** 에 설정되며, 해당 노드가 부모의 배치에서 벗어날지 결정합니다. 끔이면 부모가 위치를 정하고, 켬이면 Anchor와 Position 값으로 직접 고정합니다.

이로 인해 두 가지 결과가 생깁니다:

* 위젯을 하나의 **자유** 컨테이너에 놓으면 놓은 정확한 위치에 그대로 있습니다.
* 방향이 있는 컨테이너에 놓으면 흐름에 합류해 줄에서 자리를 차지합니다.

컨테이너를 **자유** 로 바꾸면 기존 자식들이 현재 있는 위치에 고정되어 아무것도 갑자기 움직이지 않습니다.

GUI의 루트는 항상 자유롭게 배치됩니다.

## 알아두면 좋은 다른 속성

* **불투명도**: 100%는 완전히 불투명하고 0%는 완전히 투명합니다. 기본값은 100%입니다.
* **씬 인셋**: GUI가 화면의 어느 부분에 놓일지입니다.

  * **전체 화면** 렌더링 가능한 전체 화면을 사용합니다.
  * **상호작용 가능한 안전 영역** 채팅, 미니맵, HUD 표시기 같은 클라이언트 자체 인터페이스를 피합니다.
  * **기기 안전 영역** 노치나 시스템 바 같은 물리적 가림도 피합니다. 이 옵션은 캔버스가 모바일 미리보기일 때 제공됩니다.

  편집기가 작성하는 코드에서는 이 세 가지 선택이 `screenInset: 'none'`, `'interactable'` 및 `'device'`. 다음을 참조하세요: [화면 inset 영역](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-inset-area).

GUI의 루트에 고정 픽셀 단위의 너비와 높이를 주면, 캔버스는 이를 화면이 아니라 정확히 그 크기의 아트보드로 표시합니다.

## 휴대폰에서 UI를 확인하세요

캔버스 오른쪽 아래의 제어는 미리보기를 데스크톱 화면과 휴대폰 사이에서 전환합니다. 모바일 미리보기에서는 노치가 포함된 휴대폰 프레임 안에 캔버스가 그려져 실제 화면 가장자리가 어디인지 볼 수 있습니다.

두 종류의 가이드가 플레이어가 보고 닿을 수 있는 위치에 요소를 배치하도록 도와줍니다:

* **안전 영역 가이드** GUI가 제한되는 화면 부분의 윤곽을 표시하며, **씬 인셋** 사용자가 선택한 것과 일치합니다. Scene Inset이 **기기 안전 영역** 또는 **상호작용 가능한 안전 영역** 윤곽선은 항상 켜져 있습니다. GUI가 이 영역으로 잘리기 때문입니다. Scene Inset을 **전체 화면** 로 설정하면 프레임 아이콘 버튼으로 윤곽선을 켜거나 끌 수 있습니다.
* **HUD 가이드** 조이스틱, 점프 및 액션 버튼, 이모트 휠, 프로필, 채팅, 나침반 같은 모바일 클라이언트 자체 제어를 표시합니다. 이는 참고용일 뿐이며 선택하거나 이동할 수 없습니다. 게임 컨트롤러 버튼으로 켜고 끌 수 있으며 모바일 미리보기에서만 나타나고, Scene Inset이 안전 영역이면 가이드는 기본적으로 표시됩니다.

안전 영역 밖에 배치된 콘텐츠는 숨겨지지 않고 윤곽선을 넘어 그려집니다. 그 넘침이 경고입니다. 실제 클라이언트에서는 게임 자체 HUD 아래에 놓이게 됩니다.

**상호작용 가능한 안전 영역** 모바일 클라이언트의 왼쪽 제어 열을 비우지만, 오른쪽 가장자리는 기기 영역과 공유하므로 오른쪽 아래 모서리의 액션 버튼은 의도적으로 그 위에 그려집니다. 그 모서리는 비워 두세요. 참조: [클라이언트 제어가 위치하는 곳](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/#where-the-client-controls-live).


> [!WARNING]
> **📔 참고**: 이 가이드는 편집기 전용 근사치입니다. 실제 영역은 실행 중 클라이언트가 보고하며 기기에 따라 달라지므로, 항상 실제 휴대폰에서 확인하세요. 참조: [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/).


## 모바일 제어 사용자 지정

모바일 클라이언트에서 플레이어는 조이스틱, 조준점, 그리고 액션 버튼 행으로 이루어진 기본 화면 제어를 통해 이동하고 행동합니다.  **MobileHUD** 항목을 사용하면 코드를 작성하지 않고도 해당 HUD를 재구성할 수 있으며, 진행하면서 결과가 캔버스에 표시됩니다.

씬에 GUI가 하나 이상 있으면, **MobileHUD** 이 **GUIs** 목록의 첫 번째 행으로, 사용자가 만든 GUI들 위에 나타납니다. 선택하면 오른쪽에 패널이 열립니다.

상단의 세 개 체크박스로 HUD 전체 부분을 숨길 수 있습니다:

* **조이스틱 숨기기**: 이동 스틱을 제거합니다.
* **조준점 숨기기**: 조준선을 제거합니다.
* **입력 액션 숨기기**: 모든 액션 버튼을 한 번에 제거합니다.

그 아래에는 **입력 액션** 섹션에 화면 슬롯을 채우는 순서대로 여덟 개의 버튼이 나열됩니다:

| 액션                                     | 화면의 버튼         |
| -------------------------------------- | -------------- |
| `IA_JUMP`                              | 기본적으로 중앙의 큰 버튼 |
| `IA_POINTER`                           | 상호작용 버튼        |
| `IA_PRIMARY (E)`                       | E 버튼           |
| `IA_SECONDARY (F)`                     | F 버튼           |
| `IA_ACTION_3 (1)` 에서 `IA_ACTION_6 (4)` | 번호가 매겨진 버튼들    |

각 행은 세 가지를 제공합니다:

* **메인**: 이 액션을 중앙의 큰 버튼으로 만드는 라디오 버튼입니다. `IA_JUMP` 기본적으로 메인으로 설정되며, 한 번에 하나의 액션만 메인이 될 수 있습니다.
* 하나의 **눈 아이콘** 해당 버튼을 보이거나 숨깁니다. 하나를 숨기면 아래 버튼들이 올라와 빈칸을 채웁니다.
* **사용자 지정 아이콘**: 버튼의 글리프를 `.png`, `.jpg` 또는 `.jpeg` 씬의 이미지로 바꿉니다.

이 패널에서는 모든 버튼이 처음에 표시되며 `IA_JUMP` 메인 버튼입니다.


> [!WARNING]
> **📔 참고**: HUD를 전혀 건드리지 않는 씬에는 점프, 포인터, 기본 버튼 세 개만 표시됩니다. 패널은 `src/mobile-hud.ts` 무언가를 변경한 후에만 쓰므로, 아무것도 건드리지 않은 패널은 그 기본값을 유지합니다. 파일이 생성되는 즉시 그 코드는 기본 세트를 여기서 구성한 값으로 바꾸므로, 예를 들어 조이스틱만 숨겨도 플레이어 화면에 다섯 개의 추가 버튼이 표시됩니다. 씬에서 사용하지 않는 버튼은 숨기세요. 참조: [기본적으로 표시되는 항목](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/#what-shows-by-default).



> [!NOTE]
> **💡 팁**: 다섯 개가 넘는 버튼이 표시되면 마지막 버튼들은 "+" 오버플로 토글 뒤에 배치됩니다. 위쪽 버튼들을 충분히 숨기면 나머지가 직접 표시됩니다. 참조: [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/#how-the-button-layout-works) 슬롯이 어떻게 채워지는지.


### 작성되는 파일

변경 사항은 `src/mobile-hud.ts`에 기록되며, 이는 `setupMobileHud()` SDK의 [`TouchScreenControls`](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/) 컴포넌트:

```ts
import { engine, InputAction, TouchScreenControls } from '@dcl/sdk/ecs'

export function setupMobileHud() {
  TouchScreenControls.createOrReplace(engine.RootEntity, {
    hideJoystick: true,
    hideCrosshair: false,
    mainAction: InputAction.IA_PRIMARY,
    touchInputs: [
      { inputAction: InputAction.IA_ACTION_3, hide: true },
    ],
  })
}
```

편집기는 또한 다음 가져오기와 호출을 귀하의 `src/index.ts`:

```ts
import { setupMobileHud } from './mobile-hud'

setupMobileHud()
```

이 파일은 말할 내용이 있을 때만 존재합니다. 모든 설정을 기본값으로 되돌리거나 마지막 GUI를 삭제하면 Creator Hub가 `src/mobile-hud.ts` 과 두 줄을 다시 삭제합니다.


> [!WARNING]
> **📔 참고**: 인 동안 **MobileHUD** 이 선택되어 있으면 배치할 것이 없고 HUD는 모바일 전용이므로 위젯 팔레트는 비활성화되고 미리보기 제어는 숨겨집니다. 다시 보려면 자신이 만든 GUI 중 하나를 선택하세요.


## 파일이 저장되는 위치

UI 편집기는 씬의 실제 소스 파일을 수정합니다. 별도의 저장 형식은 없습니다.

* 각 GUI는 하나의 `.tsx` 아래의 파일입니다 `src/ui/`.
* `src/ui/index.tsx` 는 편집기가 이들을 모으기 위해 생성합니다. 수동으로 편집하지 마세요.
* `src/mobile-hud.ts` 에는 사용자의 [모바일 제어가 들어 있습니다](#customize-the-mobile-controls), 변경한 내용이 있으면 그렇습니다. 이것은 바깥에 위치하며 `src/ui/` 의도적으로 그렇게 배치되어 GUI로 취급되지 않습니다.

씬에 이전의 단일 `src/ui.tsx` 파일이 있다면, 편집기는 첫 UI 편집기를 열 때 이를 `src/ui.tsx.bak` 로 백업합니다.

이들은 일반 파일이므로 언제든지 [코드 편집기](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/) 에서 열 수 있습니다. 참조: [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md) 기본 코드가 하는 일을 확인하세요.

## 편집기가 편집할 수 없는 코드

편집기는 특정 형태의 UI 코드를 이해합니다. 예를 들어 루프나 조건문 안에서 만들었거나, 사용자 지정 컴포넌트나 spread props를 사용한 등 다른 방식으로 작성된 노드는 캔버스에 회색 블록으로 그려지며, **표준이 아님, 코드에서 편집** 배지가 붙습니다. 그 자식은 표시되지 않으며, 대신 코드에서 편집해야 합니다.

주변의 나머지 GUI는 정상적으로 계속 작동합니다.

## 관련 페이지

* [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md): 코드로 씬 UI 작성.
* [코드와 함께 사용](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/): 씬을 코드 편집기에서 엽니다.
* [엔티티와 컴포넌트](/docs/creator/content-creator-ko/scene-editor/build/components/): 편집기의 3D 측면입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/build/ui-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
