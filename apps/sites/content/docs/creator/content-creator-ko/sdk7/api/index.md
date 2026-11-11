# API 참조

- [테스트 및 피드백](/docs/creator/content-creator-ko/sdk7/api/alpha-testing-guidelines/): SDK7를 사용해 보고 피드백을 제공하는 방법에 대한 가이드라인
- [릴리스 노트](/docs/creator/content-creator-ko/sdk7/api/releases/): 지난 1년 동안 SDK와 Creator Hub에서 변경된 내용 중 크리에이터에게 영향을 주는 항목만 필터링한 것.
- [API 참조](/docs/creator/content-creator-ko/sdk7/api/v7-alpha/): js-sdk- 툴체인에서 생성된 Decentraland SDK7의 전체 API 참조.
- [버전 지원 계약](/docs/creator/content-creator-ko/sdk7/api/version-support-agreement/): 버전 지원 계약


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/api.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
