> 씬을 외부 서버 및 API와 통신하는 방법.

# 네트워크 연결

씬에서는 API를 노출하는 외부 서비스를 활용할 수 있으며, 이를 통해 업데이트된 가격 데이터, 날씨 데이터 또는 API가 제공하는 다른 종류의 정보를 가져올 수 있습니다.

자신만의 외부 서버를 설정해 씬을 지원하고 플레이어 간 데이터를 동기화하는 데 사용할 수도 있습니다. 이는 REST API를 노출하는 서버로 구현할 수도 있고, WebSocket을 사용하는 서버로 구현할 수도 있습니다.

## REST API 호출

씬의 코드에서 REST API에 호출을 보내 데이터를 가져올 수 있습니다.

서버가 응답을 보내는 데 시간이 걸릴 수 있으므로, 이 명령은 다음과 같은 형태로 실행해야 합니다. [비동기 함수](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/)를 사용하여 `executeTask()`.

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('URL에 연결하지 못했습니다')
	}
})
```

fetch 명령에는 헤더, HTTP 메서드, HTTP 본문을 하나의 객체로 묶는 두 번째 선택적 인수도 포함할 수 있습니다.

* **url**: 요청을 보낼 주소
* **init**: `RequestInit`  객체로, 다음 항목을 포함할 수 있습니다:
  * **method** : 사용할 HTTP 메서드(GET, POST, DELETE 등)
  * **body**: 요청 본문의 내용. 문자열화된 JSON 객체로 보내야 합니다.
  * **headers**: 요청에 포함할 추가 헤더입니다.
  * **redirect**: 리디렉션 전략('follow' | 'error' | 'manual')
  * **timeout**: 요청이 실패하기 전에 응답을 기다릴 시간입니다. 기본값은 30000밀리초(30초)입니다.

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl, {
			headers: { 'Content-Type': 'application/json' },
			method: 'POST',
			body: JSON.stringify(myBody),
		})
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('URL에 연결하지 못했습니다')
	}
})
```

fetch 명령은 다음을 반환합니다. `response`  객체와 다음 데이터를 포함합니다:

* `headers`: `ReadOnlyHeaders`  객체입니다. 특정 헤더를 가져오려면  `get()`  메서드를 호출하고, 헤더가 존재하는지 확인하려면  `has()`  메서드를 사용합니다.
* `ok`: 불리언
* `redirected`: 불리언
* `status`: 상태 코드 번호
* `statusText`: 상태 코드에 대한 텍스트
* `type`: 다음 값 중 하나를 가집니다: *basic*, *cors*, *default*, *error*, *opaque*, *opaqueredirect*
* `url`: 전송된 URL
* `json()`: 본문을 JSON 형식으로 가져옵니다.
* `text()`: 본문을 텍스트로 가져옵니다.


> [!WARNING]
> **📔 참고**: `json()` 및 `text()` 서로 배타적입니다. 두 형식 중 하나로 응답 본문을 가져오면, 더 이상 다른 형식으로는 본문을 가져올 수 없습니다. `response`  객체에서 사용합니다.



> [!WARNING]
> **📔 참고**: 각 Decentraland 씬은 한 번에 하나의 `fetch`  명령만 실행할 수 있습니다. 이는 요청이 내부적으로 대기열에 들어가므로 씬 코드의 구조에 영향을 주지 않습니다. 씬에서 여러 엔드포인트로 여러 요청을 보내야 한다면, 각 요청은 이전 요청에 대한 응답이 온 뒤에만 전송된다는 점을 기억하세요.


## 서명된 요청

Decentraland 내부의 플레이어 세션에서 요청이 발생했음을 인증하기 위해 추가 보안 조치를 사용할 수 있습니다. 추가 서명이 포함된 요청을 보낼 수 있으며, 이 서명은 Decentraland 세션이 각 플레이어의 주소를 기반으로 생성하는 임시 키로 서명됩니다. 그런 다음 요청을 받는 서버는 서명된 메시지가 실제로 현재 월드에서 활성 상태인 주소와 일치하는지 검증할 수 있습니다.

이런 종류의 보안 조치는 게임에서 토큰이나 포인트를 파밍하기 위해 플레이어가 시스템을 악용할 유인이 있을 때 특히 유용합니다.

