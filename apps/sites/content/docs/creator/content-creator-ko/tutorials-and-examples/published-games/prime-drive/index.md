> 재사용된 배경, 거리 기반 기록 유령, 지속적인 리더보드 데이터가 있는 3레인 레이싱.

# 프라임 드라이브

![프라임 드라이브 미리보기](/docs/_remote/bba6ea4b5469647296b97b46.webp)

플레이어가 장애물을 피하고, 코인을 수집하며, 10km 코스에서 트랙 기록 고스트를 쫓는 3개 차선 자전거 경주 게임.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=primedrive.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · carlosmu

[업스트림 소스 보기](https://github.com/carlosmu/dcl-prime-drive)

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

### 재활용된 배경으로 긴 레이스 만들기

`setWorldOffset` 및 `scrollTrack` 차선 변경을 위해 공유 월드 루트를 옆으로 이동시키고, 도로 장식이 플레이어를 지나가면 다시 앞쪽으로 되돌립니다. 자전거는 제자리에 있고, 배경이 움직임의 느낌을 제공합니다.

**프로젝트 맥락:** 링 빌더와 공유 항목을 읽기 `TRACK`/`SPAWN` 간격과 래핑 거리를 이해하기 위한 설정.

**소스 참조:**

* [`setWorldOffset` 및 `scrollTrack`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/track.ts#L43-L92)

### 플레이어 기준으로 기록 고스트 표시

`updateGhosts` 거리 간격에 따라 고스트 자전거를 배치하고, 시야 범위 밖의 고스트를 숨기며, 플레이어에서 떨어진 차선으로 부드럽게 이동시킵니다.

**프로젝트 맥락:** 이 모듈은 다음을 사용합니다 `GhostRunView` 데이터; 제공된 진행 상황을 표시할 뿐, 자체적으로 리플레이를 기록하지는 않습니다.

**소스 참조:**

* [`updateGhosts`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/ghost.ts#L129-L183)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/prime-drive.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
