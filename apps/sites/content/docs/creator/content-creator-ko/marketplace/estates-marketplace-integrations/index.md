> 마켓플레이스에 Decentraland의 에스테이트를 통합하고 싶다면 고려해야 할 중요한 사항

# 에스테이트 마켓플레이스 통합

## 귀하의 마켓플레이스에 Decentraland의 Estate 통합하기

Decentraland의 Estate는 ERC721 호환 NFT이며 Ethereum Mainnet에서 실행됩니다. 따라서 많은 제3자 마켓플레이스가 이를 거래할 수 있습니다. 이를 위해서는, 사용자의 거래를 안전하게 유지하기 위해 이 마켓플레이스가 특정 규칙을 따라야 합니다.

아시다시피, 각 [Estate](https://docs.decentraland.org/decentraland/faq/#what-is-an-estate) 는 [LAND](https://docs.decentraland.org/decentraland/faq/#what-is-land) 의 그룹이며, *bytes* 그룹과 연관된 해시가 있으며 이를 *`핑거프린트`*. Estate에 LAND를 추가하거나 제거할 때마다, *`핑거프린트`* 도 함께 변경됩니다.

마켓플레이스, 특히 에스크로 시스템이 없는 곳의 경우, Estate의 기록을 보관하는 것이 100% 권장됩니다 *`핑거프린트`* 누군가 이를 판매 목록에 올리거나 오퍼를 제출할 때. 그렇게 하면 주문/오퍼가 성공적으로 실행될 때, 현재 소유자가 주문 실행을 선점하려고 시도하면서 Estate를 변경할 수 없습니다.

빈 Estate를 등록하는 것은 권장되지 않습니다. Estate 스마트 계약에는 `getEstateSize` 라는 메서드가 있으며, Estate 내 LAND의 수를 반환합니다. 결과가 `0`이면, 해당 Estate는 등록하지 않는 것이 좋습니다. 그래도 등록하고 싶다면, 다음과 같은 이미지를 보게 됩니다:

![](/docs/_remote/3ea9308d2dc5bf30fe069b53.webp)

## 예제

### Estate 핑거프린트를 사용하지 않음

* Bob은 LAND (1,1)과 (1,2)를 가진 Estate1을 보유하고 있습니다. Estate1 핑거프린트: hash1
* Bob은 Estate1에 LAND (1,3)를 추가합니다. Estate1은 LAND (1,1), (1,2), (1,3)를 보유합니다. Estate1 핑거프린트: hash2 (핑거프린트가 변경됨)
* Bob은 Estate1에서 LAND (1,1)을 제거합니다. Estate1은 LAND (1,2)와 (1,3)를 보유합니다. Estate1 핑거프린트: hash3 (핑거프린트가 변경됨)
* Bob은 Estate1을 판매에 올립니다. 목록은 Ethereum 메인넷의 온체인에 Estate 스마트 계약 주소, Estate ID, 가격, 그리고 만료 날짜와 함께 생성됩니다.
* Alice는 Estate ID와 지불할 것으로 예상되는 가격을 지정하여 Estate를 구매하는 거래를 보냅니다.
* Bob은 누군가 자신의 Estate1을 구매하려 한다는 것을 감지하고, Alice보다 더 높은 가스 수수료를 지불하는 거래를 보내 Estate1에서 LAND (1,2)와 (1,3)을 제거합니다.
* Bob의 거래가 먼저 채굴됩니다. Estate1에는 LAND가 0개 있습니다. Estate1 핑거프린트: hash4 (핑거프린트가 변경됨)
* Alice의 거래는 나중에 채굴됩니다. Alice는 LAND가 0개인 Estate1을 구매했습니다. 즉, Alice는 Bob에게 프런트런당했고(그리고 도난/사기를 당했습니다).

### Estate 핑거프린트를 사용함

* Bob은 LAND (1,1)과 (1,2)를 가진 Estate1을 보유하고 있습니다. Estate1 핑거프린트: hash1
* Bob은 Estate1에 LAND (1,3)를 추가합니다. Estate1은 LAND (1,1), (1,2), (1,3)를 보유합니다. Estate1 핑거프린트: hash2 (핑거프린트가 변경됨)
* Bob은 Estate1에서 LAND (1,1)을 제거합니다. Estate1은 LAND (1,2)와 (1,3)를 보유합니다. Estate1 핑거프린트: hash3 (핑거프린트가 변경됨)
* Bob은 Estate1을 판매에 올립니다. 목록은 Ethereum 메인넷의 온체인에 Estate 스마트 계약 주소, Estate ID, 가격, 만료 날짜와 함께 생성됩니다.
* Alice는 Estate를 구매하는 거래를 보내며, Estate ID, 지불할 것으로 예상되는 가격, 그리고 **그녀가 본 핑거프린트(hash3)**.
* Bob은 누군가 자신의 Estate1을 구매하려 한다는 것을 감지하고, Alice보다 더 높은 가스 수수료를 지불하는 거래를 보내 Estate1에서 LAND (1,2)와 (1,3)을 제거합니다.
* Bob의 거래가 먼저 채굴됩니다. Estate1에는 LAND가 0개 있습니다. Estate1 핑거프린트: hash4 (핑거프린트가 변경됨)
* Alice의 거래는 거부됩니다. 스마트 계약이 Alice가 보낸 매개변수의 핑거프린트가 현재 Estate1 핑거프린트(hash3 != hash4)와 일치하지 않는다는 것을 확인했기 때문입니다. 이 검사는 Alice가 원하지 않는 Estate를 구매하는 것을 막았습니다.

## Estate 스마트 계약 핑거프린트 인터페이스

Estate의 스마트 계약은 [핑거프린트 인터페이스](https://github.com/decentraland/land/blob/master/contracts/estate/EstateStorage.sol#L19). Estate가 판매 목록에 오르거나 오퍼를 받으면, 다음을 호출하여 핑거프린트를 기록하십시오: *`getFingerprintV2(uint256 estateId)`* 를 호출하고 반환된 값을 저장합니다. 실행 시점에 해당 주문/오퍼가 여전히 유효한지 확인하려면 다음을 호출하십시오: *`verifyFingerprint(uint256 estateId, bytes fingerprint)`* 을 저장된 값으로.

항상 *`getFingerprintV2`*&#xAC00; 반환한 값을 저장하십시오. 이전의 *`getFingerprint`* 메서드는 하위 호환성을 위해서만 유지되며 단계적으로 폐지되고 있습니다: 2026년 11월 26일부터 *`verifyFingerprint`* 는 이 메서드로 생성된 값을 더 이상 허용하지 않습니다. 새로운 통합에서는 *`getFingerprintV2`*.

작동하는 프로덕션 예시를 확인할 수 있습니다 [여기](https://github.com/decentraland/marketplace-contracts/blob/master/contracts/marketplace/MarketplaceV2.sol#L382).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/marketplace/estates-marketplace-integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