서명된 요청을 보내려면,  `signedFetch()`  함수를 사용하는 것과 동일합니다.  `fetch()` 함수입니다.

```ts
import { signedFetch } from '~system/SignedFetch'

executeTask(async () => {
	try {
		let response = await signedFetch({
			url: callUrl,
			init: {
				headers: { 'Content-Type': 'application/json' },
				method: 'POST',
				body: JSON.stringify(myBody),
			},
		})

		if (!response.ok) {
			throw new Error('잘못된 응답')
		}

		let json = JSON.parse(response.body)

		console.log('응답 수신: ', json)
	} catch {
		console.log('URL에 연결하지 못했습니다')
	}
})
```

요청에는 추가 헤더 집합이 포함되며, 여기에는 서명된 메시지와 이를 해석하기 위한 메타데이터 집합이 들어 있습니다. 서명된 메시지는 플레이어의 임시 키를 사용해 요청의 모든 내용을 암호화한 것입니다.

해당 `signedFetch()` 는 다음과 다릅니다. `fetch()`  함수는 응답이 전체 HTTP 메시지에 대한 프로미스이며, 이는  `FlatFetchResponse`  객체로 표현됩니다. 여기에는 다음 속성이 포함됩니다:

* `body`
* `headers`
* `ok`
* `status`
* `statusText`

해당 **body** 는 항상 문자열이며, 위 예시처럼 파싱할 수 있습니다.

### 서명된 요청 검증

서명된 요청을 활용하려면, 이를 받는 서버는 서명이 요청의 나머지 부분과 일치하는지, 그리고 서명된 메시지에 인코딩된 타임스탬프가 현재 시각인지 검증해야 합니다.

다음 예제 씬에서 이 작업을 수행하는 간단한 서버 예시를 확인할 수 있습니다:

[플레이어 진위 검증](https://github.com/decentraland-scenes/validate-player-authenticity)

## 요청 타임아웃

HTTP 요청이 응답받는 데 너무 오래 걸리면, 다른 요청을 보낼 수 있도록 실패합니다.  `fetch()`의 기본 타임아웃 임계값은 30초이지만,  `timeout`  속성을 구성하여 각 요청마다 다른 값을 지정할 수 있습니다.  `timeout` 의 값은 밀리초 단위로 표시됩니다.

```ts
fetch('https://some-url.com', { timeout: 1000 })
```

## WebSocket 사용

서버가  *wss*.

```ts
var socket = new WebSocket('url')

socket.onmessage = function (event) {
	console.log('WebSocket 메시지 수신:', event)
}
```

WebSocket 사용 구문은 JavaScript에서 기본적으로 구현된 것과 다르지 않습니다.  [Mozilla Web API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket) 의 문서를 참조하여 WebSocket을 통해 메시지를 수신하고 보내는 방법에 대한 자세한 내용을 확인하세요.


> [!NOTE]
> **💡 팁**: WebSocket 연결 사용을 간소화하고 Decentraland와 매우 잘 작동하는 것으로 입증된 라이브러리 중 하나는  [Colyseus](https://colyseus.io/)입니다. 다른 여러 WebSocket 라이브러리는 Decentraland SDK와 호환되지 않습니다.
> 
> WebSocket 연결 위에 추상화 계층을 구축해 변화에 반응하고 서버에 일관된 게임 상태를 원격으로 저장하는 일을 매우 쉽게 만듭니다. 다음 예시에서 실제 동작을 확인할 수 있습니다:
> 
> * [Cube Jumper](https://github.com/decentraland-scenes/cube-jumper-colyesus-sdk7)
> * [Space Traitor](https://github.com/decentraland-scenes/Space-Traitor)
> * [AI NPC](https://github.com/decentraland-scenes/inworld-ai-sdk7)


## 네트워크 요청 디버깅

디버그 패널을 열어 네트워크 요청을 디버깅할 수 있습니다.

디버그 패널을 열려면  ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp)  아이콘을 오른쪽 상단 모서리에서 클릭하세요. 그런 다음  **웹 요청**  탭을 선택하고  **Chrome Devtools 열기**.

이렇게 하면 Network 탭이 열린 새 Chrome 창이 열립니다.

참조 [미리보기에서 디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-preview/#web-requests) 자세한 내용은.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/networking/network-connections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
