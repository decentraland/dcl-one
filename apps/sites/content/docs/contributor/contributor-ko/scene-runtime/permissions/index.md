# 권한

씬이 민감한 기능에 접근해야 하는 경우, 다음을 수행해야 합니다:

1. 관련된 [권한을 매니페스트에 선언합니다](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions).
2. 다음 [RestrictedActions](/docs/contributor/contributor-ko/scene-runtime/runtime-modules/restricted-actions/) 모듈을 사용해 런타임이 작업을 수행하도록 합니다(권한이 부여된 경우).

자세한 정보는 해당 페이지를 참조하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/permissions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
