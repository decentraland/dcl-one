> 레이캐스팅을 사용해 공간에 선을 추적하고 씬의 엔티티와의 충돌을 조회합니다.

# 레이캐스팅

레이캐스팅은 게임 개발의 기본적인 도구입니다. 레이캐스팅을 사용하면 공간에 가상의 선을 그려서, 그 선과 교차하는 엔티티가 있는지 질의할 수 있습니다. 이는 시야선, 총알 궤적, 경로 탐색 알고리즘 등 여러 가지 응용에 유용합니다.

플레이어가 포인터 버튼, 또는 기본 버튼이나 보조 버튼을 누르면, 플레이어의 위치에서 바라보는 방향으로 레이가 추적됩니다. 자세한 내용은 [버튼 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/) 를 참고하세요. 이 문서는 플레이어의 동작과 무관하게 임의의 위치와 방향에서 보이지 않는 레이를 추적하는 방법을 다루며, 이는 다른 다양한 상황에서 사용할 수 있습니다.

레이캐스트는 콜라이더가 있는 오브젝트에만 충돌한다는 점에 유의하세요. 따라서 3D 모델에 대해 레이 히트를 감지하려면 다음 중 하나가 필요합니다:

* 모델에는 [콜라이더 메쉬가](/docs/creator/content-creator-ko/3d/colliders/).
* 해당 `GLTFContainer는` 다음을 사용하도록 구성되어야 합니다: [충돌 마스크가 있는 보이는 지오메트리](/docs/creator/content-creator-ko/sdk7/3d/colliders/#colliders-on-3d-models).
* 추가하세요 [MeshCollider 컴포넌트](/docs/creator/content-creator-ko/sdk7/3d/colliders/).

또한 3D 모델에 사용자 지정 [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers) 충돌 마스크를 지정하는 것도 좋은 방법입니다. 이렇게 하면 레이가 콜라이더가 있는 모든 대상이 아니라 관련 엔티티에 대해서만 충돌을 계산하면 됩니다.

## 레이 생성

모든 레이는 시작점과 방향을 가집니다. 시작점은 엔티티의 위치를 기반으로 하며, 엔티티의 Transform 컴포넌트 값이 사용됩니다. 레이의 방향은 4가지 다른 방식으로 정의할 수 있습니다:

* **로컬**: 엔티티의 정면 방향을 기준으로 한 방향이며, 부모 엔티티의 변환도 영향을 받습니다. 이는 차량의 진행 방향을 따르면서 앞쪽 장애물을 감지하는 데 유용합니다.
* **global**: 엔티티의 회전을 무시하고, 마치 엔티티의 회전이 0인 것처럼 특정 방향을 향합니다. 예를 들어 항상 아래를 향하게 할 때 유용합니다.
* **글로벌 타깃**: 엔티티의 위치와 씬의 전역 대상 위치 사이에 선을 추적합니다. 엔티티의 회전을 무시합니다. 예를 들어 타워 디펜스 게임을 만들 때 유용하며, 각 타워의 포탑이 공간의 특정 좌표를 향하게 할 수 있습니다.
* **타깃 엔티티**: 엔티티의 위치와 두 번째 타깃 엔티티의 위치 사이에 선을 추적합니다. 두 엔티티의 회전은 모두 무시됩니다.

다음 코드는 로컬 방향을 사용하는 레이캐스트를 생성합니다:

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: { direction: Vector3.Forward() },
  },
  function (raycastResult) {
    // 콜백 함수
  }
)
```

다음 함수를 사용해 방향을 다양한 방식으로 제공하여 레이캐스트를 생성할 수 있습니다:

* `raycastSystem.registerLocalDirectionRaycast()`: 다음을 사용하는 레이캐스트를 생성합니다: **로컬** 방향.  `방향` 필드는 `Vector3` 를 기대하며, 이는 엔티티 및 그 회전에 상대적인 벡터를 설명합니다(예: `Vector3.Forward()` 는 결국 엔티티의 transform 전방 벡터를 사용하게 됩니다)
* `raycastSystem.registerGlobalDirectionRaycast()`: 다음을 사용하는 레이캐스트를 생성합니다: **global** 방향.  `방향` 필드는 `Vector3` 는 전역 방향을 설명합니다.
* `raycastSystem.registerGlobalTargetRaycast()`: 다음에 의해 정의된 방향을 사용하는 레이캐스트를 생성합니다: **글로벌 타깃** 위치.  `target` 필드는 `Vector3` 필드는 씬의 전역 위치를 설명합니다.
* `raycastSystem.registerTargetEntityRaycast()`: 다음을 향하도록 정의된 방향을 사용하는 레이캐스트를 생성합니다: **타깃 엔티티** 위치.  `targetEntity` 필드는 엔티티에 대한 참조를 기대하며, 이 엔티티의 위치가 레이의 대상이 됩니다.

위의 어떤 방법으로든 레이를 생성할 때 다음과 같은 선택적 필드를 사용할 수 있습니다:

* `maxDistance`: *number* 이 레이가 추적될 길이를 설정합니다. 설정하지 않으면 기본값은 16미터입니다.
* `queryType`: *RaycastQueryType* 열거형 값으로, 레이가 모든 히트 엔티티를 반환할지 아니면 첫 번째 것만 반환할지 정의합니다. 다음 옵션을 사용할 수 있습니다:
  * `RaycastQueryType.RQT_HIT_FIRST`: *(기본값)* 시작점부터 첫 번째로 히트한 엔티티만 반환합니다.
  * `RaycastQueryType.RQT_QUERY_ALL`: 시작점에서 레이의 최대 거리까지 모든 히트 엔티티를 반환합니다.
* `originOffset`: 엔티티의 시작 위치에서 레이캐스트를 시작하는 대신, 상대 위치에서 질의를 시작하도록 오프셋을 추가합니다. 예를 들어 레이가 엔티티 자신의 콜라이더와 충돌하지 않도록 작은 오프셋을 사용할 수 있습니다. 설정하지 않으면 기본값은 `Vector3.Zero()`.
* `collisionMask`: 특정 충돌 레이어와의 충돌만 감지합니다. 사용자 지정 충돌 레이어와 함께 사용하거나, 물리 또는 포인터 이벤트 레이어만 감지하는 데 사용하세요. 다음을 참고하세요: [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers). 설정하지 않으면 기본적으로 사용되는 레이어는 `ColliderLayer.CL_PHYSICS`.
* `continuous`: true이면 게임 루프의 모든 프레임마다 레이캐스트 질의를 계속 실행합니다. false이면 현재 프레임에서만 레이가 사용됩니다. 설정하지 않으면 기본값은 false입니다.
* 로컬 또는 전역 방향으로 방향을 설정할 때, `방향` 필드는 기본값으로 `Vector3.Forward()`.
* 전역 타깃으로 방향을 설정할 때, `target` 필드는 기본값으로 `Vector3.Zero()`.
* 엔티티 타깃으로 방향을 설정할 때, `targetEntity` 필드는 씬의 루트 엔티티를 기본값으로 하며, 위치는 `Vector3.Zero()`.


> [!WARNING]
> **📔 참고**: `continuous` 속성은 주의해서 사용해야 합니다. 매 프레임마다 레이캐스트 질의를 실행하면 성능에 매우 큰 부담이 될 수 있기 때문입니다. 가능하면 시스템(또는 `interval` 함수는 Utils 라이브러리에 있음)을 사용하여, 예를 들어 1초에 한 번 또는 0.2초마다처럼 더 드문 간격으로 정기적인 레이캐스트 질의를 실행하세요. 자세한 내용은 [반복 레이캐스팅](#recurrent-raycasting).


아래에는 레이 방향을 결정하는 네 가지 방법 각각을 사용하는 예제가 있습니다:

```ts
// 로컬 방향 레이캐스트
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// 전역 방향 레이캐스트
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// 전역 타깃 위치 레이캐스트
raycastSystem.registerGlobalTargetRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      target: Vector3.Zero(),
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
// 타깃 엔티티 레이캐스트
const targetEntity = engine.addEntity()
Transform.create(targetEntity, { position: Vector3.create(8, 1, 10) })

