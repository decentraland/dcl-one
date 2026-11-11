> Rewards 캠페인에서 보상을 수령하는 착용 아이템 배포기와 에어드롭 상자입니다.

# 보상 수령 배포기

Decentraland Rewards 캠페인에서 플레이어에게 착용 아이템을 제공합니다. 씬에는 클릭하여 수령하는 디스펜서, 버튼 디스펜서, 그리고 수령 가능한 상자를 에어드롭하는 레버가 포함되어 있습니다.

## 프로젝트 정보

**관리자:** Decentraland

[소스 보기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim)

**라이선스:** [Apache-2.0](https://github.com/decentraland/sdk7-goerli-plaza/blob/main/reward-claim/LICENSE). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.

## 코드 살펴보기

이 링크들은 특정 소스 스냅샷을 가리키므로, 상위 개발이 계속되더라도 설명이 유용하게 유지됩니다. 이는 학습용 참고 자료이지, 보안 평가는 아닙니다.

### 서명된 fetch로 보상을 요청하기

`claimToken` 반복 클릭을 제한하고, 이 세션에서 이미 성공적으로 수령한 디스펜서는 건너뛰며, 플레이어에게 지갑이 있는지 확인합니다. `requestToken` 그런 다음 디스펜서 키, 현재 촉매제, 그리고 플레이어의 주소를 Rewards 서버에 게시합니다. 를 사용하여 `signedFetch`.

**프로젝트 맥락:** 테스트는 기본적으로 활성화되어 있습니다(`CONFIG_CLAIM_TESTING_ENABLED` 의 `src/config.ts`), 번들로 포함된 테스트 캠페인을 사용합니다. 직접 사용하려면, 다음에서 자리표시자 캠페인 ID와 디스펜서 키를 바꾸세요 `ClaimConfig` 그리고 테스트를 끄세요. 캠페인과 디스펜서를 만드는 방법은 Rewards 시작하기 가이드를 참조하세요.

**소스 참조:**

* [`claimToken` 및 `requestToken`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L26-L99)
* [`ClaimConfig`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claimConfig.ts#L21-L34)

### Rewards 응답을 플레이어 피드백에 매핑하기

`processResponse` 사용할 수 없는 캠페인, 재고가 없는 디스펜서, 이미 할당된 보상, 그리고 성공적인 수령에 대해 서로 다른 디스펜서 UI를 표시합니다. 시작되지 않음, 비활성화됨, 종료된 캠페인은 하나의 분기로 처리되며, 캠페인 config가 startDate 또는 endDate를 설정한 경우에만 시작되지 않음 또는 종료됨 화면을 표시하고, 그렇지 않으면 일반 종료 화면을 표시합니다. `startDate` 또는 `endDate`, 그 외에는 일반 종료 화면을 표시합니다.

**프로젝트 맥락:** UI 상태는 `dispenserUi.tsx`. 캠페인 시작 및 종료 날짜는 캠페인 config의 선택적 필드입니다.

**소스 참조:**

* [`processResponse`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L101-L197)

### 캡차로 수령을 다시 시도하기

Rewards 서버가 캡차를 요청하면, `getCaptcha` 캡차 이미지를 가져와 디스펜서 UI에 표시합니다. 답안을 제출하면 `claimToken` 다시 캡차 ID와 값으로 호출합니다.

**프로젝트 맥락:** 캡차가 필요한지는 Rewards 대시보드에서 디스펜서에 대한 설정입니다.

**소스 참조:**

* [`getCaptcha`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L209-L216)
* [캡차 제출 버튼](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/ui/dispenserUi.tsx#L640-L643)

### 에어드롭된 상자에서 수령하기

`randomCrateSpawn` 무대 영역의 무작위 지점에 낙하산이 달린 상자를 떨어뜨립니다. `DropCrate` 아래로 레이캐스트해 바닥을 찾고 상자를 그 위로 트윈 이동시키며, 상자를 클릭하면 보상을 수령하고 상자가 사라집니다.

**프로젝트 맥락:** 상자 드롭은 `Activate` main의 Creator Hub 레버 작업에 의해 트리거됩니다. `main`.

**소스 참조:**

* [`randomCrateSpawn` 및 `SpawnCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L40-L119)
* [`DropCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L142-L189)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/rewards/reward-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
