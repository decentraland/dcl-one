> Decentraland Rewards 캠페인을 설정합니다: 생성, 웨어러블 또는 이모트 재고 추가, 자금 관리, 디스펜서 활성화.

# 시작하기

이 페이지에서는 Decentraland Wearables 및 Emotes에 대한 사용자 참여를 유도하기 위해 Rewards 시스템을 사용하는 과정을 안내합니다. 캠페인 생성, 캠페인 재고에 아이템 추가, 자금 관리, 디스펜서로 보상 전달 등 보상 캠페인을 설정하고 관리하는 방법을 배우게 됩니다. 또한 캠페인 활성화 및 거래 수수료 처리에 대한 지침도 확인할 수 있습니다.

## 사전 요구 사항

Rewards 시스템을 사용하기 전에 컬렉션이 [승인되어 있고 민팅할 준비가 되어 있는지](/docs/creator/content-creator-ko/wearables-and-emotes/publishing-collections/publishing-collections/). 이 컬렉션에는 캠페인에서 보상으로 제공할 아이템이 포함되어 있어야 합니다.

## 첫 번째 캠페인 만들기

캠페인은 보상 제공을 통해 사용자 참여를 높이기 위해 설계된 인센티브 이니셔티브를 말합니다. 이 섹션에서는 보상 배포를 시작하기 위한 사전 요구 사항과 함께 캠페인 구성 세부 정보를 확인할 수 있습니다.