raycastSystem.registerTargetEntityRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      targetEntity: targetEntity,
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 참고**: `raycastSystem`, `RaycastQueryType` 및 `ColliderLayer` 는 다음을 통해 가져와야 합니다.
> 
> > `import { raycastSystem, RaycastQueryType, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


## 레이캐스트 결과

레이캐스트를 처리하는 콜백 함수는 레이 자체에 대한 데이터와 충돌한 모든 엔티티를 포함하는 객체를 받습니다.

* `globalOrigin`: 씬을 기준으로 한, 레이가 시작된 위치입니다.
* `방향`: 레이가 향하고 있던 전역 방향입니다.  `Vector3`.
* `hits`: 충돌한 각 엔티티마다 하나의 객체를 담은 배열입니다. 히트한 엔티티가 없으면 이 배열은 비어 있습니다. 레이캐스트가 `RaycastQueryType.RQT_HIT_FIRST`를 사용했다면 이 배열에는 하나의 객체만 들어 있습니다.

배열의 각 객체에는 `hits` 다음이 포함됩니다:

* `entityId`: 레이에 의해 히트된 엔티티의 ID 번호입니다.
* `meshName`: *문자열* 충돌한 3D 모델 내 특정 메쉬의 내부 이름입니다. 3D 모델이 여러 메쉬로 구성된 경우 유용합니다.
* `위치`: *Vector3* 레이가 히트한 엔티티와 교차한 위치를 나타냅니다(씬 기준)
* `length`: 엔티티와의 충돌이 발생한 위치까지 레이의 시작점부터의 길이입니다.
* `normalHit`: *Vector3* 세계 공간에서의 히트 표면의 법선입니다.
* `globalOrigin`: *Vector3* 레이가 시작되는 위치를 나타냅니다(씬 기준)
* `방향`: 레이가 향하고 있던 전역 방향입니다.  `Vector3`.

다음 예제는 히트된 엔티티들을 반복합니다:

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    if (raycastResult.hits.length > 0) {
      for (const hit of raycastResult.hits) {
        if (hit.entityId) {
          console.log('히트한 엔티티 ', hit.entityId)
        }
      }
    } else {
      console.log('히트한 엔티티가 없음')
    }
  }
)
```


