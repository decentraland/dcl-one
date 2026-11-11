> 자체 탈중앙화 앱을 만드는 참고 자료

# dApp 만들기

Decentraland의 스마트 계약과 연동하고 그 기능을 더 정교하고 친숙한 방식으로 제공하는 자신만의 분산형 앱(dApp)을 만들 수 있습니다.

## dApp이란 무엇인가

분산형 애플리케이션, 즉 dApp은 중앙 서버가 아니라 분산된 P2P 네트워크에서 실행되는 애플리케이션입니다.

블록체인 맥락에서 dApp은 Web API 서비스 대신 스마트 계약과 경우에 따라 P2P 네트워크를 사용합니다. dApp은 프런트엔드를 제공하고 블록체인의 정보를 일시적으로 캐시할 수도 있지만, 그 출력은 궁극적으로 온체인에 반영됩니다.

참조 [이 사이트](https://blockchainhub.net/decentralized-applications-dapps/) dApp에 대한 더 완전한 개요를 보려면.

## Decentraland 스마트 컨트랙트

Decentraland는 LAND 및 MANA 토큰과 상호작용하는 여러 스마트 계약을 작성하고 유지 관리합니다.

LAND 및 MANA 토큰 자체는 다음에 의해 정의됩니다 *LANDregistry* 및 *MANAtoken* 각각의 계약에 의해 정의됩니다. 목록에는 여러 필지를 모아 에스테이트를 생성하거나 필지를 담보로 설정하는 것과 같은 더 구체적인 계약도 포함되어 있습니다.

* [Decentraland 스마트 컨트랙트](https://contracts.decentraland.org/addresses.json): 각 계약과 해당 주소의 전체 목록을 확인할 수 있습니다.

각 계약에는 다음에 있는 프로덕션 버전이 있음을 참고하세요 *mainnet* 그리고 다음에 있는 테스트 버전이 있습니다 *sepolia* 그리고 각각 주소가 다릅니다.

블록체인에 공개된 정보이므로 각 계약의 전체 코드를 읽을 수 있습니다. 예를 들어, 다음에서 이름으로 계약을 찾을 수 있습니다 [Etherscan](https://etherscan.io/contractsVerified) 에서 내용을 확인할 수 있습니다.

## dApp 보일러플레이트 코드

* [보일러플레이트 코드](https://github.com/decentraland/dapp-boilerplate): 이 보일러플레이트 코드는 자신만의 dApp을 만드는 출발점이 될 수 있습니다. 이 저장소는 보관 상태이며 더 이상 유지 관리되지 않으므로, 최신 템플릿이라기보다 참고용으로 보세요.

## 도우미 라이브러리

내부적으로 자체 dApp을 개발하면서, 여러분에게도 유용할 수 있는 몇 가지 도우미 라이브러리를 마련했습니다.

* [Decentraland-dapps](https://github.com/decentraland/decentraland-dapps): 이것은 dApp용 공통 모듈을 포함한 고수준 라이브러리로, Decentraland 자체 애플리케이션에서 활발히 사용됩니다.
* [Decentraland UI](https://ui.decentraland.org/): 이 라이브러리에는 Decentraland 프로젝트에 포함된 재사용 가능한 UI 요소 모음이 들어 있습니다.
* [Decentraland-eth](https://github.com/decentraland/decentraland-eth): 이더리움 블록체인과 작업하기 위한 유틸리티 함수가 들어 있는 저수준 라이브러리입니다. 이 저장소는 보관 상태이며 더 이상 유지 관리되지 않습니다.

## 샘플 dApp

아래에는 Decentraland를 중심으로 우리가 만든 여러 dApp의 전체 코드 링크가 있습니다. 이는 여러분이 자신만의 dApp을 만드는 데 도움이 될 수 있습니다:

* [Gate](https://github.com/decentraland/gate): 이 기본 dApp은 Decentraland가 클로즈드 베타 단계에 있을 때 사용되던 초대 NFT를 생성합니다. 초기에는 이 NFT를 보유한 화이트리스트 플레이어만 메타버스에 들어갈 수 있었습니다.
* [마켓플레이스](https://github.com/decentraland/marketplace): 이것은 Decentraland를 구동하는 전체 애플리케이션입니다 [마켓플레이스](https://market.decentraland.org/). 빠르게 실행하기 위해서는 LAND 정보로 인덱스를 생성할 수 있도록 데이터베이스와 이더리움 네트워크에 연결된 백엔드 서버가 필요합니다.

## dApp 테스트 프레임워크

dApp을 프로덕션에 배포하기 전에 먼저 테스트해 보시기를 권장합니다.

* [dAppeteer](https://github.com/decentraland/dappeteer): 이 프레임워크는 여러분이 dApp에서 테스트를 실행하는 데 도움을 주기 위해 만들었습니다. 이 저장소는 보관 상태이며 더 이상 유지 관리되지 않습니다.

## Decentraland 스마트 컨트랙트

다음 링크에서 Decentraland 생태계와 관련된 이더리움 스마트 컨트랙트 목록을 찾을 수 있습니다. 이 목록에는 메인넷의 컨트랙트뿐 아니라 다른 이더리움 테스트 네트워크의 컨트랙트도 포함되어 있습니다.

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/blockchain/create-a-dapp.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
