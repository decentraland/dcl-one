# 엔진 API

이 `EngineApi` 모듈은 월드 익스플로러(게임 루프를 실행함)와 개별 씬(자체 엔티티를 관리함) 사이에서 공유되는 엔티티-컴포넌트-시스템 프레임워크와 관련 유틸리티에 대한 접근을 제공합니다.

```ts
const engine = require("~system/EngineApi");
```

이 모듈은 기능 면에서 가장 복잡하고 풍부하며, 업데이트와 확장이 가장 많이 이뤄질 가능성이 높습니다. 사람들 이 만들 수 있는 경험의 종류를 결정하기 때문에, Decentraland의 힘이 가장 많이 담겨 있는 곳이기도 합니다.

이 모듈에는 다음 메서드가 포함됩니다:

* [`function crdtSendToRenderer`](#crdtsendtorenderer)
* [`function crdtGetState`](#crdtgetstate)

## 소개

이 `EngineApi` 모듈은 업데이트, 이벤트, 명령을 교환함으로써 익스플로러와 씬 사이의 월드 상태를 동기화하도록 설계되었습니다. 확장 가능한 메시지 프로토콜을 구현하며, 이를 통해 씬은 다음과 같은 작업을 할 수 있습니다:

* 엔티티를 생성하고 삭제합니다.
* 컴포넌트를 부착, 업데이트, 제거합니다.
* 3D 환경에서 광선을 발사하고 히트를 감지합니다.
* 플레이어의 입력과 같은 이벤트를 수신합니다.

```
.------------------------------------------------------.
| 월드 익스플로러                                       |
|                                                      |
|                   [Engine API]                       |
|                        |                             |
|  .--------.            |              .------------. |
|  |        |<-----------+<-------------+  런타임   | |
|  |  게임  |            |   명령      |  .------.  | |
|  |        |   이벤트   |              |  | 씬 | | |
|  | 엔진 +----------->+------------->|  |       | | |
|  |        |            |              |  '-------' | |
|  '--------'            |              '------------' |
|                                                      |
'------------------------------------------------------'

```


> [!NOTE]
> 이 `EngineApi` 모듈은 개편 중입니다. 소스 정의를 살펴보면 더 이상 사용되지 않거나 최신 버전에서는 no-op로 구현할 수 있는 레거시 메서드가 있습니다(자세한 내용은 아래 참조).


## ECS 프레임워크

와 함께 `EngineApi` 모듈에는 공유 ECS 프레임워크의 일반적이고 확장 가능한 구현이 포함되어 있으며, 이를 통해 씬과 게임 엔진 자체가 엔티티를 생성하고 컴포넌트를 부착하며 상태를 업데이트할 수 있습니다.

양쪽에서 이루어진 변경 사항은 메시지 교환을 통해 서로 반영됩니다. 사용되는 메커니즘([아래 참조](#synchronization))은 양쪽이 어떤 업데이트 순서에 동의하고, 월드 상태에 대해 결국 일관성에 도달하도록 보장합니다.

월드 익스플로러는 기본 컴포넌트(위치, 모양, 텍스처, 미디어 등)를 잘 알려진 집합으로 구현하며, 업데이트를 받으면 해당 상태의 역직렬화와 변경 적용 방법을 알고 있습니다. 를 사용하는 씬은 [SDK](https://docs.decentraland.org/creator) 또한 사용자 정의 컴포넌트를 생성하는 유틸리티가 있지만, 그것들은 전적으로 씬에서 관리되며(동기화되지 않음) 따라서 프로토콜 범위 밖에 있습니다.


> [!NOTE]
> 대부분의 씬은 Decentraland SDK를 사용하며, 이는 `EngineApi` 모듈을 캡슐화하고 콘텐츠 개발자에게 훨씬 더 나은 고수준 인터페이스를 제공합니다. 이 모듈의 메시징 프로토콜에 직접 접근하는 씬은 매우 드뭅니다.


### 엔티티 식별 <a href="#identifying" id="identifying"></a>

엔티티 ID는 다음으로 시작하는 단순한 정수입니다 `0`.

프로토콜상, 다음보다 작은 ID는 `512` 월드 익스플로러용으로 예약되어 있으며, 따라서 씬이 생성한 엔티티에는 유효하지 않습니다. 숫자 `0`, `1` 및 `2` 실제로 사용 중입니다( [기본 엔티티](/docs/contributor/contributor-ko/scene-runtime/basic-entities/)참조), 나머지 범위는 향후 확장을 위해 사용할 수 있습니다.

각 씬은 자체 비공개 ID 범위를 가지며, 익스플로러는 이를 게임 엔진의 모든 엔티티에 걸친 전역 식별자로 투명하게 매핑할 수 있습니다.

시간이 지나 엔티티가 생성되고 삭제되면서, 이전에 해제된 ID를 재사용하는 것은 전혀 문제가 없습니다. 실제로 이는 성능상의 이유로 필요할 수도 있습니다([아래 참조](#crdtstatedeleteentities)).

## 동기화 <a href="#synchronization" id="synchronization"></a>

월드 익스플로러와 씬은 모두 공유 엔티티와 컴포넌트 집합을 관리해야 하며, 업데이트는 비동기적으로 양쪽에서 들어옵니다. 추가 조치가 없으면 월드 상태의 각 버전은 서로 다르게 끝날 수 있으며, 실제로도 그렇게 됩니다.

이를 방지하기 위해 CRDT([충돌 없는 복제 데이터 타입](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type))를 사용하여 업데이트 메시지를 교환함으로써 양쪽이 합의에 도달합니다. 이는 여러 가지 장점을 제공합니다:

* 양쪽은 상태를 독립적으로 그리고 동시에 업데이트할 수 있습니다.
* 충돌은 양쪽이 로컬에서 동일하게 적용하는 공유 해결 전략으로 관리됩니다.
* 양쪽 모두 공유된 동일한 상태로 수렴하는 것이 보장됩니다.
* 메시지 크기의 약간의 오버헤드 외에는 추가적인 조율이나 메시징 왕복이 필요하지 않습니다.


> [!NOTE]
> 이 페이지는 로컬에서 실행 중인 씬과 게임 엔진 사이에서 상태를 동기화하기 위한 CRDT 사용법을 설명하지만, 이 메커니즘의 진정한 잠재력은 멀티플레이어 시나리오에 있습니다. 이 프로토콜을 사용하면 플레이어들이 게임 상태를 조정하고 동일한 경험을 공유할 수 있습니다.


이것이 동작하려면 모든 메시지는 가환적이고 멱등적이어야 합니다. 순서가 뒤바뀐 메시지도 중간 업데이트가 아직 수신되지 않았더라도 적용할 수 있으며, 같은 메시지는 일관성을 깨지 않고 다시 처리할 수 있습니다.

위의 아키텍처 다이어그램을 다시 살펴보되, 이제 CRDT에 초점을 맞춰 보겠습니다:

```
.-------------------------------------------------------.
| 월드 익스플로러                                        |
|                                                       |
| .-------------.                       .------------.  |
| | 게임 엔진 |                       |   씬     | |
| |  .-------.  |                       |  .-------.  | |
| |  |       +--+-----------------------+->|       |  | |
| |  | CRDT  |  |     동기화 프로토콜     |  | CRDT  |  | |
| |  |       |<-+-----------------------+--+       |  | |
| |  '-------'  |                       |  '-------'  | |
| |             |                       |             | |
| '-------------'                       '-------------' |
'-------------------------------------------------------'

```

CRDT 동기화 메커니즘의 구현은 세 부분으로 구성됩니다:

1. 모든 엔티티와 컴포넌트의 상태에 대한 내부 표현.
2. 경합하는 상태들 중에서 선택하는 충돌 해결 전략.
3. 상태 업데이트를 전달하고 처리하는 메시징 프로토콜.

### CRDT 상태 <a href="#crdtstate" id="crdtstate"></a>

CRDT는 모든 부착된 엔티티에 대한 모든 컴포넌트의 상태를 보유하며, 각 엔티티에 대해 컴포넌트를 타임스탬프가 있는 상태로 매핑하여 저장할 수 있습니다. 의사코드로 보면:

```ts
// 특정 시점의 특정 엔티티에 대한 컴포넌트의 (완전히 일반적인) 상태:
type EntityComponentState = { timestamp: number; state: byte[] };

// 부착된 모든 엔티티에 대한 컴포넌트의 상태:
type ComponentState = Map<EntityId, EntityComponentState>;

// CRDT의 전체 상태:
type CRDTState = Map<ComponentId, ComponentState>;
```

타임스탬프는 실제로 Unix 스타일의 타임스탬프가 아닙니다. 이는 업데이트의 순서를 정하는 증분 카운터의 한 종류이며, 시계 시간과는 아무런 관련이 없습니다. 이에 대한 더 자세한 내용은 [아래](#timestamps).

#### 상태 항목 생성 <a href="#crdtstateautocreate" id="crdtstateautocreate"></a>

CRDT 계층은 사용 가능한 모든 컴포넌트를 알지 못하기 때문에(알 필요도 없기 때문에), 초기 상태는 비어 있는 맵입니다. 작업이 다음을 포함할 때 항목이 즉석에서 생성됩니다 `ComponentId` 또는 `EntityId` 아직 매핑되지 않은 것입니다. 엔티티가 제거되면 연결된 상태는 모든 컴포넌트에서 삭제됩니다.

이를 좀 더 의사코드로 설명해 보겠습니다.

```ts
const crdtState = new Map(); // 비어 있음, 새 컴포넌트와 엔티티를 위해 자동으로 공간을 마련합니다
```

```ts
function putEntityComponentState(componentId, entityId, entityComponentState) {
  // 이 컴포넌트가 아직 알려져 있지 않으면 추가합니다:
  if (!crdtState[componentId]) {
    crdtState[componentId] = new Map();
  }

  // 이 엔티티에 이 컴포넌트에 대한 상태가 없으면 추가하고 끝냅니다:
  if (!crdtState[componentId][entityId] && shouldCreate(entityId)) {
    crdtState[componentId][entityId] = entityComponentState;
    return;
  }

  // 이 시점에서 우리는 두 개의 경쟁 상태를 가지고 있습니다. 방금 받은 것이 반드시
  // 유지해야 하는 것은 아닙니다. 결정을 내려야 합니다.
  const existingEntityComponentState = crdtState[componentId][entityId];

  // 충돌 해결 규칙이 그렇게 말하는 경우에만(자세한 내용은 나중에 설명) 상태를 교체합니다:
  if (shouldReplace(entityComponentState, existingEntityComponentState)) {
    crdtState[componentId][entityId] = entityComponentState;
  }
}
```

위의 의사코드(실제 코드로 옮긴다면)는 분명히 불완전하고 최적이 아닙니다. 특히 다음의 정의가 빠져 있습니다 `shouldCreate` 및 `shouldReplace`, 이는 충돌 해결 전략을 캡슐화합니다. 이들에는 요구사항이 있습니다:

* `shouldCreate` 새 엔티티를 추가해야 하지만, 삭제된 엔티티를 다시 생성하는 것은 허용하지 않아야 합니다.
* `shouldReplace` 더 큰 타임스탬프를 가진 상태를 유지하는 쪽을 선호해야 합니다(몇 가지 예외 상황이 있음).

#### 삭제된 엔티티 추적 <a href="#crdtstatedeleteentities" id="crdtstatedeleteentities"></a>

를 지원하려면 `shouldCreate` 요구사항에 따라, 이후의 상태 업데이트를 무시할 수 있도록 삭제된 엔티티 ID를 추적해 두고 싶을 것입니다. 이를 좀 더 의사코드로 살펴보겠습니다:

```ts
const crdtDeletedEntities = new Set();

function shouldCreate(entityId) {
  return !crdtDeletedEntities.contains(entityId);
}

function deleteEntity(entityId) {
  // 이 엔티티를 삭제된 것으로 표시합니다:
  crdtDeletedEntities.add(entityId);

  // 모든 컴포넌트에서 이 엔티티의 상태를 삭제합니다:
  for (componentState of crdtState) {
    delete componentState[entityId];
  }
}
```


> [!NOTE]
> 위의 단순한 구현에서는 삭제된 엔티티 집합이 계속 커지기만 한다는 점에 유의하세요. 엔티티를 빠르게 재활용하는 씬에서는 메모리 사용량이 폭증할 수 있습니다.
> 
> 이를 피하기 위해 Foundation의 익스플로러는 생성 인덱스를 사용하여 제한된 수치 공간에서 ID를 재사용하고, 활성 및 삭제된 엔티티를 추적하는 데 필요한 메모리 양을 제한합니다.


그리고 `shouldReplace` 요구사항은 [충돌 해결](#crdtconflicts) 아래를 참조하세요.

### 타임스탬프 <a href="#timestamps" id="timestamps"></a>

위에서 언급했듯이, CRDT 타임스탬프를 이야기할 때는 특정 시점의 실제 Unix 타임스탬프를 가리키는 것이 아닙니다. 대신, [Lamport 타임스탬프](https://en.wikipedia.org/wiki/Lamport_timestamp) (특수한 종류의 공유 증분 카운터)를 사용하여 양쪽의 이벤트 순서를 추적합니다.

이 카운터의 값은 다음 규칙에 따라 업데이트됩니다:

1. 로컬 카운터를 `0`.
2. 원격 당사자에게 메시지를 보내기 전에 로컬 카운터를 증가시킵니다.
3. 원격 당사자로부터 메시지를 받은 후, 로컬 카운터를 현재 값과 수신한 값 중 더 큰 값에 `1`.

의사코드로 보면:

```ts
const localTimestamp = 0;

function sendState(state) {
  send({ state, timestamp: ++localTimestamp });
}

function receiveState() {
  const { state, timestamp } = receive();
  localTimestamp = max(localTimestamp, remoteTimestamp) + 1;
}
```

### 충돌 해결 <a href="#crdtconflicts" id="crdtconflicts"></a>

CRDT가 두 개의 경쟁 상태를 만나면, 모든 당사자가 동일하게 적용하는 공유 해결 전략이 필요하며, 이를 통해 모두가 같은 결론에 도달하도록 보장합니다. Decentraland 프로토콜은 매우 단순한 규칙을 사용합니다:

1. 엔티티가 삭제되었고(ID가 절대 재사용되지 않는다면), 새로운 상태는 무시합니다.
2. 이전 상태가 없었다면, 새 상태를 유지합니다.
3. 새 상태의 타임스탬프가 더 크면, 새 상태를 유지합니다.
4. 새 상태의 타임스탬프가 더 작으면, 기존 상태를 유지합니다.
5. 두 타임스탬프가 같다면, 상태를 바이트 단위로 비교하여 더 낮은 값을 유지합니다.

대부분의 경우 규칙 `1`, `2` 및 `3`.

### 초기 동기화

씬이 자체 코드를 실행하기 전에, 런타임은 모든 것의 상태로 CRDT를 채웁니다 [기본 엔티티](/docs/contributor/contributor-ko/scene-runtime/basic-entities/) 및 그 컴포넌트들.

이 초기 동기화 동안에는 런타임만 공유 상태를 설정할 수 있습니다. 프로세스가 완료될 때까지 씬은 수정을 할 수 없습니다.

### 메시징 프로토콜 <a href="#messages" id="messages"></a>

월드 익스플로러와 씬 런타임 간의 메시지는 직렬화된 이진 표현의 구조체입니다. 모든 메시지는 유형과 길이를 나타내는 헤더를 포함하고, 그 뒤에 각 메시지 유형에 대한 특정 페이로드가 이어집니다.

```
.----------------.--------------.---------------------------------.
| length: uint32 | type: uint32 |     payload: byte[length]       |
'----------------'--------------'---------------------------------'
╵         공통 필드         ╵         유형 종속          ╵

```

CRDT 프로토콜에는 세 가지 메시지 유형이 있습니다:

* [`PutComponent`](#putcomponent)
* [`DeleteComponent`](#deletecomponent)
* [`DeleteEntity`](#deleteentity)

다음과 같은 `CreateComponent` 또는 `CreateEntity` 메시지는 없습니다. CRDT 규칙은 앞서 [설명한 대로](#crdtstateautocreate).

#### `PutComponentMessage` <a href="#putcomponent" id="putcomponent"></a>

의 `상태를 업데이트합니다` 의 `컴포넌트` 특정 `엔티티에 대해`필요한 경우 CRDT에서 알려지지 않은 컴포넌트와 엔티티를 생성합니다. [충돌 해결](#crdtconflicts) 에 따라 `타임스탬프`.

```
.----------------.-------------------.-------------------------------------------.
| entity: uint32 | component: uint32 | timestamp: uint32 |     state: byte[]     |
'----------------'-------------------'-------------------------------------------'
╵                       공통 필드                    ╵  컴포넌트 종속  ╵

```

이 `상태를 업데이트합니다` 필드는 컴포넌트가 정의한 이진 직렬화입니다. 대부분의 컴포넌트는 [프로토콜 버퍼](https://protobuf.dev) 를 사용하여 메시지를 인코딩하며, 이는 `.proto` 파일에 지정된 [Decentraland 프로토콜 패키지의](https://github.com/decentraland/protocol). 이 규칙의 주목할 만한 예외는 `변환` 컴포넌트이며, 이는 가장 흔한 컴포넌트인 만큼 최적화된 직렬화 형식을 가집니다.

이 추가적인 직렬화 계층에는 중요한 장점이 있습니다. CRDT 프로토콜은 현재 또는 미래의 어떤 컴포넌트 구현에도 구애받지 않습니다.

이 메시지에 포함된 업데이트가 CRDT에 적용되면, `상태를 업데이트합니다` 필드는 구조체에 있는 그대로 복사됩니다.

#### `DeleteComponentMessage` <a href="#deletecomponent" id="deletecomponent"></a>

의 상태를 제거합니다 `컴포넌트` 에 대한 `엔티티에 대해`. [충돌 해결](#crdtconflicts) 에 따라 `타임스탬프`.

```
.----------------.-------------------.-------------------.
| entity: uint32 | component: uint32 | timestamp: uint32 |
'----------------'-------------------'-------------------'

```

#### `DeleteEntityMessage` <a href="#deleteentity" id="deleteentity"></a>

삭제 `엔티티에 대해` (즉, 모든 관련 컴포넌트 상태를)하며, 식별자가 다른 엔티티에 대해 다시 사용되지 않을 것으로 예상합니다.

```
.----------------.
| entity: uint32 |
'----------------'

```

다음과 같은 `타임스탬프` 필드가 없다는 점에 유의하세요. 의 수명이 끝났기 때문에 `엔티티에 대해` , 순서가 뒤바뀐 업데이트에 대한 충돌 해결 전략은 단순히 그것들을 무시하는 것입니다.

## 메서드

의 메서드 집합은 `EngineApi` 메시지를 교환하고 처음부터 월드 상태를 재구성하기 위한 인터페이스를 제공합니다.

### `crdtSendToRenderer` <a href="#crdtsendtorenderer" id="crdtsendtorenderer"></a>

씬에서 렌더러로 직렬화된 메시지를 보내고, 렌더러가 씬에 대해 가지고 있는 직렬화된 메시지 배열을 반환합니다.

```ts
인터페이스 Request {
  // 이 요청에 대한 직렬화된 메시지:
  data: byte[];
}

인터페이스 Response {
  // 렌더러로부터 온 직렬화된 메시지 배열(있는 경우):
  data: byte[][];
}

function crdtSendToRenderer(Request): Promise<Response>;
```

### `crdtGetState` <a href="#crdtgetstate" id="crdtgetstate"></a>

```ts
interface Request {}

인터페이스 Response {
  // 상태에 씬이 생성한 엔티티가 있는지 여부:
  hasEntities: boolean;

  // CRDT 상태를 재구성할 수 있는 메시지 배열:
  data: byte[][];
}

function crdtGetState(Request): Promise<Response>;
```

## 레거시 씬

구형 씬(즉, SDK 버전 6 이하로 빌드된 씬)은 최신 CRDT 메커니즘을 사용하지 않고, 대신 `EngineApi` 의 메서드를 호출합니다. 지금은 더 이상 사용되지 않습니다.

이러한 씬에 대한 지원은 프로토콜을 준수하는 Decentraland 애플리케이션의 요구사항이 아닙니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/runtime-modules/engine-api.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
