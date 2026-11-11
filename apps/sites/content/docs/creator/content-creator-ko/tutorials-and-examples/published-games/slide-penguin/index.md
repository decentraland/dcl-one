> 목표물과의 거리 기반으로 감속과 점수를 변경하는 시간 제한 쓸기 동작이 있는 펭귄 미끄러지기.

# 슬라이드 펭귄

펭귄을 목표를 향해 미끄러지게 하고, 시간에 맞춘 스와이프로 얼마나 빨리 감속되는지에 영향을 줍니다.

배포 참고: 현재 배포되지 않았습니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?position=-140%2C101)

## 프로젝트 정보

**관리자:** 커뮤니티 · ToxSam

[업스트림 소스 보기](https://github.com/ToxSam/SlidePenguin)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation과 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 스와이프를 일시적인 이동 수정자로 전환하기

`handleSwipe` 감속 감소에 쿨다운을 적용하고, 해당 효과가 만료되는 시점을 기록합니다. `penguinMovementSystem` 해당 시간창이 지나면 기본 감속을 복원하고, 속도를 업데이트하며, 프레임 시간을 사용해 트랜스폼을 진행합니다.

**프로젝트 맥락:** 프로젝트의 `PenguinMovement` 및 `SwipeBox` 입력과 움직임 사이의 연결을 이해하기 위한 컴포넌트와 조정 상수.

**소스 참조:**

* [`handleSwipe`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L519-L598)
* [`penguinMovementSystem`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L864-L913)

### 오브젝트가 목표에 얼마나 가깝게 착지했는지 점수화하기

`calculateDistance2D` 및 `calculateScore` 높이는 무시하고, 수평 거리를 최대 거리 기준으로 정규화한 뒤, 반올림된 점수로 매핑하며 그 상한은 `MAX_SCORE`.

**프로젝트 맥락:** 이 작은 모듈은 근접 점수를 이해하기 위한 유용한 출발점입니다. 입력은 최종 펭귄 위치와 목표 위치입니다.

**소스 참조:**

* [`calculateDistance2D` 및 `calculateScore`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/scoring.ts#L5-L39)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/slide-penguin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
