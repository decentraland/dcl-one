> 플레이어 아바타에 힘과 충격량을 적용합니다

# 플레이어 물리

씬 코드에서 플레이어의 아바타에 물리적 힘을 적용할 수 있습니다. 이를 통해 점프 패드, 바람 영역, 넉백 효과, 폭발 등 다양한 게임플레이 메커니즘을 만들 수 있습니다.

적용할 수 있는 힘에는 두 가지 종류가 있습니다:

* **임펄스**: 한 번만 즉시 가해지는 밀기입니다. 플레이어를 하늘로 발사하거나 뒤로 밀어내는 등 갑작스럽고 개별적인 효과에 사용합니다.
* **연속 힘**: 활성화되어 있는 동안 매 틱마다 지속적으로 가해지는 밀기입니다. 바람 영역, 물살, 중력장 같은 지속 효과에 사용합니다.

둘 다 다음을 통해 적용됩니다 `Physics` 헬퍼이며, 다음에서 가져옵니다 `@dcl/sdk/ecs`.


> [!WARNING]
> **📔 참고**: 이 힘들은 로컬 플레이어의 아바타에만 영향을 줍니다. 다른 플레이어는 다른 플레이어의 위치 변화는 보지만, 멀티플레이어에서 힘 자체는 다른 플레이어에게 동기화되지 않습니다. 각 플레이어의 물리는 자신의 인스턴스에서 로컬로 실행됩니다.


## 임펄스 적용

다음을 사용하세요 `Physics.applyImpulseToPlayer()` 를 사용해 플레이어에게 특정 방향으로 한 번만 밀어주는 힘을 줍니다.

* `벡터`: 방향과 세기가 결합된 값입니다. 벡터의 길이는 임펄스 크기를 나타냅니다.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 플레이어를 곧장 위로 발사
Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
```

방향과 크기를 따로 전달할 수도 있습니다. 이 경우 방향 벡터는 자동으로 정규화됩니다:

* `방향`: 밀어줄 방향 — 스케일링하기 전에 자동으로 정규화됩니다.
* `크기`: 임펄스 세기.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 플레이어를 위쪽으로 크기 50으로 발사
Physics.applyImpulseToPlayer(Vector3.create(0, 1, 0), 50)
```

다음을 호출하면 `applyImpulseToPlayer()` 같은 프레임 안에서 여러 번 호출하더라도 임펄스는 누적됩니다 — 서로 더해져 하나의 합산 임펄스로 적용됩니다.

### 임펄스의 세기

아바타의 질량은 1이므로, 임펄스의 크기는 아바타 속도의 변화량이며 단위는 미터/초입니다. 임펄스 `50` 는 그 방향으로 50 m/s를 더합니다.

* 임펄스가 추가하는 속도는 시간이 지나며 감소합니다. 공중에서는 초당 1.5의 비율로 감소하므로, 1초 후에는 약 5분의 1만 남습니다. 아바타가 지면에 있을 때는 더 빠르게 감소합니다.
* 임펄스와 힘으로 추가되는 속도는 50 m/s로 상한이 있습니다.
* 위쪽으로 가해지는 임펄스는 먼저 낙하로 인해 아바타가 가진 아래쪽 속도를 상쇄하므로, 점프 패드는 플레이어가 위에서 내려와 착지하든, 걸어서 올라가든 동일하게 작동합니다.

### 플레이어 진입 시 임펄스 트리거

