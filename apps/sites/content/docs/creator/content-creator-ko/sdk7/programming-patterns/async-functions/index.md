> 씬 코드에서 언제, 어떻게 비동기 함수를 실행해야 하는지 알아봅니다.

# 비동기 함수

## 개요

씬의 코드 대부분은 단일 스레드를 사용하여 동기적으로 실행됩니다. 즉, 명령은 한 줄씩 순차적으로 실행됩니다. 각 명령은 다음 명령이 시작되기 전에 먼저 이전 명령의 실행이 끝날 때까지 기다려야 합니다.

씬의 시스템에 있는 함수들조차도 하나씩 실행되며, 다음의 [우선순위 순서](/docs/creator/content-creator-ko/sdk7/architecture/systems/#system-execution-order).

코드를 동기적으로 실행하면 일관성이 보장됩니다. 코드의 명령들이 어떤 순서로 실행되는지 항상 알 수 있기 때문입니다.

반면에, 씬은 다음 프레임을 만들기 위해 초당 여러 번 업데이트되어야 합니다. 코드의 일부가 응답하는 데 너무 오래 걸리면 전체 메인 스레드가 멈추고, 그 결과 프레임 속도가 느려집니다.

그래서 경우에 따라 일부 명령을 비동기적으로 실행하고 싶을 수 있습니다. 이는 작업을 시작해 두고, 그동안 씬은 다음 코드 줄들을 계속 실행할 수 있다는 뜻입니다. 씬은 여전히 모든 것을 단일 스레드에서 실행하지만, 비동기 작업이 응답을 기다리는 동안 나머지 코드는 차단되지 않습니다.

응답에 시간이 걸릴 수 있는 외부 서비스에 의존하는 작업에 특히 유용합니다. 응답을 기다리는 동안의 유휴 시간이 다른 작업을 막지 않도록 하고 싶기 때문입니다.

예를 들면:

* REST API에서 데이터를 가져올 때
* 블록체인에서 거래를 수행할 때


> [!WARNING]
> **📔 참고**: 작업이 끝나기 전에 씬의 여러 프레임이 렌더링될 수 있다는 점을 유의하세요. 비동기 작업이 완료되는 동안 중간 상황을 처리할 수 있도록 씬의 코드가 충분히 유연한지 확인하세요.


## 비동기 함수를 실행하기

async로 `비동기` 표시하여, 대기하는 동안 나머지 씬의 코드를 차단하지 않고 비동기적으로 실행되게 합니다.

```ts
// 비동기 함수 선언
async function myAsyncTask() {
	// 함수의 단계 실행
}

// 비동기 함수 호출
myAsyncTask()

// 나머지 코드는 계속 실행됩니다
```

## executeTask 함수

해당 `executeTask()` 함수는 람다 함수를 비동기적으로 실행합니다. `executeTask()` 를 사용하면 함수 선언과 실행을 한 문장으로 모두 할 수 있습니다.

```ts
executeTask(async () => {
	let data = await myAsyncTask()
	console.log(data)
})

// 나머지 코드는 계속 실행됩니다
```

## then 함수

해당 `then` 함수는 인수로 람다 함수를 받으며, 이전 문장이 끝난 후에야 실행됩니다. 이 람다 함수는 선택적으로 이전 문장의 반환값에 매핑된 입력을 가질 수 있습니다.

```ts
myAsyncTask().then((data) => {
	console.log(data)
})
```


> [!WARNING]
> **📔 참고**: 일반적으로는 `executeTask` 방식을 사용하는 편이 `then` then `then` 함수보다 낫습니다. 여러 중첩된 수준에서 then 함수에 너무 많이 의존하면 "콜백 지옥(callback hell)"으로 알려진 상태에 빠질 수 있으며, 코드가 읽고 유지보수하기 매우 어려워질 수 있습니다.


## PointerEvents 및 RayCast 함수

씬이 `PointerEvent` 또는 `RayCast` 컴포넌트를 사용할 때, 충돌 계산은 엔진에서 비동기적으로 수행됩니다. 그러면 엔진은 결과 이벤트를 씬에 반환하며, 이는 이벤트가 호출된 시점보다 게임 루프의 한 틱 또는 여러 틱 뒤에 도착할 수 있습니다.

그런 다음 결과가 도착한 프레임에서 이를 처리할 시스템을 만들어야 합니다.


> [!WARNING]
> **📔 참고**: 클릭을 [**콜백 등록하기**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/) 방식으로 처리하는 경우, 이를 처리하기 위해 명시적으로 시스템을 만들 필요는 없지만, 동일한 일이 백그라운드에서 발생합니다.


참조 [클릭 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/) 및 [레이캐스팅](/docs/creator/content-creator-ko/sdk7/interactivity/raycasting/).


> [!NOTE]
> **💡 팁**: 레이캐스트 결과를 처리하는 데 많은 계산이 필요하다면(예: 경로 탐색 알고리즘 실행) 해당 계산을 비동기 함수에서 실행하는 것이 좋을 수 있습니다.


## await 문

하나의 `await` 문은 다음 코드 줄로 넘어가기 전에 응답을 기다리도록 실행을 강제합니다. `await` 문은 비동기 코드 블록 안에서만 사용할 수 있습니다.

```ts
// 함수 선언
async function myAsyncTask() {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('URL에 연결하지 못했습니다')
	}
}

// 함수 호출
myAsyncTask()

// 나머지 코드는 계속 실행됩니다
```

위의 예시는 다음을 포함하는 함수를 실행합니다. `fetch()` 외부 API에서 데이터를 가져오는 `fetch()` 작업입니다. 서버가 응답하는 데 얼마나 걸릴지 예측할 수 없으므로 이 작업은 비동기적입니다. 하지만 다음 줄에서는 이를 JSON으로 파싱하기 전에 이 작업의 출력이 준비되어 있어야 합니다.  `await` 문은 여기서 다음 줄이 해당 작업이 값을 반환할 때만 실행되도록 보장합니다. 마찬가지로, `response.json()` 함수도 비동기적이지만, 다음 줄에서는 이를 로그로 출력하기 전에 JSON이 파싱되어 있어야 합니다. 두 번째 `await` 문은 JSON 파싱이 끝날 때까지, 얼마나 오래 걸리든 다음 줄이 호출되는 것을 강제합니다.

## 함수 호출에 타임아웃 설정하기

다음을 사용하세요 `setTimeout` 는 특정 코드 줄들이 실행되기 전에 일정 시간을 기다리게 합니다. 여기에는 두 개의 인수가 필요합니다:

* 실행할 함수
* 해당 함수를 실행하기 전에 기다릴 밀리초 수

아래 예시는 콘솔에 메시지를 로그로 출력하는 간단한 함수를 실행하기 전에 1000밀리초(1초에 해당)를 기다립니다.

```ts
import { timers } from '@dcl/sdk/ecs'

console.log('이것은 바로 출력됩니다')

timers.setTimeout(() => {
	// 지연 후 실행할 함수
    console.log('이것은 1초 후에 출력됩니다')
}, 1000)
```

해당 `clearTimeout` 는 `setTimeout` 아직 실행되기를 기다리고 있는 함수의 실행을 취소하는 데 사용할 수 있습니다. `setTimeout` 는 타이머 ID(숫자)를 반환하며, 이를 나중에 `clearTimeout`에 전달할 수 있습니다. 이 경우 변수 `timeoutId` 는 setTimeout을 수행할 때 얻어지며, 그런 다음 `setTimeout`clearTimeout에 `clearTimeout` 전달하여 취소합니다.

```ts
import { timers } from '@dcl/sdk/ecs'

const timeoutId = timers.setTimeout(() => {
    console.log('이 함수를 실행하기까지 1초를 기다립니다')
}, 1000)

timers.clearTimeout(timeoutId)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/programming-patterns/async-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
