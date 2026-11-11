> 트리거 영역에서 박수 이모트를 하면 플레이어가 획득하는 착용 아이템 배포기입니다.

# 박수로 받기 배포기

플레이어가 디스펜서 앞에서 박수 이모트를 하면 웨어러블을 획득할 수 있게 합니다. 이는 Metaverse Fashion Week에서 사용된 메커니즘입니다. README에서는 디스펜서를 자신의 Rewards 캠페인에 연결하는 과정을 안내합니다.

## 프로젝트 정보

**관리자:** Decentraland

[소스 보기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 플레이어가 앞에 서 있는 동안 디스펜서를 준비합니다

각 디스펜서는 해당 캠페인을 `DispenserComponent`. 구체 `TriggerArea` 앞에 두면 설정됩니다 `isActive` 그리고 진입 시 "clap to claim" 토스트를 표시하며, 종료 시 둘 다 지웁니다.

**프로젝트 맥락:** 캠페인 ID와 디스펜서 키는 `ClaimConfig` 의 `src/modules/claiming/claimConfig.ts`.

**소스 참조:**

* [`createDispenserWithWearable`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/dispensers.ts#L28-L89)

### 플레이어가 박수 이모트를 실행하면 획득합니다

`initClapToClaim` 를 감시합니다 `AvatarEmoteCommand` 플레이어 엔티티에서. 이 이모트가 `clap`, 애니메이션을 기다린 뒤 현재 활성화된 모든 디스펜서에서 획득합니다.

**프로젝트 맥락:** 청구 요청 및 응답 처리는 보상 청구 디스펜서 예제와 동일한 Rewards 흐름을 따릅니다.

**소스 참조:**

* [`initClapToClaim`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/clapToClaim.ts#L6-L33)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/rewards/clap-to-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
