> Decentraland DAO를 구성하는 다양한 플랫폼과 주체에 대한 개요.

# DAO는 어떻게 작동하나요

전면적인 온체인 거버넌스와 관련된 매우 높은 가스비를 피하기 위해, Decentraland의 DAO는 커뮤니티를 위한 무료 오프체인 투표와, "DAO 위원회"가 관리하는 멀티시그 지갑을 함께 사용해 이러한 오프체인 결정을 이더리움 블록체인에서 실행합니다. 이를 통해 MANA, NAMES 또는 LAND를 보유한 사람은 누구나 투표하거나 제안을 열 때마다 수수료를 지불하지 않고도 DAO에 참여할 수 있습니다.

신뢰할 수 있는 인물들로 구성된 위원회가 관리하는 멀티시그 지갑을 사용하면, 통과된 제안이 온체인 조치로 이어진다는 점과 함께 DAO의 보안이 보장됩니다. SAB가 소유한 두 번째 멀티시그는 DAO 위원회 위에 추가적인 보호 계층을 제공합니다.

DAO는 여러 계층의 조직과 플랫폼으로 구성된 복잡한 시스템이며, 각 항목은 아래에 나열되고 설명되어 있습니다:

## 거버넌스 dApp

Decentraland DAO의 주요 인터페이스는 다음에 있습니다 [governance.decentraland.org](https://governance.decentraland.org). 여기서 사용자는 로그인하고, 제안을 생성하고, 투표합니다.

*현재 이는 다음의 지원을 받는 개발팀이 유지 관리하고 있습니다.* [*DAO 보조금*](https://governance.decentraland.org/proposal/?id=ed53e850-5e70-11ec-8188-4352ce3d30e7)*.*

## Snapshot

[Snapshot](https://snapshot.org/#/) 오프체인 투표 플랫폼으로, 토큰 보유자가 제안에 무료로 투표할 수 있는 방법을 제공합니다. 제안과 투표를 모두 암호학적으로 서명된 메시지로 IPFS에 저장함으로써, Snapshot은 안전하고 이의 제기가 쉬운 결과를 가능하게 합니다.

Decentraland의 DAO는 커뮤니티가 생성한 제안과 투표를 호스팅하기 위해 Snapshot을 사용합니다.

governance.decentraland.org에서 제안이 열릴 때마다, 또한 자동으로 다음 곳에 생성됩니다 [Decentraland의 Snapshot 공간](https://snapshot.org/#/snapshot.dcl.eth). 이를 통해 DAO는 제안, 투표 및 결과를 안전하고 분산된 방식으로 기록하고 저장할 수 있으며, 결과는 governance.decentraland.org에 다시 표시됩니다. Snapshot 공간에서 사용자는 또한 [의결권을 위임할 수 있습니다](https://snapshot.org/#/delegate/snapshot.dcl.eth) 다른 커뮤니티 구성원에게.

## Aragon

[Aragon](https://aragon.org/) DAO를 운영하는 데 필요한 스마트 계약 모음을 생성하고 관리하기 위한 안전한 플랫폼입니다. Decentraland DAO의 백엔드는 Aragon을 사용해 구축되어 있으므로, 구속력 있는 조치로 이어지는 제안이 Snapshot에서 커뮤니티에 의해 통과될 때마다 DAO 위원회가 Aragon에서 이더리움 메인넷에 반영합니다.

## DAO 위원회

DAO 위원회는 커뮤니티가 다중 서명 지갑의 키를 보유하도록 선정한 3명의 신뢰할 수 있는 개인으로 구성된 그룹입니다. 이 멀티시그는 보조금 자금 지원, 이름 차단, POI 추가 또는 제거, 거버넌스 제안 이행 또는 카탈리스트 노드 추가와 같은 구속력 있는 조치가 포함된 통과된 모든 투표를 실행하는 역할을 맡고 있습니다.

DAO 위원회는 SAB의 감독을 받으며, SAB는 위원회가 시작한 모든 조치를 일시 중지하고 취소할 수 있습니다.

DAO 위원회가 시작한 모든 온체인 거래는 완료되기 전에 자동으로 24시간 지연되며, 이를 통해 SAB 또는 위원회가 해당 거래를 철회할 수 있습니다.

![DAO 위원회](/docs/_remote/063ee68d732926eeb99173e9.webp)

## 보안 자문 위원회(SAB)

보안 자문 위원회는 Decentraland의 스마트 계약 보안을 보증하는 역할을 하며, DAO 위원회의 업무를 감독하고 Decentraland의 모든 계약에서 발생하는 취약점 및 버그 보고에 대응하는 임무를 맡고 있습니다.

SAB에는 Decentraland 개발팀이 처음 선정한 5명의 Solidity 전문가가 포함됩니다.

LAND 또는 Estate 계약을 수정해야 할 때마다, 해당 업데이트는 SAB의 멀티시그의 만장일치 지지를 받아야 합니다. LAND 또는 Estate 계약을 변경하려면 최소 3명의 서명자가 필요하며, 반대 표결이 없어야 합니다.

SAB는 DAO 위원회가 취한 모든 조치를 일시 중지, 재개 또는 취소할 수 있습니다.

SAB 구성원의 추가 또는 제거를 시작하는 것은 다음에서 거버넌스 제안 절차를 시작함으로써 할 수 있습니다 [governance.decentraland.org](https://governance.decentraland.org).

![보안 자문 위원회](/docs/_remote/199ca9caded989d293a0f36d.webp)

## 웨어러블 큐레이션 위원회

큐레이션 위원회는 Decentraland 커뮤니티가 제출한 웨어러블과 이모트를 검토하고 승인하는 책임을 맡고 있습니다.

![웨어러블 큐레이션 위원회](/docs/_remote/dc4a02e026b26163106c05ac.webp)

## 철회 위원회

철회 위원회는 커뮤니티가 Grants Program과 관련해 제기한 사례를 검토할 책임이 있으며, 최종 조치로 베스팅 계약을 철회하는 해결책을 제시할 책임이 있습니다.

[철회 위원회](https://github.com/lordlikedao/documentation/blob/main/static/images/DAO%20Organizational%20Chart/Revocation.png?raw=true)

## 거버넌스 스쿼드

거버넌스 스쿼드는 DAO의 기술 인프라와 의사결정 도구를 개발, 개선 및 유지 관리하는 책임을 맡고 있습니다.

![거버넌스 스쿼드](/docs/_remote/f6663bc09324c4879744664c.webp)

## 퍼실리테이션 스쿼드

퍼실리테이션 스쿼드는 커뮤니케이션을 관리하고 다양한 이해관계자를 정렬하며, 집단적 의사결정과 효율적인 거버넌스 운영을 촉진하는 책임을 맡고 있습니다.

![퍼실리테이션 스쿼드](/docs/_remote/058f90f95aed65ab975d706d.webp)

## 그랜트 지원 스쿼드

그랜트 지원 스쿼드는 수혜자 커뮤니티와 지속적으로 대화하여 그들의 필요를 파악하고, 그들의 장애 요소와 요청에 대한 지원을 제공할 책임이 있습니다.

![그랜트 지원 스쿼드](/docs/_remote/28cc4e3f4a37dfd9819f791d.webp)

## 포럼

디 [Decentraland 포럼](https://forum.decentraland.org) DAO의 커뮤니케이션 허브입니다. 새로운 제안이 생성될 때마다, 이에 대응하는 스레드가 자동으로 포럼의 [거버넌스 섹션](https://forum.decentraland.org/c/governance/). 이러한 제안 주제는 포럼을 둘러보거나, 거버넌스 UI의 어떤 제안 상세 페이지에서든 "포럼에서 논의하기'' 버튼을 클릭하면 찾을 수 있습니다. 이러한 스레드는 유권자들이 제안으로 인해 발생할 수 있는 다양한 영향과 문제를 논의할 수 있는 공간입니다.

## Discord 서버

DAO는 두 개의 Discord 서버에서 운영됩니다:

* [#dao 채널](https://discord.com/channels/417796904760639509/538106198419832844) Decentraland Discord 서버에서 일반 공지, 토론 및 질문용입니다.
* 하나의 [전용 Discord 서버](https://dcl.gg/daodiscord) 더 심층적인 논의를 위한 DAO 운영 전용이며, 복잡한 이슈를 다루는 작업 그룹을 위한 전용 채널과 커뮤니티 그랜트 수혜자들이 자신의 프로젝트 진행 상황을 게시할 수 있는 공간이 있습니다. 이 채널에서 DAO는 매월 타운홀도 개최합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/dao/dao/how-does-the-dao-work.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