> [!WARNING]
> **📔 참고**: 다른 씬의 엔티티를 히트하여 레이캐스트 결과를 얻을 수 있습니다.


## 히트된 엔티티 처리

엔티티를 히트한 레이캐스트 결과를 얻으면 `entityId` 를 사용하여 엔티티와 그 컴포넌트와 상호작용할 수 있습니다. 엔티티는 [그저 숫자에 불과하므로](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#overview), `entityId` 값 자체를 `Entity` 타입으로 해석할 수 있습니다.

```ts
const hitEntity = hit.entityId as Entity
const transform = Transform.get(hitEntity)
console.log(transform.position)
```

## 충돌 레이어

씬의 성능을 높이려면 관련 있는 엔티티에 대해서만 충돌을 확인하는 것이 좋습니다. `collisionMask` 필드를 사용하면 특정 충돌 레이어만 나열할 수 있습니다. 물리 레이어(씬의 벽과 바닥), 포인터 레이어(포인터 이벤트), 플레이어 레이어(아바타), 또는 자유롭게 지정할 수 있는 8개의 사용자 지정 레이어를 사용할 수 있습니다. 다음을 참고하세요: [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers).

기본적으로 `collisionMask` 필드는 `ColliderLayer.CL_PHYSICS`로 설정되어 있습니다. 이 값을 변경하여 다른 레이어를 나열하거나, `|` 구분자를 사용하여 여러 레이어를 결합할 수 있습니다.

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      collisionMask:
        ColliderLayer.CL_CUSTOM1 |
        ColliderLayer.CL_CUSTOM3 |
        ColliderLayer.CL_POINTER,
    },
  },
  (raycastResult) => {
    log(raycastResult.hits)
  }
)
```

## 반복 레이캐스팅

의 함수를 사용할 때, `raycastSystem`기본 동작은 충돌을 한 번만 질의하는 단일 레이를 생성하는 것입니다. 대안으로, `continuous` 필드를 *true* 로 설정하여 게임 루프의 매 틱마다 질의와 콜백 함수를 실행할 수 있습니다.

다음 예제는 이 시점부터 계속해서 레이캐스트 질의를 실행합니다

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      continuous: true,
    },
  },
  function (raycastResult) {
    log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 참고**: `continuous` 속성은 주의해서 사용해야 합니다. 매 프레임마다 레이캐스트 질의를 실행하면 성능에 매우 큰 부담이 될 수 있기 때문입니다.


더 이상 필요하지 않다면 반복 레이캐스트를 제거하세요. 그러려면 `raycastSystem.removeRaycasterEntity`.

```ts
raycastSystem.removeRaycasterEntity(myEntity)
```

가능하면 시스템(또는 `interval` 함수는 Utils 라이브러리에 있음)을 사용하여 1초에 한 번 또는 0.2초마다처럼 규칙적이고 더 드문 간격으로 레이캐스트 질의를 실행하세요.

```typescript
// 사용자 지정 컴포넌트
const CubeOscilator = engine.defineComponent('CubeOscilator', {
  t: Schemas.Float,
})

