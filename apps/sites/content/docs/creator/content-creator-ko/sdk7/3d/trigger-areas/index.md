# 트리거 영역

씬에서 트리거 영역을 사용하는 방법을 알아봅니다

트리거 영역을 사용하면 플레이어가 어떤 영역에 들어오거나 나갈 때, 또는 다른 어떤 엔티티가 영역에 들어오거나 나갈 때 발생하는 이벤트에 반응할 수 있습니다. 이는 인터랙티브한 장면을 만드는 데 기본이 되는 도구입니다. 플레이어가 다가오면 문이 열리게 하거나, 공이 골대에 들어가면 점수를 얻는 용도로 사용할 수 있습니다.

## 트리거 영역 사용하기

트리거 영역을 사용하려면 다음을 추가해야 합니다 `TriggerArea` 컴포넌트를 엔티티에 추가한 다음, 다음을 사용하세요 `triggerAreaEventsSystem` 를 사용해 이벤트에 반응합니다.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 엔티티 생성
const triggerEntity = engine.addEntity()

// Transform 설정
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8)
  })

// 트리거 영역
TriggerArea.setBox(triggerEntity)

// 트리거 영역이 활성화되었을 때의 이벤트
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  if (result.trigger?.entity !== engine.PlayerEntity) return;
  console.log('플레이어가 트리거 영역에 들어왔습니다!')
})
```

기본적으로, `TriggerArea` 컴포넌트는 다음 이벤트에 반응합니다 *모든* 플레이어가 영역으로 들어오는 것. 위 코드는 `if (result.trigger?.entity !== engine.PlayerEntity) return` 이벤트를 발생시킨 엔티티가 현재 플레이어인지, 다른 사람의 아바타가 아닌지 확인하기 위해

## 트리거 영역 모양

트리거 영역은 상자 또는 구일 수 있습니다.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

// 상자
TriggerArea.setBox(triggerEntity)

// 구
TriggerArea.setSphere(triggerEntity)
```


> [!NOTE]
> **💡 팁**: 구는 중심에서의 거리를 확인하는 방식으로 구현되므로 엔진이 계산하기 가장 쉬운 형태입니다. 확실하지 않다면 구를 사용하세요.


트리거 영역의 크기를 변경하려면 다음을 사용할 수 있습니다 `scale` 속성 `변환` 을 가진 엔티티의 `TriggerArea`.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  scale: Vector3.create(4, 2, 4),
})
```

### 디버깅

장면을 디버그하고 트리거 영역이 차지하는 범위를 보려면 다음을 추가할 수 있습니다 `MeshRenderer` 컴포넌트를 트리거 영역이 있는 엔티티에 추가하고, 모양을 디버그하려는 형태로 설정하세요. 기본 메시의 크기는 트리거 영역의 크기와 일치합니다.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

MeshRenderer.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

## 트리거 영역 이벤트

다음을 사용해 `triggerAreaEventsSystem` 트리거 영역의 다양한 이벤트에 반응할 수 있습니다:

* `onTriggerEnter`: 엔티티가 트리거 영역에 들어오면 발생합니다.
* `onTriggerExit`: 엔티티가 트리거 영역에서 나가면 발생합니다.
* `onTriggerStay`: 엔티티가 트리거 영역 안에 있는 동안, 매 프레임마다 발생합니다.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

// 진입 시
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('플레이어가 트리거 영역에 들어왔습니다!')
})

// 이탈 시
triggerAreaEventsSystem.onTriggerExit(triggerEntity, function(result) {
  console.log('플레이어가 트리거 영역에서 나갔습니다!')
})

// 머무는 동안
triggerAreaEventsSystem.onTriggerStay(triggerEntity, function(result) {
  console.log('플레이어가 트리거 영역 안에 있습니다!')
})
```

## 트리거 이벤트 응답

트리거 영역 이벤트가 발생하면, 다음을 사용할 수 있습니다 `result` 매개변수로 트리거된 엔티티와 이벤트를 발생시킨 엔티티에 대한 정보를 얻을 수 있습니다.

다음 속성들을 `result` 매개변수에서 사용할 수 있습니다:

* `triggeredEntity`: 트리거된 엔티티의 ID입니다(트리거 영역을 소유한 엔티티입니다)
* `triggeredEntityPosition`: 트리거된 엔티티의 위치
* `triggeredEntityRotation`: 트리거된 엔티티의 회전
* `eventType`: 트리거 이벤트의 유형(ENTER, EXIT, STAY)
* `timestamp`: 트리거 이벤트의 타임스탬프
* `trigger`: 다음 필드를 가진 객체:
  * `entity`: 트리거를 발생시킨 엔티티의 ID입니다(트리거 영역에 들어온 엔티티)
  * `layers`: 트리거를 발생시킨 엔티티의 충돌 레이어
  * `position`: 트리거를 발생시킨 엔티티의 위치
  * `rotation`: 트리거를 발생시킨 엔티티의 회전
  * `scale`: 트리거를 발생시킨 엔티티의 크기


> [!WARNING]
> **자주 혼동되는 점**: 이름과 달리, `result.triggeredEntity` 는 **아니며** 이벤트를 발생시킨 엔티티를 가리키지 않습니다. 이 두 필드는 쉽게 혼동됩니다:
> 
> * `result.trigger?.entity`: 트리거 영역으로 들어온 엔티티입니다. 보통은 어떤 플레이어나 오브젝트가 들어왔는지 식별하기 위해 이것을 확인하고자 합니다.
> * `result.triggeredEntity`: 다음을 소유한 엔티티입니다 `TriggerArea` 컴포넌트(들어온 엔티티가 아니라 트리거 구역 자체).
> 
> 다음을 사용하세요 `result.trigger?.entity` 들어온 엔티티를 식별해야 할 때.


```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 트리거 영역이 있는 엔티티
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 진입 시
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('엔티티가 트리거 영역에 들어왔습니다!', result.triggeredEntity)
  console.log('트리거된 엔티티 위치: ', result.triggeredEntityPosition)
  console.log('트리거된 엔티티 회전: ', result.triggeredEntityRotation)
  console.log('이벤트 유형: ', result.eventType)
  console.log('타임스탬프: ', result.timestamp)
  console.log('트리거 엔티티: ', result.trigger.entity)
  console.log('트리거 레이어: ', result.trigger.layers)
  console.log('트리거 위치: ', result.trigger.position)
  console.log('트리거 회전: ', result.trigger.rotation)
  console.log('트리거 크기: ', result.trigger.scale)
})
```

## 영역에 들어오는 플레이어 감지하기

트리거 영역의 가장 일반적인 용도는 플레이어가 들어오거나 나가는 것에 반응하는 것입니다. 두 개의 충돌 레이어가 아바타를 감지하며, 어떤 것을 사용할지는 장면이 누구에게 반응해야 하는지에 따라 달라집니다:

* `ColliderLayer.CL_MAIN_PLAYER`: 감지합니다 **로컬 머신을 사용하는 플레이어만**. 플레이어 한 명에게만 반응하려는 경우에 사용하세요. 예를 들어 해당 플레이어를 순간이동시키거나, 소리를 재생하거나, 퀘스트 진행 상황을 추적할 때입니다. 또한 성능 면에서 가장 저렴한 옵션입니다.
* `ColliderLayer.CL_PLAYER`: 감지합니다 **모든 아바타**, 로컬 플레이어와 장면에 렌더링되는 다른 모든 플레이어를 모두 포함합니다. 누군가 들어오는 것이 중요할 때 사용하세요. 예를 들어 누군가 다가오면 열리는 문이나, 방 안에 몇 명이 있는지 세는 경우입니다. 아무것도 지정하지 않으면 이 레이어가 기본값입니다.

다음 예제는 로컬 플레이어에만 반응합니다:

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 엔티티 생성
const triggerEntity = engine.addEntity()

// Transform 설정
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 로컬 플레이어만 감지하는 트리거 영역
TriggerArea.setBox(triggerEntity, ColliderLayer.CL_MAIN_PLAYER)

// 트리거 영역이 활성화되었을 때의 이벤트
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  console.log('트리거 영역에 들어왔습니다!')
})
```

사용할 때 `CL_PLAYER`, 모든 아바타가 트리거 영역을 활성화합니다. 이를 발생시킨 아바타가 로컬 플레이어인지 확인하려면 결과의 엔티티를 다음과 비교하세요: `engine.PlayerEntity`:

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 엔티티 생성
const triggerEntity = engine.addEntity()

// Transform 설정
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 기본 CL_PLAYER 레이어를 사용하는 트리거 영역
TriggerArea.setBox(triggerEntity)

// 트리거 영역이 활성화되었을 때의 이벤트
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  if (result.trigger?.entity === engine.PlayerEntity) {
    console.log('트리거 영역에 들어왔습니다!')
  } else {
    console.log('다른 플레이어가 트리거 영역에 들어왔습니다!')
  }
})
```

