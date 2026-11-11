> 씬에서 사용자 클릭을 처리하는 방법을 알아봅니다.

# 고급 버튼 이벤트

씬의 상호작용이 다음과 호환되지 않는 사용자 지정 로직을 따르도록 해야 한다면 [콜백 등록](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/) 또는 [시스템 기반](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/) 방식이라면, 원시 입력 이벤트 데이터를 직접 다룰 수 있습니다. 이 방식은 가장 어렵지만 가장 유연합니다.

## PointerEventsResult 컴포넌트

엔진이 포인터 이벤트를 감지하면, 클릭된 엔티티에는 다음이 할당됩니다. `PointerEventsResult` 컴포넌트입니다. 이 컴포넌트에는 히트 이벤트에 대한 모든 원시 데이터가 포함되어 있으며, 이전 이벤트의 기록 데이터도 저장됩니다.

다음에 있는 헬퍼들은 `inputSystem`, 예를 들어 `inputSystem.isTriggered` 또는 `inputSystem.getInputCommand` 대부분의 단순한 시나리오에 적합하지만, 히트 이벤트에 대한 더 자세한 정보가 필요하다면 다음의 원시 데이터를 확인하세요. `PointerEventsResult`.

해당 `PointerEventsResult` 이 컴포넌트는 포인터 이벤트 목록을 보유하며, 각 이벤트에 대한 객체 하나를 포함합니다. 최대 100개의 이벤트 목록을 저장하며, 최신 이벤트는 목록 끝에 저장됩니다. 목록의 길이가 100에 도달하면 새 이벤트가 들어올 때마다 오래된 이벤트를 버리기 시작합니다.

목록의 각 이벤트에는 다음 데이터가 있습니다:

* `아날로그`: 선택적 숫자이며, 조이스틱 같은 아날로그 입력의 이벤트에만 존재하고 입력의 아날로그 값을 저장합니다.
* `버튼`: 어떤 버튼 ID가 눌렸는지 나타냅니다. 이 숫자는 다음과 일치합니다. `InputAction` 사용 가능한 모든 버튼을 나열한 enum입니다.
* `state`: enum에서 가져온 포인터 이벤트 유형입니다. `PointerEventType`. *0* 는 다음을 가리킵니다 `PointerEventType.PET_UP`, *1* 을(를) `PointerEventType.PET_DOWN`, *2* 을(를) `PointerEventType.PET_HOVER_ENTER`, *3* 을(를) `PointerEventType.PET_HOVER_LEAVE`, *4* 을(를) `PointerEventType.PET_PROXIMITY_ENTER`, *5* 을(를) `PointerEventType.PET_PROXIMITY_LEAVE`
* `타임스탬프`: [람포트 타임스탬프](https://en.wikipedia.org/wiki/Lamport_timestamp) 는 각 버튼 이벤트를 식별하는 데 사용됩니다.

  > 참고: 이 타임스탬프는 현재 시간을 기준으로 번호가 매겨지지 않습니다. 0에서 시작해 각 이벤트마다 1씩 증가하는 카운터라고 생각하세요.
* `히트`: 히트 이벤트에 대한 다음 데이터를 포함하는 객체입니다:
  * `entityId`: 레이에 의해 히트된 엔티티의 ID 번호입니다.
  * `meshName`: *문자열* 충돌한 3D 모델 내 특정 메쉬의 내부 이름입니다. 3D 모델이 여러 메쉬로 구성된 경우 유용합니다.
  * `globalOrigin`: *Vector3* 레이가 시작되는 위치를 나타냅니다(씬 기준)
  * `방향`: *Vector3* 전역 좌표계에서 레이의 방향 벡터와 함께
  * `위치`: *Vector3* 레이가 히트한 엔티티와 교차한 위치를 나타냅니다(씬 기준)
  * `length`: 엔티티와의 충돌이 발생한 위치까지 레이의 시작점부터의 길이입니다.
  * `normalHit`: *Vector3* 정규화된 방향 벡터와 함께, 월드 공간에서 히트의 법선 각도를 설명합니다.

```ts
import { engine, PointerEventsResult } from '@dcl/sdk/ecs'

function PointerReadingSystem() {
  for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
    const events = PointerEventsResult.get(entity)
    for (const event of events) {
      console.log('포인터 이벤트 데이터:', event)
    }
  }
}

engine.addSystem(PointerReadingSystem)
```

## 플레이어 이동 추적

플레이어 이동 타이밍이 중요한 실시간 멀티플레이어 게임에서는 제3자 서버를 단일 진실 소스로 사용해 각 플레이어의 위치를 추적하고 싶을 수 있습니다. 버튼 입력을 미리 감지하고 아바타가 위치를 옮기기 전에 서버에서 그 효과를 예측하면 응답 시간을 개선할 수 있습니다.

이 방법은 네트워크 지연을 보완하는 데 도움이 되지만, 반드시 불일치를 초래하므로 플레이어의 현재 위치도 정기적으로 조회해 보정을 해야 합니다. 이러한 예측과 보정의 균형을 맞추려면 많은 미세 조정이 필요할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/button-events/advanced-button-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