const TimerComponent = engine.defineComponent('TimerComponent', {
  t: Schemas.Float,
})

const RAY_INTERVAL = 0.1

// 레이 확인
engine.addSystem((dt) => {
  for (const [entity] of engine.getEntitiesWith(TimerComponent)) {
    const timer = TimerComponent.getMutable(entity)
    timer.t += dt

    if (timer.t > RAY_INTERVAL) {
      timer.t = 0
      raycastSystem.registerGlobalDirectionRaycast(
        {
          entity: myEntity,
          opts: {
            queryType: RaycastQueryType.RQT_HIT_FIRST,
            direction: Vector3.Forward(),
            maxDistance: 16,
          },
        },
        function (raycastResult) {
          log(raycastResult.hits)
        }
      )
    }
  }
})

TimerComponent.create(engine.addEntity())

// 진동하는 큐브 시스템
engine.addSystem((dt) => {
  for (const [entity, cube] of engine.getEntitiesWith(
    CubeOscilator,
    Transform
  )) {
    CubeOscilator.getMutable(entity).t += dt
    Transform.getMutable(entity).position.y = 2 + Math.cos(cube.t)
  }
})

// 큐브 생성
const cubeEntity = engine.addEntity()
Transform.create(cubeEntity, { position: { x: 8, y: 1, z: 8 } })
CubeOscilator.create(cubeEntity)
MeshRenderer.setBox(cubeEntity)
MeshCollider.setBox(cubeEntity)
```

위 예제는 0.1초마다 반복 레이캐스트를 실행합니다. 이 예제는 타이머 컴포넌트와 시스템의 `dt` 속성을 사용해 이를 균등하게 시간 조정합니다. 또한 다른 시스템이 제어하는 위아래로 진동하는 큐브를 포함하여, 레이의 경로 안팎으로 움직이게 합니다.


> [!NOTE]
> **💡 팁**: 다음을 사용하세요 `interval` 함수는 [SDK Utils 라이브러리의](https://github.com/decentraland/sdk7-utils) 에서 일정한 간격으로 함수를 실행하는 더 간단한 방법을 제공합니다.


## 시스템을 통한 레이캐스트

반복 레이캐스트를 수행하는 또 다른 방법은 시스템의 반복 함수 내부에서 실행하는 것입니다. 이를 통해 언제, 어떻게 동작할지 훨씬 더 세밀하게 제어할 수 있습니다. 콜백 함수를 등록하는 대신 `raycastSystem.registerRaycast` 를 사용하여 레이캐스트 질의를 수행한 다음, 시스템 함수 내부에서 이 작업이 반환한 데이터를 확인하면 됩니다.

레이캐스트가 시스템에서 실행되므로 결과는 다음 틱에만 사용할 수 있으며, 시스템이 두 번 실행되어야 한다는 점에 유의하세요. 하나는 다음 프레임을 위해 레이캐스트를 등록하는 것이고, 다음 프레임은 그 결과를 처리하는 것입니다.

```ts
engine.addSystem((deltaTime) => {
		const result = raycastSystem.registerRaycast(
			myEntity,
			raycastSystem.localDirectionOptions({
				collisionMask: ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM3 | ColliderLayer.CL_POINTER,
				originOffset: Vector3.create(0, 0.4, 0),
				maxDistance: 16,
				queryType: RaycastQueryType.RQT_HIT_FIRST,
				direction: Vector3.Forward(),
				continuous: true // raycasting은 성능에 비용이 크므로 'continuous' 속성을 과도하게 사용하지 마세요
			})
		)
		if (result) {
			// 무언가 수행
		}
	})
