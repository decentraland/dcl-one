# 시뮬레이터

시뮬레이터 전송은 전송 인터페이스 외에는 지정된 동작이 없는, 완전히 임의적인 구현입니다.

수신 메시지를 내보내고 송신 메시지를 원하는 대로 처리할 수 있으며, 의미론적 요구 사항은 없습니다. 구현은 메시지를 전혀 내보내지 않고 모든 메서드 호출을 무시하도록 선택할 수도 있지만, [오프라인 전송](/docs/contributor/contributor-ko/communications/transport-types/offline/) 이 그 용도에 더 적합합니다.

시뮬레이터 URI는 다음과 같습니다:

```
simulator:<custom suffix>
```

다음에 오는 것은 `simulator:` 접두사는 지정되지 않았으며, 구현체가 적절하다고 판단하는 방식으로 사용하거나(또는 비워 둘 수 있습니다).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/transport-types/simulator.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
