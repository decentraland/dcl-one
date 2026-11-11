> SDK7를 사용해 보고 피드백을 제공하는 방법에 대한 가이드라인

# 테스트 및 피드백

## 문서

SDK7의 전체 문서는 다음의 **SDK7** 문서 섹션에서 찾을 수 있습니다.

## 플레이그라운드 사용하기

SDK7을 쉽게 체험하려면 다음을 방문하세요: [플레이그라운드](https://playground.decentraland.org/). 화면 왼쪽에 SDK7 코드를 작성하고, 오른쪽에서 생성된 장면을 확인하세요. 코드를 변경하면 장면이 자동으로 업데이트됩니다.

문제가 발생하면, 다음을 사용하여 현재 장면 설정의 URL을 복사한 뒤 **공유** 버튼을 클릭하고, [문제를 신고하세요](#reporting-issues) 해당 링크를 코드에 포함하세요.

## SDK 7 사용하기

먼저 [크리에이터 허브](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/).


> [!WARNING]
> **📔 참고**: SDK7을 사용하는 장면에는 `decentraland-ecs` 패키지가 설치되어 있지 않습니다. 대신 다음 `@dcl/sdk` 패키지가 있습니다.


## 프로덕션 환경에서

SDK7이 포함된 장면은 Creator Hub의 Scene Editor에 있는 **게시** 버튼을 통해 배포할 수 있습니다.

## 예제 장면

SDK7로 작성된 더 많은 예제 장면은 여기에서 확인할 수 있습니다:

* [장면 예제](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)

## 문제 신고

문제가 발생하면 다음을 참조하세요 [버그 신고](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/api/alpha-testing-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
