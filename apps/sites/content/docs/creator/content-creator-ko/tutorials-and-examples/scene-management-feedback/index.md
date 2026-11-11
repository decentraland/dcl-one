> 씬을 관리하고 플레이어 피드백을 수집하는 도구.

# 씬 관리 및 피드백

씬을 관리하고 플레이어 피드백을 수집하기 위한 도구입니다.

| 예시                                                                                                         | 보여주는 내용                                                                    | 관리 주체        | 태그                       | 소스                                                                                             |
| ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------ | ------------------------ | ---------------------------------------------------------------------------------------------- |
| [씬 피드백](/docs/creator/content-creator-ko/tutorials-and-examples/scene-management-feedback/playtest-feedback/) | 플레이 중 또는 피드백 버튼, 키오스크, 영역을 통해 플레이어 평점과 댓글을 수집하며, CSV 결과는 씬 Storage에 저장됩니다. | Decentraland | `UI`, `멀티플레이어 서버`, `저장소` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback) |

[모든 예시 장면 둘러보기](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/scene-management-feedback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