1. 다음에 [보상 dApp](https://decentraland.org/rewards)
2. 지갑 연결
3. 다음을 클릭하세요: **캠페인** 탭
4. 다음을 클릭하세요: **캠페인 만들기** 버튼을 클릭하면 캠페인 생성 양식으로 이동합니다.

![](/docs/_remote/6288b97cb788a168a929488f.webp)

5. 다음 항목을 입력하세요:

* **캠페인 이름**: 캠페인 이름을 선택하세요. 이 이름은 사용자의 보상 페이지에 표시됩니다.
* **캠페인 네트워크**: wearables가 민팅될 네트워크를 선택하세요. 이 선택은 최종이며 캠페인이 생성된 후에는 변경할 수 없습니다.
* **최대 가스 가격**: 거래당 지불할 의사가 있는 최대 가스 가격을 설정하세요. 거래의 가스 가격이 이 한도를 초과하면 가격이 내려갈 때까지 보류됩니다. UI는 이 설정을 기준으로 각 아이템을 민팅하는 데 드는 비용을 추정해 보여줍니다.

6. 다음을 클릭하세요: **캠페인 만들기** 버튼을 클릭하면 캠페인 페이지로 이동합니다.

![](/docs/_remote/2c39dab8808bf032e1c51198.webp)

캠페인을 생성한 후 보상을 전달하기 전에 완료해야 할 단계가 몇 가지 더 있습니다:

* **MATIC 토큰을 추가로 준비하기**: 거래 수수료를 충당할 수 있도록 계정에 충분한 MATIC 토큰이 있는지 확인하세요.
* **캠페인 재고에 아이템 추가하기**: 보상으로 제공할 아이템으로 캠페인을 채우세요.
* **디스펜서 생성**: 사용자에게 보상을 배포할 디스펜서를 설정하세요.
* **캠페인 활성화**: 마지막으로 캠페인을 활성화하여 공개하고 보상 전달을 시작하세요.

### 거래 수수료 지불을 위해 MATIC 추가

아이템을 민팅하려면 캠페인 주소에 MATIC 토큰을 추가하여 거래 수수료를 충당해야 합니다. 이 주소는 캠페인 이름 아래에서 확인할 수 있습니다.

지갑에서 MATIC을 전송하려면 먼저 캠페인과 동일한 네트워크에 연결되어 있는지 확인하세요. 그런 다음 캠페인 주소를 복사해 원하는 양의 MATIC을 전송하세요. 지갑에 MATIC이 없다면 다음 가이드를 따라 [이 가이드를](https://github.com/decentraland/docs/tree/main/player/blockchain-integration/transactions-in-polygon.md#where-can-i-get-matic-to-pay-for-transaction-fees) 참고하여 일부를 확보할 수 있습니다.


> [!NOTE]
> **💡 팁**: 테스트 **AMOY** 네트워크에서 MATIC을 다음에서 얻을 수 있습니다: [이 faucet](https://faucet.polygon.technology/).


거래소에서 MATIC을 보내려면 MATIC 토큰을 Polygon 네트워크로 출금하기만 하면 됩니다(보통 거래소에서는 Amoy 네트워크를 사용할 수 없습니다).

### 캠페인 재고에 아이템 추가하기

새 캠페인을 만들면 기본적으로 빈 재고로 시작합니다. 보상을 전달하려면 캠페인에 아이템을 추가해야 합니다:

![](/docs/_remote/ec2d216749ea170e1a5f766a.webp)

* **민팅 권한 부여**: 먼저 캠페인이 아이템을 민팅할 수 있도록 권한을 부여하세요. 다음을 따르세요 [이 가이드를](/docs/creator/content-creator-ko/wearables-and-emotes/publishing-collections/publishing-collections/#adding-minters-to-the-collection) 그리고 캠페인 주소(캠페인 이름 아래에 있음)를 민터 주소로 사용하세요. 제출한 후 거래가 확인될 때까지 기다리세요.
* **캠페인 재고에 아이템 추가하기**: 민팅 권한이 부여되면 캠페인 재고에 아이템을 추가할 수 있습니다.

  a) 다음을 클릭하세요: **재고 추가** 버튼을 클릭하면 캠페인에 민팅 권한이 있는 모든 컬렉션을 볼 수 있습니다.

![](/docs/_remote/0e60e96b9eeeac0ec6fcde02.webp)

```
b) 재고에 추가할 컬렉션의 아이템을 선택한 다음 **재고 추가** 버튼을 클릭하세요.
```

![](/docs/_remote/f1349ec843ec2ae7fa0e04a0.webp)

```
c) 캠페인 재고에 추가할 아이템 수량을 선택한 다음 **재고 추가** 버튼을 클릭하세요.
```

![](/docs/_remote/1281e9ab96332e98d58a7e9a.webp)


> [!NOTE]
> Priority 및 Group 필드는 전용 섹션에서 다룰 고급 기능입니다.


### 보상 전달을 위한 디스펜서 생성

디스펜서는 재고에서 보상을 보내거나 청구하는 데 사용되는 도구입니다. 각 디스펜서는 고유 키와 연결되어 있으며, 이는 HTTP API 인증에 필요합니다. 키는 UI에 표시되지 않지만 복사 버튼을 클릭하여 복사할 수 있습니다.

새 캠페인을 만들면 기본적으로 마스터 키가 있는 기본 디스펜서가 테스트 목적으로만 자동 제공됩니다. 새 디스펜서를 설정하려면 다음을 클릭하세요: **디스펜서 추가** 버튼이 있습니다.

![](/docs/_remote/80ef5f28ed3dbc20c04f5aaa.webp)

아이템을 가져올 아이템 그룹을 선택한 다음 **저장** 버튼이 있습니다.


> [!NOTE]
> 디스펜서에는 전용 섹션에서 다룰 몇 가지 구성 옵션이 있습니다.


### 캠페인 활성화

기본적으로 캠페인은 비활성화되어 있습니다. 이렇게 하면 Rewards 서비스가 새 보상을 할당하지 못합니다. 캠페인을 활성화하려면 다음을 클릭하세요: **캠페인 활성화** 버튼이 있습니다.


> [!WARNING]
> **📔 참고**: 활성 캠페인을 비활성으로 설정하더라도 보류 중인 보상은 계속 전송됩니다. 그러나 새 청구 또는 어떤 이유로 재시도해야 하는 청구는 차단됩니다.


**이제 다음을 사용하여 wearables/emotes 민팅을 시작할 준비가 되었습니다:** [**API**](/docs/creator/content-creator-ko/rewards/integrations/)

### 자금 관리

캠페인이 완료되면 거래 수수료에서 남은 자금이 있을 경우 소유자는 자금을 회수하거나 다른 캠페인에 재할당할 수 있습니다. 자금을 회수하려면 캠페인 상세 페이지에서 자금 관리 버튼을 클릭하기만 하면 됩니다.

![](/docs/_remote/3ff911494b211a0d72e187c5.webp)


> [!WARNING]
> **📔 참고**: 캠페인에서 모든 자금이 제거되면 해당 캠페인과 관련된 보류 중이거나 새 거래는 더 많은 자금이 할당될 때까지 차단됩니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/rewards/getting-started.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
