# 개발

- [플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/): 씬이 모바일, 데스크톱 또는 웹에서 실행 중인지 감지합니다.
- [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/): Creator Hub 또는 CLI에서 실제 모바일 기기에서 씬을 미리 봅니다.
- [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/): 씬 UI가 모바일에서 안전하게 배치될 수 있는 영역 — 기기의 하드웨어 여백과 클라이언트 자체 컨트롤을 피한 곳.
- [UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/): 모바일에서 잘 작동하는 씬 UI를 디자인하기 위한 권장 사항과 금지 사항입니다.
- [모바일 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/): 입력 동작이 Decentraland 모바일 클라이언트의 터치와 어떻게 매핑되는지.
- [성능](/docs/creator/content-creator-ko/build-for-mobile/develop/optimize-performance/): Decentraland 씬의 모바일 제한 및 성능 목표, 그리고 Creator Hub에서 이를 미리 보고 측정하는 방법입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
