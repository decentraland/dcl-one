> SDK7 씬을 위한 재사용 가능한 NPC 동작과 대화 UI.

# NPC 툴킷

SDK7 씬에 재사용 가능한 NPC 동작과 대화 UI를 사용하세요.

## 프로젝트 정보

**관리자:** Decentraland

[소스 보기](https://github.com/decentraland-scenes/dcl-npc-toolkit)

**라이선스:** [Apache-2.0](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/main/LICENSE). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 대화 표시와 진행을 분리하기

`handleDialogClick`, `rushText`, 그리고 `confirmText` 텍스트가 아직 입력 중이라면 먼저 남은 문자를 표시합니다. 이후 입력은 스크립트를 진행하거나 숫자 대화 분기를 선택하며, 게임 동작을 위한 콜백을 포함합니다.

**프로젝트 맥락:** 이것을 툴킷의 `대화` 데이터와 `npcDialogComponent` 상태와 함께 읽어 UI 입력이 대화를 어떻게 구동하는지 이해하세요.

**소스 참조:**

* [`handleDialogClick`, `rushText`, 그리고 `confirmText`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/dialog.ts#L489-L549)

### 근접 및 포인터 입력을 NPC 상호작용에 연결하기

`addClickReactions` 및 `addTriggerArea` 클릭 전용 활성화와 플레이어가 멀어졌을 때 대화를 계속하는 설정을 통해 포인터 콜백과 하위 구형 트리거를 구성합니다. `활성화` 및 `endInteraction` 활성 NPC, 쿨다운, 대화, 그리고 선택적 플레이어 대상 동작을 조정합니다.

**프로젝트 맥락:** 이 함수들은 툴킷의 콜백 맵과 추적 시스템에 의존합니다.

**소스 참조:**

* [`addClickReactions` 및 `addTriggerArea`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L410-L531)
* [`활성화` 및 `endInteraction`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L769-L823)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/npc-toolkit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
