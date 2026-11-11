> DAO는 Decentraland의 의사결정 플랫폼입니다.

# DAO란 무엇인가


> [!NOTE]
> **DAO의 사명:** 대표성 있는 거버넌스, 집단 자원의 효율적인 관리, 기술 혁신을 통해 Decentraland 생태계의 지속가능성을 보장합니다. 다음을 읽어보세요 [사명, 비전 및 역할](/docs/player/player-ko/dao/dao/mission-vision-and-roles/) DAO, DAO 위원회, DCL Regenesis Labs 및 Decentraland Foundation의.


Decentraland DAO는 Decentraland의 가상 세계에서 MANA, NAMES 및 LAND 보유자를 위한 의사결정 도구입니다. DAO의 투표를 통해 커뮤니티는 보조금을 지급하고 금지된 이름, POI, 촉매 노드 목록을 변경할 수 있습니다. DAO는 LAND 및 Estate 스마트 계약도 제어합니다.

DAO가 소유한 기록과 계약에 대한 보조금 지급 및 변경은 다음에서 접근할 수 있는 미리 정의된 제안안을 사용해야만 가능합니다 [governance.decentraland.org](https://governance.decentraland.org).

이 제안안, 제출된 투표, 최종 결과는 모두 가스가 없는 투표 클라이언트인 Snapshot을 통해 IPFS에 저장됩니다. 구속력 있는 조치가 포함된 승인된 제안안은 멀티시그 지갑을 통해 위원회가 이더리움 블록체인에서 실행합니다. 이 위원회는 신뢰할 수 있는 키 보유자들로 구성된 또 다른 멀티시그인 Security Advisory Board(SAB)의 감독을 받습니다. 이 위원회는 이전 DAO 배포에서 커뮤니티 투표로 구성되었습니다. [원본 제안안은 여기에서 확인할 수 있습니다](https://forum.decentraland.org/t/proposal-for-a-more-accessible-and-affordable-dao/450).

이 문서의 나머지 부분에서는 DAO가 무엇인지, 어떻게 작동하는지, 무엇에 사용할 수 있는지를 더 자세히 설명합니다.

Decentraland DAO 사용 방법에 대한 자세한 튜토리얼은 다음을 방문하세요: [DAO 사용자 가이드](/docs/player/player-ko/dao/dao-userguide/).

## DAO는 스마트 계약으로 구동됩니다

모든 DAO, 즉 탈중앙화 자율 조직은 이더리움이 가능하게 한 조직 관리와 의사결정에 대한 새로운 접근 방식의 일부입니다.

이더리움은 토큰 소유권 기록보다 더 복잡한 데이터 처리를 탈중앙화할 수 있는 기능을 추가함으로써 블록체인이 할 수 있는 일을 확장했습니다. 이더리움은 사람들이 블록체인에 스마트 계약을 올릴 수 있게 함으로써 이를 가능하게 했습니다.

### 스마트 계약이란 무엇인가요?

스마트 계약은 이더리움 블록체인에서 실행되는 컴퓨터 프로그램입니다. 함수(무언가를 수행하는 코드 조각)와 데이터(정보)를 모두 저장할 수 있습니다. 스마트 계약은 종종 자판기에 비유됩니다. 특정 입력을 넣으면 특정 출력을 받게 됩니다. 내가 자판기 앞으로 가서 1달러를 넣고 "오렌지 소다" 버튼을 누르면, 기계에 남아 있다면 오렌지 소다를 받게 됩니다. 오렌지 소다가 더 이상 없다면, 1달러를 돌려받게 됩니다.

스마트 계약도 같은 방식으로 작동하며, 사람들은 특정 결과나 정보를 받을 것이라는 기대를 가지고 정보를 보내 상호작용할 수 있습니다. 자판기 안에 음료를 나눠주는 작은 사람이 없는 것처럼, 스마트 계약은 자동적입니다(감히 말하자면, 자율적입니다).

이더리움 스마트 계약에 대해 더 알아보고 싶다면, 다음이 [이더리움 문서](https://ethereum.org/en/developers/docs/smart-contracts/) 가장 좋은 시작점입니다.

### DAO는 Decentraland의 핵심 스마트 계약을 제어합니다

스마트 계약의 두 번째 중요한 특성은 그들의 **다른 스마트 계약을 소유할 수 있는 능력입니다**.

맞습니다. 모든 스마트 계약은 자체 주소(당신의 이더리움 지갑 주소와 마찬가지로)를 가지고 있어, 다른 스마트 계약과 암호화폐를 소유할 수 있습니다.

그러므로 조금 더 기술적으로 말하면, DAO는 특정하고 미리 정의된 작업을 수행하고 암호화폐의 소유권을 유지할 수 있는 하나 이상의 스마트 계약입니다. DAO는 MANA, NAMES 또는 LAND와 같은 특정 토큰을 소유한 사람들의 그룹이 투표한 제안안이 통과되는 등 특정 조건에서만 작업을 수행하도록 설계됩니다. 이 모든 것은 블록체인에서 이루어집니다. 그래서 이름이 "탈중앙화 자율 조직"입니다.

Decentraland의 DAO는 LAND 및 Estate 스마트 계약과 함께 일정량의 MANA 및 기타 토큰도 소유하고 있습니다. [이 기금은](https://governance.decentraland.org/transparency/) 커뮤니티가 투표한 결정과 방향에 따라 커뮤니티 보조금을 후원하고 Decentraland 플랫폼의 성장을 돕기 위해 따로 마련되었습니다.


> [!WARNING]
> **📔 참고** DAO는 소유하지 않으므로 수정할 수 없는 것은 다음과 같습니다 [MANA 스마트 계약](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#readContract).
> 
> MANA 계약의 소유자는 [TokenSale 계약입니다](https://etherscan.io/address/0xa66d83716c7cfe425b44d0f7ef92de263468fb3d#readContract). TokenSale 계약의 소유자는 배포 시 자체 소멸한 별도의 계약입니다([여기 Etherscan에서 볼 수 있듯이](https://etherscan.io/address/0xdf861993edbe95bafbfa7760838f8ebbd5afda9f)). 이는 MANA 공급량을 수정하거나 일시 중지할 권한이 있는 다른 계약이나 지갑이 없다는 뜻입니다.


DAO가 제어하는 다른 정보도 있습니다. 예를 들면 Decentraland에서 허용되지 않는 유해하거나 공격적인 이름 목록, 신규 사용자에게 홍보될 주요 위치(POI, 관심 지점) 목록, 그리고 Decentraland의 가상 세계를 호스팅하는 커뮤니티 운영 서버 목록입니다.

DAO의 MANA를 전송하거나, LAND 또는 Estate 스마트 계약을 수정하거나, DAO가 제어하는 다른 나열된 정보를 수정하는 것은 **오직** MANA, NAMES 및 LAND 보유자의 승인을 받아야만 가능합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/dao/dao/what-is-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
