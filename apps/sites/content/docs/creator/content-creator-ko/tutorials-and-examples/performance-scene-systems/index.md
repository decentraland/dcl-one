> 에셋 로딩 및 장면 수준 시스템.

# 성능 및 씬 시스템

에셋 로딩 및 씬 수준 시스템.

| 예시                                                                                                                             | 보여주는 내용                         | 관리 주체        | 태그                   | 소스                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------- | ------------ | -------------------- | ------------------------------------------------------------------------------------------------- |
| [에셋 로딩](/docs/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems/asset-loading/)                         | 씬 에셋을 한꺼번에가 아니라 필요할 때 로드합니다.    | Decentraland | `성능`, `로딩`, `에셋`     | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load)      |
| [머티리얼 및 텍스처 갤러리](/docs/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems/material-and-texture-gallery/) | 머티리얼, 텍스처 및 GLTF 노드 수정자 패턴.     | Decentraland | `머티리얼`, `텍스처`, `렌더링` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) |
| [수정자 영역](/docs/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems/modifier-areas/)                       | 플레이어 이름표 숨기기와 같은 영역별 설정을 적용합니다. | Decentraland | `플레이어`, `영역`, `수정자`  | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas)    |

[모든 예시 장면 둘러보기](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
