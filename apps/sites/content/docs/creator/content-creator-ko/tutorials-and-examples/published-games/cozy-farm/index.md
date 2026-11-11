> 이동을 위한 가속과 제동이 있는 펫 수집, 그리고 에디터에서 배치한 알의 단계 오프셋 애니메이션.

# 코지 팜

![Cozy Farm 미리보기](/docs/_remote/f005f5c4d6dce02d9535e094.webp)

작물을 키우고, 농장을 확장하며, 다른 플레이어를 만나는 데 중점을 둔 소셜 농장 게임입니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=cozyfarm.dcl.eth)

## 프로젝트 정보

**관리자:** Decentraland DAO

[소스 보기](https://github.com/dcl-regenesislabs/cozy-farm)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 경과 시간을 작물 성장 단계로 전환

`growthSystem` 심기 타임스탬프에서 진행도를 계산하고, 튜토리얼 작물과 비료의 지속 시간을 조정하며, 단계가 바뀔 때만 모델을 교체합니다.

**프로젝트 맥락:** 함께 따라가 보세요 `PlotState`, `CROP_DATA`, 그리고 모델 헬퍼에서 `game/actions` 시뮬레이션 상태가 시각적 피드백을 어떻게 이끄는지 보세요.

**소스 참조:**

* [`growthSystem`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/growthSystem.ts#L62-L142)

### 하나의 엔티티 계층 구조로 여러 농장을 구축하기

`cloneEntityTree` 템플릿의 자식을 재귀적으로 다시 만들고, 로컬 변환을 유지하며, 선택한 렌더링 컴포넌트를 복사하고, 각 복제본을 이름으로 기록합니다. 이는 반복되는 영역이 제작된 레이아웃을 공유할 수 있음을 보여줍니다.

**프로젝트 맥락:** 이 헬퍼는 씬의 자식 맵을 기대하고 명시적인 컴포넌트 목록을 복사하므로, 새로운 컴포넌트 유형은 의도적인 처리가 필요합니다.

**소스 참조:**

* [`cloneEntityTree`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/farmInstances.ts#L127-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/cozy-farm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
