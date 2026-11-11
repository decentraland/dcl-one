> 차지된 공 킥, 즉각적인 로컬 이동 피드백, 반응형 일정 패널이 있는 월드컵 예측 씬.

# 킥 오프

![킥오프 미리보기](/docs/_remote/e381e39b01875ac25f3f33f9.webp)

시즌별 월드컵 예측 게임.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=kickoff.dcl.eth)

## 프로젝트 정보

**관리자:** Decentraland DAO

[소스 보기](https://github.com/dcl-regenesislabs/kickoff-2026)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 충전된 킥을 즉각적인 시각적 피드백과 연결합니다

충전-해제 루프는 키보드와 모바일 입력을 결합하고, 충전 바를 확대하며 색을 바꾸고, 킥 메시지를 전송하며, 로컬 공의 움직임을 시작합니다. `releaseOptimistic` 그 움직임을 초기화합니다.

**프로젝트 맥락:** 프로젝트의 `prodeNet` 룸 및 공 메시지 핸들러를 살펴보며 입력, 피드백, 멀티플레이어 메시지 간의 관계를 이해하세요.

**소스 참조:**

* [충전-해제 루프](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L491-L536)
* [`releaseOptimistic`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L155-L178)

### 기준 캔버스를 기준으로 UI를 스케일링

`layoutScale` 읽습니다 `UiCanvasInformation`, 두 차원을 1920×1080 디자인 영역과 비교하고, 결과를 제한한 뒤 추가적인 모바일 계수를 적용합니다.

**프로젝트 맥락:** 이것은 공유 레이아웃 헬퍼의 간결한 예시입니다. 이 프로젝트의 패널에 맞게 제한값과 모바일 계수가 조정되어 있습니다.

**소스 참조:**

* [`layoutScale`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/schedule/responsive.ts#L1-L21)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/kick-off.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
