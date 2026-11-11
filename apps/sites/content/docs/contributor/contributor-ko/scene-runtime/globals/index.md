# 전역 변수

씬 런타임은 샌드박스화된 JavaScript 환경 안에 특정 객체와 함수가 존재하도록 보장하며, 다음과 같은 표준 유틸리티에 더해 `Promise`, `Date` 또는 `Math`.


> [!NOTE]
> 아래에 설명된 전역 변수들은 일반적인 JavaScript 개념이지만, Decentraland 씬 런타임에 맞게 조정되어 있으며 브라우저나 Node 환경의 대응 항목과 정확히 동일하게 동작하지 않을 수 있습니다.


### 전역 변수

런타임은 씬의 전역 범위에 다음 6가지 정의를 주입합니다:

1. [`console`](#console): 일반적인 `console` 객체.
2. [`exports`](#module): 씬이 자신의 [공개 인터페이스](/docs/contributor/contributor-ko/scene-runtime/execution/).
3. [`module`](#module): 에 대한 컨테이너 `exports` 객체.
4. [`require`](#module): 런타임이 제공하는 모듈을 이름으로 로드하는 함수.
5. [`fetch`](#http): 브라우저의 제한된 구현 `fetch` 함수.
6. [`WebSocket`](#http): 브라우저의 제한된 구현 `WebSocket` 클래스.

이들 모두는 읽기 전용 속성으로 정의되어 있으므로 다시 할당할 수 없습니다. 일부는 특정 [권한](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 가 씬에 부여되지 않으면 사용할 때 예외를 던집니다.

### 콘솔 <a href="#console" id="console"></a>

씬은 `console` 객체에 접근할 수 있으며, 브라우저나 Node 환경이 제공하는 것과 비슷하지만, 보통 볼 수 있는 메서드들 중 일부만 제한적으로 사용할 수 있습니다.

```ts
type Console = {
  log(...args: any): void
  info(...args: any): void
  debug(...args: any): void
  warning(...args: any): void
  error(...args: any): void
}
```

표준 대응 항목과 마찬가지로, 각 메서드는 임의의 유형의 가변 인수를 받아 사람이 읽을 수 있는 메시지로 렌더링합니다. 예를 들어, 다음은 유효합니다:

```js
console.log("The thing just appeared", { thing: "foo" }, [1, 2, 3])
```

이 메서드들의 정확한 동작은 제공자에 따라 다르지만, 메시지는 코드 디버깅을 하는 씬 개발자가 접근할 수 있어야 합니다.


> [!NOTE]
> Foundation의 브라우저 기반 World Explorer에서는 로그 메시지가 개발자 도구 패널에 표시됩니다.


### 가져오기 및 내보내기 <a href="#module" id="module"></a>

씬은 전통적인 [CommonJS](https://wiki.commonjs.org/wiki/Modules/1.0) 모듈 인터페이스를 사용하여 객체를 가져오고 내보낼 수 있습니다.

```ts
exports: Object
require(moduleName: string): Object
```

이 `require`  함수는 씬이 런타임이 제공하는 모듈(예: [EngineApi](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/engine-api/) 또는 [RestrictedActions](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/restricted-actions/)), 그리고 그 외에는 아무것도 접근하지 못하게 합니다(즉, **하지** NPM 패키지나 경로를 통한 모듈에 접근할 수 없습니다).

에 추가된 속성은 `exports`  객체가 씬의 공개 인터페이스이며 런타임에 노출됩니다. 실제로 씬은 *반드시* 올바르게 실행되려면 최소한 하나의 메서드를 노출해야 합니다( [실행](/docs/contributor/contributor-ko/scene-runtime/execution/)).


> [!NOTE]
> TypeScript와 같은 언어로 작성된 씬은 더 현대적인 `import` 및 `export` 문을 사용하며, 이는 CommonJS 호환 방식의 `require` 및 `exports`.


### HTTP 및 WebSockets <a href="#http" id="http"></a>

이 `fetch` 및 `WebSocket` 전역 변수는 잘 알려진 대응 항목과 정확히 동일하게 동작합니다( [fetch](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) 및 [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API) MDN의 항목 참조), 다만 런타임에서 강제하는 몇 가지 제한이 있습니다.

를 호출할 때 `fetch` 함수:

* URL이 `https://`.
* 씬에 [`USE_FETCH` 권한](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions).
* 구현 정의된 타임아웃이 요청을 중단할 수 있습니다.

를 사용할 때 `WebSocket` 클래스:

* URL이 `wss:`
* 씬에 [`USE_WEBSOCKET` 권한](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions).

이러한 차이점을 제외하면, 두 경우 모두 표준 동작을 따릅니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/globals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
