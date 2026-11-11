> 멀티플레이어 패턴과 서버 소유 공유 상태.

# 멀티플레이어

멀티플레이어 패턴과 서버 소유 공유 상태.

| 예시                                                                                                                                 | 보여주는 내용                                      | 관리 주체        | 태그                            | 소스                                                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ------------ | ----------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| [미니 게임 라이브러리](/docs/creator/content-creator-ko/tutorials-and-examples/multiplayer-data-services/mini-games-library/)                  | 멀티플레이어 미니 게임을 위한 플레이어 대기열, 점수판, 지원 로직.       | Decentraland | `멀티플레이어`, `점수판`, `게임`         | [저장소 열기](https://github.com/decentraland/sdk-mini-games)                                                          |
| [멀티플레이어 서버 리더보드](/docs/creator/content-creator-ko/tutorials-and-examples/multiplayer-data-services/authoritative-server-leaderboard/) | 점수를 검증하고, 총점을 저장하며, 순위를 브로드캐스트하는 서버 소유 리더보드. | Decentraland | `멀티플레이어`, `리더보드`, `서버`, `저장소` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard) |

[모든 예시 장면 둘러보기](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/multiplayer-data-services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
