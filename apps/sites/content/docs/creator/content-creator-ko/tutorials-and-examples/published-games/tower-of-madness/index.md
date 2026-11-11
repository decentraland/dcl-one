> 변하는 탑 구간을 통과하는 시간 제한 플랫폼 게임으로, 에셋 사전 로딩과 레벨의 건너뛸 수 있는 카메라 투어가 있습니다.

# 광기의 탑

![Tower of Madness 미리보기](/docs/_remote/ec24d5b235cc62f71cd1eae3.webp)

플레이어들이 정상에 도달하기 위해 경쟁하는 시간 제한 플랫폼 게임.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=towerofmadness.dcl.eth)

## 프로젝트 정보

**관리자:** Decentraland DAO

[소스 보기](https://github.com/dcl-regenesislabs/towerofmadness)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 변하는 레벨의 조각들을 미리 로드하기

`setupChunkPreload` 청크 에셋 경로를 중복 제거하고 임시 GLTF 엔티티를 생성합니다. 시스템이 이들의 로딩 상태를 감시하고, 로딩이 완료되거나 실패하면 엔티티를 제거하며, 남은 것이 없으면 스스로 등록 해제합니다.

**프로젝트 맥락:** 이것은 프리로드 라이프사이클을 관리하는 방법을 보여줍니다. 에셋 목록은 다음에서 가져옵니다 `shared/chunks` 그리고 타워 전용입니다.

**소스 참조:**

* [`setupChunkPreload`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/chunkPreload.ts#L22-L61)

### 변하는 지형을 따라 건너뛸 수 있는 카메라 투어를 구성하기

`playCinematic` 현재 시작 및 종료 청크를 기준으로 카메라 키프레임을 조정한 다음 가상 카메라를 선택합니다. 업데이트 시스템은 위치와 바라볼 대상을 보간하며, 버튼을 길게 눌러 건너뛰기를 지원합니다.

**프로젝트 맥락:** 청크 조회 헬퍼를 따라가고 `skipCinematic` 투어가 앵커를 찾고 플레이어 카메라에 제어를 되돌려주는 방식을 확인해 보세요.

**소스 참조:**

* [`playCinematic`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L110-L209)
* [업데이트 시스템](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L286-L334)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/tower-of-madness.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