```

## 플레이어와 충돌

마스크에 아바타 충돌 레이어 중 하나를 포함하면 레이캐스트로 아바타를 직접 감지할 수 있습니다:

* `ColliderLayer.CL_PLAYER`: 로컬 플레이어와 씬에 렌더링된 다른 모든 플레이어를 포함한 모든 아바타와 일치합니다.
* `ColliderLayer.CL_MAIN_PLAYER`: 로컬(메인) 플레이어만 일치합니다.

두 레이어는 함께 사용하거나 독립적으로 사용할 수 있습니다. 레이캐스트의 기본 `collisionMask` 은 `CL_PHYSICS`이며, 아바타에는 충돌하지 않습니다. 명시적으로 선택해야 합니다.

```ts
// 로컬 플레이어만 히트(다른 아바타는 무시)
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_MAIN_PLAYER,
    },
  },
  (raycastResult) => {
    if (raycastResult.hits.length > 0) {
      console.log('로컬 플레이어를 히트함')
    }
  }
)

// 모든 아바타를 히트(로컬 + 원격)
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_PLAYER,
    },
  },
  (raycastResult) => {
    // raycastResult.hits[i].entityId는 원격 아바타 히트의 경우 0입니다(씬 로컬 엔티티 ID 없음)
    console.log(raycastResult.hits)
  }
)
```

레이캐스트가 로컬 플레이어를 히트하면 `hit.entityId` 가 `engine.PlayerEntity`는, 원격 아바타에 대한 히트에는 씬 로컬 엔티티 ID가 포함되지 않습니다(필드는 `0`). 원격 플레이어는 여러분의 씬 세계에 있는 엔티티가 아니기 때문입니다. 하지만 히트는 여전히 `위치`, `length`, `normalHit`와 다른 기하학적 데이터와 함께 보고됩니다.


> [!NOTE]
> **💡 팁**: 감지하려면 **다른** 플레이어만(로컬 플레이어 제외) 감지하려면 `CL_PLAYER` 를 사용하고 `hit.entityId !== engine.PlayerEntity` 로 콜백에서 필터링하세요.


## 플레이어로부터의 레이캐스트

플레이어의 위치에서 카메라가 바라보는 방향으로 레이를 추적하려면, 카메라 또는 아바타를 사용하여 레이를 추적할 수 있습니다. [예약된 엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#reserved-entities).


> [!NOTE]
> **💡 팁**: 대부분의 경우에는 [포인터 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/) 대신 레이캐스트를 사용하는 것이 더 나을 수 있습니다.


다음 예제는 `engine.CameraEntity` 엔티티를 사용하여 플레이어의 카메라 위치에서 앞으로 레이를 추적합니다.

```ts
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: engine.CameraEntity,
    opts: {
      queryType: RaycastQueryType.RQT_HIT_FIRST,
      direction: Vector3.rotate(
        Vector3.Forward(),
        Transform.get(engine.CameraEntity).rotation
      ),
    },
  },
  function (raycastResult) {
    console.log(raycastResult)
  }
)
```


> [!WARNING]
> **📔 참고**: 3인칭에서는 커서가 앞으로 1인칭과 같은 방식으로 동작하지 않을 수도 있다는 점을 염두에 두세요. 플레이어가 1인칭일 때만 사용하는 것이 좋습니다.


## 커서 위치에서 레이캐스트

플레이어의 커서 위치에서 3D 월드로 레이를 추적할 수도 있습니다. 오브젝트를 끌어다 놓는 기능, 슈팅 게임 등에 사용할 수 있습니다.

이 예제에서는 플레이어가 E 키를 누르면 이를 감지한 다음 커서 위치에서 3D 월드로 레이를 추적합니다. 그런 다음 레이가 어떤 엔티티를 히트했는지 확인하고, 히트했다면 그 엔티티에 대해 어떤 작업을 수행합니다.

```ts
import { engine, Entity, InputAction, inputSystem, PointerEventType, RaycastQueryType, raycastSystem, TextShape, Transform } from '@dcl/sdk/ecs'
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

