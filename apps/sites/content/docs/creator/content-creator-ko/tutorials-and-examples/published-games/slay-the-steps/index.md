> 커스텀 아바타, 마켓플레이스 통합, 레이캐스트 기반 컬러 휠, 추적 카메라가 있는 멀티플레이어 페스티브 트레일 씬.

# 슬레이 더 스텝스

![Slay the Steps 미리보기](/docs/_remote/009ced18986123a766b6a4fb.webp)

Decentraland Festive Trail 2025를 위해 제작된 Studio 제작 멀티플레이어 서버 경험입니다. 맞춤 아바타, Marketplace API 연동, 그리고 맞춤 색상 휠 선택기를 포함합니다. 코드에서는 클라이언트, 서버, 공유 모듈을 분리하며, 배포 가능한 씬과 함께 소스 자산도 포함합니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=slaythesteps.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · stom66

[업스트림 소스 보기](https://github.com/stom66/dcl-slay-the-steps)

**라이선스:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-slay-the-steps/blob/main/LICENSE.md). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation과 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 주요 기능

* 시네마틱 **가상 카메라**
* **AvatarShapes** 웨어러블을 보여주기 위한
* **멀티플레이어** 라운드/플레이어 의상을 동기화하기 위한
* 맞춤형 3D 색상 선택기

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 3D 포인터 히트를 색상 선택으로 변환

`GetLocalMousePosition` 카메라에서 레이캐스트를 쏘아 히트 지점을 휠의 로컬 위치로 변환합니다. `SampleColorWheel` 각도와 반경을 색상과 채도에 매핑하고, 콜백을 호출하며, 값 슬라이더에 색조를 입힙니다.

**프로젝트 맥락:** 구현은 프로젝트의 휠 메시, HSV 변환 헬퍼, 그리고 머티리얼 노드 이름을 사용합니다.

**소스 참조:**

* [`GetLocalMousePosition`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L306-L335)
* [`SampleColorWheel`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L203-L238)

### 가상 카메라로 이동하는 대상을 추적하기

카메라 위치 시스템과 트윈 체인은 주기적으로 플레이어와 대상에 대한 상대 위치를 계산한 다음, 그쪽으로 트윈 이동합니다. `TrackEntity` 카메라를 다음과 함께 생성합니다 `lookAtEntity` 그리고 활성화합니다.

**프로젝트 맥락:** 읽기 `ResetCamera` 그리고 추적 수명 주기 종료 시 게임의 이벤트 바인딩.

**소스 참조:**

* [카메라 위치 시스템과 트윈 체인](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L46-L96)
* [`TrackEntity`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L99-L137)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/slay-the-steps.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
