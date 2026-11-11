> Scene Editor에서 수정 영역을 사용해 아바타 동작을 변경하거나 씬의 특정 지역에서 카메라 모드를 강제합니다.

# 수정 영역

수정자 영역은 플레이어가 씬의 일부를 경험하는 방식을 바꾸는 보이지 않는 영역입니다. 두 가지 유형이 있습니다:

* **아바타 수정 영역**: 영역 안에 있는 플레이어에게 아바타가 행동하거나 보이는 방식을 바꿉니다. 예: 아바타 숨기기 또는 패스포트 팝업 비활성화.
* **카메라 수정 영역**: 플레이어가 영역 안에 있는 동안 카메라를 1인칭 또는 3인칭으로 강제합니다.

두 유형 모두 플레이어가 나가면 정상 동작으로 돌아갑니다.

## 수정자 영역 추가

수정자 영역은 두 가지 방법으로 추가할 수 있습니다:

**에셋 카탈로그에서:** 에셋 카탈로그를 열고 다음을 검색하세요 **아바타 수정 영역** 또는 **카메라 수정 영역** 아래의 **Utils** 카테고리입니다. 씬으로 드래그하세요. 이 항목은 에디터에서만 보이는 반투명 플레이스홀더 큐브로 표시되며, 실행 중인 씬에서는 보이지 않습니다.

**컴포넌트 메뉴에서:** 기존 엔티티를 선택하고, 다음을 클릭하세요 **+** 속성 패널 상단의 버튼을 클릭한 다음, 다음을 선택하세요 **아바타 수정 영역** 또는 **카메라 수정 영역** 목록에서. 다음을 참조하세요 [컴포넌트 추가](/docs/creator/content-creator-ko/scene-editor/build/components/#add-components) 자세한 내용은.

![Stage area라는 엔티티에서 컴포넌트 추가 메뉴가 열린 상태로, 3D Content 및 Interaction 그룹 아래의 컴포넌트 목록이 표시되어 있으며, Avatar Modifier Area와 Camera Modifier Area가 포함되어 있습니다.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

## 영역 크기 조정

수정자 영역의 크기는 엔티티의 **Transform 스케일**. 엔티티를 4 x 3 x 4로 스케일하면 수정자 영역은 엔티티 위치를 중심으로 한 4 x 3 x 4미터 박스가 됩니다. 필요한 크기를 설정하려면 스케일 기즈모를 사용하거나 Transform 컴포넌트에 정확한 값을 입력하세요.


> [!WARNING]
> **참고:** 에디터 패널에는 별도의 "Area" 필드가 없습니다. 에디터가 내부 area 속성을 엔티티의 스케일과 자동으로 동기화하므로, 엔티티 크기만 조정하면 됩니다.


## 아바타 수정 영역

Avatar Modifier Area 패널에는 두 가지 설정이 있습니다:

![Avatar Modifier Area 컴포넌트 패널로, Modifiers 드롭다운에서 Hide Avatars가 선택되어 있고, Exclude Player IDs 섹션과 Add Player ID 버튼이 표시되어 있습니다.](/docs/_remote/d49ca42bf27a1995edcc536d.webp)

### 수정자

다음 옵션이 있는 다중 선택 드롭다운입니다:

* **아바타 숨기기**: 영역 안의 아바타가 보이지 않게 됩니다. 영역 안의 플레이어들은 서로를 볼 수 없지만, 영역 밖의 플레이어들은 여전히 정상적으로 볼 수 있습니다.
* **패스포트 비활성화**: 영역 안에서 플레이어의 아바타를 클릭해도 더 이상 프로필 팝업이 열리지 않습니다.

같은 영역에 두 수정자 중 하나 또는 둘 다를 선택할 수 있습니다. 컴포넌트를 추가하면, **아바타 숨기기** 가 기본으로 선택됩니다. 다음을 클릭하세요 **x** 수정자에서 제거하려면.

### 제외할 플레이어 ID

수정자의 영향을 받지 않는 지갑 주소 목록입니다. 이 목록에 있는 주소의 플레이어는 영역 안에 있어도 정상적으로 동작합니다.

클릭 **플레이어 ID 추가** 를 눌러 항목을 추가한 다음 지갑 주소를 붙여넣으세요. 제거하려면 옆의 점 세 개 메뉴를 클릭하고 다음을 선택하세요 **플레이어 ID 제거**.

이 기능은 무대에서 관객 아바타는 숨기고 공연자만 보이게 하고 싶을 때 유용합니다.


> [!WARNING]
> **참고:** 지갑 주소는 소문자로 입력하세요. 수정자 영역은 정확한 문자열 일치를 사용합니다.


### 사용 사례

* **개인정보 보호 구역**: 백스테이지나 탈의 공간에서 아바타를 숨깁니다.
* **깔끔한 무대**: 무대에서 관객 아바타를 숨겨서 공연자(Exclude Player IDs에 나열된 사람)만 보이게 합니다.
* **게임 경기장**: 전투 구역에서 패스포트를 비활성화하여 플레이 중 실수로 프로필이 열리지 않게 합니다.

## 카메라 수정 영역

Camera Modifier Area 패널에는 한 가지 설정이 있습니다:

![Camera Modifier Area 컴포넌트 패널로, Camera Mode 드롭다운이 First Person으로 설정되어 있습니다.](/docs/_remote/199bb09636ac6f76d722633e.webp)

### 카메라 모드

다음 두 옵션이 있는 드롭다운입니다:

* **1인칭**: 플레이어가 영역 안에 있는 동안 1인칭 시점을 강제합니다.
* **3인칭**: 플레이어가 영역 안에 있는 동안 3인칭 시점을 강제합니다.

기본값은 **1인칭**. 플레이어가 영역을 벗어나면 이전 카메라 모드가 복원됩니다.


> [!NOTE]
> **팁:** 여기서는 시네마틱 카메라 모드를 사용할 수 없습니다. 1인칭/3인칭 전환을 넘어 카메라를 제어하려면 다음을 사용하세요 [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/#using-virtual-cameras) 코드로.


### 사용 사례

* **좁은 복도 또는 퍼즐 룸**: 작은 오브젝트를 정확하게 클릭해야 할 때 1인칭으로 강제합니다.
* **경치 좋은 전망대**: 플레이어가 배경과 함께 자신의 아바타를 볼 수 있도록 3인칭으로 강제합니다.
* **튜토리얼 구역**: 안내형 체험 동안 모든 플레이어가 동일한 카메라 모드를 사용하도록 합니다.

## 수정자 영역 결합

같은 엔티티에 두 종류의 수정자 영역을 모두 배치하거나, 같은 공간에 별도의 수정자 영역을 겹쳐 배치할 수 있습니다. 각 영역은 독립적으로 효과를 적용합니다.


> [!WARNING]
> **참고:** 씬의 수정자 영역 수는 적게 유지하세요. 너무 많으면 성능에 영향을 줄 수 있습니다. 대부분의 씬에서는 몇 개 정도면 충분합니다.


## 코드로 수정자 영역 사용하기

Scene Editor 컴포넌트는 SDK7 `AvatarModifierArea` 및 `CameraModeArea` 컴포넌트에 직접 대응합니다. 사용 가능한 수정자, 런타임 동작, 그리고 제외 목록을 동적으로 변경하는 것과 같은 고급 옵션에 대한 자세한 내용은 다음을 참조하세요:

* [아바타 수정자 영역](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas/) (SDK7)
* [카메라 모드 영역](/docs/creator/content-creator-ko/sdk7/3d/camera/#1st-and-3rd-person-camera-modes) (SDK7)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
