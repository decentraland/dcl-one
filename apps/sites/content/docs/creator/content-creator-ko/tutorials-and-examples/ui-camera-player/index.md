> 인터페이스, 카메라, 그리고 플레이어를 위한 시스템.

# UI, 카메라, 플레이어 경험

인터페이스, 카메라, 그리고 플레이어용 시스템.

| 예시                                                                                               | 보여주는 내용                             | 관리 주체        | 태그                     | 소스                                                                                                    |
| ------------------------------------------------------------------------------------------------ | ----------------------------------- | ------------ | ---------------------- | ----------------------------------------------------------------------------------------------------- |
| [NPC 툴킷](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/npc-toolkit/)     | SDK7 씬을 위한 재사용 가능한 NPC 동작과 대화 UI.   | Decentraland | `NPC`, `대화`, `UI`      | [저장소 열기](https://github.com/decentraland-scenes/dcl-npc-toolkit)                                      |
| [관전 모드](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/spectate-mode/)    | 관전자 스타일의 카메라와 플레이어 시청 흐름.           | Decentraland | `카메라`, `관전자`, `플레이어`   | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode)        |
| [UI 애니메이션](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/ui-animations/) | 애니메이션 UI 요소, 스프라이트 시트, 로딩 표시기.      | Decentraland | `UI`, `애니메이션`, `스프라이트` | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations)                    |
| [UI 안전 영역](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/ui-safe-areas/) | 화면 인셋과 안전 영역을 기준으로 인터페이스 요소를 배치합니다. | Decentraland | `UI`, `HUD`            | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) |
| [가상 카메라](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/virtual-cameras/) | 씬에서 정의된 카메라를 생성하고 전환합니다.            | Decentraland | `카메라`, `시네마틱`, `조작`    | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras)       |

[모든 예시 장면 둘러보기](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/ui-camera-player.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