일반적인 패턴은 플레이어가 영역에 들어올 때 임펄스를 트리거하는 것입니다. 트리거 영역을 사용해 플레이어의 진입을 감지하세요:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const launchPad = engine.addEntity()
Transform.create(launchPad, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(launchPad)
TriggerArea.setBox(launchPad, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(launchPad, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
})
```

## 넉백 임펄스 적용

다음을 사용하세요 `Physics.applyKnockbackToPlayer()` 를 사용해 단일 임펄스로 플레이어를 한 지점에서 멀어지게 밀어냅니다. 폭발, 충돌 효과, 기타 단발성 방향성 폭발에 이상적입니다. 방향은 소스 지점에서 플레이어의 현재 위치까지 자동으로 계산됩니다.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const explosionPosition = Vector3.create(8, 1, 8)

// 폭발 중심에서 플레이어를 밀어냄
Physics.applyKnockbackToPlayer(explosionPosition, 40)
```

다음으로 효과 범위를 제한할 수 있습니다 `반경`을 사용하고, 다음을 사용해 거리와 함께 크기가 어떻게 감소할지 제어할 수 있습니다 `KnockbackFalloff` 옵션:

* `fromPosition`: 넉백의 월드 공간 원점(폭발 중심, 적 위치 등).
* `크기`: 기본 임펄스 세기.
* `반경`: 효과의 최대 거리(기본값: `Infinity`).
* `falloff`: 거리와 함께 크기가 얼마나 감소하는지(기본값: `CONSTANT`).

```ts
import { Physics, KnockbackFalloff } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

Physics.applyKnockbackToPlayer(
	Vector3.create(8, 1, 8),
	40,                          // 크기
	10,                          // 반경: 10미터를 넘으면 효과 없음
	KnockbackFalloff.LINEAR      // 반경 가장자리에서 크기가 0으로 감소
)
```

다음 `KnockbackFalloff` 열거형은 힘이 거리와 함께 어떻게 감소하는지 제어합니다:

* `KnockbackFalloff.CONSTANT` — 반경 안의 어느 거리에서나 동일한 크기(기본값)
* `KnockbackFalloff.LINEAR` — 반경 가장자리에서 0까지 부드럽게 선형 감소
* `KnockbackFalloff.INVERSE_SQUARE` — 급격하고 물리적으로 현실적인 감쇠

플레이어가 소스 위치와 정확히 같으면, 플레이어는 곧장 위로 밀립니다. 음수 크기를 사용하면 플레이어를 밀어내는 대신 그 지점으로 끌어당깁니다.

동일한 `KnockbackFalloff` 값을 다음에서도 사용할 수 있습니다 `applyRepulsionForceToPlayer()`.

## 연속 힘 적용

다음을 사용하세요 `Physics.applyForceToPlayer()` 를 사용해 플레이어에게 지속적인 힘을 가합니다. 임펄스와 달리 이 힘은 활성화되어 있는 동안 매 틱마다 적용됩니다.

* `소스`: 이 힘의 소스임을 식별하는 엔티티 — 나중에 힘을 업데이트하거나 제거할 때 사용합니다.
* `벡터`: 방향과 세기가 결합된 값입니다. 벡터의 길이는 힘의 크기를 나타냅니다.

소스 엔티티의 위치는 중요하지 않습니다 — 식별자로만 사용됩니다.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windZoneEntity = engine.addEntity()

// 플레이어를 지속적으로 오른쪽으로 밀기
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(10, 0, 0))
```

힘 벡터는 항상 **월드 공간**에 있습니다. 회전된 엔티티를 기준으로 한 방향이 필요하면, 먼저 `Transform.localToWorldDirection()` 를 사용해 변환하세요 — 다음을 참조하십시오 [로컬 방향을 월드 공간으로 변환](#convert-a-local-direction-to-world-space).

다음과 마찬가지로 `applyImpulseToPlayer()`다음도 전달할 수 있습니다 `방향` 과 `크기` 를 별도로 — 다음을 참조하십시오 [임펄스 적용](#apply-an-impulse):

```ts
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(0, 1, 0), 50)
```

다음을 호출하면 `applyForceToPlayer()` 를 같은 소스 엔티티로 다시 호출하면, 해당 소스의 이전 힘을 대체합니다. 여러 힘 소스가 누적되면 각 틱마다 벡터가 합산됩니다.

### 연속 힘 제거

힘 적용을 중지하려면 `Physics.removeForceFromPlayer()` 를 호출하고, 힘을 생성할 때 사용한 소스 엔티티 참조를 전달하세요. 엔티티가 현재 힘을 적용하고 있지 않더라도 이 호출은 안전하게 무시됩니다.

* `소스`: 힘이 적용될 때 사용된 엔티티.

```ts
Physics.removeForceFromPlayer(windZoneEntity)
```

### 제한된 시간 동안 힘 적용

다음을 사용하세요 `Physics.applyForceToPlayerForDuration()` 를 사용해 특정 시간 동안 힘을 적용합니다. 지속 시간의 단위는 초입니다. 시간이 지나면 힘은 자동으로 제거됩니다. 같은 소스 엔티티로 다시 호출하면 타이머가 재설정됩니다.

* `소스`: 이 힘 소스를 식별하는 엔티티.
* `지속 시간`: 힘이 지속되는 시간(초).
* `벡터`: 방향과 세기가 결합된 값입니다. 벡터의 길이는 힘의 크기를 나타냅니다.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const gustEntity = engine.addEntity()

// 1.5초 동안 강한 위쪽 힘 적용
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 50, 0))
```

다음 `방향` + `크기` 오버로드도 사용할 수 있습니다 — 다음을 참조하십시오 [임펄스 적용](#apply-an-impulse):

```ts
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 1, 0), 50)
```

### 바람 영역 예제

이 예제는 플레이어가 내부에 있는 동안 밀어주고, 나가면 멈추는 바람 터널을 만듭니다:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, MeshCollider, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windTunnel = engine.addEntity()
Transform.create(windTunnel, {
	position: Vector3.create(8, 1, 8),
	scale: Vector3.create(4, 3, 4),
})
MeshRenderer.setBox(windTunnel)
TriggerArea.setBox(windTunnel, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyForceToPlayer(windTunnel, Vector3.create(15, 0, 0))  // X축을 따라 옆으로 밀기
})

