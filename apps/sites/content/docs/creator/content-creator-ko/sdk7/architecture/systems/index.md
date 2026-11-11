> 시스템이 씬 상태를 업데이트하는 데 어떻게 사용되는지 알아봅니다

# 시스템

Decentraland 씬은 *시스템* 시간이 지남에 따라 모든 데이터를 업데이트하는 데 사용되며, 각 엔티티의 [구성 요소](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/).

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

*시스템* 씬을 동적으로 만드는 요소이며, 씬의 게임 루프가 한 틱 돌 때마다 주기적으로 실행되어 렌더링될 내용을 바꾸는 함수입니다.

다음 예제는 기본 시스템 선언을 보여줍니다:

```ts
// 시스템 정의
function mySystem() {
  console.log("모든 틱마다 실행됨. 내 시스템이 실행 중입니다")
}
// 엔진에 시스템 추가
engine.addSystem(mySystem)
```

시스템의 함수는 원하는 어떤 작업이든 수행할 수 있습니다. 일반적으로 특정 조건을 만족하는 모든 엔티티에 대해 동작합니다 [쿼리](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/)를 기준으로, 특정 로직에 따라 엔티티의 컴포넌트에 저장된 값을 변경합니다.

```ts
function moveSystem(dt: number) {
  // Transform이 있는 모든 엔티티를 반복
  for (const [entity] of engine.getEntitiesWith(Transform)) {

  // 변경 가능한 Transform 컴포넌트를 가져옴
  const transform = Transform.getMutable(entity)

  // 위치 값을 업데이트
    transform.position.z += 0.01
  }
}

engine.addSystem(moveSystem)
```

위의 예제에서 시스템 `MoveSystem` 은 게임 루프의 각 틱마다 실행되어 Transform을 가진 씬의 모든 엔티티의 위치를 변경하는 함수입니다.

![](/docs/_remote/7e85b65f2626d96c8f892499.webp)

씬에 여러 시스템을 두어 서로 다른 동작을 분리하면 코드가 더 깔끔해지고 확장 및 재사용도 쉬워집니다. 예를 들어, 한 시스템은 물리를 처리하고, 다른 시스템은 장애물 엔티티가 계속 앞뒤로 움직이게 하며, 또 다른 시스템은 캐릭터의 AI를 처리할 수 있습니다.

여러 시스템이 하나의 엔티티에 작동할 수도 있습니다. 예를 들어, 플레이어가 아닌 캐릭터는 AI에 따라 스스로 움직일 수 있지만, 절벽 밖으로 실수로 걸어 나갔을 때는 중력의 영향을 받을 수도 있습니다. 그런 시나리오에서 물리 시스템과 AI 시스템은 서로를 알 필요조차 없습니다. 각 시스템은 게임 루프의 매 틱마다 현재 상태를 독립적으로 재평가하고, 각자 별도의 로직을 구현합니다.

## 시스템 함수

시스템의 함수는 게임 루프의 각 틱마다 한 번씩 주기적으로 실행됩니다. 이는 자동으로 발생하므로 코드의 어느 곳에서도 이 함수를 명시적으로 호출할 필요가 없습니다.

Decentraland 씬에서는 게임 루프를 씬에 있는 모든 시스템 함수의 집합으로 생각할 수 있습니다.


> [!WARNING]
> **📔 참고**: 동일한 시스템 함수를 엔진에 두 번 이상 추가할 수 없습니다. 이미 추가된 시스템을 다시 추가하려고 하면 엔진이 오류를 발생시킵니다: `시스템 "<name>"이 이미 엔진에 추가되어 있습니다`.


## 참조로 엔티티 다루기

일부 컴포넌트와 시스템은 씬에서 하나의 엔티티에만 사용되도록 만들어졌습니다. 예를 들어, 게임 점수를 저장하는 엔티티나 씬에서 유일한 메인 게이트 같은 경우입니다. 시스템 안에서 그런 엔티티 중 하나에 접근하려면, 시스템 함수에서 엔티티나 그 컴포넌트를 이름으로 간단히 참조하면 됩니다.

```ts
// 시스템이 접근할 수 있도록 모듈 범위에 엔티티 참조를 선언
let game: Entity

export function main(){
	// 새 엔티티 생성
	game = engine.addEntity()

	// 해당 엔티티에 컴포넌트 추가
	ScoreComponent.create(game)
}

// 시스템 정의
export function UpdateScore() {

  // 개별 엔티티에 대한 참조 호출
  const points = ScoreComponent.get(game).points
  console.log(points)
}

// 엔진에 시스템 추가
engine.addSystem(UpdateScore)
```

더 큰 프로젝트에서는 시스템 정의를 엔티티와 컴포넌트를 인스턴스화하는 파일과 분리해 두는 것을 권장합니다.

## 컴포넌트 쿼리 반복하기

