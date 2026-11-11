> 장면을 외부 API, 데이터 소스 및 타사 서버에 연결합니다.

# 외부 서비스

장면을 외부 API, 데이터 소스 및 타사 서버에 연결합니다.

| 예시                                                                                                                      | 보여주는 내용                                  | 관리 주체        | 태그                             | 소스                                                                                                |
| ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- | ------------ | ------------------------------ | ------------------------------------------------------------------------------------------------- |
| [스프레드시트의 데이터](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/spreadsheet-data/)                 | Google 시트로 구동되는 장면 콘텐츠로, 재배포 없이 업데이트됩니다. | Decentraland | `api`, `스프레드시트`, `비디오`, `웨어러블` | [저장소 열기](https://github.com/decentraland/Read_Spreadsheet_Scene)                                  |
| [타사 방명록](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/third-party-guest-book/)                | 외부 HTTP 서비스에 의해 지원되는 방명록입니다.             | Decentraland | `api`, `저장소`                   | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)               |
| [타사 리더보드](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/third-party-leaderboard/)              | 외부 HTTP 서비스에 의해 지원되는 영속적 리더보드입니다.        | Decentraland | `api`, `저장소`, `리더보드`           | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)                 |
| [타사 플레이어 인증](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/third-party-player-authentication/) | 외부 서비스로 전송되는 요청에 대한 플레이어 신원을 확인합니다.      | Decentraland | `api`, `인증`, `플레이어`            | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity) |

[모든 예시 장면 둘러보기](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/external-services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
