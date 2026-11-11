# 아키텍처

- [엔티티 및 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/): Decentraland 씬의 엔티티와 컴포넌트에 대한 필수 사항을 알아봅니다
- [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/): 시스템이 씬 상태를 업데이트하는 데 어떻게 사용되는지 알아봅니다
- [사용자 지정 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/): 엔티티와 관련된 특정 데이터를 처리할 사용자 지정 컴포넌트를 만듭니다
- [복합체](/docs/creator/content-creator-ko/sdk7/architecture/composites/): 복합 파일에서 런타임에 엔티티와 컴포넌트의 트리를 생성합니다
- [컴포넌트 조회](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/): 공통 컴포넌트를 가진 엔티티 목록을 가져오는 방법을 알아보고, 이를 더 쉽게 확인하거나 업데이트하는 방법을 배웁니다.
- [변경 사항 구독](/docs/creator/content-creator-ko/sdk7/architecture/subscribe-to-changes/): 컴포넌트의 변경 사항을 감지하고 변경될 때마다 함수를 실행합니다
- [데이터 지향 프로그래밍](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/): 데이터 지향 프로그래밍은 성능을 최대한 활용하는 강력한 프로그래밍 접근 방식입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
