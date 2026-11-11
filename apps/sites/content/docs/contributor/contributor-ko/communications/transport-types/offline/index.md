# 오프라인

오프라인 전송은 realm에서 사용할 수 있는 통신 서버가 없음을 알리기 위한 더미 구현입니다. 주로 테스트 및 개발 환경에서 사용됩니다.

이 전송은 들어오는 메시지를 전혀 내보내지 않고, 모든 나가는 메시지를 무시합니다. 시뮬레이션된 동작이 있는 오프라인 변형을 원한다면, 다음을 참조하세요 [시뮬레이터](/docs/contributor/contributor-ko/communications/transport-types/simulator/) 대신.

URI는 다음과 같은 형태입니다:

```
offline:<사용자 지정 접미사>
```

다음에 오는 것은 `offline:` 접두사는 지정되지 않았으며, 구현체가 적절하다고 판단하는 방식으로 사용하거나(또는 비워 둘 수 있습니다).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/transport-types/offline.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
