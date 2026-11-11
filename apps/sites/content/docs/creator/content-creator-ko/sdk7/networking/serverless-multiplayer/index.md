> 플레이어 간에 씬 상태를 동기화합니다.

# 서버리스 멀티플레이어

Decentraland는 플레이어의 익스플로러 인스턴스에서 씬을 로컬로 실행합니다. 기본적으로 플레이어들은 서로를 보고 직접 상호작용할 수 있지만, 각 플레이어는 환경과 독립적으로 상호작용합니다. 기본적으로 환경의 변경 사항은 플레이어 간에 공유되지 않습니다.

플레이어가 더 의미 있는 방식으로 상호작용하려면 동일한 상태의 동일한 콘텐츠를 보는 것이 매우 중요합니다.

모든 플레이어가 동일한 내용을 보도록 씬 상태를 동기화하는 방법은 세 가지입니다:

* **엔티티를 동기화됨으로 표시**: 가장 쉬운 옵션입니다. 다음을 참조하세요 [엔티티를 동기화됨으로 표시](#mark-an-entity-as-synced)
* **명시적 MessageBus 메시지 전송**: 특정 메시지를 수동으로 전송하고 수신합니다. 다음을 참조하세요 [명시적 MessageBus 메시지 전송](#send-explicit-messagebus-messages)
* **멀티플레이어 서버 사용**: 다음을 참조하세요 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/). 서버는 모든 상태 변경을 검증하며 단일 진실 공급원입니다. 더 많은 설정이 필요하지만, 플레이어가 씬을 악용할 유인이 있는 경우 강력히 권장됩니다.

처음 두 옵션은 이 문서에서 다룹니다. 서버가 필요하지 않아 더 간단합니다. 단점은 플레이어의 연결 속도에 더 의존하게 되며, 모든 플레이어가 씬을 떠나면 씬 상태가 유지되지 않는다는 점입니다.

## 엔티티를 동기화됨으로 표시

다음에서 [Creator Hub](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/), 다음을 추가하여 엔티티를 동기화됨으로 표시합니다 **Multiplayer 컴포넌트** 에 추가합니다. 엔티티의 다른 각 컴포넌트에 대한 체크박스가 포함되어 있어 업데이트할 항목을 선택할 수 있습니다.

![](/docs/_remote/710c6ed40b2f7a539e355f1b.webp)

코드로 엔티티를 동기화됨으로 표시하려면 다음을 사용하세요 `syncEntity` 함수:

```ts
import { syncEntity } from "@dcl/sdk/network";

export function main() {
  const doorEntity = engine.addEntity();

  syncEntity(doorEntity, [Transform.componentId, Animator.componentId], 1);
}
```


> [!WARNING]
> **📔 참고**: 항상 다음을 호출하세요 `syncEntity()` 다음 내부에서 `main()` 함수 또는 다음 이후에 실행되는 함수에서 호출하세요 `main()` (콜백 또는 시스템 등). 파일의 최상위 수준에서 호출하면 오류가 발생합니다. 그 시점에는 플레이어의 프로필이 아직 초기화되지 않았기 때문입니다.



> [!WARNING]
> **📔 참고**: 서버리스 멀티플레이어에서는 모든 클라이언트가 다음을 호출합니다 `syncEntity` 각자 자체적으로 호출합니다. 다음으로 업그레이드하는 경우 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/), 패턴이 변경됩니다. 서버만 다음을 호출해야 합니다 `syncEntity`, 다음으로 보호됩니다 `isServer()`. 클라이언트가 생성한 엔티티가 아닌 한, 클라이언트는 공유 엔티티의 동기화를 선언하지 않아야 합니다.


다음 `syncEntity` 함수는 다음 입력을 받습니다:

* **entityId**: 동기화할 엔티티에 대한 참조
* **componentIds**: 해당 엔티티에서 동기화해야 하는 컴포넌트 목록입니다. 필요한 만큼의 컴포넌트를 포함할 수 있는 배열입니다. 모든 값은 다음이어야 합니다 `componentId` 속성입니다.
* **entityEnumId**: (선택 사항) 모든 플레이어가 일관되게 사용하는 고유 ID입니다. 다음을 참조하세요 [enum ID 정보](#about-the-enum-id).

모든 엔티티나 컴포넌트를 동기화할 필요는 없습니다. 같은 위치에 계속 있는 나무와 같은 정적 요소는 동기화가 필요하지 않습니다. 동기화하는 엔티티에서는 시간이 지남에 따라 변경되는 컴포넌트만 동기화해야 합니다. 예를 들어 클릭할 때 큐브의 색상이 바뀐다면, 절대 변경되지 않는 MeshRenderer나 Transform이 아니라 Material 컴포넌트만 동기화해야 합니다.


> [!NOTE]
> **💡 팁**: 공유하려는 데이터가 컴포넌트로 존재하지 않는다면 다음을 정의하세요 [사용자 지정 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 에 해당 데이터를 저장합니다.


### enum ID 정보

다음 **entityEnumId** 엔티티의 는 고유해야 합니다. 다음에서 할당되는 로컬 entityId와는 관련이 없습니다 `engine.addEntity()`, 이는 자동으로 생성되며 동일한 씬을 실행하는 플레이어마다 달라질 수 있습니다. 엔티티의 entityEnumId는 코드에서 명시적으로 정의되어야 하며 고유해야 합니다.

경합 조건으로 인해 씬의 한 부분이 다른 부분보다 먼저 로드될 경우 불일치를 방지하려면 이 ID를 명시적으로 설정하는 것이 중요합니다. 플레이어 A에게는 씬의 문이 엔티티 *512*일 수 있지만, 플레이어 B에게는 동일한 문이 엔티티 *513*일 수 있습니다. 이 경우 플레이어 A가 문을 열면 플레이어 B는 대신 건물 전체가 움직이는 것을 봅니다.


> [!NOTE]
> **💡 팁**: 씬에서 enum을 만들어 씬의 각 동기화 가능한 ID를 명확하게 참조하세요.
> 
> ```ts
> import { syncEntity } from "@dcl/sdk/network";
> 
> enum EntityEnumId {
>   DOOR = 1,
>   DRAW_BRIDGE = 2,
>   ELEVATOR = 3,
> }
> 
> export function main() {
>   syncEntity(
>     doorEntity,
>     [Transform.componentId, Animator.componentId],
>     EntityEnumId.DOOR
>   );
> }
> ```
> 
> 여기서 EntityEnumId enum은 엔티티에 고유 식별자를 태그하는 데 사용되며, 생성 순서와 관계없이 모든 클라이언트가 수정된 엔티티를 인식하도록 보장합니다.



> [!WARNING]
> **📔 참고**: 씬에 Smart Item도 포함되어 있다면 다음보다 큰 숫자는 사용하지 마세요 **8001** 씬에 Smart Item도 포함된 경우입니다. 다음에 의해 생성되는 아이템 [Creator Hub](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/) Multiplayer 컴포넌트가 있는 아이템은 8001부터 자동 할당된 ID를 사용합니다. 8001보다 낮은 ID는 동기화된 엔티티에 안전하게 할당할 수 있습니다.


**플레이어가 생성한 엔티티**

플레이어 상호작용의 결과로 엔티티가 생성되고 이 엔티티가 다른 플레이어와 동기화되어야 하는 경우, 엔티티에 entityEnumId가 필요하지 않습니다. 다음을 사용할 수 있습니다 `syncEntity()` 엔티티와 컴포넌트 목록만 전달합니다. entityEnumId의 고유 값은 내부적으로 자동 할당됩니다.

씬 초기화 시 인스턴스화되는 모든 엔티티에는 수동으로 할당한 ID가 있어야 합니다. 이는 모든 플레이어가 각각에 동일한 ID를 사용하도록 보장하기 위함입니다. 한 명의 플레이어가 엔티티 인스턴스화를 담당하는 경우에는 명시적 ID가 필요하지 않습니다. 다른 플레이어는 이미 ID가 할당된 이 새 엔티티에 대한 업데이트를 받으므로 ID 불일치의 위험이 없습니다.

예를 들어 눈싸움 씬에서 플레이어가 눈덩이를 던질 때마다 다른 플레이어와 동기화되는 새 엔티티를 인스턴스화합니다. 눈덩이에는 고유한 entityEnumId가 필요하지 않습니다.

```ts
import { syncEntity } from "@dcl/sdk/network";

function onThrow() {
  const ball = engine.addEntity();
  Transform.create(ball, {});
  GltfContainer.create(ball, { src: "assets/snowBall.glb" });
  syncEntity(ball, [Transform.componentId, GltfContainer.componentId]);
}
```

### 부모가 있는 엔티티 <a href="#parented-entities" id="parented-entities"></a>

엔티티의 부모는 일반적으로 다음을 통해 정의됩니다 `parent` 다음의 속성 `변환` 컴포넌트입니다. 그러나 이 속성은 달라질 수 있는 부모의 로컬 엔티티 ID를 가리킵니다. 다음을 참조하세요 [enum ID 정보](#about-the-enum-id). 동기화해야 하는 엔티티 또는 동기화해야 하는 자식이 있는 엔티티를 부모로 지정하려면 다음을 사용하세요 `parentEntity()` 함수를 다음 대신 사용하세요 `변환`.

```ts
import { syncEntity, parentEntity } from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  parentEntity(child, parent);
}
```

부모와 자식 모두 다음으로 동기화된다는 점에 유의하세요 `syncEntity`, 따라서 모든 플레이어가 두 엔티티에 사용되는 ID를 공통으로 이해할 수 있습니다. 부모의 컴포넌트가 절대 변경될 필요가 없더라도 이는 필수입니다. 이 예에서 `syncEntity` 에는 불필요한 컴포넌트의 동기화를 피하기 위해 빈 컴포넌트 배열이 포함됩니다.


> [!WARNING]
> **📔 참고**: 엔티티가 다음 둘 모두에 의해 부모로 지정된 경우 `parentEntity()` 그리고 다음 `parent` 다음의 속성 `변환` 컴포넌트의 속성은 `변환` 컴포넌트에서는 무시됩니다.


엔티티가 다음을 통해 부모로 지정된 경우 `parentEntity()` 함수로 부모를 지정하면 다음 헬퍼 함수도 사용할 수 있습니다:

* **removeParent()**: 다음의 효과를 취소합니다 `parentEntity()`. 자식 엔티티만 전달해야 합니다. 엔티티의 새 부모는 씬의 루트 엔티티가 됩니다. 원래 부모 엔티티는 씬에서 제거되지 않습니다.
* **getParent()**: 전달한 엔티티의 부모 엔티티를 반환합니다.
* **getChildren()**: 전달한 엔티티의 자식 목록을 iterable로 반환합니다.
* **getFirstChild()**: 전달한 엔티티의 목록에서 첫 번째 자식을 반환합니다.

```ts
import {
  syncEntity,
  parentEntity,
  getParent,
  getFirstChild,
  getChildren,
  removeParent,
} from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  // parent를 부모로 설정
  parentEntity(child, parent);

  // getParent
  const getParentResult = getParent(child);
  // parent 반환

  // getFirstChild
  const getFirstChildResult = getFirstChild(parent);
  // child 반환

  // getChildren
  const getChildrenResult = Array.from(getChildren(parent));
  // [child] 반환

  // child에서 parent 제거
  removeParent(child);
}
```

## 동기화 상태 확인

플레이어가 씬에 막 로드되었을 때는 주변의 다른 플레이어와 아직 동기화되지 않았을 수 있습니다. 플레이어가 동기화되기 전에 게임 상태를 변경하기 시작하면 게임에 문제가 발생할 수 있습니다. 플레이어가 씬의 어떤 항목이든 수정할 수 있도록 허용하기 전에 항상 동기화 여부를 확인하는 것이 좋습니다.

플레이어가 씬의 구획 밖으로 나가면, 밖에 서 있는 동안에도 씬과 동기화되지 않은 상태가 됩니다. 플레이어가 근처에 있는 동안에도 씬은 계속 실행되므로, 씬의 시스템이 이 시나리오를 처리하는 것도 중요합니다. 플레이어가 다시 들어오면 씬 상태의 모든 변경 사항으로 자동 업데이트됩니다.

플레이어의 씬 상태가 현재 동기화되었는지는 다음을 통해 확인할 수 있습니다 `isStateSyncronized()` 함수입니다. 이 함수는 플레이어가 이미 씬과 동기화되었으면 true인 불리언을 반환합니다.

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

const isConnected = isStateSyncronized();
```

예를 들어 이 검사를 시스템에 포함하고, 이 함수가 false를 반환하면 모든 상호작용을 차단할 수 있습니다.

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

engine.addSystem(() => {
  if (isStateSyncronized() && !button.enabled) {
    console.log("게임 시작 활성화");
    button.enable();
  }

  if (!isStateSyncronized() && button.enabled) {
    console.log(`게임 시작 비활성화.`);
    button.disable();
  }
});
```

## 명시적 MessageBus 메시지 전송


> [!WARNING]
> **📔 참고**: `MessageBus` API는 SDK에서 더 이상 사용되지 않는 것으로 표시되며 향후 버전에서 제거될 수 있습니다. 대부분의 사용 사례에서는 다음을 사용하는 것이 좋습니다 [엔티티를 동기화됨으로 표시](#mark-an-entity-as-synced).


**메시지 버스 초기화**

플레이어 간에 메시지를 전송하고 수신하는 데 필요한 메서드를 처리할 메시지 버스 객체를 만듭니다.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();
```

**메시지 전송**

다음을 사용하세요 `.emit` 메시지 버스의 명령을 사용하여 씬의 다른 모든 플레이어에게 메시지를 전송합니다.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

const myEntity = engine.addEntity();
MeshRenderer.setBox(myEntity);
MeshCollider.setBox(myEntity);

pointerEventsSystem.onPointerDown(
  {
    entity: myEntity,
    opts: { button: InputAction.IA_PRIMARY, hoverText: "클릭" },
  },
  function () {
    sceneMessageBus.emit("box1Clicked", {});
  }
);
```

각 메시지는 두 번째 인수로 페이로드를 포함할 수 있습니다. 페이로드의 유형은 `Object`,이며 전송하려는 관련 데이터를 포함할 수 있습니다.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

sceneMessageBus.emit("spawn", { position: { x: 10, y: 2, z: 10 } });
```


> [!NOTE]
> **💡 팁**: 단일 메시지에 둘 이상의 변수에서 가져온 데이터를 포함해야 한다면, 이 모든 데이터를 단일 객체에 담을 사용자 지정 타입을 만드세요.


**메시지 수신**

해당 씬의 다른 모든 플레이어로부터 온 메시지를 처리하려면 다음을 사용하세요 `.on`. 이 함수를 사용할 때는 메시지 문자열을 제공하고 실행할 함수를 정의합니다. 일치하는 문자열을 가진 메시지가 도착할 때마다 지정된 함수가 한 번 실행됩니다.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  const myEntity = engine.addEntity();
  Transform.create(myEntity, {
    position: { x: info.position.x, y: info.position.y, z: info.position.z },
  });
  MeshRenderer.setBox(myEntity);
  MeshCollider.setBox(myEntity);
});
```


> [!WARNING]
> **📔 참고**: 플레이어가 전송한 메시지는 같은 플레이어도 수신합니다. 다음 `.on` 메서드는 같은 플레이어가 전송한 메시지와 다른 플레이어가 전송한 메시지를 구분할 수 없습니다.


**전체 MessageBus 예제**

이 예제는 메인 큐브를 클릭할 때마다 메시지 버스를 사용해 새 메시지를 전송하고, 임의의 위치에 새 큐브를 생성합니다. 메시지에는 새 큐브의 위치가 포함되므로 모든 플레이어가 동일한 위치에서 새 큐브를 봅니다.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

/// --- 메시지 버스 생성 ---
const sceneMessageBus = new MessageBus();

// 큐브 팩토리
function createCube(x: number, y: number, z: number): Entity {
  const meshEntity = engine.addEntity();
  Transform.create(meshEntity, { position: { x, y, z } });
  MeshRenderer.setBox(meshEntity);
  MeshCollider.setBox(meshEntity);

  // 큐브를 클릭하면 메시지를 전송하여 다른 큐브 생성
  pointerEventsSystem.onPointerDown(
    {
      entity: meshEntity,
      opts: { button: InputAction.IA_PRIMARY, hoverText: "생성하려면 E를 누르세요" },
    },
    function () {
      sceneMessageBus.emit("spawn", {
        position: {
          x: 1 + Math.random() * 8,
          y: Math.random() * 8,
          z: 1 + Math.random() * 8,
        },
      });
    }
  );

  return meshEntity;
}

// 초기화
createCube(8, 1, 8);

// 데이터 타입 정의
type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

// spawn 메시지에서 새 큐브 생성
sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  createCube(info.position.x, info.position.y, info.position.z);
});
```

## 멀티플레이어 씬을 로컬에서 테스트하기

씬 미리 보기를 실행하여 두 개 이상의 서로 다른 explorer 창에서 열면, 각 열린 창은 별도의 플레이어로 인식되며 모의 통신 서버가 이 플레이어들을 동기화 상태로 유지합니다.

한 창에서 씬과 상호작용한 다음 다른 창으로 전환하면 그 상호작용의 결과도 거기에서 보입니다.

Creator Hub에서 다음을 켭니다: **멀티 인스턴스 미리 보기** 다음의 **데스크톱 클라이언트** 의 하위 메뉴 **실행 옵션**, 그런 다음 다음을 클릭하세요: **실행** 버튼을 두 번째로 눌러 두 번째 Decentraland explorer 창을 여세요. 두 창 모두 서로 다른 주소로 연결해야 합니다. 씬이 다시 로드되어도 같은 세션이 열린 상태로 유지됩니다.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

대안으로, 다음을 브라우저 URL에 입력하여 두 번째 Decentraland explorer 창을 열 수 있습니다:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## 싱글 플레이어 씬

씬이 다음에 배포된 경우 [Decentraland World](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds), 싱글 플레이어 씬으로 만들 수 있습니다. 플레이어는 서로를 볼 수 없고, 채팅하거나 서로의 행동 효과를 볼 수도 없습니다.

이렇게 하려면 씬의 다음을 구성하세요 `scene.json` 파일에서 다음을 설정합니다 **fixedAdapter** 다음으로 `offline:offline`. 씬에는 Communication Service가 전혀 없으며, 해당 월드에 참여하는 각 사용자는 항상 혼자입니다.

**예시:**

```json
{
  "worldConfiguration": {
    "name": "my-name.dcl.eth",
    "fixedAdapter": "offline:offline"
  }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/networking/serverless-multiplayer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
