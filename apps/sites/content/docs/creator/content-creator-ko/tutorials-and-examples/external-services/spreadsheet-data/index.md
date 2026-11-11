> Google 시트로 구동되는 장면 콘텐츠로, 재배포 없이 업데이트됩니다.

# 스프레드시트의 데이터

Google Sheet에서 씬 콘텐츠를 제어합니다. 씬을 다시 배포하지 않고도 시트를 편집하여 아티스트 이름, 비디오 스트림, 특집 착용 아이템을 변경할 수 있습니다.

## 프로젝트 정보

**관리자:** Decentraland

[소스 보기](https://github.com/decentraland/Read_Spreadsheet_Scene)

**라이선스:** [Apache-2.0](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/main/LICENSE). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 게시된 Google Sheet를 JSON으로 읽기

`getSpreadsheetData` Google Visualization 엔드포인트(`gviz/tq?tqx=out:json`)를 통해 시트를 요청하고, `setResponse(...)` 래퍼를 제거한 다음 각 행을 열 헤더를 키로 하는 객체로 변환합니다.

**프로젝트 맥락:** 시트는 링크가 있는 누구나 볼 수 있도록 공유되어 있어야 합니다. `gid` 탭을 선택하고 `headers=1` 첫 번째 행을 열 이름으로 취급합니다.

**소스 참조:**

* [`getSpreadsheetData`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/scheduler.ts#L1-L47)

### 행을 씬에 적용

`updateFromSpreadsheet` 한 행을 읽고 배너를 업데이트합니다 `TextShape`, 화면의 `VideoPlayer` 소스를 교체하고, 디스펜서에 표시된 착용 아이템을 바꾸며, 디스펜서의 클릭 동작이 해당 행의 구매 링크를 가리키도록 합니다.

**프로젝트 맥락:** 다음과 같은 열 이름은 `NowPlaying`, `VideoURL`, `WearableURN`, 그리고 `BuyURL` 시트 헤더와 일치해야 합니다. 엔티티는 Creator Hub 씬에 지정된 이름으로 조회됩니다.

**소스 참조:**

* [`updateFromSpreadsheet`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L10-L64)
* [`createWearable`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L100-L130)

### Creator Hub 버튼으로 행을 순환하기

`main` 다음을 수신합니다 `다음` 및 `이전` 의 동작을 `아티스트 전환` 엔티티에서 행 인덱스를 다운로드한 데이터 범위 내에서 순환시킵니다.

**프로젝트 맥락:** 사용합니다 `getActionEvents` 에서 `@dcl/asset-packs`, 따라서 동작은 Creator Hub의 엔티티에 구성됩니다.

**소스 참조:**

* [`main`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L69-L97)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/external-services/spreadsheet-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