많은 경우 씬에는 비슷한 동작을 하는 같은 유형의 엔티티가 여러 개 있습니다. 예를 들어 열 수 있는 문이 많거나, 플레이어를 공격할 수 있는 적이 많을 수 있습니다. 이런 유사한 엔티티들을 하나의 시스템에서 처리하면서 목록을 반복하고 각 엔티티에 같은 검사를 수행하는 것이 합리적입니다.

시스템 함수가 *전체* 씬의 엔티티 집합을 반복하도록 하는 것은 바람직하지 않습니다. 처리 성능 측면에서 매우 큰 비용이 들 수 있기 때문입니다. 이를 피하려면 [컴포넌트를 조회](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/)를 사용해 관련 엔티티만 반복하면 됩니다.

예를 들어, 씬에는 `PhysicsSystem` 씬의 엔티티들에 대한 중력 효과를 계산하는 시스템이 있을 수 있습니다. 나무처럼 절대 움직이지 않아야 하는 엔티티도 있으므로, 이런 대상에 중력 효과를 계산하지 않는 것이 현명합니다. 다음과 같은 `HasPhysics` 컴포넌트를 정의해 중력의 영향을 받을 수 있는 엔티티를 표시한 다음, `PhysicsSystem` 이 쿼리가 반환한 엔티티만 다루면 됩니다.

```ts
// 시스템 정의
export function PhysicsSystem() {
  // HasPhysics가 있는 모든 엔티티를 반복
  for (const [entity] of engine.getEntitiesWith(HasPhysics)) {

  // 변경 가능한 Transform 컴포넌트를 가져옴
  const transform = Transform.getMutable(entity)

  // 물리 효과 계산
  }
}

// 엔진에 시스템 추가
engine.addSystem(PhysicsSystem)
```

## 프레임 간 델타 시간

시스템의 함수에는 선택적으로 다음과 같은 인수를 포함할 수 있습니다: `dt`, 타입은 `number` (을 나타내는 *델타 시간*).

```ts
function MySystem(dt: number) {

  // 씬 업데이트
  console.log("마지막 틱 이후 경과 시간: ", dt)
}

engine.addSystem(MySystem)
```

*델타 시간* 은 게임 루프의 마지막 틱 이후 경과한 시간을 초 단위로 나타냅니다.

해당 `dt` 인수는 게임 루프의 마지막 틱 이후 경과한 실제 시간이며, 초 단위로 측정됩니다. SDK는 고정된 틱 속도를 강제하지 않습니다. 호스트 런타임이 이 값을 전달하며, 이는 씬의 프레임레이트에 따라 달라집니다.

씬이 더 부드럽게 실행될수록 틱은 더 자주 발생하고 각 `dt` 값은 더 작아집니다. 틱 처리에 더 오래 걸리면 다음 틱 전까지 더 많은 시간이 지나며 `dt` 그에 따라 커집니다.

이 씬 측 프레임레이트는 초당 30틱으로 제한되며, 이 경우 `dt` 가 *1/30* (0.0333...).

![](/docs/_remote/d1bc4cb88306d00b1e56431f.webp)

프레임 처리에 더 오래 걸리면 해당 프레임의 그리기가 지연되고, `dt` 다음 틱에서 전달되는 값은 그 더 긴 간격을 반영합니다. 엔진은 지연을 소급해서 보정하지 않습니다; `dt` 항상 실제 경과 시간을 보고하므로, 시스템이 그에 맞게 조정할 수 있습니다.

![](/docs/_remote/5ee6f60ba5826b88ac1603b7.webp)

이상적으로는 씬에서 프레임이 떨어지는 것을 피해야 합니다. 이는 플레이어 경험의 품질에 영향을 주기 때문입니다. 이는 플레이어 기기의 처리 성능에 의존하므로, 씬이 이를 우아하게 처리할 준비가 항상 되어 있어야 할 가능성이 있습니다.

해당 `dt` 변수는 프레임 처리 시간이 기본 시간을 초과할 때 유용합니다. 현재 프레임이 이전 프레임만큼 시간이 걸릴 것이라고 가정하면, 이 정보는 점진적인 변화를 얼마나 조정해야 하는지 계산하는 데 사용될 수 있으며, 변화 속도가 일정하고 프레임 간 지연에 비례해 보이도록 할 수 있습니다.

참조 [엔티티 위치 지정](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/) 사용 방법 예시를 보려면 `dt` 를 사용해 움직임을 더 부드럽게 만드세요.

## 정해진 간격으로 반복

시스템이 일정한 시간 간격으로 어떤 작업을 실행하게 하려면, `dt` 인수와 타이머를 결합하면 됩니다.

```ts
let timer: number = 10

function LoopSystem(dt: number) {
  timer -= dt
  if (timer <= 0) {
      timer = 10
      // 무언가 수행
    }
}

engine.addSystem(LoopSystem)
```