let cooldown = 1
let rayFrequency = 0.1
let mousePressed = false

export function main() {
   engine.addSystem(rayCastSystem)
}

const rayCastSystem = (t: number) => {

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN)) {
      mousePressed = true
    }

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_UP)) {
      mousePressed = false
    }

    if (!mousePressed) {
      cooldown = 0
      raycastSystem.removeRaycasterEntity(engine.CameraEntity)
      return
    }

    cooldown += t
    if (cooldown < rayFrequency) return
    cooldown = 0

    const pointerInfo = PrimaryPointerInfo.getOrCreateMutable(engine.RootEntity)
    let dir = pointerInfo.worldRayDirection

    raycastSystem.registerGlobalDirectionRaycast(
      {
        entity: engine.CameraEntity,
        opts: {
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          direction: dir,
        },
      },
      function (raycastResult) {
        let result = raycastResult.hits[0]

        // 히트 위치에서 무언가를 수행
        if (result && result.position) {
          console.log("x:", result.position.x, ", y:", result.position.y, ", z:", result.position.z)
        }

        // 히트된 엔티티로 무언가를 수행
        const entity = result.entityId as Entity
        if (entity) {
          console.log("entity: ", entity)
        }
      }
    )
}

```


> [!NOTE]
> **💡 팁**: 이 예제에서는 기본 버튼(E)을 사용하여 레이캐스트를 트리거합니다. 포인터 버튼(왼쪽 클릭)은 사용하지 않는데, 클릭 후 드래그를 하면 기본적으로 카메라 각도도 함께 바뀌기 때문입니다. 드래그하는 동안 카메라 회전을 막고 싶다면 [가상 카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/) 를 사용하여 카메라 각도를 고정할 수 있습니다.


## 고급 구문

### 레이캐스트 컴포넌트 생성

Raycast 컴포넌트는 교차하는 엔티티를 질의하는 데 사용되는 보이지 않는 레이를 설명합니다. 레이는 Transform 컴포넌트에 의해 정의된 엔티티의 위치에서 시작해 추적되며, 부모 엔티티의 영향도 받습니다. 방향은 다양한 방식으로 정의할 수 있으며,

레이는 다음 데이터로 정의됩니다:

* `방향`: 를 포함하는 객체입니다. `$case` 필드는 방향의 유형을 선택하는 데 사용되며, 이 유형에 따라 이 방향을 결정하는 추가 필드가 필요합니다. 다음은 `$case`:
  * `localDirection`: 엔티티의 정면 방향을 기준으로 한 방향이며, 부모 엔티티의 변환도 영향을 받습니다. 이는 차량의 진행 방향을 따르면서 앞쪽 장애물을 감지하는 데 유용합니다. 회전은 `localDirection` 필드에 의해 `Vector3` 회전을 설명하는
  * `globalDirection`: 엔티티의 회전을 무시하고, 마치 엔티티의 회전이 0인 것처럼 방향을 향합니다. 예를 들어 항상 아래를 향하게 할 때 유용합니다. 회전은 `globalDirection` 필드에 의해 `Vector3` 회전을 설명하는
  * `globalTarget`: 엔티티의 위치와 씬의 전역 대상 위치 사이에 선을 추적합니다. 엔티티의 회전을 무시합니다. 타워 디펜스 게임을 만들 때 유용하며, 각 타워의 포탑이 공간의 특정 좌표를 향하게 할 수 있습니다. 대상은 `globalTarget` 필드에 의해 `Vector3` 로 정의되며, 전역 위치를 설명합니다.
  * `targetEntity`: 엔티티의 위치와 두 번째 타깃 엔티티의 위치 사이에 선을 추적합니다. 두 엔티티의 회전은 모두 무시됩니다. 대상은 `targetEntity` 필드에 의해 정의되며, 엔티티에 대한 참조를 담고 있습니다.
* `maxDistance`: *number* 이 레이가 추적될 길이를 설정합니다.
* `queryType`: *RaycastQueryType* 열거형 값으로, 레이가 모든 히트 엔티티를 반환할지 아니면 첫 번째 것만 반환할지 정의합니다. 다음 옵션을 사용할 수 있습니다:
  * `RaycastQueryType.RQT_HIT_FIRST`: 시작점부터 첫 번째로 히트한 엔티티만 반환합니다.
  * `RaycastQueryType.RQT_QUERY_ALL`: 시작점에서 레이의 최대 거리까지 모든 히트 엔티티를 반환합니다.
* `collisionMask`: 특정 충돌 레이어와의 충돌만 감지합니다. 사용자 지정 충돌 레이어와 함께 사용하거나, 물리 또는 포인터 이벤트 레이어만 감지하는 데 사용하세요. 다음을 참고하세요: [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers). 기본값은 `ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS`.
* `originOffset`: 엔티티의 시작 위치에서 레이캐스트를 시작하는 대신, 상대 위치에서 질의를 시작하도록 오프셋을 추가합니다. 예를 들어 레이가 엔티티 자신의 3D 모델과 충돌하지 않도록 작은 오프셋을 사용할 수 있습니다.
* `continuous`: true이면 매 프레임마다 레이캐스트 질의를 계속 실행합니다. false이면 레이는 현재 프레임에서만 사용됩니다. 기본값은 false입니다.


> [!WARNING]
> **📔 참고**: `continuous` 속성은 주의해서 사용해야 합니다. 매 프레임마다 레이캐스트 질의를 실행하면 성능에 매우 큰 부담이 될 수 있기 때문입니다. 가능하면 시스템(또는 `interval` 함수는 Utils 라이브러리에 있음)을 사용하여, 예를 들어 1초에 한 번 또는 0.2초마다처럼 더 드문 간격으로 정기적인 레이캐스트 질의를 실행하세요. 자세한 내용은 [반복 레이캐스팅](#recurrent-raycasting).


다음 예제는 전역 회전을 사용하여 방향을 결정하며, 레이가 발사된 프레임에 히트한 첫 번째 엔티티만 반환합니다.

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST
})
```

