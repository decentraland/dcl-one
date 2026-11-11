> LAND 임대

# 임대

## 용어집

**LAND 소유자:** LAND를 소유한 계정(주소)입니다. Parcel, Estate 또는 둘 다일 수 있습니다.

**임차인:** LAND 소유자로부터 LAND를 임대하는 계정(주소)입니다. 또한 운영자 권한이 있는 주소를 변경할 수 있는 유일한 계정이기도 합니다.

**운영자 권한:** 이 권한이 있는 주소만이 해당 LAND에 씬을 배포할 수 있습니다.

**트랜잭션:** 가스 비용이 드는 Ethereum 블록체인 트랜잭션입니다.

## 소개

새로운 임대 시스템을 통해 LAND 소유자와 임차인은 **안전하고 신뢰 없이 LAND를 임대할 수 있습니다** 이는 Decentraland Foundation이 관리하는 서버(오프체인)에 저장된 서명과 Ethereum 트랜잭션(온체인)을 결합하여 이루어집니다.

예를 들어, 한 DJ가 멋진 LAND 구획을 찾아 임대한 뒤, 매주 토요일마다 공연할 나이트클럽을 배포할 수 있습니다. 한 대학은 Estate를 임대해 학생들을 위한 캠퍼스를 지을 수 있습니다.

아래에서 LAND를 임대하는 데 필요한 모든 단계와 양측이 관련된 트랜잭션을 확인할 수 있습니다.

## LAND 소유자의 경우

### LAND를 임대용으로 등록

