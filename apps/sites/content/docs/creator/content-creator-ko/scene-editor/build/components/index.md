> 아이템의 컴포넌트가 어떻게 작동하는지 이해합니다

# 엔티티와 컴포넌트

캔버스나 엔티티 트리에서 항목을 클릭하여 선택하세요. 그러면 화면 오른쪽의 속성 패널에 해당 구성 요소가 표시됩니다. 항목마다 다른 구성 요소가 있으며, 각 구성 요소는 특정 설정을 표시합니다.

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

대부분의 비상호작용 항목에는 다음 구성 요소가 있습니다:

* **변환**: 항목의 위치, 회전, 크기를 설정합니다. 항목이 다음의 자식인 경우 [엔티티 트리](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#the-entity-tree), 이 값들은 부모의 값에 상대적입니다.
* **GLTF**: 로드할 3D 모델입니다. 이 3D 모델 파일의 로컬 경로가 포함됩니다. 또한 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/#colliders-on-3d-models) 를 모델에 구성하기 위한 몇 가지 속성도 포함합니다.

씬의 항목들은 모두 **엔티티**입니다. 씬의 모든 것은 엔티티이며, 씬의 기본 구성 요소입니다. 항목은 최소한 위치와 보이는 형태를 가진 엔티티입니다.

## 구성 요소 추가

모든 엔티티에 구성 요소를 추가하려면 속성 탭 상단의 **+** 기호를 클릭하고 목록에서 구성 요소를 선택하세요. 다음을 참조하세요. [모든 항목을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/)

![엔티티에서 열려 있는 구성 요소 추가 메뉴로, 3D 콘텐츠와 상호작용으로 그룹화된 구성 요소가 나열됩니다.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

엔티티에서 구성 요소를 삭제하려면 오른쪽의 세 점 아이콘을 클릭하고 **구성 요소 삭제**.

## 처음부터 엔티티 만들기

새로운 엔티티를 만들려면 루트 **씬** 의 엔티티 트리에서 엔티티를 마우스 오른쪽 버튼으로 클릭하거나 다른 엔티티를 마우스 오른쪽 버튼으로 클릭한 뒤 **자식 추가**

![](/docs/_remote/a6edc747060b504b764ee35b.webp)

이렇게 하면 **변환** 구성 요소만 있는 빈 엔티티가 만들어집니다. 새 엔티티는 클릭한 부모 엔티티의 자식입니다. 이후 원하는 다른 구성 요소를 추가하여 원하는 형태로 만들 수 있습니다.

## 사용 가능한 구성 요소

다음 구성 요소는 씬 에디터 UI를 통해 모든 엔티티에 추가할 수 있습니다:

* **메시 렌더러**: 기본 도형(큐브, 평면, 원통, 구)을 기반으로 한 보이는 형태를 엔티티에 부여합니다.
* **메시 콜라이더**: 엔티티에 보이지 않는 콜라이더 지오메트리를 부여합니다. 이는 플레이어가 항목을 통과하지 못하게 막거나 클릭 가능하게 만들 수 있습니다. 다음을 참조하세요. [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/).
* **머티리얼**: 엔티티의 색상, 텍스처 및 기타 속성을 정의합니다. 여기에는 **메시 렌더러** 구성 요소. [머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/).

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p>**📔 참고**: 항목에는 **메시 렌더러** 구성 요소가 있어야 합니다. **GLTF** 보이는 형태가 있는 항목에는 영향을 주지 않습니다.</p></div>
* **가시성**: 엔티티를 보이지 않게 할지 정의합니다.
* **광원**: 엔티티에 빛을 추가합니다.
* **머티리얼 바꾸기**: 구성 요소가 있는 엔티티의 머티리얼을 바꿉니다. **GLTF** 3D 모델에 여러 메쉬가 있으면 각 메쉬의 머티리얼을 개별적으로 바꿀 수 있습니다.
* **오디오 소스**: 엔티티 위치에서 사운드 파일의 소리를 재생합니다. 다음을 참조하세요. [사운드](/docs/creator/content-creator-ko/sdk7/3d/sounds/).
* **텍스트 모양**: 3D 공간에 텍스트를 표시합니다. 다음을 참조하세요. [텍스트](/docs/creator/content-creator-ko/sdk7/3d/text/).
* **포인터 이벤트**: 엔티티를 클릭 가능한 것으로 표시하고, 호버 힌트를 표시합니다.


> [!WARNING]
> **📔 참고**: **포인터 이벤트** 구성 요소는 피드백만 제공합니다. 엔티티와 상호작용할 때 작업을 수행하려면 다음을 참조하세요. [모든 항목을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/)


* **멀티플레이어**: 씬의 모든 플레이어가 볼 수 있도록 엔티티에서 발생한 모든 변경 사항을 공유합니다. 특정 구성 요소의 변경 사항만 공유하도록 구성할 수 있습니다. 다음을 참조하세요. [서버리스 멀티플레이어](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/#mark-an-entity-as-synced) 에서 자세한 내용을 확인하세요.
* **애니메이터**: 다음이 있는 엔티티의 애니메이션 재생을 제어합니다. **GLTF** 3D 모델. 다음을 참조하세요. [3D 모델 애니메이션](/docs/creator/content-creator-ko/sdk7/3d/3d-model-animations/).
* **트윈**: 엔티티를 일정 시간 동안 점차 이동, 회전 또는 크기 조정합니다. 다음을 참조하세요. [엔티티 이동](/docs/creator/content-creator-ko/sdk7/3d/move-entities/).
* **빌보드**: 엔티티가 항상 플레이어를 향하도록 회전하게 합니다. 다음을 참조하세요. [사용자를 향하기](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#face-the-player).
* **아바타 부착**: 엔티티를 플레이어의 아바타에 부착하여 따라다니게 합니다. 다음을 참조하세요. [엔티티를 아바타에 부착](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#attach-an-entity-to-an-avatar).
* **비디오 플레이어**: 장면 안에서 비디오를 재생하며, 해당 엔티티의 **머티리얼**. 다음을 참조하세요. [비디오 재생](/docs/creator/content-creator-ko/sdk7/media/video-playing/).
* **NFT 모양**: 프레임 안에 NFT 이미지를 표시합니다. 다음을 참조하세요. [NFT 표시하기](/docs/creator/content-creator-ko/scene-editor/build/nfts/).
* **파티클 시스템**: 연기, 불꽃 또는 반짝임 같은 효과를 위해 엔티티의 위치에서 파티클을 방출합니다. 다음을 참조하세요. [파티클 시스템](/docs/creator/content-creator-ko/sdk7/3d/particle-system/).
* **가상 카메라**: 씬이 플레이어의 시점을 전환할 수 있는 사용자 지정 카메라 각도를 정의합니다. 다음을 참조하세요. [카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/).
* **아바타 수정 영역**: 이 엔티티를 중심으로 한 영역 내부의 플레이어에게 아바타의 동작이나 외형을 변경합니다. 엔티티의 크기가 영역의 크기를 설정합니다. 다음을 참조하세요. [수정 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/modifier-areas/).
* **카메라 수정 영역**: 이 엔티티를 중심으로 한 영역 내부에서 플레이어의 카메라를 1인칭 또는 3인칭으로 강제합니다. 엔티티의 크기가 영역의 크기를 설정합니다. 다음을 참조하세요. [수정 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/modifier-areas/).
* **스크립트**: 구성 가능한 매개변수가 있는 사용자 지정 코드 모듈을 엔티티에 연결합니다. 다음을 참조하세요. [스크립트 구성 요소](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/).


> [!WARNING]
> **📔 참고**: SDK에는 현재 코드로만 사용할 수 있는 다른 구성 요소도 있습니다. 또한 직접 [사용자 지정 구성 요소](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 를 코드로 생성할 수 있으며, UI 표현은 없지만 코드로 추가하고 편집할 수 있습니다.
> 
> 다음을 참조하세요. [코드와 함께 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/) 에서 씬의 코드를 편집하는 방법을 확인하세요.
> 
> 또한 엔티티는 각각의 구성 요소를 **하나** 만 가질 수 있다는 점에 유의하세요. 엔티티에 이미 존재하는 구성 요소의 두 번째 인스턴스는 할당할 수 없습니다. 예를 들어, 두 개의 **액션** 구성 요소를 동일한 엔티티에 추가할 수 없습니다.


## 알아두면 좋은 설정

대부분의 구성 요소 필드는 위에 연결된 SDK 페이지에 일대일로 대응합니다. 몇 가지는 놓치기 쉽거나, 필드 이름만으로는 명확하지 않은 방식으로 작동합니다.

### 오디오 소스

**피치** 는 재생 속도를 곱하고 그에 따라 피치도 바꿉니다. `1` 원래 사운드를 재생합니다, `2` 두 배 빠르게 재생합니다(한 옥타브 위), `0.5` 절반 속도로 재생합니다(한 옥타브 아래). 0보다 커야 하며, 기본값은 `1`.

### 비디오 플레이어

* **재생 속도**: 비디오에 대한, 피치와 같은 개념입니다. `1` 는 정상 속도입니다. 기본값을 사용하려면 비워 두세요.
* **시작 위치(초)**: 비디오가 시작될 때 재생이 시작되는 지점입니다. 처음부터 시작하려면 비워 두세요.
* **공간 오디오**: **공간** 을 켜서 비디오의 소리를 위치 기반으로 만들면, 플레이어가 멀어질수록 소리가 희미해집니다. **공간 거리** 는 범위를 설정합니다. 오디오는 **최소** 미터까지는 최대 볼륨으로 재생되고, **최대** 미터에서 무음으로 희미해집니다. 기본값은 0과 60입니다.

에서 **공간** 오디오는 씬 전체 어디서나 같은 음량으로 재생됩니다.

### 텍스트 모양

* **크기**: 텍스트 박스의 너비(**W**)와 높이(**H**)를 미터 단위로 나타냅니다. 정렬과 패딩은 이 박스를 기준으로 측정됩니다.
* **텍스트 줄바꿈**: 에 설정된 너비에 도달하면 텍스트를 새 줄로 나눕니다. **크기**. 비활성화하면 텍스트는 한 줄로 유지됩니다.

### 머티리얼

* **색상 알파**: 주요 **색상**의 불투명도로, 0에서 1까지입니다. 이는 **투명도 모드** 가 **불투명**.
* **알파 테스트**: 픽셀이 보이지 않게 되는 차단점입니다. 다음일 때만 나타납니다. **투명도 모드** 가 **알파 테스트** 또는 **알파 테스트 및 블렌드**, 이들만 사용하기 때문입니다.
* **없음**: 색상 드롭다운에 이제 색상을 지우는 **없음** 항목이 있고, 텍스처 **유형** 드롭다운은 **없음** 을 제공하여 텍스처를 제거할 수 있습니다.

### 파티클 시스템

해당 **회전** 섹션은 파티클의 방향과 회전을 제어합니다:

* **빌보드**: 각 파티클은 항상 카메라를 향합니다. 기본적으로 켜져 있습니다. 3D에서 자유롭게 굴러가야 하는 파티클에는 끄세요.
* **이동 방향을 향하기**: 각 파티클은 화살표처럼 이동하는 방향을 향해 시작합니다.
* **초기 회전(도)**: 각 파티클이 태어날 때 축별로 어느 정도 회전하는지입니다.
* **시간에 따른 회전(도/초)**: 축별 회전 속도입니다. 값은 축당 180으로 제한됩니다.

다음일 때 **텍스처 사용** 이 켜져 있으면, 두 개의 드롭다운이 더 나타납니다:

* **랩 모드**: 텍스처가 가장자리를 지나서 어떻게 동작하는지입니다. **클램프** (기본값)은 가장자리 픽셀을 늘리고, **반복** 은 타일처럼 반복하며, **미러** 는 번갈아 뒤집힌 타일로 반복합니다.
* **필터 모드**: 텍스처가 어떻게 샘플링되는지입니다. **바이리니어** (기본값)은 부드럽고, **포인트** 는 픽셀화되어 픽셀 아트에 좋고, **트릴리니어** 는 밉맵 레벨 간을 부드럽게 합니다.

### 트윈

**트윈 유형** 에는 다섯 가지 모드가 있습니다:

| 모드                                 | 하는 일                                    |
| ---------------------------------- | --------------------------------------- |
| **항목 이동**, **항목 회전**, **항목 크기 조정** | 에 걸쳐 시작 값에서 종료 값으로 애니메이션합니다. **지속 시간**. |
| **연속 이동**                          | 고정된 방향으로, 일정한 속도로, 영원히 계속 이동합니다.        |
| **연속 회전**                          | 고정된 축을 중심으로, 일정한 속도로, 영원히 계속 회전합니다.     |

두 연속 모드에는 종료 값이 없으므로 **시작**, **종료**, **지속 시간**, **이징 함수** 및 **루프** 는 숨겨집니다. 대신 다음을 사용합니다:

* **연속 이동**: 하나의 **방향** 벡터와 하나의 **속도(미터/초)**. 길이가 1인 방향을 사용하면 속도가 정확히 초당 미터가 됩니다. 음수 속도는 반대 방향으로 이동합니다.
* **연속 회전**: 하나의 **회전 축(도)** 와 하나의 **속도(도/초)**. 축의 방향만 중요하고 각도의 크기는 중요하지 않습니다.

전환 **트윈 유형** 은 새 모드에서 적절한 기본값으로 이 값을 재설정합니다. 각 모드에서 숫자가 의미하는 바가 다르기 때문입니다.

해당 **재생 중** 체크박스(이전에는 "자동 시작"이라고 불림)는 씬이 로드되자마자 트윈이 실행될지 결정합니다. 끄면 트윈은 시작 값에서 일시 정지된 상태로 생성되며, 코드에서 재생으로 설정해야만 실행됩니다.


> [!NOTE]
> **💡 팁**: 일부 구성 요소 속성은 코드에서만 설정할 수 있습니다. 이제 인스펙터에서 엔티티를 편집해도 더 이상 사라지지 않으므로, 씬 코드에서 값을 설정한 다음 해당 엔티티를 계속 편집해도 잃지 않습니다.


## 스마트 항목

[스마트 항목](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 에도 엔티티의 상호작용성을 제어하는 특수 구성 요소를 포함할 수 있습니다. 일반적으로 다음과 같습니다:

* **액션**: 항목이 수행할 수 있는 모든 가능한 작업을 나열합니다.
* **트리거**: Actions 구성 요소의 작업이 언제 실행되는지 결정합니다.
* **상태**: 항목의 현재 상태를 추적합니다. 조건부 로직에 사용할 수 있으며, 항목이 특정 상태일 때만 특정 작업을 트리거하도록 할 수 있습니다.
* **카운터**: 카운터를 추적합니다. 카운터 값이 주어진 값과 같거나/크거나/작을 때만 특정 작업을 트리거하는 조건부 로직에 사용할 수 있습니다.

다음을 참조하세요. [스마트 항목 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/) 에서 자세한 내용을 확인하세요.

## 엔티티와 구성 요소에 관하여

씬의 모든 것은 엔티티입니다. 씬의 모든 항목과 스마트 항목은 엔티티입니다.

엔티티의 모든 특성은 구성 요소에 의해 결정됩니다. 구성 요소는 엔티티가 무엇인지, 어디에 있는지, 어떻게 들리는지, 어떻게 동작하는지를 정의합니다. 예를 들어, **변환** 구성 요소는 엔티티의 좌표, 회전, 크기를 저장합니다.  **MeshRenderer** 구성 요소는 엔티티에 보이는 형태(예: 큐브나 구)를 부여하고, **머티리얼** 구성 요소는 엔티티에 색상이나 텍스처를 부여합니다.

구성 요소의 값은 시간이 지남에 따라 바뀔 수 있습니다. 씬 에디터에서는 이러한 구성 요소의 초기 값을 설정합니다. 하지만 씬이 실행되면, 플레이어의 행동이나 시간의 경과로 인해 해당 값들이 바뀔 수 있습니다.

예를 들어, 움직이는 플랫폼 스마트 항목에는 **변환** 구성 요소를 통해 설정하는 초기 위치가 있지만, 이 항목의 동작으로 움직이기 시작하면, **변환** 는 다른 값을 가지게 됩니다.

다음을 참조하세요. [엔티티 및 구성 요소](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/) 의 개념과 Decentraland 씬에서 어떻게 사용되는지에 대한 자세한 설명을 보려면


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/build/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
