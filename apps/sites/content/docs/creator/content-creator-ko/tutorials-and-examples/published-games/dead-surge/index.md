> 무기 진행, 풀링된 보상 텍스트, 다른 좀비와 장애물을 피하는 로컬 적 스티어링이 있는 좀비 생존.

# 데드 서지

![Dead Surge 미리보기](/docs/_remote/fbb6dfc582416446805d17d3.webp)

전투와 무기 성장 요소가 있는 좀비 생존 게임.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=deadsurge.dcl.eth)

## 프로젝트 정보

**관리자:** Decentraland DAO

[소스 보기](https://github.com/dcl-regenesislabs/dead-surge)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 떠다니는 보상 텍스트에 엔티티 재사용

`spawnZcRewardTextAtPosition` 그리고 해당 풀 초기화기는 고정된 텍스트 엔티티 집합을 순환합니다. 그 `rewardTextSystem` 엔티티가 위로 떠오르고, 카메라를 향하며, 사라진 뒤 재사용을 위해 숨겨지게 합니다.

**프로젝트 맥락:** 씬의 `RewardTextComponent` 그리고 게임 시간 업데이트를 이 함수들과 함께 살펴보면 수명 추적을 이해할 수 있습니다.

**소스 참조:**

* [`spawnZcRewardTextAtPosition` 그리고 해당 풀 초기화기](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L592-L640)
* [`rewardTextSystem`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L815-L850)

### 적들이 서로와 단순한 장애물을 피해 이동하도록 유도

`getZombieSeparationDirection` 주변 좀비로부터의 반발력을 결합합니다. `resolveZombieMovePosition` 원하는 방향, 대체 방향, 그리고 개별 이동 축을 원형 벽돌 장애물에 대해 시도합니다.

**프로젝트 맥락:** 이는 로컬 조향을 보여줍니다. 동작은 씬의 이동 항목, 장애물 위치, 그리고 반지름 상수에 따라 달라집니다.

**소스 참조:**

* [`getZombieSeparationDirection`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L868-L896)
* [`resolveZombieMovePosition`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L898-L939)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/dead-surge.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