LAND 소유자로서, 당신의 LAND(Parcels 또는 Estates)를 임대로 등록할 수 있습니다. [마켓플레이스](https://market.decentraland.org/) > 내 자산 > LAND.

이를 온체인에서 수행하려면 LAND 소유자가 렌트 스마트 컨트랙트가 자신의 대신 LAND를 사용하도록 승인해야 합니다. 그러면 모든 등록에 소유자의 서명도 필요하게 됩니다.

![](/docs/_remote/1a34b8c7a8542b15716940f1.webp)

MANA 기준으로 하루 임대료와 사람들이 임대할 수 있도록 허용할 일수를 설정할 수 있습니다. 기간의 일수에 하루 임대료를 곱한 금액이 임차인이 **선결제해야 하는 총액이며** 해당 임대에 대한 금액입니다.

![](/docs/_remote/d0b5696229c03215afc73455.webp)

하루 가격을 정한 후에는 사용자가 LAND를 임대할 수 있는 일수를 선택해야 합니다. 예를 들어 7일과 30일만 선택하면 임차인은 그 두 옵션 중에서만 선택할 수 있습니다. 임차인이 30일 옵션을 선택하면, 확인된 날부터 임대 기간은 그 기간이 됩니다.

![](/docs/_remote/f0859bad5d58c27a481d636a.webp)

등록 만료일도 설정할 수 있습니다. 이는 선택한 날짜까지 LAND가 임대되지 않으면 해당 등록이 마켓플레이스에서 제거된다는 뜻입니다. 또한 스마트 컨트랙트는 만료된 서명을 거부하므로, 아무도 이전에 선택한 등록 가격과 기간으로 임대할 수 없습니다. 이는 원하지 않는 가격이나 기간으로 임대되는 것을 방지하기 위한 보안 조치입니다.

![](/docs/_remote/f4c314e8b64de94a0f2045e5.webp)

가격, 임대 기간, 등록 만료일을 설정하면 당신의 LAND는 마켓플레이스에서 임대 가능 상태로 표시됩니다.


> [!NOTE]
> 💡 LAND가 임차인에게 임대되면, 다시 회수되기 전까지는 판매할 수 없습니다. 잠재 구매자의 입찰도 받을 수 없습니다.



> [!NOTE]
> 💡 LAND가 임대 중이더라도 투표 권한은 LAND 소유자가 유지합니다.


### 등록 수정 또는 취소

LAND가 마켓플레이스에 임대 등록된 후, 그리고 아무도 임대하기 전까지는 LAND 상세에서 연필 아이콘을 클릭하여 등록 조건을 수정할 수 있습니다. 또한 등록을 마켓플레이스와 블록체인에서 제거할 수도 있습니다.

![](/docs/_remote/89bcf7626763f867ddbc8c00.webp)


> [!NOTE]
> 💡 수정과 취소에는 트랜잭션이 필요하며, 가스 비용이 발생합니다. 자세한 내용은 아래 트랜잭션 섹션을 참조하세요.


### 임대 종료 후

임대가 끝난 후에는 다음 중 하나를 할 수 있습니다. **LAND를 다시 회수하거나, 다시 임대 등록하기**.

**운영자 권한은 자동으로 LAND 소유자에게 다시 이전되지 않습니다**. 이를 다시 받으려면 LAND 소유자가 해당 트랜잭션을 보내고 가스 비용을 지불하여 LAND를 다시 회수해야 합니다. 트랜잭션을 확인하면 임차인의 운영자 권한이 해제되고 LAND 소유자에게 다시 부여됩니다.

![](/docs/_remote/f43022686a5bd15b6c02ac8a.webp)

다른 방법으로는 LAND를 다시 회수하는 대신 다시 임대 등록할 수도 있습니다. 이 경우 다른 트랜잭션 비용을 지불할 필요는 없지만 **운영자 권한은 새 임차인이 새 임대를 확인할 때까지 이전 임차인이 유지합니다.**

LAND 소유자는 새 등록에 대해 가격, 임대 기간, 등록 만료일을 수정할 수 있습니다.

![](/docs/_remote/6f6b27950ff42602c5b34fda.webp)

두 작업 모두 마켓플레이스의 LAND 상세 페이지에서 수행할 수 있습니다.

![](/docs/_remote/2b6877fde55556d4100fd7e0.webp)

### 임대 상태

내 자산 > 저장소 > 임대 중에서 임대된 LAND의 상태를 확인할 수 있습니다. 가능한 상태는 다음과 같습니다.

* 임대 등록됨 - 등록이 확인되었으며 사용자가 마켓플레이스에서 임대할 수 있습니다
* 임대 기간 종료 - 이 단계에서는 LAND 소유자가 LAND를 다시 회수하거나 다시 임대 등록할 수 있습니다
* \~까지 임대됨 *"날짜"* - LAND는 이미 임대 중이며, 임차인은 다시 회수되거나 다른 사용자가 임대할 때까지 운영자 권한을 갖습니다

![](/docs/_remote/486197d9f7fa915ceb7f0183.webp)

## 임차인의 경우

### LAND 임대

모든 사용자는 마켓플레이스의 LAND 섹션에서 임대용으로 등록된 LAND를 찾을 수 있습니다.

![](/docs/_remote/95cd665129b4cbe803dc76df.webp)

판매 또는 임대가 가능한 LAND가 있습니다. 두 옵션이 모두 가능한 경우, 판매/임대 토글을 클릭하여 각 옵션의 조건을 확인할 수 있습니다.

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

임대하려는 LAND를 찾으면 임대 기간을 선택해야 합니다. 이는 LAND를 보유하게 될 일수입니다. 임대 기간을 선택하면 임대에 대해 지불할 총 금액이 표시됩니다.

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

진행하기 전에 렌트 스마트 컨트랙트가 계정에서 MANA를 가져갈 수 있도록 승인해야 합니다.

임대를 확인하기 전에 누가 LAND를 관리할지(운영자 권한) 결정할 수 있습니다. 본인이나 선택한 다른 주소가 될 수 있습니다.

![](/docs/_remote/2d83c8c5fc075dc07df8532c.webp)

운영자 권한은 나중에 임차인(처음에 LAND를 임대한 주소)이 [Builder](https://builder.decentraland.org/).

![](/docs/_remote/506dbf3015f1fd2ea5698b48.webp)

모든 세부 사항을 선택하고 렌트 스마트 컨트랙트가 MANA를 처리하도록 승인한 후, 트랜잭션을 보내 임대를 확인할 수 있습니다.

이제 모든 준비가 끝났습니다! LAND 작업을 시작하고 Builder 또는 SDK를 사용해 씬을 배포할 수 있습니다.

![](/docs/_remote/76317d506da755beb44eeaf7.webp)

참고: 임대가 끝난 후에도 LAND 소유자가 다시 회수하거나 다른 사람이 임대하기 전까지는 임차인이 계속 운영자 권한을 갖습니다. **임대가 끝나기 전에 콘텐츠를 반드시 저장하세요. 그렇지 않으면 내용이 사라질 수 있습니다.**


> [!NOTE]
> 💡 LAND를 임대해도 투표 권한은 임차인에게 이전되지 않습니다. 투표 권한은 이 [제안서](https://governance.decentraland.org/proposal/?id=c98bd010-74b1-11ed-a9bf-f772a12a0556)


## 트랜잭션

…을 위해 **보안과 탈중앙화를** 위해, 임대 시스템은 진실의 원천으로 Ethereum 블록체인에 의존합니다.

하지만 관련된 모든 작업에 블록체인 기록이 필요한 것은 아닙니다. 그렇다면 양측 모두에게 너무 비쌀 것입니다.

블록체인 내 트랜잭션은 다음을 제공하기 위해 최소한으로 유지됩니다. **비용 부담은 낮추면서도 LAND 임대를 위한 견고하고 신뢰 없는 시스템을** 고려해야 할 모든 트랜잭션은 다음과 같습니다.

### LAND 소유자의 경우

#### 임대 등록

첫 번째 Parcel 또는 Estate를 임대 등록하기 전에 LAND 소유자는 렌트 스마트 컨트랙트가 자신의 대신 LAND를 운영하도록 허용해야 합니다. 이는 Parcel에는 한 번만, Estate에는 한 번만 하면 됩니다.

![](/docs/_remote/24e324959f412a5ef38535ed.webp)

#### LAND 다시 회수 또는 다시 임대 등록

임대 기간이 끝난 후, **운영자 권한은 자동으로 LAND 소유자에게 다시 이전되지 않습니다**. 이를 다시 받으려면 LAND 소유자가 해당 트랜잭션을 보내고 가스 비용을 지불하여 LAND를 다시 회수해야 합니다.

![](/docs/_remote/9b7b0037cff4903f1d9e7022.webp)

다른 방법으로는 LAND를 다시 회수하는 대신 다시 임대 등록할 수도 있습니다. 이 경우 다른 트랜잭션 비용을 지불할 필요는 없지만, 운영자 권한은 새 임차인이 새 임대를 확인할 때까지 이전 임차인이 유지합니다.

#### 등록 수정

가격, 임대 기간 또는 만료일이 변경되면, LAND 소유자는 이전 등록 서명을 스마트 컨트랙트에 직접 사용하여(마켓플레이스 UI가 아닌) 원하지 않는 더 낮은 가격이나 원하지 않는 기간으로 얻는 일을 방지하기 위해 트랜잭션을 보내야 합니다.

![](/docs/_remote/985e0faa31bf88ac207e0846.webp)

### 임차인의 경우

#### 렌트 컨트랙트가 당신의 MANA를 운영하도록 허용

Parcel이든 Estate든, LAND를 임대하려는 모든 사용자는 렌트 스마트 컨트랙트가 자신의 대신 MANA를 운영하도록 허용하는 트랜잭션을 한 번 보내야 합니다. 이는 임대가 활성화될 때 스마트 컨트랙트가 MANA를 가져와 LAND 소유자에게 전송해야 하기 때문입니다. 이는 그 시점부터 임대되는 모든 LAND에 대해 한 번만 수행하면 됩니다.

#### LAND 임대

렌트 스마트 컨트랙트가 MANA를 운영하도록 승인한 후에는 첫 임대를 확인할 준비가 된 것입니다. 원하는 LAND를 찾은 뒤 임대 기간을 선택하고 임대 트랜잭션을 확인하면 운영자 권한이 선택한 주소로 이전됩니다.

다른 Parcel 또는 Estate를 임대하려면, 확인을 위한 트랜잭션 한 번만 보내면 되며, 스마트 컨트랙트가 다시 MANA를 운영하도록 승인할 필요는 없습니다.

![](/docs/_remote/34fb306b490b37363a4d6562.webp)

#### 운영자 변경

LAND를 임대할 때 사용자는 해당 LAND에 대해 어느 주소가 운영자 권한을 가질지 선택할 수 있습니다. 그 주소를 변경하려면 트랜잭션을 보내야 합니다.

![](/docs/_remote/e38a6d9159fa451ba7b2570b.webp)

## 스마트 컨트랙트 지갑

임대 기능은 오프체인 서명에 크게 의존합니다. 오프체인 작업을 통해 LAND 소유자는 트랜잭션 비용을 지불하지 않고도 LAND를 임대용으로 등록할 수 있습니다.

등록에 서명하면 렌트 스마트 컨트랙트는 해당 등록이 서명자에 의해 생성되었는지 확인할 수 있습니다.

서명에는 개인 키가 필요하다는 특징이 있습니다. 모든 EOA(외부 소유 계정)는 개인 키를 가지며, 이를 사용해 등록에 서명할 수 있습니다. 그러면 임대 스마트 컨트랙트는 임대 실행 시 EOA가 생성한 서명을 검증합니다.

스마트 컨트랙트 지갑은 스마트 컨트랙트입니다. 개인 키가 없으므로 메시지에 서명할 수 없습니다. 대신 스마트 컨트랙트 지갑이 승인한 EOA가 서명해야 합니다.

이러한 서명을 지원하기 위해 렌트 스마트 컨트랙트는 다음을 따르는 방식으로 스마트 컨트랙트 지갑에 서명이 유효한지 검증합니다. [EIP-1271](https://eips.ethereum.org/EIPS/eip-1271) 표준입니다. 서명이 유효하면 임대를 실행할 수 있습니다.

스마트 컨트랙트 지갑은 EIP-1271에 정의된 표준 서명 검증 메서드뿐 아니라 [EIP-721 표준](https://eips.ethereum.org/EIPS/eip-721)에 정의된 토큰 수신자 메서드도 구현해야 합니다. 이는 LAND를 다시 회수할 때 필요합니다. 렌트 스마트 컨트랙트가 `safeTransferFrom` 을 호출하여 NFT를 스마트 컨트랙트 지갑으로 반환하는데, 적절한 `onERC721Received` 함수를 구현하지 않았다면 LAND를 복구하는 데 실패하기 때문입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/marketplace/rentals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