아래 예제는 엔티티의 정면 방향으로 레이를 발사하여, 처음 맞은 항목만 반환합니다. 이는 연속적으로 수행됩니다. 또한 레이가 엔티티 자신의 콜라이더에 맞는 것을 방지하기 위해 0.5의 작은 오프셋을 포함합니다.

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "localDirection",
    localDirection: Vector3.Forward()
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST,
  originOffset: Vector3.create(0.5, 0, 0),
  continuous: true
})
```

이 예제는 두 엔티티 사이에 레이를 추적합니다. 그 사이에서 맞은 모든 엔티티를 반환합니다.

```ts
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

const entity2 = engine.addEntity()

Transform.create(entity2, {
  position: Vector3.create(0, 1, 8)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "targetEntity",
    targetEntity: entity2
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})
```

### 레이캐스트 결과 컴포넌트


> [!WARNING]
> **📔 참고**: 레이캐스트 결과를 다루는 가장 쉬운 방법은 `raycastSystem`, 그리고 레이를 생성하는 동일한 문장에서 콜백 함수를 등록합니다. 그`RaycastResult` 이 컴포넌트는 해당 인터페이스에서 내부적으로 사용되지만, 더 고급의 사용자 지정 로직을 위해 외부에도 노출됩니다.


Raycast 컴포넌트를 만든 후, 이 컴포넌트가 추가된 엔티티는 `RaycastResult` 컴포넌트를 갖게 됩니다. 이 컴포넌트에는 레이가 충돌한 모든 히트 정보가 포함됩니다. 이 데이터를 확인할 시스템을 설정하세요.

해당 `RaycastResult` 컴포넌트에는 다음 데이터가 포함됩니다:

* `globalOrigin`: 씬을 기준으로 한, 레이가 시작된 위치입니다.
* `방향`: 레이가 향하고 있던 전역 방향입니다.  `Vector3`.
* `hits`: 충돌한 각 엔티티마다 하나의 객체를 담은 배열입니다. 히트한 엔티티가 없으면 이 배열은 비어 있습니다. 레이캐스트가 `RaycastQueryType.RQT_HIT_FIRST`를 사용했다면 이 배열에는 하나의 객체만 들어 있습니다.

배열의 각 객체에는 `hits` 다음이 포함됩니다:

* `entityId`: 레이에 의해 히트된 엔티티의 ID 번호입니다.
* `meshName`: *문자열* 충돌한 3D 모델 내 특정 메쉬의 내부 이름입니다. 3D 모델이 여러 메쉬로 구성된 경우 유용합니다.
* `위치`: *Vector3* 레이가 히트한 엔티티와 교차한 위치를 나타냅니다(씬 기준)
* `length`: 엔티티와의 충돌이 발생한 위치까지 레이의 시작점부터의 길이입니다.
* `normalHit`: *Vector3* 세계 공간에서의 히트 표면의 법선입니다.
* `globalOrigin`: *Vector3* 레이가 시작되는 위치를 나타냅니다(씬 기준)
* `방향`: 레이가 향하고 있던 전역 방향입니다.  `Vector3`.

아래 예제는 시스템을 사용하여 개별 엔티티의 결과에 접근하는 방법을 보여줍니다:

```typescript

