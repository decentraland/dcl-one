> 서버가 운영하는 경기, 지속적인 리더보드, 단계 기반 카메라, 동기화된 공 애니메이션이 있는 멀티플레이어 페널티 슛아웃.

# 골 레전드

![Goal Legends 미리보기](/docs/_remote/aa680097357467a542aed322.webp)

멀티플레이어 페널티 슛아웃 게임입니다. 다른 플레이어 또는 GL-Bot과 1대1로 겨루세요: 왼쪽, 중앙, 오른쪽 중 하나를 골라 득점하거나 막아내세요. 각 라운드는 TV 중계 스타일의 리플레이로 끝나며, 관중의 반응이 실시간으로 나타납니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=goallegends.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · carlosmu

[업스트림 소스 보기](https://github.com/carlosmu/goal-legends-arena)

**라이선스:** [Apache-2.0 고지](https://github.com/carlosmu/goal-legends-arena/blob/master/LICENCE). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation과 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 주요 기능

* **멀티플레이어 서버** 경기를 실행합니다
* **저장소** 일간/역대 PvP/PvE 순위표와 각 플레이어의 국가 깃발을 위한.
* **씬** **이모트**
* 킥, 다이브, 골, 세이브에 맞춰 애니메이터와 동기화되는 공 프롭.
* **가상 카메라** 게임플레이 화면과 모두가 보는 승자 시네마틱을 위한.
* **아바타 형태** 각 골과 세이브에 반응하는 관중 NPC.
* **파티클** 불꽃놀이를 위한
* 조명과 함께 있는 포털 **`GltfNodeModifiers`** 당신을 다른 월드로 데려다주는.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 경기 단계에 따라 카메라를 전환

`initGameplayCamera` 클라이언트 경기 스냅샷에서 게임플레이, 승자, 또는 기본 카메라를 선택합니다. 승자 카메라는 피벗을 중심으로 회전하며, 관중은 경기 중 자유 카메라를 선택할 수 있습니다.

**프로젝트 맥락:** 이것을 다음과 함께 따라보세요 `clientSnapshot`, `GameState` 열거형과 명명된 씬 카메라 피벗.

**소스 참조:**

* [`initGameplayCamera`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/gameplayCamera.ts#L109-L165)

### 프롭 애니메이션을 게임 상태와 조율

`tickAnimationManager` 단계가 바뀌면 공 클립을 시작하고, 슛 방향에 따라 골/세이브 애니메이션을 선택하며, 클립을 전환하는 동안 프롭을 잠시 숨깁니다. 이 경로는 봇 킥커 전용이며, 사람 킥커는 이모트에서 공을 들고 있습니다.

**프로젝트 맥락:** 이것은 씬의 공 모델과 명명된 애니메이션 클립에 따라 달라집니다.

**소스 참조:**

* [`tickAnimationManager`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/animationManager.ts#L78-L114)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/goal-legends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
