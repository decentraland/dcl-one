> 핵심 게임플레이 메커니즘과 플레이어 상호작용.

# 게임플레이 및 상호작용

핵심 게임플레이 메커니즘 및 플레이어 상호작용.

| 예제                                                                                                                            | 무엇을 보여 주는지                                | 관리 주체        | 태그                    | 소스                                                                                                   |
| ----------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | ------------ | --------------------- | ---------------------------------------------------------------------------------------------------- |
| [입력 수정기](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/input-modifier/)                           | 씬의 일부에서 플레이어 이동 입력을 변경하거나 제한합니다.          | Decentraland | `이동`, `입력`, `플레이어`    | [씬 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier)         |
| [파티클 시스템](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/particle-system/)                         | 불, 연기, 불꽃, 눈 및 기타 파티클 효과.                 | Decentraland | `파티클`, `효과`, `비주얼`    | [씬 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system)        |
| [포인터 잠금 제어](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/pointer-lock-control/)                  | 사용자 지정 컨트롤을 위해 마우스 입력을 캡처합니다.             | Decentraland | `입력`, `마우스`, `카메라`    | [씬 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control) |
| [레이캐스팅 및 콜라이더 레이어](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/raycasting-and-collider-layers/) | 씬 지오메트리를 가리키고 어떤 콜라이더가 레이캐스트에 반응할지 제어합니다. | Decentraland | `상호작용`, `레이캐스트`, `충돌` | [씬 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests)   |
| [트리거 영역](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/trigger-areas/)                            | 콜라이더 기반 트리거를 포함해 플레이어의 영역 진입 및 이탈을 감지합니다. | Decentraland | `상호작용`, `트리거`, `충돌`   | [씬 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas)        |
| [좀비 공격](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/zombie-attack/)                             | 적과 점수가 있는 완전한 플레이 가능 게임 씬.                | Decentraland | `게임`, `완전한 씬`, `점수`   | [씬 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/zombie-attack)                    |

[모든 예제 씬 둘러보기](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
