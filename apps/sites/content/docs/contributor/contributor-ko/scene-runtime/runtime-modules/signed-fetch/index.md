# 서명된 가져오기

이 `SignedFetch` 이 모듈은 다음의 구현을 제공합니다. `fetch` 다음과 투명하게 호환되는 인터페이스 [서명된 fetch](/docs/contributor/contributor-ko/authentication/signed-fetch/) 서명된 [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 을 발신 요청에 적용하는 방법을 정의하는 프로토콜입니다.

이 요청을 서명하고 검증하는 절차는 다음에 자세히 설명되어 있습니다. [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 페이지.

사용 `SignedFetch` 장면에서 사용하려면 다음이 필요합니다. [`USE_FETCH`](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 권한.

### 메서드

**`signedFetch`**

다음과 같이 HTTP 요청을 보냅니다([`fetch`](/docs/contributor/contributor-ko/scene-runtime/globals/#http)), 검증 헤더를 자동으로 추가합니다.

```ts
interface Request {
  // 요청 대상 URL:
  url: string;

  // 요청에 대한 선택적이고 자명한 매개변수:
  init?: {
    method?: string;
    body?: string;
    headers: { [key: string]: string };
  };
}

interface Response {
  // HTTP 요청이 성공적으로 수행되었는지 여부(2xx 이외의 코드는 실패가 아닙니다)
  ok: boolean;

  // 응답의 자명한 세부 정보:
  status: number;
  statusText: string;
  headers: { [key: string]: string };
  body: string;
}

function signedFetch(Request): Promise<Response>;
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/runtime-modules/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
