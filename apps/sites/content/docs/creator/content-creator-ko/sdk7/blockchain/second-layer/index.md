> 씬에서 Polygon의 사이드체인을 사용하여 훨씬 더 빠르고 저렴한 블록체인 트랜잭션을 가능하게 합니다.

# 2계층

## 2계층 솔루션에 관하여

블록체인에 영향을 미치는 모든 거래는 완료하는 데 시간이 걸리고 가스 비용이 듭니다. 이 두 가지는 블록체인 게임을 대중화하는 데 장애물이 됩니다. 플레이어들은 보통 자신의 행동이 반영되기까지 그렇게 오래 기다릴 인내심이 없고, 플레이하면서 여러 거래에 돈을 쓸 의향도 없기 때문입니다.

흔한 우회 방법은 게임플레이의 대부분을 오프체인으로 유지하고, 게임 아이템을 획득하거나 높은 점수를 등록하는 것과 같은 핵심 이벤트에 대해서만 블록체인 거래를 수행하는 것입니다.

그러나 이러한 한계와 블록체인에 내재된 다른 확장성 문제를 극복하는 또 다른 방법은 *2계층* 블록체인, 또한 *사이드체인*.

2계층은 탈중앙화 앱과 메인 체인 사이에서 중개 역할을 하는 또 다른 블록체인입니다. 이 계층은 더 가벼우므로 더 빠른 응답과 훨씬 낮은 가스 비용을 제공할 수 있습니다.

거래를 메인 체인에 직접 수행하는 대신, 거래는 사이드체인에서 이루어지고, 이후 그 변경 사항을 메인 체인과 동기화하는 것은 사이드체인의 역할입니다.

사이드체인은 더 적은 노드를 가진 작은 네트워크이기 때문에 더 빠른 응답을 제공할 수 있습니다. 2계층에서 수행되는 거래는 처음에는 보안성이 낮지만, 결국 대량으로 메인 체인에 커밋되며 메인 체인의 모든 보안 검사가 그곳에서 적용될 수 있습니다.

사이드체인은 메인 체인과 동기화할 때 많은 거래를 하나로 묶기 때문에 개별 거래 비용도 크게 낮출 수 있습니다. 따라서 메인 체인에서 한 번의 거래에 지불해야 할 가스 비용을 수천 건의 거래에 걸쳐 나눌 수 있습니다.

또한 메인 체인과 동기화할 때 중복되는 여러 거래를 완전히 피할 수 있어 거래 비용을 더욱 줄일 수 있습니다. 예를 들어, 앨리스가 밥에게 1 ETH를 전송한 다음 밥이 캐럴에게 1 ETH를 전송한다면, 앨리스가 캐럴에게 직접 전송하는 것으로 기록하여 이 두 거래를 하나로 단순화할 수 있습니다.

디센트럴랜드는 [폴리곤](https://polygon.technology/)과 파트너십을 맺고 있으며, 이들은 이더리움에서 자체 사이드체인을 제공합니다. 해당 사이드체인은 장면(scene)에서 빠르고 매우 저렴한 거래를 지원하는 데 사용할 수 있습니다.

## 폴리곤 테스트넷 사용해 보기

폴리곤의 Matic 테스트넷을 사용하려면 먼저 다음이 필요합니다:

* 세폴리아 네트워크의 이더. 다음과 같은 여러 외부 faucet에서 무료로 받을 수 있습니다: [이것](https://www.alchemy.com/faucets/ethereum-sepolia).

그런 다음 Matic 테스트넷을 포함하도록 메타마스크 계정을 구성해야 합니다.

방법은 다음과 같습니다:

1. 현재 네트워크 이름을 클릭하고 *사용자 지정 RPC* 를 선택하여 메타마스크에서 네트워크를 전환하세요.
2. 'Polygon Amoy Testnet'을 다음 항목에 입력하세요 *네트워크 이름*, [**https://rpc-amoy.polygon.technology**](https://rpc-amoy.polygon.technology) 를 *새 RPC URL* 및 [**https://amoy.polygonscan.com**](https://amoy.polygonscan.com) 를 *블록 탐색기 URL*. 다음 항목에 대해 *심볼* 를 **POL**
3. '저장'을 클릭하여 이 새 네트워크를 메타마스크에 추가하세요.


> [!WARNING]
> **📔 참고**: 폴리곤 설정 방법에 대해 더 알아보려면 해당 [문서 페이지](https://docs.polygon.technology/)



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/blockchain/second-layer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