바로 사용할 수 있는 단축 함수도 있습니다 `setInterval` 및 `clearInterval` 는 백그라운드에서 시스템을 사용합니다. 특정 함수를 X초마다 실행하려는 경우 더 쉽고 짧은 버전을 사용할 수 있습니다.

```ts
import { timers } from '@dcl/sdk/ecs'

const intervalId = timers.setInterval(() => {
    console.log('10초마다 이 내용을 출력합니다')
}, 10000)
```

여기서 첫 번째 인수인 `콜백`는 실행될 함수입니다(이 경우 `console.log()`), 그리고 두 번째 인수인 `ms`(이 경우 10000)은 각 함수 실행 사이에 기다릴 밀리초입니다.

중지하려면 `setInterval` 함수를 사용하세요, `clearInterval` 를 사용합니다.

```
timers.clearInterval(intervalId)
```

여기서 `intervalId` 은 다음에 대한 참조입니다 `setInterval` 앞에서 정의한 반환값입니다.

여러 지연과 루프가 동적으로 생성될 수 있는 더 복잡한 사용 사례에서는 각 엔티티별로 개별 타이머 값을 저장하는 사용자 정의 컴포넌트를 정의하는 것이 좋을 수 있습니다. 다음을 참조하세요: [커스텀 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/).

## 시스템 실행 순서

경우에 따라 여러 시스템이 실행 중일 때 씬에서 어떤 시스템이 먼저 실행되는지가 중요할 수 있습니다.

예를 들어, *물리* 씬의 엔티티 위치를 업데이트하는 시스템이 있을 수 있고, 다른 *경계* 시스템은 어떤 엔티티도 씬 경계 밖에 위치하지 않도록 보장합니다. 이 경우에는 *경계* 시스템이 마지막에 실행되도록 해야 합니다. 그렇지 않으면, *물리* 시스템이 엔티티를 씬의 범위 밖으로 이동시킬 수 있지만, *경계* 시스템은 다음 프레임에서 다시 실행될 때까지 이를 알지 못합니다.

엔진에 시스템을 추가할 때는 선택적 `우선순위` 필드를 설정해 다른 시스템과의 관계에서 언제 실행될지 결정합니다.

```ts
engine.addSystem(PhysicsSystem, 5)
engine.addSystem(BoundariesSystem, 1)
```

우선순위 숫자가 더 높은 시스템이 먼저 실행되므로, *5* 우선순위가 다음인 시스템보다 먼저 실행됩니다 *1*.

명시적 우선순위가 주어지지 않은 시스템의 기본 우선순위는 *100000*. 숫자가 클수록 먼저 실행되므로, 이러한 시스템은 더 낮은 명시적 우선순위를 가진 시스템보다 먼저 실행됩니다.

두 시스템의 우선순위 숫자가 같다면, 어느 쪽이 먼저 실행될지 확실히 알 수 없습니다.

## 시스템 제거

시스템 인스턴스는 엔진에 추가하거나 제거하여 켜거나 끌 수 있습니다.

시스템이 정의되어 있어도 엔진에 추가되지 않으면 해당 함수는 엔진에 의해 호출되지 않습니다.

시스템을 제거하려면, 나중에 참조할 수 있도록 엔진에 추가할 때 먼저 이름을 지정해야 합니다.

```ts
// 시스템 선언
function mySystem(dt: number){
  console.log("마지막 틱 이후 지연 시간: ", dt)
}

// 시스템 추가(우선순위와 이름 지정)
engine.addSystem(mySystem, 1, "DelaySystem")

// 시스템 제거
engine.removeSystem("DelaySystem")
```

다음에 전달하는 문자열은 `engine.removeSystem()` 시스템을 추가할 때 지정한 이름과 일치해야 합니다.

시스템을 제거하는 또 다른 방법은 시스템 함수 자체를 다음에 전달하는 것입니다: `engine.removeSystem()` 메서드. 이렇게 하면 추가할 때 시스템에 이름을 붙일 필요가 없습니다.

```ts
// 시스템 선언
function mySystem(dt: number){
  console.log("마지막 틱 이후 지연 시간: ", dt)
}

// 시스템 추가
engine.addSystem(mySystem)

// 함수 자체를 전달하여 시스템 제거
engine.removeSystem(mySystem)
```

`engine.removeSystem()` 는 시스템 추가 시 지정한 이름 문자열 또는 시스템 함수 자체에 대한 참조를 허용합니다.

아래 메서드를 사용하면 시스템의 목적이 끝났을 때 스스로 종료하게 만들 수 있습니다.

```ts
   const mySystem = function(dt: number){
        time += dt
        if(time > 3){
		engine.removeSystem(mySystem)
        }
    }
    engine.addSystem(mySystem)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/systems.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