const rayEntity = engine.addEntity()

Transform.create(rayEntity, {
  position: Vector3.create(8, 1, 0)
})

// 모든 엔티티 반환
Raycast.createOrReplace(rayEntity, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})

engine.addSystem(() => {
  const rayResult = RaycastResult.get(rayEntity)
  console.log(rayResult.hits)
})
```

다음 예제는 어떻게 접근할 수 있는지 보여줍니다 `RaycastResult` 장면의 모든 엔티티에 있는 컴포넌트를, 다음을 사용하여 [컴포넌트 쿼리](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/).

```typescript
engine.addSystem(() => {
  for (const [_, result] of engine.getEntitiesWith(RaycastResult)) {
    console.log(result.hits)
  }
})
```


> [!WARNING]
> **📔 참고**: 레이캐스트 결과는 레이캐스트를 생성한 게임 루프의 동일한 틱에 도착하지 않습니다. 결과가 도착하는 데 한 틱 또는 여러 틱이 걸릴 수 있습니다.


경로 찾기, 시야 확인, 투사체 추적 등과 같이 서로 다른 목적을 위해 여러 종류의 레이를 사용하는 장면에서는, 서로 다른 [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers), 불필요한 충돌 계산을 피하기 위해 사용할 수 있습니다.


> [!NOTE]
> **💡 팁**: 동작하는 레이캐스팅 예제를 보려면 다음을 참조하세요. [`77,-1-raycast-unit-tests`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests) 테스트 씬으로, 다음을 통해 레이캐스트 동작을 검증합니다 `@dcl/sdk/testing` 전반에 걸친 단위 테스트 `localDirection`, `globalDirection`, `globalTarget` 및 `targetEntity` 방향 형식과, 변환된 부모에서 발사된 레이를 포함합니다. 레이가 충돌할 수 있는 콜라이더 레이어의 동작을 보려면 다음을 참조하세요. [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/raycasting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
