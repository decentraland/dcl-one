> 게시한 씬의 플레이어 방문, 참여, 유지율을 추적합니다

# 씬 분석

크리에이터에게는 플레이어의 방문과 플레이어가 씬과 상호작용하는 방식을 추적하는 것이 매우 중요합니다. 추측하는 대신, 실제 데이터를 얻어 디자인을 반복 개선하면서 정보에 기반한 결정을 내릴 수 있습니다.

해당 [크리에이터 허브](https://decentraland.org/create) 에는 **분석** 섹션이 있으며, 게시했거나 권한이 있는 모든 씬의 자세한 지표를 제공합니다. 씬에 아무것도 추가할 필요가 없으며, 이 데이터는 게시된 모든 씬에 대해 자동으로 수집됩니다.


> [!NOTE]
> **📔 참고**: 분석 데이터는 하루에 한 번 업데이트됩니다. 씬의 소유자와 운영자만 해당 데이터를 볼 수 있습니다.


## 씬의 분석 보기

1. 크리에이터 허브를 열고 계정으로 로그인하세요.
2. 다음을 **분석** 상단 탐색 바에서 탭을 선택하세요. 소유하거나 운영하는 모든 게시된 씬 목록이 표시되며 각 씬의 주요 지표를 볼 수 있습니다.
3. 씬을 클릭하면 지표 전체가 탭으로 정리된 상세 페이지가 열립니다.

데이터는 씬별로 추적됩니다. 하나의 월드에 여러 씬이 있으면 각 씬은 고유한 지표와 함께 목록에 별도의 항목으로 표시됩니다.

## 씬 목록

목록에는 씬당 한 행이 표시되며, 다음 필드가 포함됩니다:

* **고유 방문**: 씬을 방문한 서로 다른 사용자로, 각 사용자당 한 번만 집계됩니다.
* **신규 사용자**: 처음으로 씬을 방문한 사용자입니다.
* **7일 유지율**: 첫 방문 후 7일 동안 최소 한 번 이상 씬으로 돌아온 신규 사용자의 비율입니다.
* **수익**: 씬에서 획득한 MANA입니다.
* **중간 플레이 시간**: 각 사용자가 하루에 씬에서 보내는 시간의 중간값입니다. 일일 방문의 절반은 더 길고, 절반은 더 짧습니다.

목록을 정렬하거나 이름으로 검색할 수 있으며, 가장 중요한 씬을 상단에 고정할 수 있습니다.

## 상세 페이지

목록에서 아무 씬이나 클릭하면 해당 씬의 상세 페이지가 열립니다. 지표 옆에는 씬에 대한 일반 정보 카드가 있으며, 좋아요 비율, 공개/비공개 접근 여부, 게시된 월드, 마지막으로 게시한 사람, 마지막 업데이트 시점을 확인할 수 있습니다. 또한 이 페이지에서 씬의 URL을 복사하거나, 씬으로 바로 이동하거나, 이벤트를 직접 생성할 수도 있습니다.

다음을 사용하세요 **날짜 범위** 선택기를 사용해 지난 7일, 30일 또는 60일 동안의 지표를 계산하고, **분석 내보내기** 버튼을 눌러 데이터를 .csv로 다운로드하세요.

유지율 코호트, 주간 활성 사용자, 사회적 상호작용 방문자와 같은 주차별 차트는 선택한 날짜 범위와 관계없이 항상 최근 16개의 완전한 주를 포함합니다.

아직 데이터가 없는 지표는 대시 (`-`)로 표시되며 숫자가 아닙니다.

지표는 네 개의 탭으로 구성되어 있습니다.

### 개요

씬의 성장, 참여도, 사용자 유지율을 한눈에 볼 수 있는 요약입니다.

* **총 방문**: 같은 사용자의 재방문을 포함한 씬의 모든 방문입니다.
* **고유 방문**: 씬을 방문한 서로 다른 사용자로, 각 사용자당 한 번만 집계됩니다.
* **신규 사용자**: 처음으로 씬을 방문한 사용자입니다.
* **동시 사용자**: 동시에 씬에 있었던 사용자 수의 최댓값입니다.
* **수익**: 씬에서 획득한 MANA입니다.
* **7일차 유지율**: 첫 방문 후 7일 동안 최소 한 번 이상 씬으로 돌아온 신규 사용자의 비율입니다.
* **중간 플레이 시간**: 각 사용자가 하루에 씬에서 보내는 시간의 중간값입니다. 일일 방문의 절반은 더 길고, 절반은 더 짧습니다.
* **자리 비움 시간**: 사용자가 상호작용 없이 씬에 머무는 평균 시간입니다.
* **데스크톱 / 모바일**: 데스크톱 또는 모바일 클라이언트에서 씬을 방문한 사용자입니다.

### 방문

몇 명의 사용자가 씬을 방문했는지, 그리고 그중 신규 방문자와 재방문자가 몇 명인지 보여줍니다.

* **최근 60일간 고유 방문**: 플랫폼별로 구분한 고유 방문자 수입니다(전체 플랫폼, 데스크톱, 모바일). 사용자는 플랫폼 간 중복 제거됩니다. 데스크톱과 모바일 모두에서 플레이한 사람은 전체 플랫폼 합계에서는 한 번, 각 플랫폼 합계에서도 각각 한 번씩 집계됩니다.
* **주간 활성 사용자**: 매주 활동한 고유 사용자 수입니다.
* **주간 사용자 흐름**: 주간 사용자가 다음 항목으로 어떻게 나뉘는지:
  * **신규 사용자**: 처음으로 씬을 방문한 사용자입니다.
  * **재방문 사용자**: 지난주에 방문했고 이번 주에 다시 돌아온 사용자입니다.
  * **재활성 사용자**: 방문을 중단했다가 이번 주에 돌아온 사용자입니다.

### 참여도

사용자가 씬에 들어온 뒤 어떻게 상호작용하는지 보여줍니다.

* **평균 일일 플레이 시간**: 각 사용자가 한 번 방문할 때 씬에서 보내는 평균 시간입니다. 주차별 차트와 이전 주 대비 변화가 함께 표시됩니다.
* **평균 주간 플레이 시간**: 각 사용자가 주당 씬에서 보내는 평균 시간입니다.
* **사회적 상호작용**: 세 가지 사회적 신호로, 주간 합계와 해당 주 방문자 중 각 행동을 한 비율 모두로 표시됩니다:
  * **전송된 메시지**: 씬 방문자가 보낸 채팅 메시지입니다.
  * **사용된 이모트**: 씬 방문자가 사용한 이모트입니다.
  * **새로운 친구 관계**: 씬 방문자 사이에서 수락된 친구 요청입니다.

### 유지율

첫 세션 이후 다시 돌아오는 사용자의 비율입니다.

* **플랫폼**: 데스크톱과 모바일을 비교한 60일 이동 평균 유지율입니다.
* **코호트 주차별 7일차 유지율**: 처음 씬에 들어온 주차별로 그룹화한, 첫 방문 후 7일 동안 최소 한 번 이상 돌아온 사용자의 비율입니다. 차트에는 건강한 기준값으로 20% 벤치마크 선이 표시됩니다.
* **주간 이탈률**: 지난주 활성 사용자 중 이번 주에 돌아오지 않은 비율입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/other/scene-analytics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
