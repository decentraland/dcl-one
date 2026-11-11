> Decentraland DAO 사용 방법

# DAO 사용자 가이드

## 목차

* [로그인](#logging-in)
* [투표 권한](#voting-power)
* [승인/거부 조건](#approvereject-conditions)
* [제안 찾아보기](#browsing-all-proposals)
* [제안 보기](#viewing-a-proposal)
* [감시 목록에 제안 추가하기](#adding-proposals-to-your-watch-list)
* [투표](#voting)
* [포럼 참여하기](#participating-in-the-forum-discussions)
* [제안 만들기](#creating-a-proposal)
* [제안 카테고리](#proposal-categories)
* [제안 삭제하기](#deleting-a-proposal)

## 로그인

DAO를 시작하려면 [governance.decentraland.org](https://governance.decentraland.org). 환영 화면과 빠른 시작 튜토리얼이 표시됩니다.

빠른 시작 튜토리얼을 읽은 후 **로그인**을 클릭하세요. DAO를 사용하려면 지갑을 연결해야 합니다. 현재는 다음 중 하나를 사용할 수 있습니다. **Metamask** 또는 **Fortmatic**. DAO에 참여하는 데 필요한 관련 토큰(MANA, NAMES 또는 LAND)을 지갑에 보유하고 있는지 확인하세요.

지갑을 연결하면 현재 활성화된 모든 제안 목록이 있는 DAO 홈페이지로 이동합니다.

## 투표 권한

투표 권한은 DAO에 연결된 지갑의 MANA, NAMES 및 LAND 총 잔액으로 계산됩니다. **DAO는 래핑된 MANA와 래핑되지 않은 MANA 모두를 확인하므로, 전체 투표 권한을 얻기 위해 MANA를 언래핑할 필요는 없습니다.** 하지만 이전에 래핑한 MANA는 언제든지 "투표 권한" 탭에서 언래핑할 수 있습니다. MANA 언래핑은 이더리움 메인넷에서 이루어지는 거래이므로 가스비가 발생합니다.

Decentraland DAO에서의 당신의 투표는 로그인한 계정에 연결된 MANA, NAMES 및 LAND 잔액에 따라 가중치가 부여됩니다.

현재 투표 권한을 보려면 다음으로 이동하세요. **투표 권한** 탭.

#### 투표 권한은 어떻게 계산되나요?

투표 권한은 **"VP"**&#xB85C; 표시됩니다. MANA, NAME, LAND 및 레거시 웨어러블은 다음과 같이 총 투표 권한에 기여합니다:

* MANA 1개는 VP 1을 기여합니다
* NAME 1개는 VP 100을 기여합니다
* LAND 부지 1개는 VP 2000을 기여합니다
* 다음 레거시 웨어러블 컬렉션은 각각 다음에 대해 VP 1을 기여합니다 **언커먼**, 5 VP는 각각 **레어**, 10 VP는 각각 **에픽**, 100 VP는 각각 **레전더리**, 1000 VP는 각각 **미식**:
  * [커뮤니티 콘테스트 컬렉션](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
  * [DCL 퍼블릭 익스플로러 런치 컬렉션](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
  * [익스클루시브 마스크 컬렉션](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
  * [할로윈 2019 컬렉션](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
  * [마이 크립토 히어로즈 컬렉션](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
  * [크리스마스 2019 컬렉션](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* 각 에스테이트의 가치는 해당 에스테이트에 있는 단일 LAND 부지 수에 2000을 곱한 값입니다. 예를 들어, 2개 부지가 있는 에스테이트는 총 투표 권한에 4000 VP를 기여합니다.

#### 제안이 종료되기 전에 투표 권한이 변경되면 어떻게 되나요?

MANA 또는 LAND 잔액이 변경되면 투표 권한에 영향을 미치지만, 잔액이 변경된 이후에 생성된 제안에만 적용됩니다.

새 제안이 생성되는 순간 DAO는 모든 투표자의 MANA 및 LAND 잔액을 확인해 투표 권한을 계산합니다. 다시 말해, 제안에 투표할 때 해당 투표의 투표 권한은 제안이 처음 생성된 순간의 잔액과 동일합니다. 그 VP로 투표한 뒤 MANA/LAND 잔액을 변경해도 투표의 가중치는 변하지 않습니다.

#### Decentraland DAO는 왜 가중 투표를 사용하나요?

Decentraland DAO가 각 이더리움 주소에 한 표씩 부여하고 각 표의 가중치를 동일하게 한다면, 사용자는 더 많은 투표 권한을 얻기 위해 원하는 만큼 여러 개의 별도 주소를 만들 수 있습니다.

MANA, NAME 및 LAND 잔액을 고려하여 투표 권한을 결정하는 것이 현재로서는 각 투표자가 가질 수 있는 영향력의 양을 제한하는 가장 안전한 방법입니다. 또한 Decentraland에서 보유한 MANA, NAME 또는 LAND가 많을수록 개인의 지분이 더 크다고 간주되므로, DAO 내에서 당신의 표가 더 큰 영향력을 갖게 됩니다.

## 승인/거부 조건

제안 유형에 따라 투표를 유효하다고 간주하기 위해 일정량의 참여 VP가 필요합니다. 거버넌스 dApp에서는 이것이 제안의 **승인 임계값** 으로 표시됩니다. 승인 임계값을 충족하면, 찬성 측의 총 투표 권한이 반대 측의 총 투표 권한보다 클 경우 제안이 승인됩니다. 이는 우리가 대부분의 민주적 투표에서 보게 되는 전형적인 50/50 다수결 모델입니다.

이 규칙을 따르지 않는 유일한 제안 유형은 **사전 제안 설문조사**입니다. 이러한 제안은 아이디어에 대한 커뮤니티 피드백을 수집하기 위한 구속력 없는 메커니즘으로 간주되므로, 투표를 위한 예/아니오 옵션이 미리 정해져 있지 않습니다. 사용자는 두 개보다 많은 옵션을 추가하여 객관식 설문조사로 만들 수 있습니다. 사전 제안 설문조사가 정의된 500k VP의 승인 임계값에 도달하면 **초안 제안** 으로 승격되어 **거버넌스 제안**.

## 모든 제안 찾아보기

DAO의 모든 제안을 보려면 다음을 방문하세요. [governance.decentraland.org](https://governance.decentraland.org). DAO 홈페이지에는 최근 추가된 모든 제안이 기본적으로 최신순에서 오래된 순으로 정렬되어 표시됩니다.

다음 기준으로 제안을 필터링할 수 있습니다. **결과**:

* **활성** – 현재 투표 중인 제안
* **통과** – 커뮤니티에 의해 승인된 제안
* **거부됨** – 이미 투표가 끝났으며, 커뮤니티에 의해 거부되었거나 승인 임계값을 충족하지 못한 제안
* **실행됨** – DAO 위원회에 의해 온체인에 실행된 제안
* **종료됨** – 닫혔지만 예/아니오 결과가 없는 제안, 예를 들어 객관식 설문조사

또한 다음 기준으로 제안을 필터링할 수 있습니다. **카테고리** UI 왼쪽의 카테고리 열을 사용하여:

* **모든 제안** – 카테고리나 결과와 관계없이 모든 제안을 표시합니다
* **카탈리스트 노드** – 새로운 카탈리스트 노드를 추가하는 제안만 표시합니다
* **관심 지점** – 새로운 POI를 추가하는 제안만 표시합니다
* **이름 금지** – 이름을 금지하는 제안만 표시합니다
* **보조금 요청** – 보조금 요청만 표시합니다
* **사전 제안 설문조사** – 구속력 없는 설문조사만 표시합니다
* **초안 제안** – 초안 제안만 표시합니다
* **거버넌스 제안** – 최종 구속력 있는 거버넌스 제안만 표시합니다

통과한 제안만 보려면 **실행됨** 탭을 클릭하세요. 이 탭의 실행된 제안은 기본적으로 최신순에서 오래된 순으로 정렬됩니다.

## 제안 보기

제안을 읽으려면 제안 제목을 클릭하여 세부 정보 페이지를 보세요.

각 제안의 세부 정보 페이지에는 제안을 제출한 사람이 제공한 모든 설명 정보가 포함되어 있습니다.

또한 포럼의 제안 토론 스레드 링크, 제안을 감시 목록에 추가하는 버튼, 현재 투표 결과, 투표 버튼, 승인 임계값, 현재 투표 권한, 그리고 누가 제안에 투표했는지도 확인할 수 있습니다.

제안 세부 정보 페이지에는 제안을 올린 이더리움 주소와 연결된 고유한 아바타 이름이 있는 경우 해당 이름과 제안의 시작 및 종료 날짜도 표시됩니다.

마지막으로 Decentraland DAO가 사용하는 투표 플랫폼인 Snapshot의 제안 항목 링크도 볼 수 있습니다.

## 제안을 감시 목록에 추가하기

제안을 감시 목록에 추가하려면 제안의 세부 정보 페이지를 보고 다음을 클릭하세요. **감시 목록에 추가**.

제안을 감시 목록에서 제거하려면 다음을 클릭하세요. **감시 목록에서 제거** 를 제안의 세부 정보 페이지에서 클릭하세요. 감시 목록에 있는 어떤 제안에서든 빨간 깃발 아이콘을 클릭하여 제거할 수도 있습니다.

## 포럼 토론에 참여하기

거버넌스 플랫폼으로서 Decentraland DAO는 커뮤니티 내에서 활발한 토론과 함께할 때 가장 효과적입니다. 새 제안이 **governance.decentraland.org**에 게시될 때마다, 관련 주제가 자동으로 [**forum.decentraland.org**](https://forum.decentraland.org).

에 열립니다. 투표하기 전에 시간을 내어 이러한 포럼 토론을 보고 참여해 주세요. 버튼을 클릭하세요. **포럼에서 토론하기** 토론하고 싶은 DAO 제안의 세부 정보 페이지에서. 또한 다음으로 이동하여 열린 주제를 둘러볼 수도 있습니다. [forum.decentraland.org](https://forum.decentraland.org), 그리고 **거버넌스** 카테고리로 이동해 열린 주제를 둘러보세요.

이러한 토론에 참여하기 위해 토큰을 보유할 필요는 없습니다! 누구나 대화에 기여할 수 있습니다.

## 투표

제안에 투표하려면 [governance.decentraland.org](https://governance.decentraland.org) 에서 MANA, NAME 또는 LAND를 보유한 지갑으로 DAO에 로그인하세요.


> [!NOTE]
> **투표에 필요한 최소 잔액:** MANA, NAME 또는 LAND 보유자만 Decentraland DAO의 제안에 투표할 수 있습니다. **DAO 제안에 가중 투표를 하려면 현재 필요한 최소 잔액은 1VP입니다** 잔액이 0인 상태로 투표하면 가중치 0의 표가 되어 최종 결과에 영향을 주지 않습니다.


지갑을 연결한 후 제안에 투표하려면 제안의 세부 정보 페이지를 보고 다음 **찬성 투표** 또는 **반대 투표** 버튼을 클릭하거나, 설문조사인 경우 원하는 투표 방식에 따라 객관식 옵션 중 하나를 선택하세요.

제안이 승인되거나 거부되면 어떤 일이 일어나는지 이해할 수 있도록 전체 제안을 반드시 읽어보세요.

투표 버튼을 클릭한 후, 연결된 이더리움 지갑에서 거래 서명을 요청합니다. 이는 서명된 거래일 뿐이며 가스비가 필요하지 않다는 점을 기억하세요.

제안을 감시 목록에 추가할지 선택할 수 있으며, 이는 관심 있는 제안을 추적하는 좋은 방법입니다. 제안을 감시 목록에 추가하고 싶지 않다면 다음을 클릭하세요. **괜찮아요**.

투표를 제출한 후에는 투표 기간이 끝날 때까지 언제든지 변경할 수 있습니다. 이는 제안의 세부 정보 페이지에 표시됩니다.

#### 제안이 완료되기 전에 VP가 변경되면 어떻게 되나요?

DAO는 각 개별 제안이 생성되는 순간 각 제안의 투표 권한을 계산합니다. 이 순간 이후 VP가 변경되어도 해당 제안에 대한 투표에는 영향을 미치지 않습니다.

투표 권한과 그것이 어떻게, 언제 계산되는지에 대한 전체 설명은 사용자 가이드의 [투표 권한](#voting-power) 섹션을 참조하세요.

## 제안 만들기

Decentraland DAO에서 새 제안을 만들려면 먼저 [governance.decentraland.org](https://governance.decentraland.org) 에서 로그인하고 MANA, NAME 또는 LAND를 보유한 지갑을 연결하세요.

로그인하고 지갑을 연결한 후 다음을 클릭하세요. **제안 제출** 을 클릭한 다음 사용하려는 제안 카테고리를 선택하세요. 각 카테고리에는 제안에 필요한 관련 정보를 제공할 수 있는 양식이 제공됩니다.

카테고리마다 양식이 다르므로 제안에 맞는 올바른 카테고리를 선택해야 합니다. DAO의 제안 양식은 Markdown을 지원하므로, 제안 내용을 더 읽기 쉽도록 서식을 지정할 수 있습니다. 이는 특히 긴 제안에 유용합니다.

렌더링된 Markdown 텍스트를 미리 보려면 **미리보기** 스위치를 켜세요. Markdown에 익숙하지 않다면 간단한 일반 텍스트를 사용할 수 있습니다.

일부 제안 카테고리에는 최소 VP 제출 기준이 있어, 제안을 제출하려면 최소한 일정량의 VP를 보유해야 합니다. 이는 거버넌스 dApp에서 안내됩니다.

선택한 카테고리의 제안 양식을 완료한 후 다음을 클릭하세요. **제안 제출**. 제안을 성공적으로 제출하면 새 제안의 세부 정보 페이지로 이동하며, 거기에서 감시 목록에 추가하고 투표할 수 있습니다.

## 제안 카테고리

### 구속력 있는 제안

#### 📍 관심 지점

관심 지점은 Decentraland의 주목할 만한 위치입니다. 이러한 "POI"는 가상 세계 UI의 여러 영역에서 강조되며, 특히 Decentraland를 처음 접하는 사람들에게 탐색하기 좋은 장소로 홍보됩니다.

위치를 만들었거나 이 목록에 포함되어야 한다고 생각하는 위치를 찾았거나, 더 이상 흥미롭지 않아 현재 POI를 제거해야 한다고 생각한다면 이 제안 카테고리를 사용해 DAO에 제안을 제출할 수 있습니다.

#### 🚫 이름 금지

"금지된 이름" 목록에는 Decentraland에서 허용되지 않는 공격적이거나 유해한 아바타 및 위치 이름이 포함됩니다. 이 목록의 어떤 이름도 주장, 사용 또는 사용자 간 이전할 수 없습니다. 이름을 금지하자고 제안하려면 DAO에서 이름 금지 제안 카테고리를 사용하면 됩니다.

#### 🌐 카탈리스트 노드

카탈리스트 노드는 Decentraland의 가상 세계를 계속 작동시키는 데 필요한 콘텐츠를 제공하고 P2P 연결을 설정하는 커뮤니티 운영 서버입니다. 사용자가 Decentraland를 열 때마다 이 노드 중 하나에 연결됩니다. 하지만 DAO에서 승인한 노드만 Decentraland 네트워크에서 사용됩니다.

네트워크에 새 노드를 추가하자고 제안하려면 카탈리스트 노드 카테고리를 사용할 수 있습니다.

### 거버넌스 프로세스 제안

일부 제안은 목록에 항목을 추가하거나 제거하는 것처럼 단순하지 않으며, 커뮤니티의 신호, 토론 및 구현 경로가 필요합니다. 이러한 제안은 설문조사로 시작해 구속력 있는 제안으로 끝나는 3단계 거버넌스 프로세스를 통해 제출해야 합니다.

투표 과정은 사전 제안 설문조사, 초안 제안, 거버넌스 제안의 세 단계로 구성됩니다. 각 단계는 중요한 거버넌스 결정이 대표성 있는 다수(투표 권한 기준)에 의해 이루어지도록 제출 및 통과 기준이 점차 높아집니다. 다음 단계로 승격되려면 각 단계가 정의된 VP 기준에 도달해야 합니다.

#### 📊 사전 제안 설문조사

이는 최종 구속력 있는 거버넌스 제안으로 가는 첫 단계입니다. Decentraland DAO의 설문조사는 구속력이 없고 객관식 질문지로, 다양한 문제에 대한 커뮤니티의 전반적인 의견이나 정서를 측정하는 데 사용될 수 있습니다. DAO가 이러한 설문조사 결과에 자동으로 조치하지 않기 때문에 구속력이 없습니다. 사전 제안 설문조사가 충분한 참여 VP(승인 임계값은 500k VP)를 모으면 초안 제안으로 승격될 수 있습니다.

#### 📊 초안 제안

초안 제안은 잠재적인 정책을 구조화된 형식으로 커뮤니티에 제시하고, 제안의 잠재적 영향과 구현 경로에 대한 논의를 공식화합니다. 이 기준을 충족하지 못하거나 도달하지 못한 초안 제안은 한 번 수정하여 다시 제출할 수 있습니다. 초안 제안이 충분한 참여 VP(승인 임계값은 1M VP)를 모으면 구속력 있는 거버넌스 제안으로 승격될 수 있습니다.

#### 📊 거버넌스 제안

이는 거버넌스 프로세스의 마지막 단계이며 유일하게 구속력이 있습니다. 이 제안은 이 제안을 구현하는 데 필요한 모든 세부 정보, 데이터, 방법, 평가 또는 기타 관련 정보를 구체적으로 담아야 합니다. 거버넌스 제안은 승인 임계값인 6M VP를 충족하고 찬성 옵션이 단순 과반수를 얻으면 채택됩니다.

## 제안 삭제하기

제안은 만든 사람만 삭제할 수 있습니다.

자신의 제안 중 하나를 삭제하려면 삭제하려는 제안의 세부 정보 페이지로 이동하세요. 다음을 클릭하세요. **제안 삭제** 를 오른쪽 열 하단에서 클릭하세요. 삭제를 확인해야 합니다.

주의하세요! 삭제된 제안은 복원할 수 없습니다!

사람들이 이미 투표한 뒤 제안을 삭제해도 아무 일도 일어나지 않습니다. 구속력 있는 조치가 포함된 삭제된 제안은 DAO 위원회에 의해 실행되지 않습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/dao/dao-userguide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
