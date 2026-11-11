# 서명된 가져오기

프로토콜 클라이언트가 인증된 HTTPS 요청을 만들고자 할 때, 표준을 활용할 수 있습니다 *서명된 fetch* 메커니즘.

서명된 fetch는 다음을 포함하는 요청입니다: [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/), 헤더를 통해 표현됩니다. Decentraland 호환 API를 가진 서버는 예를 들어, 다음을 하기 전에 신원을 검증할 수 있습니다 [씬에서의 요청을 허용하거나](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/signed-fetch/) 또는 생성하기 [어댑터](/docs/contributor/contributor-ko/communications/overview/).

## 헤더

서버가 인증 체인을 검증하는 데 필요한 정보는 3개 이상의 헤더로 전달됩니다:

* `X-Identity-Timestamp`: 다음의 `타임스탬프` 필드로, 서명된 페이로드에 포함됩니다(아래 참조).
* `X-Identity-Metadata`: 다음의 `메타데이터` 필드로, 서명된 페이로드에 포함됩니다(아래 참조).
* `X-Identity-AuthChain-<index>`: 다음의 [JSON으로 직렬화된 인증 단계](/docs/contributor/contributor-ko/authentication/authchain/#constructing) `<index>`, 다음부터 시작하여 `0`.

전송된 체인은 서버에서 검증됩니다 [지정된 대로](/docs/contributor/contributor-ko/authentication/authchain/).

## 본문

요청 본문은 지정되지 않습니다. 서비스는 원하는 프로토콜이나 형식을 자유롭게 사용할 수 있습니다.

## 페이로드

인증 체인의 [페이로드](/docs/contributor/contributor-ko/authentication/authchain/#constructing) 서명된 fetch의 경우 다음과 같은 **소문자, 콜론으로 구분된** 문자열로, 요청 요소 일부를 포함합니다:

```
<method>:<path>:<timestamp>:<metadata>
```

이 `메서드` 및 `경로` 필드는 요청의 것과 일치해야 하며, `타임스탬프` 는 다음과 동일합니다: `X-Identity-Timestamp` 헤더.

마지막 필드인 `메타데이터`에는 임의의 내용을 넣을 수 있습니다.

예를 들면:

```
get:/some/path:1682790056:{"some":"custom json"}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/authentication/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
