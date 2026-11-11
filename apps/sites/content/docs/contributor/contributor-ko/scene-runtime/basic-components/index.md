# 기본 컴포넌트

World Explorer는 씬과 정의를 공유하는 기본 컴포넌트 목록을 지원하며, 위치 지정, 애니메이션, 미디어, 월드 상태 쿼리에 대한 내장 지원을 제공합니다

컴포넌트에는 다섯 가지 유형이 있습니다:

1. [오브젝트 컴포넌트](#object) 엔티티에 시각적 및 물리적 속성을 추가합니다.
2. [게임 컴포넌트](#game) 게임 엔진에 동작을 요청하고 정보를 받을 수 있습니다.
3. [미디어 컴포넌트](#media) 이미지를 표시하고 소리를 재생할 수 있습니다.
4. [영역 컴포넌트](#area-components) 지정된 구역에서 엔티티의 동작을 변경합니다.
5. [UI 컴포넌트](#ui) 씬이 떠 있는 인터페이스를 렌더링할 수 있게 합니다.

다음을 제외하고 [`변환`](#transform)는 아래에 자세히 설명되어 있으며, 모든 컴포넌트의 상태는 프로토콜 버퍼를 사용해 직렬화됩니다. 각 제목의 링크를 따라가면 전체 정의를 볼 수 있습니다.

### 오브젝트 컴포넌트 <a href="#object" id="object"></a>

가장 일반적인 컴포넌트는 엔티티에 시각적 및 물리적 속성을 추가하는 컴포넌트입니다. 이를 사용해 게임 내 오브젝트의 위치를 정하고, 크기를 조정하고, 색을 입히고, 충돌을 추가할 수 있습니다.

***

#### `변환` [↗ 명세](https://adr.decentraland.org/adr/ADR-153) <a href="#transform" id="transform"></a>

엔티티에 위치, 회전, 크기를 추가합니다.

이 컴포넌트는 씬의 수명 동안 압도적으로 가장 많이 사용되고 자주 업데이트됩니다. 이 때문에, `변환` 는 프로토콜 버퍼를 사용해 직렬화되지 않고, 대신 사용자 정의 구조체에 패킹됩니다.

`변환` 크기는 `44` 바이트이며, 정확한 레이아웃은 다음과 같습니다:

```
.-----------------------.-------------------------------.-----------------------.-----------------.
|   x   |   y   |   z   |   x   |   y   |   z   |   w   |   x   |   y   |   z   | parent (uint32) |
'-----------------------'-------------------------------'-----------------------'-----------------'
╵  position (3x float)  ╵      rotation (4x float)      ╵   scale (3x float)    ╵

```

이 방식은 런타임(특히 저수준 코드 환경)에서 중간 직렬화 및 역직렬화 단계를 피할 수 있게 합니다. a의 메모리는 `변환` 존재하는 곳은 복사, 공유 및 직접 포인터 참조할 수 있습니다.

좌표는 소수점 값을 가질 수 있으므로 4바이트 부동소수점 숫자를 사용합니다.

이 `parent` 필드는 이 엔티티가 다른 엔티티를 기준으로 위치해야 함을 나타내며, 해당 엔티티는 ID로 식별됩니다.

에 대한 null 값이 `parent` 또한 [`루트 엔티티`](/docs/contributor/contributor-ko/scene-runtime/basic-entities/#rootentity) ID (`0`) `변환` 을 기본적으로 기준으로 삼습니다.

프로토콜상, `변환` 는 빅 엔디언 바이트 순서로 직렬화되어야 합니다.

***

#### `MeshRenderer` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_renderer.proto#L7) <a href="#meshrenderer" id="meshrenderer"></a>

엔티티에 대한 기본 렌더링 동작을 제공합니다.

평면, 구, 큐브 또는 실린더로 렌더링하도록 설정할 수 있습니다. 임의의 3D 모양을 렌더링하려면 [`GltfContainer`](#gltfcontainer).

***

#### `MeshCollider` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_collider.proto#L15) <a href="#meshcollider" id="meshcollider"></a>

엔티티에 대한 기본 충돌 동작과 마우스 포인터 감지를 제공합니다.

평면, 구, 큐브 또는 실린더처럼 동작하도록 설정할 수 있습니다. 사용자 지정 충돌 메시를 사용하려면 [`GltfContainer`](#gltfcontainer).

***

#### `Material` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/material.proto#L19) <a href="#material" id="material"></a>

다음도 가지고 있는 엔티티의 텍스처, 조명, 색상 및 투명도 속성을 설정합니다. [`MeshRenderer`](#meshrenderer) 컴포넌트를.

모든 일반적인 경우를 포괄하도록 설계된 복잡한 구조이지만, 모든 필드에는 합리적인 기본값이 있어 지정하지 않아도 됩니다.

엔티티의 시각적 속성을 완전히 사용자 지정하려면 [`GltfContainer`](#gltfcontainer).

***

#### `GltfContainer` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/gltf_container.proto#L7) <a href="#gltfcontainer" id="gltfcontainer"></a>

다음이 주어졌을 때 이 엔티티에 3D 모델을 연결합니다. [파일 경로](/docs/contributor/contributor-ko/content/entities/#files) 의 `.gltf` 씬 매니페스트에 있는 자산.

모델은 자체 메쉬와 머티리얼을 가지므로 이 컴포넌트는 [`MeshRenderer`](#meshrenderer), [`MeshCollider`](#meshcollider) 및 [`Material`](#material).

***

#### `Billboard` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/billboard.proto#L17) <a href="#billboard" id="billboard"></a>

엔티티가 자동으로 자신의 `변환` 를 카메라를 향하게 하거나, 선택적으로 `target_entity`를 통해 참조되는 다른 엔티티를 향하게 합니다. 이름에서 알 수 있듯이, 게임 내 빌보드를 표시하는 데 사용되며, 자주 [`TextShape`](#textshape).

기본적으로 모든 방향에 영향을 주지만, 한 축에서만 회전하도록 설정할 수도 있습니다. 참조된 대상 엔티티가 존재하지 않으면, 존재할 때까지 방향 재설정이 중단됩니다.

***

#### `Animator` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/animator.proto#L7) <a href="#animator" id="animator"></a>

엔티티에 동시에 적용할 수 있는 하나 이상의 애니메이션을 정의합니다.

씬은 컴포넌트 상태를 설정하여 애니메이션을 사용자 지정하고, 재생하고, 전환할 수 있으며, 이를 읽어 어떤 애니메이션이 사용 가능한지 또는 현재 재생 중인지 확인할 수도 있습니다.

***

#### `AvatarAttach` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_attach.proto#L16) <a href="#avatarattach" id="avatarattach"></a>

엔티티의 위치가 아바타 몸체의 특정 앵커 포인트를 따라가야 함을 나타냅니다.

사용자 ID를 설정하면 플레이어뿐 아니라 어떤 아바타에도 영향을 줄 수 있습니다.

***

#### `TextShape` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/text_shape.proto#L11) <a href="#textshape" id="textshape"></a>

다음으로 주어진 엔티티의 위치에 텍스트를 렌더링합니다. `변환`.

크기, 여백, 패딩, 색상, 그림자 등과 같은 매개변수를 지원하며, 매우 세밀하게 설정할 수 있습니다.

***

#### `Visibility` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/visibility_component.proto#L7) <a href="#visibility" id="visibility"></a>

엔티티가 보이는 상태(기본값)인지 보이지 않는 상태인지 설정합니다.

보이지 않는 객체도 계속 존재하며, 연결된 다른 컴포넌트의 동작은 그대로 나타납니다.

***

#### `AvatarShape` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_shape.proto#L8) <a href="#avatarshape" id="avatarshape"></a>

플레이어의 아바타에 대한 정보를 포함하며, 몸체 모양, 색상, 착용 아이템, 일시적 상태를 포함합니다.

이 컴포넌트는 [아바타 씬의 엔티티에 부착됩니다.](/docs/contributor/contributor-ko/scene-runtime/execution/#avatarscene).

### 게임 컴포넌트 <a href="#game" id="game"></a>

몇 가지 기본 컴포넌트는 씬과 런타임이 정보를 교환하는 데 사용할 수 있습니다. 이들은 [ECS 동기화 메커니즘](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/engine-api/#synchronization) 을 활용하여 상태 변경이나 이벤트에 대해 일관성과 순서를 보장합니다.

***

#### `CameraMode` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode.proto#L8) <a href="#cameramode" id="cameramode"></a>

플레이어가 1인칭인지 3인칭인지 판별하는 데 사용할 수 있습니다.

***

#### `PointerLock` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_lock.proto#L7) <a href="#pointerlock" id="pointerlock"></a>

마우스 포인터가 카메라의 초점에 자동으로 따라가고 있는지(잠김), 아니면 화면에서 자유롭게 움직일 수 있는지(잠금 해제) 판별하는 데 사용할 수 있습니다.

이것은 [`카메라 엔티티`](/docs/contributor/contributor-ko/scene-runtime/basic-entities/#cameraentity)에 부착되며, 그 상태는 씬에서 읽을 수 있지만 쓸 수는 없습니다.

***

#### `PointerEvents` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events.proto#L17) <a href="#pointerevents" id="pointerevents"></a>

포인터가 엔티티를 클릭하거나 올려놓을 때 시각적 피드백을 보여줍니다.

***

#### `PointerEventsResult` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events_result.proto#L13) <a href="#pointereventsresult" id="pointereventsresult"></a>

키와 포인터 동작을 포함한 플레이어의 최근 입력에 대한 정보를 담고 있습니다.

이것은 `루트 엔티티`며, 매 프레임마다 런타임이 새 이벤트로 업데이트합니다.

***

#### `Raycast` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast.proto#L15) <a href="#raycast" id="raycast"></a>

게임 엔진에 레이캐스트를 요청하기 위해 엔티티에 부착할 수 있습니다.  `RaycastResult` 컴포넌트는 나중에 동일한 엔티티에 부착됩니다.

레이의 시작점, 방향, 최대 길이를 설정할 수 있습니다.

***

#### `RaycastResult` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast_result.proto#L9) <a href="#raycastresult" id="raycastresult"></a>

런타임이 다음을 가진 엔티티에 부착합니다. [`Raycast`](#raycast) 컴포넌트의 대기 중인 결과.

원래 레이에 대한 정보를 담고 있으며, 맞은 엔티티를 식별합니다.

### 미디어 컴포넌트 <a href="#media" id="media"></a>

씬은 이미지를 표시하고, 비디오를 보여주거나, 소리를 재생하기 위해 특수 컴포넌트를 부착할 수 있습니다.

***

#### `AudioSource` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_source.proto#L7) <a href="#audiosource" id="audiosource"></a>

다음이 주어졌을 때 씬에 포함된 오디오 클립을 재생합니다. [매니페스트에 있는 파일 경로](/docs/contributor/contributor-ko/content/entities/#files).

소리는 연결된 엔티티의 위치에서 발생합니다. 피치, 볼륨, 반복 동작을 설정할 수 있으며, 오디오 플레이어의 상태를 읽을 수 있습니다.


> [!NOTE]
> 향후 업그레이드에 대비하여, `AudioSource` 의 필드는 `audio_clip_url`라고 불리지만, 현재 프로토콜 버전에서는 실제로 매니페스트에 정의된 경로입니다.


***

#### `AudioStream` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_stream.proto#L7) <a href="#audiostream" id="audiostream"></a>

다음과 유사하지만 [AudioSource](#audiosource)오디오는 외부 URL에서 실시간으로 스트리밍됩니다.

특정 엔티티에 부착되어 있어도 소리는 그 위치의 영향을 받지 않습니다. 볼륨을 설정할 수 있으며, 오디오 플레이어의 상태를 읽을 수 있습니다.

***

#### `NftShape` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/nft_shape.proto#L36) <a href="#nftshape" id="nftshape"></a>

이미지 또는 비디오 자산과 연결된 NFT를 표시합니다.

구성 가능한 장식 프레임이 있는 2D 캔버스를 렌더링합니다.

### 영역 컴포넌트

이 컴포넌트를 통해 씬은 지정된 경계 내에서 엔티티의 기본 동작을 수정할 수 있습니다.

***

#### `AvatarModifierArea` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_modifier_area.proto#L15) <a href="#avatarmodifierarea" id="avatarmodifierarea"></a>

엔티티를 중심으로 한 공간 내에서 아바타의 동작을 변경합니다.

3D 크기 벡터로 정의되며, 아바타가 보이는지 또는 클릭 가능한지에 영향을 줄 수 있습니다. 특정 아바타는 해당 사용자 ID를 기준으로 이 효과에서 제외할 수 있습니다.

***

#### `CameraModeArea` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode_area.proto#L11) <a href="#cameramodearea" id="cameramodearea"></a>

엔티티를 중심으로 한 공간 내에서 카메라 모드(1인칭 또는 3인칭)를 변경합니다.

3D 크기 벡터와 원하는 카메라 모드로 정의됩니다.

### UI 컴포넌트 <a href="#ui" id="ui"></a>

다음 컴포넌트는 게임 월드 위에 떠 있는 그래픽 인터페이스를 만드는 데 사용됩니다.

대개 계층형 [`UiTransform`](#uitransform) 컴포넌트를 가진 엔티티 집합에 부착되며, 서로는 `parent` 속성을 통해 연결됩니다.

예를 들어, 떠 있는 옵션 창은 [`UiTransform`](#uitransform) 그리고 하나의 [`UiBackground`](#uibackground) 컴포넌트를 가진 엔티티, 그리고 자식 [`UiTransform`](#uitransform) 그리고 하나의 [`UiDropdown`](#uidropdown).

***

#### `UiTransform` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_transform.proto#L77) <a href="#uitransform" id="uitransform"></a>

UI 컴포넌트의 크기, 위치, 여백, 패딩을 설명합니다.

flexbox 모델을 기반으로 하며, 매우 사용자 지정이 가능합니다.

***

#### `UiBackground` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_background.proto#L11) <a href="#uibackground" id="uibackground"></a>

UI 엔티티의 배경으로 사용할 색상 또는 텍스처를 설명합니다.

***

#### `UiDropdown` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown.proto#L11) <a href="#uidropdown" id="uidropdown"></a>

드롭다운 위젯에 표시될 상호 배타적인 옵션 목록을 정의합니다.

***

#### `UiDropdownResult` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown_result.proto#L9) <a href="#uidropdownresult" id="uidropdownresult"></a>

다음의 선택된 값을 포함합니다. [`UiDropdown`](#uidropdown)는 런타임이 설정하고 씬이 읽습니다.

***

#### `UiInput` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input.proto#L11) <a href="#uiinput" id="uiinput"></a>

약간의 여백으로 사용자 지정이 가능한 텍스트 입력 위젯을 정의합니다.

***

#### `UiInputResult` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input_result.proto#L9) <a href="#uiinputresult" id="uiinputresult"></a>

다음의 텍스트 값을 포함합니다. [`UiInput`](#uiinput)는 런타임이 설정하고 씬이 읽습니다.

***

#### `UiText` [↗ 소스](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_text.proto#L11) <a href="#uitext" id="uitext"></a>

약간의 여백으로 사용자 지정이 가능한 간단한 텍스트 보기를 정의합니다.

***


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/basic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
