> Decentraland Rewards 서비스는 HTTP 요청을 통해 플레이어에게 웨어러블과 이모트를 민팅하고 전달하며, 트랜잭션과 수수료를 처리합니다.

# 개요

해당 [보상 dApp](https://decentraland.org/rewards/) HTTP 요청을 통해 크리에이터가 Decentraland Wearables 또는 이모트로 사용자에게 보상을 제공할 수 있게 해주는 강력한 도구입니다. 사용자의 참여를 유도하고, 씬을 홍보하거나, 유지율을 높이는 데 사용할 수 있습니다. 보상은 씬에서 직접 또는 서버에서 부여할 수 있습니다.

보상 시스템은 블록체인에서 아이템을 민팅하는 데 관련된 대부분의 복잡성을 관리하며, 여기에는 다음이 포함됩니다:

* 여러 개의 민팅된 아이템을 하나의 거래로 묶어 대기 시간과 수수료를 줄임
* 거래 수수료 결제 처리
* 블록체인 오류 또는 재정렬 발생 시 재시도 관리
* 사용자 주소당 민팅되는 아이템 수 제한
* 캡차 시스템을 통해 자동화 악용 최소화
* Catalyst 노드를 통해 씬 내 사용자 존재 여부 확인
* 거래 상태 모니터링
* 거래가 확인되기 전에 사용자가 받게 될 아이템에 대해 안내
* 보상이 부여되고 수신될 때 알림을 전송합니다
* ![](/docs/_remote/0aefd219f02433f2b3736381.webp)

## 제한 사항

이 서비스는 한 사람이 웨어러블을 대량으로 민팅하는 것을 방지하기 위한 일부 자동화 보호 기능을 제공하지만, 악용을 100% 완전히 막을 수는 없습니다. **희귀도가 \[EPIC]\(**[**문서 보기**](/docs/creator/content-creator-ko/readme/)**#rarity)보다 더 희귀한 수준(100개 이하의 복사본)으로 Wearables/Emotes를 민팅하는 데 Rewards를 사용하는 것은 권장되지 않습니다.**.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/rewards/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
