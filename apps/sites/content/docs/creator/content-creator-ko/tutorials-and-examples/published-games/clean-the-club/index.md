> 공유 컴포넌트 폴링과, 들고 있는 아이템을 반영하는 아바타 부착형 소품이 있는 협동 나이트클럽 청소.

# 클린 더 클럽

![Clean The Club 미리보기](/docs/_remote/a8f103a70354f2dfa74d73d7.webp)

플레이어들이 다음 파티 전에 나이트클럽을 복구하기 위해 경쟁하는 협동 청소 게임.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=cleantheclub.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · CleanTheClub

[상위 소스 보기](https://github.com/CleanTheClub/CleanTheClub)

**라이선스:** [Apache-2.0](https://github.com/CleanTheClub/CleanTheClub/blob/main/LICENSE). 재사용하기 전에 상위 원본의 조건을 확인하세요. 코드와 에셋의 조건은 다를 수 있습니다.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation 및 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 실시간 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점의 경우 상위 프로젝트의 보안 정책을 따르세요. 공개 이슈에는 익스플로잇 세부 정보나 인증 정보를 포함하지 마세요.


## 코드 살펴보기

이 링크들은 특정 소스 스냅샷을 가리키므로, 상위 개발이 계속되더라도 설명이 유용하게 유지됩니다. 이는 학습용 참고 자료이지, 보안 평가는 아닙니다.

### 여러 시스템에서 하나의 컴포넌트 스캔을 공유

onClutterPoll은 폴링 간격마다 ClutterSync 항목을 한 번 수집하고, 아이템 ID 조회 테이블을 만든 다음, 동일한 스냅샷을 구독자에게 전달합니다. 각 소비자는 자체적으로 필터링과 변경 감지를 수행할 수 있습니다.

**프로젝트 맥락:** 이는 반복되는 ECS 스캔을 피하는 방법을 연구하는 데 유용합니다. 이는 공유 스키마와 phase-gate 폴링 간격에 의존하며, 구독은 클라이언트 초기화에 포함되어야 합니다.

**소스 참조:**

* [onClutterPoll](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/clutterWatcher.ts#L15-L54)

### 다른 플레이어가 들고 있는 장비를 표시

updateRemoteCarry는 소품 계층 구조를 다른 아바타의 왼손에 부착하고, 현재 장비 또는 운반 상태에서 모델을 선택하며, 가방을 채움 비율에 따라 크기 조정합니다. 또한 표시할 것이 없을 때는 계층 구조를 제거합니다.

**프로젝트 맥락:** 이것은 carry 메시지 처리기, 테마 모델 조회, 모델 맞춤 오프셋과 함께 읽으세요. 그것들이 상태와 에셋별 위치 정보를 제공합니다.

**소스 참조:**

* [updateRemoteCarry](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/carrySystem.ts#L805-L846)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/clean-the-club.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
