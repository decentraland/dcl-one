# 기본 엔티티

월드 익스플로러 [범위를 예약합니다](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/engine-api/#identifying) 엔티티 ID의 범위를 자체 용도로 예약합니다. 이 범위에서 식별자는 `0`, `1` 및 `2` 존재가 보장되는 기본 엔티티 집합에 미리 할당됩니다.

다음과 같습니다:

* `0`: `루트 엔티티`
* `1`: `플레이어 엔티티`
* `2`: `카메라 엔티티`

이 엔티티에는 런타임이 관리하는 알려진 첨부 컴포넌트 집합이 있으며, 즉 씬에 대해 해당 상태는 읽기 전용입니다. 이러한 컴포넌트의 초기 상태를 가져오는 것은 [씬 초기화](/docs/contributor/contributor-ko/scene-runtime/execution/#onstart).


> [!NOTE]
> 이 엔티티에 대한 추가 컴포넌트는 개발 중입니다.  [CRDT 동기화 메커니즘](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/engine-api/#synchronization) 는 서로 다른 모듈이 제공하는 기능의 일부를 제공하도록 확장되고 있습니다.
> 
> 예를 들어, 씬은 네트워크 및 CPU 사용을 최적화하기 위해 익스플로러 창의 상태를 감지하거나, 플레이어, 통신 시스템 및 게임 카메라에 대한 더 풍부한 정보에 접근할 수 있게 됩니다.


#### `루트 엔티티` (`0`) <a href="#rootentity" id="rootentity"></a>

씬의 컨텍스트에 대한 일반 정보를 보관합니다.

* [`변환`](/docs/contributor/contributor-ko/scene-runtime/basic-components/#transform): 씬의 원점에 대한 위치 정보로, 명시적인 부모가 없는 다른 모든 변환은 이 원점을 기준으로 합니다.

그 `변환` 의 `루트 엔티티` 는 다음 값으로 초기화됩니다:

```yml
scale: [1, 1, 1] # XYZ
position: [0, 0, 0] # XYZ
rotation: [0, 0, 0, 1] # XYZW
```

#### `플레이어 엔티티` (`1`) <a href="#playerentity" id="playerentity"></a>

플레이어, 그 신원, 아바타 및 위치에 대한 데이터를 보관합니다.

* [`변환`](/docs/contributor/contributor-ko/scene-runtime/basic-components/#transform): 플레이어 아바타의 위치 정보.

#### `카메라 엔티티` (`2`) <a href="#cameraentity" id="cameraentity"></a>

게임 카메라에 대한 정보를 보관합니다. 모든 컴포넌트는 렌더러가 관리합니다.

* [`변환`](/docs/contributor/contributor-ko/scene-runtime/basic-components/#transform): 카메라 자체의 위치 정보.
* [`포인터 잠금`](/docs/contributor/contributor-ko/scene-runtime/basic-components/#pointerlock): 포인터가 카메라 방향을 따르는지, 아니면 독립적으로 움직이는지 여부.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/basic-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