대부분의 경우, 모든 플레이어를 감지하려고 하기보다 현재 플레이어만 감지하고( `CL_MAIN_PLAYER`) 그런 다음 이 트리거의 효과를 모든 플레이어 간에 동기화하는 것이 이상적입니다. 예를 들어, 미닫이문은 오직 당신의 아바타에게만 열리지만, 그 문이 열린 상태는 다른 모든 플레이어와 공유됩니다.


> [!NOTE]
> **💡 팁**: 마스크에 아바타 레이어만 포함하는 트리거 영역(`CL_PLAYER`, `CL_MAIN_PLAYER`, 또는 둘 다)은 최적화됩니다. 엔진은 이를 아바타 전용 물리 레이어에 배치하므로, 아바타가 아닌 것과의 겹침은 씬 코드에 도달하기 전에 물리 엔진에 의해 제거됩니다. 마스크가 정확히 `CL_MAIN_PLAYER` 와 같은 트리거 영역은 더욱 최적화됩니다. 로컬 플레이어가 아닌 충돌체와의 겹침을 핸들러에 도달하기 전에 바로 차단하므로, 로컬 플레이어만 감지하는 비용은 사실상 없습니다.


## 트리거 영역 레이어

다음의 선택적 두 번째 인수를 사용하세요 `TriggerArea` 컴포넌트의 두 번째 인수로 트리거 영역을 활성화할 레이어를 설정합니다. 기본적으로 트리거 영역은 레이어에 의해서만 활성화됩니다. `ColliderLayer.CL_PLAYER`, 다음을 참조하세요 [영역에 들어오는 플레이어 감지하기](#detect-players-entering-an-area).

다른 엔티티를 감지하도록 하려면, 이를 다음의 두 번째 인수로 전달하여 충돌 레이어를 변경할 수 있습니다: `TriggerArea` 컴포넌트가

```ts
import { engine, Transform, TriggerArea, MeshCollider, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// 트리거 영역
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// 트리거 영역을 활성화할 엔티티
const movingEntity = engine.addEntity()

Transform.create(movingEntity, {
  position: Vector3.create(8, 0, 8),
})

MeshCollider.setBox(movingEntity, ColliderLayer.CL_CUSTOM1)
```

허용되는 값은 다음과 동일합니다 `MeshCollider` 컴포넌트를 사용하여 모든 플레이어를 순회하세요. 자세한 내용은 [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers) 을 참고하세요.

* `ColliderLayer.CL_PLAYER`: 모든 아바타(로컬 + 원격)
* `ColliderLayer.CL_MAIN_PLAYER`: 로컬 플레이어만
* `ColliderLayer.CL_PHYSICS`
* `ColliderLayer.CL_POINTER`
* `ColliderLayer.CL_CUSTOM1` 부터 `CL_CUSTOM8`
* `ColliderLayer.CL_NONE`


> [!NOTE]
> **💡 팁**: 레이어 `CL_CUSTOM1` 부터 `CL_CUSTOM8` 는 자체적으로는 특별한 동작이 없으며, 장면에 가장 적합한 용도로 사용할 수 있습니다.


여러 레이어를 한 번에 감지하도록 트리거 영역을 설정할 수도 있습니다.

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM2)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

이는 레이어가 `CL_CUSTOM1` 또는 `CL_CUSTOM2` 인 어떤 엔티티가 트리거 영역에 들어오면 트리거 영역을 활성화합니다.


> [!NOTE]
> **💡 팁**: 작동하는 예제를 살펴보려면 `TriggerArea` 끝에서 끝까지 확인하려면, 다음을 보세요 [`75,-9-trigger-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas) 테스트 씬을 보세요. 이 씬은 ENTER/EXIT 와이어 계약과 SDK의 틱별 `onTriggerStay` 합성, 회전된 영역, 사용자 지정 충돌 레이어의 이동하는 트리거 발생체, 그리고 떨어지는 엔티티에 부착된 영역을 다룹니다. 충돌 레이어 자체가 어떻게 동작하는지 보려면 다음을 참조하세요 [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/trigger-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
