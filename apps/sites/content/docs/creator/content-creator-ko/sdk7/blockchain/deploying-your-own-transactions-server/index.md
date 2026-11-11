> 사용자에게 비용 없는 트랜잭션을 제공합니다

# 트랜잭션 서버 배포

다음은 [transactions-server](https://github.com/decentraland/transactions-server/tree/1.8.0) 트랜잭션을 전달하는 프록시 서버로서 [Gelato](https://www.gelato.network/relay). 클라이언트로부터 서명된 트랜잭션을 받으면, 이를 백그라운드에서 적절한 네트워크로 전송합니다. 이를 통해 서버 소유자는 사용자에게 비용 없는 트랜잭션을 제공할 수 있습니다

트랜잭션 서버는 여러 네트워크를 사용하는 UX를 개선하고 사용자가 즉시 네트워크 제공자를 전환하지 않도록 돕기 위해 사용됩니다. 사용자는 계속 [Ethereum](https://ethereum.org/en/) 에 연결된 상태를 유지하면서 [Polygon](https://polygon.technology/) [과 상호작용할 수 있으며, 이는 트랜잭션에 서명하는 것만으로 가능합니다](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/)

Decentraland DAO는 우리 dapp에서 사용되는 서버를 설정했으며, 몇 가지 [제한 사항](#restrictions)과 함께 일정 한도까지의 비용을 부담합니다. 이 문서는 다음 방법을 설명합니다. [이 서버를 배포하여](#running-the-server) 필요한 제한 사항이 있다면 이를 적용한 상태로 사용자가 트랜잭션을 전달할 수 있게 하는 방법을 설명합니다.

## 제한 사항

모든 제한은 사용자가 보내려는 각 트랜잭션 단위로 적용됩니다. 실제로는 서버로 보내는 POST 요청으로 해석됩니다.

서버에 구성할 수 있는 제한은 다음과 같습니다:

* 하루 최대 트랜잭션 수 할당량을 검사합니다. 자세한 내용은 [트랜잭션 섹션](#transactions) 에서 확인하세요.
* 허용된 계약만 검사하므로, 트랜잭션이 인식되지 않은 계약과 상호작용하려고 하면 실패합니다. 이를 위해
  * 배포된 계약과 컬렉션을 사용합니다. 자세한 내용은 [계약 및 컬렉션 섹션](#contracts-and-collections) 에서 확인하세요
* 판매 가격이 임계값보다 낮으면 이를 제한합니다. 자세한 내용은 [최소 판매 금액 섹션](#min-sale-value) 에서 확인하세요

## Gelato 구성

[Gelato](https://www.gelato.network/relay) 은 멀티체인 릴레이어 프로토콜입니다. 우리는 이 인프라를 사용해 비용 없는 트랜잭션을 가능하게 합니다. 실제로는, 트랜잭션을 보내려 할 때 트랜잭션을 전송하는 대신 메시지에 서명해서 Gelato로 보내게 됩니다. 이 서비스가 여러분을 대신해 트랜잭션을 보내고 응답(트랜잭션 해시)을 돌려줍니다.

트랜잭션을 전달하려면 계약이 필요하지만, 다행히 Decentraland에서 사용 중인 계약을 재사용할 수 있습니다(아래 참조).

[Gelato](https://www.gelato.network/relay) 서버의 API로 작동합니다. [구성하려면](#configuring-the-server) 먼저 API KEY가 필요하며, 우리는 이를 [나중에 사용할 것입니다](#gelato). 이를 얻으려면:

* [등록하세요](https://app.gelato.network/relay) 서비스에서
* 대상으로 삼으려는 네트워크에 대한 새 dapp를 생성하세요. Decentraland의 설정을 따라 하려면:
  * 다음을 선택하세요. `Mainnets` 옵션
  * 적절한 `App name` 을 dApp에 설정하세요
  * 다음을 네트워크로 선택하세요. `Polygon` 해당 `Smart Contract`
  * 다음을 활성화하세요. `Any Contract` 토글 옵션
* 다음에서 API KEY를 복사하세요. `API Key` 섹션

마지막으로, 새로 생성한 dapp에 자금을 공급해야 합니다. 왼쪽 사이드바의 [1Balance](https://app.gelato.network/1balance) 섹션에서 지갑을 연결하면 이를 할 수 있습니다. 연결하면 [USDC](https://polygonscan.com/token/0x3c499c542cef5e3811e1192ce70d8cc03d5c3359) 를 입금하여 사용자들이 보낼 트랜잭션의 자금을 마련할 수 있습니다. MATIC이 필요하다면, 이 [게시물](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/#where-can-i-get-matic-to-pay-for-transaction-fees).

### 테스트넷

앱을 정식 출시하기 전에 테스트하고 싶고 Polygon을 사용 중이라면 `Polygon Amoy`에서 할 수 있습니다. Polygon 테스트넷입니다.

이를 하려면 간단히 [프로세스](#configuring-gelato) 를 반복하되 `Matic Testnet (Amoy)` 을 네트워크 필드에서 선택하세요.

dapp에 자금을 공급해야 하지만, [faucet](https://sepoliafaucet.com/).

## 트랜잭션 서버 다운로드

먼저, Decentraland의 transactions-server 코드 사본이 필요합니다. 이 코드는 [github에서](https://github.com/decentraland/transactions-server/tree/v1)찾을 수 있습니다. 여기서 두 가지 방법이 있습니다:

1. **코드 다운로드**: 코드를 다운로드하려면 먼저 초록색 `Code` 버튼을 클릭한 다음 다음 중 하나를 선택하세요.

* 다음을 클릭하세요. `Download ZIP`
* 다음 아래의 URL을 복사하세요. `Clone` 제목을 복사한 다음 `$ git clone THE_URL_HERE`

2. **코드 포크하기**: 페이지 오른쪽 상단의 `fork` 버튼을 클릭할 수 있습니다. 절차가 완료되면 첫 번째 방법과 같은 방식으로 코드를 다운로드할 수 있습니다. 이를 위해 GitHub 계정이 필요하며, 저장소 포크에 대한 자세한 내용은 [여기](https://docs.github.com/en/enterprise-server@3.5/get-started/quickstart/fork-a-repo)

## 서버 구성

트랜잭션 서버는 [NodeJS](https://nodejs.org/en/) 를 사용하여 [Typescript](https://www.typescriptlang.org/)로 작성되었습니다. 실행하기 전에 몇 가지 환경 변수를 구성해야 합니다.

이를 위해:

* 다음 파일을 복사하세요. `.env.defaults` 파일을 이름을 바꿔 붙여넣으세요. `.env`
* 파일을 여세요. 기본값이 지정된 변수들이 보일 것입니다. 예를 들면 `.env` 다음과 같습니다. `HTTP_SERVER_PORT=5000` (서버를 실행할 포트)
* 대부분의 값은 그대로 두어도 되지만, 고려해야 할 중요한 값들이 몇 가지 있습니다:
  * [Gelato](#gelato)
  * [트랜잭션](#transactions)
  * [계약 및 컬렉션](#contracts-and-collections)
  * [최소 판매 금액](#min-sale-value)

### Gelato

Gelato를 구성할 때 얻은 API KEY를 사용하세요 [gelato 구성](#configuring-gelato).

```
GELATO_API_KEY=<YOUR_GELATO_API_KEY>
```

### 트랜잭션

새 트랜잭션 요청이 도착하면 그날 **주소가** 보낸 금액을 확인합니다. 설정한 값보다 크면 트랜잭션은 실패합니다.

```
MAX_TRANSACTIONS_PER_DAY=10
```

이 검사를 완전히 제거하려면 코드로 들어가 다음을 제거하면 됩니다.

```ts
await checkQuota(components, transactionData)
```

메서드를 `async function checkData(transactionData: TransactionData): Promise<void> {` 에서 `src/ports/transaction/component.ts`

### 계약 및 컬렉션

서버는 Contract addresses URL을 가져와 로컬에 저장하고 서브그래프를 조회합니다. 새 트랜잭션 요청이 도착하면 트랜잭션이 상호작용하려는 계약이 URL에 있는 배포된 계약 또는 서브그래프에 있는 배포된 컬렉션에 속하는지 확인합니다.

자체 계약을 제공하려면 URL을 변경하고 현재의 구조를 그대로 유지하세요. <https://contracts.decentraland.org/addresses.json> 가 가지고 있는 구조를 유지하세요. 사용되는 네트워크는 COLLECTIONS\_CHAIN\_ID로 결정되며, 캐시를 다시 가져오는 간격은 COLLECTIONS\_FETCH\_INTERVAL\_MS입니다.

자체 컬렉션이 있다면 서브그래프 URL도 변경할 수 있습니다.

이 검사를 완전히 제거하려면 코드로 들어가 다음을 제거하면 됩니다.

```ts
await checkContractAddress(components, transactionData)
```

메서드를 `async function checkData(transactionData: TransactionData): Promise<void> {` 에서 `src/ports/transaction/component.ts`

```
CONTRACT_ADDRESSES_URL=https://contracts.decentraland.org/addresses.json
COLLECTIONS_FETCH_INTERVAL_MS=3600000
COLLECTIONS_CHAIN_ID=80002

COLLECTIONS_SUBGRAPH_URL=https://subgraph.decentraland.org/decentraland/collections-matic-amoy
```

### 최소 판매 금액

새 트랜잭션 요청이 도착하면 먼저 전달하려는 데이터의 내용을 파싱합니다. 판매(마켓플레이스 구매, 입찰 등)를 감지하면 값을 MIN\_SALE\_VALUE\_IN\_WEI와 비교합니다. 더 낮으면 트랜잭션은 실패합니다.

```
MIN_SALE_VALUE_IN_WEI=1000000000000000000
```

관련 판매 메서드를 확인하려면 다음을 보세요. `src/ports/transaction/validation/checkSalePrice.ts` 이 검사를 완전히 제거하려면 코드로 들어가 다음을 제거하면 됩니다.

```ts
await checkSalePrice(components, transactionData)
```

메서드를 `async function checkData(transactionData: TransactionData): Promise<void> {` 에서 `src/ports/transaction/component.ts`

## 서버 실행

이제 모든 구성이 완료되었고, 남은 것은 실제로 서버를 실행하는 일입니다. 다음을 따라할 수 있습니다. [README](/docs/creator/content-creator-ko/sdk7/blockchain/deploying-your-own-transactions-server/) 를 참고하세요. 하지만 간단히 말하면, 다음을 수행해야 합니다:

* 설치되어 있어야 합니다. [NodeJS](https://nodejs.org/en/) 설치된
* 원하는 터미널을 여세요
* 다음 명령을 실행하세요:

```bash
npm install
npm run migrate # 첫 실행에서만
npm run start
```

물론 AWS와 같은 원하는 서비스에 배포하고 싶을 것입니다. [AWS](https://aws.amazon.com/) 예를 들어. 프로젝트의 [Dockerfile](https://github.com/decentraland/transactions-server/blob/master/Dockerfile) 을 사용하면 됩니다.

## 서버 사용

이제 모든 것이 설정되고 실행되었으니, 실제로 서버를 사용할 시간입니다.

실제로 트랜잭션을 보내려면 다음으로 POST 요청을 해야 합니다. `/transactions`. 요청에 필요한 스키마는 다음에 의해 정의됩니다. `transactionSchema` 에서 `src/ports/transaction/types.ts`.

반대로, 삶을 더 편하게 해주는 미리 만들어진 라이브러리를 사용하고 싶다면 다음을 시도해 볼 수 있습니다. [decentraland-transactions](https://github.com/decentraland/decentraland-transactions)입니다. 다음을 통해 사용됩니다. [decentraland-dapps](https://github.com/decentraland/decentraland-dapps) 우리 dapp들에서 다음과 같은 [Marketplace](https://market.decentraland.org)처럼 사용하며, 유틸은 [`sendTransaction`](https://github.com/decentraland/decentraland-dapps/blob/master/src/modules/wallet/utils.ts#L104). 다음을 확인하세요. [이 코드](https://github.com/decentraland/marketplace/blob/a2191515c6ae7ede54a685cc2dd9f9fafa35366b/webapp/src/modules/vendor/decentraland/OrderService.ts#L33) 를 예시로 참고하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/blockchain/deploying-your-own-transactions-server.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