triggerAreaEventsSystem.onTriggerExit(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.removeForceFromPlayer(windTunnel)
})
```

## 반발력 적용

다음을 사용하세요 `Physics.applyRepulsionForceToPlayer()` 를 사용해 공간의 고정된 지점에서 플레이어를 지속적으로 멀어지게 밀어냅니다. 자력 반발장, 힘 장벽, 또는 플레이어를 일정 거리 이상 가까이 오지 못하게 하는 떠 있는 물체 같은 효과에 적합합니다. 다음과 달리 `applyKnockbackToPlayer()`이 힘은 플레이어가 움직일 때마다 매 틱 다시 계산됩니다. 크기는 반경과 감쇠에 따라 거리와 함께 약해집니다 — 다음을 참조하십시오 [넉백 임펄스 적용](#apply-a-knockback-impulse) 에 대한 `KnockbackFalloff` 옵션. 음수 크기를 사용하면 효과가 반전되어, 소용돌이나 중력 우물처럼 플레이어를 그 지점으로 지속적으로 끌어당깁니다.


> [!WARNING]
> **📔 참고**: 반발의 원점은 `fromPosition` 벡터입니다 — **아니라** 엔티티의 `소스` 위치입니다. 이 `소스` 엔티티는 나중에 힘을 업데이트하거나 제거할 수 있도록 식별자로만 사용됩니다. 위치, 회전, 크기는 완전히 무시됩니다.


* `소스`: 이 힘의 식별자로 사용되는 엔티티 — 위치는 사용되지 않습니다.
* `fromPosition`: 플레이어가 밀려나는 월드 공간의 지점.
* `크기`: 기본 힘 세기. 음수 값은 밀어내는 대신 끌어당깁니다.
* `반경`: 효과의 최대 거리(기본값: `Infinity`).
* `falloff`: 거리와 함께 크기가 얼마나 감소하는지(기본값: `CONSTANT`).

```ts
import { engine, Physics, Transform, timers } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const repulsionSource = engine.addEntity()
Transform.create(repulsionSource, { position: Vector3.create(8, 1, 8) })

// 엔티티의 위치를 반발 원점으로 명시적으로 전달
Physics.applyRepulsionForceToPlayer(
	repulsionSource,
	Transform.get(repulsionSource).position,  // 반발 원점 — 자동으로 읽지 않고 명시적으로 전달
	50,                                         // 크기
	10,                                         // 효과 반경(미터)
)

// 0.5초 후 힘 제거
timers.setTimeout(() => {
    Physics.removeForceFromPlayer(repulsionSource)
}, 500)
```

## 글라이딩 중의 힘

플레이어가 글라이딩 중일 때는 연속 힘의 동작이 다릅니다:

* 연속 힘 — 다음에서 온 `applyForceToPlayer()`, `applyForceToPlayerForDuration()` 또는 `applyRepulsionForceToPlayer()` — 는 **1.5배 더 강해집니다**. 열린 글라이더가 공기 흐름을 더 잘 받기 때문입니다. 바람 영역과 물살은 글라이딩 중인 플레이어에게 더 반응적으로 느껴집니다.
* 연속 힘의 위쪽 성분은 글라이딩 중인 플레이어를 **들어 올릴 수 있습니다** . 글라이더의 하강 속도 제한은 플레이어가 내려가는 속도만 제한할 뿐, 위쪽 움직임을 취소하지 않으므로, 비스듬하거나 수직인 바람 흐름은 힘의 전체 방향으로 플레이어를 밀어줍니다.

단발 임펄스 — 다음에서 온 `applyImpulseToPlayer()`  또는 `applyKnockbackToPlayer()` — 는 글라이딩의 영향을 받지 않습니다. 글라이더가 열려 있든 닫혀 있든 동일하게 작동합니다.

이러한 동작을 조합해 글라이딩 플레이어를 위로 실어 나르는 열 상승기류나, 글라이더를 연 상태에서 더 쉽게 통과할 수 있는 바람 통로 같은 메커니즘을 만들 수 있습니다. 글라이더의 하강 속도나 전진 속도를 조정하려면 다음을 사용하세요 `AvatarLocomotionSettings` 컴포넌트, 다음을 참조하십시오 [이동 설정](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#locomotion-settings).

## 로컬 방향을 월드 공간으로 변환

다음을 사용하세요 `Transform.localToWorldDirection()` 엔티티의 로컬 좌표 공간에서 방향 벡터를 월드 공간으로 변환하며, 전체 부모 계층을 고려합니다. 이는 회전된 엔티티를 기준으로 힘을 적용할 때 유용합니다. 예를 들어, 회전하는 장애물의 특정 면에서 플레이어를 밀어내는 경우입니다.

* `엔티티`: 방향을 정의하는 로컬 공간의 소스 엔티티.
* `localDirection`: 엔티티의 로컬 좌표계에 있는 방향 벡터.

월드 좌표의 방향 벡터를 반환합니다.

이것은 **회전만** 적용하며 — 이동이나 크기는 고려하지 않기 때문에 — 힘 및 임펄스 함수에 전달되는 방향 벡터에 직접 사용하기에 적합합니다.

```ts
import { Physics, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 엔티티의 로컬 +Z축이 가리키는 방향을 월드 공간에서 플레이어에게 밀기
const worldDir = Transform.localToWorldDirection(myEntity, Vector3.create(0, 0, 1))
Physics.applyImpulseToPlayer(worldDir, 20)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/player-physics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
