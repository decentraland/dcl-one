> DAO는 Decentraland의 가장 중요한 스마트 계약을 소유하고 통제합니다.

# DAO 스마트 계약

Decentraland DAO는 Decentraland 플랫폼의 가장 중요한 스마트 계약 몇 가지를 소유하고 있습니다. 아래에 나열되어 있습니다:

## ⛰️ LAND 계약

이 계약은 LAND 토큰을 관리하는 계약입니다. DAO는 LAND 스마트 계약의 소유자입니다. 이는 해당 계약에 대한 어떠한 변경이나 수정도 DAO와 SAB가 수행해야 함을 의미합니다.

## 🏘️ 에스테이트 계약

LAND 계약과 마찬가지로, DAO는 에스테이트 계약을 소유하고 있으며, 이 계약은 DAO가 커뮤니티 투표를 통해 승인을 한 후에만 SAB가 수정할 수 있습니다.

## 📍 POI

관심 지점(Points of Interest) 목록(Decentraland의 주목할 만한 장소로, 가상 세계를 탐험하기 시작하기에 좋은 곳으로 사용자에게 안내되는 장소) 또한 DAO가 소유하고 있습니다. 이 목록은 계약에 저장되며, 커뮤니티 투표를 통과한 후 DAO 위원회가 온체인에서 이를 실행한 뒤에만 수정할 수 있습니다.

## 🏷️ 이름

Decentraland에서 고유한 아바타 이름에 대한 NFT를 발행하는 데 사용되는 계약은 DAO가 소유하고 관리합니다. 이름 계약에 대한 어떠한 변경도 DAO의 승인을 받아야 합니다.

## 🚫 차단된 이름

Decentraland 클라이언트에서 차단된 이름 목록은 DAO가 소유한 계약에 저장됩니다. 이 목록은 커뮤니티 투표를 통과한 후 DAO 위원회가 온체인에서 이를 실행한 뒤에만 수정할 수 있습니다.

## 🌐 카탈리스트 노드

콘텐츠를 제공하고 Decentraland의 가상 세계를 계속 운영하는 데 필요한 P2P 연결을 구축하는 Catalyst 노드 목록 또한 DAO가 소유하고 관리합니다. 이 목록은 계약에 저장되며, 커뮤니티 투표를 통과한 후 DAO 위원회가 온체인에서 이를 실행한 뒤에만 수정할 수 있습니다.

## 👕 웨어러블 컬렉션

Decentraland에서는 웨어러블을 발행하기 전에 컬렉션으로 그룹화할 수 있습니다. 웨어러블 컬렉션을 관리하는 데 사용되는 계약은 DAO가 소유하고 관리합니다.

## 🛒 마켓플레이스 계약

Decentraland 마켓플레이스 dApp은 LAND, 에스테이트, 기타 NFT의 판매 및 입찰 과정을 관리하기 위해 여러 스마트 계약을 사용합니다. 이러한 계약에는 마켓플레이스 수수료도 정의되어 있으며, DAO의 승인이 있어야만 변경할 수 있습니다.

## 💰 보조금

DAO의 보조금 프레임워크의 일부로 정기 지급을 수행하는 데 사용되는 베스팅 계약 또한 DAO가 소유하고 있습니다. 이러한 계약은 DAO를 대신하여 DAO 위원회가 생성하며, 위원회의 실수나 취약점으로 인한 금전적 손실 위험을 방지하기 위해 SAB가 감독합니다. 모든 베스팅 계약의 상태는 [이 도구](https://vesting.decentraland.org/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/dao/dao/what-smart-contracts-does-the-dao-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
