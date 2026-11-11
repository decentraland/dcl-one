> DAO를 통해 사용자는 메타버스를 형성하는 제안을 생성하고 투표할 수 있습니다.

# DAO로 무엇을 할 수 있나요

DAO는 두 가지 일반적인 유형의 제안을 허용합니다: **직접 구속력이 있는 조치가 포함된 제안**, 그리고 **거버넌스 제안**.

## 직접 구속력이 있는 조치가 포함된 제안

DAO는 커뮤니티가 다음 항목에 대해 투표할 수 있도록 합니다 **구속력 있는 조치** 이는 Ethereum 네트워크에서 Decentraland의 스마트 계약에 변경이 이루어지게 됩니다. 이러한 구속력 있는 조치는 다음과 같습니다:

* DAO의 자원 일부를 보조금 베스팅 계약으로 이전하여 커뮤니티 프로젝트에 자금을 지원하는 것.
* Decentraland의 가상 세계를 호스팅하고 운영하는 서버 네트워크에 카탈리스트 노드를 추가하는 것.
* 사용자에게 표시되는 목록에 관심 지점(POI) 또는 가상 세계 내의 강조된 위치를 추가하거나 제거하는 것. 이 목록은 사용자가 탐험할 인기 있고 흥미로운 장소를 찾는 데 도움이 됩니다.
* Decentraland에서 이름을 금지하는 것. 이 제안 유형은 아바타에 공격적이거나 해로운 이름이 부여되지 않도록 보장할 수 있게 합니다.

## 거버넌스 제안

일부 제안은 목록에서 항목을 추가하거나 제거하는 것처럼 단순하지 않으며, 커뮤니티 신호, 논의, 구현 경로가 필요합니다. 이러한 제안은 투표로 시작해 구속력 있는 제안으로 끝나는 3단계 거버넌스 절차를 통해 제출해야 합니다.

투표 과정은 사전 제안 설문조사, 초안 제안, 거버넌스 제안의 세 단계로 구성됩니다. 각 단계는 중요한 거버넌스 결정이 대표성 있는 다수(투표 권한 기준)에 의해 이루어지도록 제출 및 통과 기준이 점차 높아집니다. 다음 단계로 승격되려면 각 단계가 정의된 VP 기준에 도달해야 합니다.

### 1단계: 사전 제안 투표

* 제출 기준: 100 VP
* 통과 기준: 500K VP (최소 500K VP에 도달했지만 참여한 투표권의 과반을 얻지 못한 투표도 초안 제안 단계로 진행될 수 있습니다. 이를 통해 충분한 지지를 받은 모든 사안이 정책으로 통과될 초기 경로를 갖게 됩니다.)
* 투표 기간: 5일
* 목표: 거버넌스 이슈를 커뮤니티에 소개하고, 커뮤니티의 반응을 파악하며, 초기 제안 초안을 작성할 만큼 충분한 지지가 있는지 판단하는 것.

### 2단계: 초안 제안

* 제출 기준: 1,000 VP
* 통과 기준: 1M VP 및 참여한 투표권의 단순 과반수(51%) (실패하거나 이 기준에 도달하지 못한 초안 제안은 1회에 한해 수정하여 다시 제출할 수 있습니다.)
* 투표 기간: 1주
* 목표: 구조화된 형식으로 잠재적 정책을 커뮤니티에 제시하고, 제안의 잠재적 영향과 구현 경로에 대한 논의를 공식화하는 것. 이 기준에 도달하지 못하거나 실패한 초안 제안은 1회에 한해 수정하여 다시 제출할 수 있습니다.

### 3단계: 거버넌스 제안

* 제출 기준: 2,500 VP
* 통과 기준: 6M VP 및 참여한 VP의 단순 과반수(또는 해당 범주의 필요한 승인 기준)
* 투표 기간: 2주
* 목표: 통과된 초안 제안 버전을 구속력 있는 거버넌스 결과로 공식화하는 것.

제출 기준을 충족하는 사람이라면 누구나 제안 작성자뿐만 아니라, 통과된 제안(투표 또는 초안)을 다음 단계로 넘길 수 있다는 점을 유의해야 합니다.

## LAND 또는 Estate 스마트 계약을 수정하는 것은 어떻습니까?

현재 DAO는 LAND와 Estate 스마트 계약을 모두 소유하고 있습니다. 이 두 계약 중 어느 하나라도 수정하려면 보안 자문 위원회(SAB)와 DAO 위원회가 수행해야 합니다. 이들은 Decentraland 인프라의 중요한 요소들의 지속적인 보안을 보장하도록 임무를 맡은 신뢰받고 선출된 사람들로 구성된 그룹입니다.

SAB는 LAND 또는 Estate 계약의 변경을 승인하기 위해 멀티시그 지갑을 통해 투표해야 하며, 이를 통해 일탈한 SAB 구성원이 취약점을 도입하는 것을 방지합니다.

현재 DAO의 UI를 통해 LAND 또는 Estate 스마트 계약을 수정하기 위한 사전 정의된 제안 범주는 없습니다. 이는 SAB가 아닌 구성원이 변경을 시작하는 것이 불가능하다는 뜻은 아니지만, 코드를 수정할 기술적 전문성이 필요하고 그 변경 사항을 감사할 신뢰할 수 있는 제3자가 필요하므로, 더 넓은 커뮤니티의 지지를 얻어야 하는 더 긴 절차가 필요합니다.

LAND 및 Estate 계약의 소스 코드는 GitHub에서 확인할 수 있습니다 [여기](https://github.com/decentraland/land/tree/master/contracts).

일반적으로 말하면, 두 계약 중 하나를 수정하는 절차는 다음과 같습니다:

* DAO를 통해 **사전 제안 투표를**게시하여 커뮤니티에 의견을 묻고, 제안된 변경 사항을 공개적으로 논의해 지지를 얻습니다
* 초기 커뮤니티의 지지를 얻은 후, **초안 제안** 를 게시하여 변경 사항을 상세히 설명하고 투표를 통해 수락을 받습니다.
* 계약에 병합될 업데이트된 코드를 작성합니다
* 신뢰할 수 있는 제3자로부터 성공적인 코드 리뷰와 감사를 받습니다
* 감사를 마친 코드를 구속력 있는 제안을 사용해 커뮤니티에 제시하고 **거버넌스 제안** 계약에 병합되도록 승인을 받습니다
* 다른 커뮤니티 투표에서 승인되면, DAO 위원회 또는 SAB가 새 코드로 계약 업그레이드를 수행합니다


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/dao/dao/what-can-you-do-with-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
