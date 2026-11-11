> Decentraland 게임의 플레이테스트를 진행하고 플레이어 피드백을 개선 사항으로 반영하는 방법

# 플레이테스트

플레이테스트는 실제 플레이어가 게임을 어떻게 경험하는지 알아봄으로써 더 나은 게임을 만드는 데 도움이 됩니다.

시작하려면 완성된 게임이나 대규모 테스터 그룹이 필요하지 않습니다. 초기에 테스트하고, 개선하고, 게임이 발전함에 따라 다시 테스트할 수 있습니다.

이 가이드는 플레이테스트를 조직하고, 플레이어 피드백을 수집하고, 배운 내용을 활용해 게임을 개선하는 방법을 설명합니다.

## 왜 게임을 플레이테스트해야 하나요?

게임을 만들 때는 이미 어떻게 작동하는지 알고 있습니다. 새로운 플레이어는 그렇지 않습니다.

플레이테스트는 놓치기 쉬운 부분을 발견하는 데 도움이 됩니다:

* **명확성:** 플레이어가 무엇을 해야 하는지 이해하나요?
* **사용 편의성:** 플레이어가 막히지 않고 행동을 완료할 수 있나요?
* **재미:** 어떤 부분이 재미있고, 어떤 부분이 답답한가요?
* **동기 부여:** 플레이어가 계속 플레이하거나 다시 오고 싶어 하나요?

**가능한 한 빨리 플레이테스트를 시작하세요.** 첫 번째 플레이 가능한 버전을 몇 명만 지켜봐도 많은 것을 배울 수 있습니다.

## 테스트할 내용을 선택하세요

각 플레이테스트 전에 무엇을 배우고 싶은지 결정하세요.

한 번에 모든 것을 테스트하려 하기보다 한두 가지 질문에 집중하세요.

| 목표          | 살펴볼 것                                                             |
| ----------- | ----------------------------------------------------------------- |
| 온보딩 테스트     | 새로운 플레이어가 도움 없이 무엇을 해야 하는지 이해할 수 있나요?                             |
| 핵심 메커니즘 테스트 | 플레이어가 주요 게임 메커니즘을 사용할 수 있나요? 재미있나요?                               |
| 게임 루프 테스트   | 플레이어가 어떻게 진행하고 보상을 얻는지 이해하나요?                                     |
| 난이도 테스트     | 플레이어가 어디에서 막히거나, 지루해하거나, 답답해하나요?                                  |
| 감정 반응 테스트   | 플레이어가 흥분하거나, 호기심을 느끼거나, 계속 플레이하고 싶어 하나요? 어떤 순간이 가장 강한 반응을 이끌어내나요? |

가능하다면 당신의 게임에 익숙하지 않은 플레이어를 선택하세요. 경험 많은 플레이어가 더 이상 알아차리지 못하는 문제를 발견할 가능성이 더 높습니다.

### 게임의 단계에 맞게 플레이테스트를 조정하세요

어떤 반응을 원하는지 결정하세요. 초반에는 주로 플레이어가 **이해하는지** 게임을 이해하고 **전략을**세울 수 있는지가 중요합니다: 무엇을 해야 하는지 알고, 그것을 어떻게 잘 해내는지도 파악할 수 있나요? 이것이 탄탄해지면, 게임이 플레이어에게 어떤 **감정**을 주는지 알아야 합니다: 적절한 순간에 흥미롭고, 만족스럽고, 긴장되나요? 그리고 다시 오고 싶어 하나요?

초대하는 플레이어의 유형도 단계에 따라 달라집니다. Decentraland에 익숙한 사람은 클라이언트를 빠르게 통과해 바로 당신의 게임으로 들어갈 것이므로, 메커니즘을 테스트할 때 이상적입니다. 완전한 초보자는 모든 부분에서 헤매게 되는데, 이는 전체 첫 방문 경험을 테스트할 때 בדיוק 원하는 모습입니다.

| 단계             | 배우고 싶은 것                                             | 초대할 사람                                                     |
| -------------- | ---------------------------------------------------- | ---------------------------------------------------------- |
| V0(버티컬 슬라이스)   | 핵심 메커니즘이 이해되고 사용할 수 있나요? 플레이어가 이를 중심으로 전략을 세울 수 있나요? | 이미 Decentraland를 알고 있는 몇몇 플레이어. 그래야 클라이언트가 테스트를 방해하지 않습니다. |
| V1(첫 번째 전체 루프) | 플레이어가 스스로 진행할 수 있나요? 어디에서 막히거나 흥미를 잃나요?              | Decentraland 플레이어와 당신의 게임을 한 번도 해본 적이 없는 사람들의 혼합.          |
| V2 및 이후        | 게임이 플레이어에게 어떤 느낌을 주나요? 다시 오나요? 추천하나요?                | Decentraland 커뮤니티 밖의 사람들을 포함한 새로운 플레이어.                    |

지금의 단계에 맞는 질문을 작성하세요. V0 슬라이스에 대해 “재미있었나요?”라고 묻는 것은 거의 의미가 없습니다. 아직 게임이 충분히 갖춰져 있지 않아 그 답변의 의미가 크지 않기 때문입니다. 이미 여러 차례를 거친 V2에 대해 “무엇을 해야 하는지 이해했나요?”라고 묻는 것은 이미 알고 있는 것에 세션을 낭비하는 셈입니다.

## Scene Feedback로 피드백을 자동 수집하세요

Scene Feedback는 플레이어에게 Decentraland 게임 안에서 직접 질문할 수 있게 해주는 재사용 가능한 모듈입니다.

직접 플레이를 지켜보지 못하더라도 플레이어가 게임을 어떻게 경험하는지 이해하는 데 도움이 됩니다.

다음이 가능합니다:

* 게임플레이 중 특정 순간에 플레이어에게 질문하기.
* 1\~5점 평점과 서면 의견 수집하기.
* World의 Scene Storage에서 CSV 파일로 응답 다운로드하기.

### 먼저 필요한 것

Scene Feedback는 답변을 서버에 저장하므로, 당신의 씬에는 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/)가 필요하며, 다음을 사용해 활성화됩니다 `@dcl/sdk@auth-server` 와 다음을 설정하여 `"authoritativeMultiplayer": true` in `scene.json`.

### Scene Feedback 설치하기

1. 다음을 여세요 [Scene Feedback 저장소](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback).
2. README를 따라 당신의 씬에 모듈을 설치하세요. 설치 안내를 AI 코딩 에이전트에게 줄 수도 있습니다.
3. 플레이테스트 목표에 맞는 작은 질문 세트를 선택하세요.
4. 각 질문이 언제 나타날지 결정하세요.
5. 플레이어 응답을 CSV 파일로 다운로드하세요. Creator Hub에서 다음을 여세요 **관리**그런 다음 당신의 World의 **⋮** 메뉴를 연 다음 **저장소 보기**. 응답은 한동안 여러 파일에 흩어져 있을 수 있습니다(아직 병합되지 않은 최신 저장본과, 이전 버전 모듈의 오래된 단일 파일이 함께 있을 수 있습니다). 파일 이름이 같은 `scene-feedback-<number>`로 시작하는 모든 파일을 다운로드하고, 중복된 `id`s.

저장소에는 바로 사용할 수 있는 질문, 예제, 질문 선택 가이드가 포함되어 있습니다.

AI 코딩 어시스턴트를 사용한다면 [Decentraland SDK Skills](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/)를 설치하거나 업데이트하세요. 무엇을 언제 물어볼지 결정하고, 모듈을 설치하고, 이후 응답을 분석하는 데 도움이 되는 Scene Feedback 스킬이 포함되어 있습니다:

```bash
npx skills add decentraland/sdk-skills --all
```

## 플레이테스트를 조직하세요

플레이테스트를 조직하는 방법은 두 가지가 있습니다.

**옵션 1: 라이브 플레이테스트 진행**

모든 사람을 같은 시간에 초대해 플레이하게 하세요. 플레이어를 관찰하고, 경험을 논의하며, 이후 피드백을 수집할 수 있습니다.

또한 다음에서 이벤트를 만들 수 있습니다 [Decentraland 이벤트](https://decentraland.org/events/) 를 통해 플레이어가 당신의 플레이테스트를 찾고 참여할 수 있습니다.

**옵션 2: 며칠에 걸쳐 플레이테스트 진행**

정해진 기간 동안 플레이어가 가능할 때 언제든지 게임을 테스트하도록 하세요.

플레이하는 동안 응답을 수집하려면 Scene Feedback을 설치하세요.

라이브 세션을 조직하지 않고도 더 많은 플레이어로부터 피드백을 받고 싶을 때 유용합니다.

## 라이브 플레이테스트 진행

라이브 플레이테스트를 통해 플레이어가 게임을 경험하는 모습을 지켜보고 이후에 질문할 수 있습니다.

### 1단계. 게임 준비하기

플레이어가 게임에 접근할 수 있고, 테스트하려는 행동을 완료할 수 있는지 확인하세요. 플레이어가 시도해 볼 두세 가지 과제를 준비하세요.

예를 들면:

* 게임 시작 방법 알아내기.
* 한 라운드 완료하기.
* 업그레이드 구매하기.

### 2단계. 플레이테스터 초대하기

작은 그룹의 플레이어를 초대하세요. 3\~5명의 플레이어가 좋은 시작점입니다. 아직 당신의 게임을 해보지 않은 사람들을 포함하려고 하세요.

다음의 #playtesters 채널에서 관심 있는 플레이어를 찾을 수 있습니다 [Decentraland Discord](https://discord.gg/decentraland). 다음을 참조하세요 [플레이테스터 찾기](#find-playtesters) 를 참고해 다른 찾아볼 곳을 확인하세요.

### 3단계. 플레이어가 플레이하는 모습을 보기

모든 것이 어떻게 작동하는지 설명하지 말고 게임을 해보라고 하세요. 무엇을 하는지, 어디에서 막히는지, 무엇을 즐기는지 관찰하세요.

**계속할 수 없을 때만 플레이어를 도와주세요.** 설명해야 한다면, 그건 게임에 더 명확한 안내가 필요하다는 신호일 수 있습니다.

### 4단계. 피드백 요청하기

플레이 후 몇 가지 열린 질문을 하세요:

* 가장 좋았던 점은 무엇이었나요?
* 헷갈리거나 답답했던 부분이 있었나요?
* 무엇을 해야 할지 몰랐던 순간이 있었나요?
* 다시 플레이할 건가요? 왜 또는 왜 아니죠?
* 이 게임을 친구에게 추천하겠나요? 왜 또는 왜 아니죠?

"새 업그레이드 시스템이 마음에 드셨나요?" 같은 유도 질문은 피하세요. 이런 질문은 받는 답변에 영향을 줄 수 있습니다.

무엇을 물어봐야 할지 잘 모르겠다면, 위에서 언급한 Scene Feedback 스킬이 대면으로 묻든 게임 안에서 묻든 초점을 정하고 질문의 문구를 만드는 과정을 안내해 줄 수 있습니다.

플레이어가 제안한 것뿐 아니라, 플레이어가 겪은 문제도 적어 두세요.

## RITE 방법으로 게임을 개선하세요

RITE는 다음의 약자입니다 **Rapid Iterative Testing and Evaluation**.

플레이테스트를 위한 간단한 접근법입니다: 문제를 찾아 고치고, 가능한 한 빨리 다시 테스트하세요. 한 번의 큰 플레이테스트로 긴 문제 목록을 수집하는 대신, 여러 번의 작은 플레이테스트를 진행하고 각 라운드 사이에 가장 분명한 문제를 수정합니다.

### RITE 사이클을 따르세요

1. **테스트:** 작은 그룹의 플레이어가 당신의 게임을 해보는 모습을 지켜보세요. 라운드당 3\~5명이면 충분합니다.
2. **식별:** 경험에 영향을 주는 문제를 찾으세요.
3. **수정:** 라운드 사이에 문제와 해결책이 모두 명확할 때 변경하세요.
4. **반복:** 새로운 플레이어와 함께 다음 라운드를 진행하세요. 플레이어가 큰 문제 없이 경험을 완료할 수 있을 때까지 계속하세요.

예를 들어 플레이어가 게임 시작 위치를 찾지 못한다면, 다음 플레이테스트 전에 시작 버튼을 더 명확하게 만드세요. 그런 다음 새로운 플레이어가 도움 없이 그것을 찾을 수 있는지 지켜보세요.

**라운드가 진행되는 동안에는 게임을 안정적으로 유지하세요.** RITE는 라운드 사이에 수정하라는 뜻이지, 진행 중에 수정하라는 뜻이 아닙니다. 플레이어가 세션 중간일 때 다시 배포하면 게임이 깨질 수 있고, 어떤 버전에 대한 피드백인지도 더 이상 알 수 없습니다. 한 라운드의 모든 것을 수집한 뒤, 변경하고, 다시 테스트하세요.

### Decentraland에서 RITE를 적용하기

몇 가지 습관만으로도 Decentraland 씬에서 RITE를 실용적으로 사용할 수 있습니다:

* **작게, 빠르게 수정하세요.** 라운드 사이에는 직전 라운드에서 명확하게 문제로 드러난 것만 바꾸세요. 수정에 새 메커니즘이나 재설계가 필요하다면 나중으로 미루고, 라운드 간 변경 사항은 하루 안에 배포할 수 있을 만큼 작게 유지하세요.
* **테스트에는 World를 사용하세요.** 테스트 빌드를 다음에 게시하세요 [World](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/) 라이브 LAND나 메인 World가 아니라. 플레이어는 항상 알려진 빌드에 접속하게 되고, 게시된 게임을 건드리지 않고도 라운드 사이에 다시 게시할 수 있습니다.
* **Scene Feedback 질문은 라운드마다 동일하게 유지하세요.** 질문의 문구를 바꾸면 한 라운드에서 다음 라운드로 평점을 비교할 수 없습니다. 필요하면 질문을 추가하되, 추적 중인 질문은 다시 쓰지 마세요.
* **무엇이 “명확한지” 결정하세요.** 문제가 명확하다는 것은 그 일이 실제로 일어나는 것을 보았고, 플레이어가 대신 무엇을 기대했는지 말할 수 있으며, 무엇을 바꿔야 하는지도 안다는 뜻입니다. 한 명만 걸렸고 왜 그런지 확실하지 않다면, 다음 라운드로 미루고 다시 발생하는지 확인하세요.
* **모든 라운드를 기록하세요.** 각 라운드마다 짧은 기록을 남기세요: 어떤 빌드였는지, 누가 플레이했는지, 무엇을 봤는지, 무엇을 바꿨는지. 세 번째 라운드쯤이면 이 기록이 변경이 실제로 효과가 있었는지 알려줍니다.

RITE는 빠르게 변경할 수 있는 초기 개발 단계에서 특히 잘 맞습니다.

## 플레이테스터 찾기

일반적인 Decentraland 플레이어가 가장 찾기 쉽지만, 다음도 살펴보세요 [Decentraland 커뮤니티 밖의](#outside-the-decentraland-community)사람들: 경험 많은 플레이어가 놓치는 문제를 발견합니다.

### Discord에서

다음의 #playtesters 채널은 [Decentraland Discord](https://discord.gg/decentraland) 게임 테스트를 좋아하고 크리에이터가 게임을 개선하도록 돕는 사람들을 위한 곳입니다. 서버에 가입해 채널을 찾으세요.

### 플레이테스트를 게시하세요

준비가 되면 #playtesters에 다음을 포함한 메시지를 게시하세요:

* **게임:** 게임 이름과 간단한 설명.
* **위치:** Decentraland의 게임 링크.
* **언제:** 플레이테스트 날짜와 시간, 또는 피드백을 받을 기간.
* **테스트할 내용:** 피드백을 원하는 특정 기능이나 게임플레이.
* **피드백 제공 방법:** 라이브 대화, Discord 또는 Scene Feedback를 통해.
* **보상:** 참여에 대해 제공하는 인센티브.

### 플레이어의 참여를 유도하세요

플레이어의 시간을 감사하는 의미로 작은 보상을 제공하는 것을 고려하세요.

예를 들면:

* 참가자에게 특별한 Wearable 또는 이모트를 선물하세요.
* 독점 인게임 보상을 제공하세요. Scene Feedback 예제에는 다음을 보여주는 레시피가 포함되어 있습니다. **플레이테스트 기여자** 라는 라벨을 플레이어의 아바타 위에 [`AvatarNametag`](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-nametags/) 양식의 모든 질문에 답하면 표시합니다. 이 라벨은 해당 플레이어에게만 보이며 방문 기간 동안 유지됩니다. 다음을 참조하세요 [참여에 대한 씬 로컬 보상](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0%2C-1-scene-feedback/EXAMPLES.md#6-a-scene-local-reward-for-participation).
* 자격이 되는 플레이어에게 Marketplace 크레딧을 제공하세요.

**Wearable과 이모트 선물하기.** 데스크톱 클라이언트에서 직접 보유한 Wearable 또는 이모트를 다른 플레이어에게 선물할 수 있습니다. 플레이어를 마우스 오른쪽 버튼으로 클릭하고 다음을 선택하세요. **선물**. 아이템은 상대의 계정으로 전송되며 Backpack에 나타납니다. 소유한 아이템은 [마켓플레이스](https://decentraland.org/marketplace/) 를 사용해 받는 사람의 지갑 주소로도 전송할 수 있습니다. Wearable 또는 이모트 컬렉션을 직접 만들었다면, 인게임에서 그 플레이어를 만나지 않아도 Builder에서 어떤 지갑 주소로든 아이템을 바로 민팅할 수 있습니다. 다음을 참조하세요 [Wearable 민팅](/docs/creator/content-creator-ko/wearables-and-emotes/publishing-collections/publishing-collections/#minting-wearables).

**Marketplace Credits 선물하기는 현재 베타이며 초대된 스튜디오만 이용할 수 있습니다.** 자격이 되는 스튜디오는 플레이어에게 크레딧을 선물할 수 있고, 플레이어는 이를 Decentraland Shop에서 사용할 수 있습니다. 다음을 참조하세요 [크레딧 선물하기(베타)](/docs/creator/content-creator-ko/rewards/gifting-credits/).

보상은 참여를 유도할 수 있지만, 비판을 포함한 진솔한 피드백을 원한다는 점을 분명히 하세요.

### Decentraland 커뮤니티 밖에서

Decentraland를 한 번도 사용해 본 적 없는 사람도 초대할 수 있습니다. Decentraland 플레이어는 이미 클라이언트 작동 방식을 알고 있으므로 바로 당신의 게임으로 갈 것입니다. 커뮤니티 밖의 사람들은 더 이상 보지 않던 부분까지 포함해 모든 것을 새 눈으로 봅니다. 이들과 플레이테스트하는 것은 새로운 플레이어에게 Decentraland를 소개하는 좋은 방법이기도 합니다.

인디 개발자들은 보통 다음에서 테스터를 모집합니다:

* **친구와 가족.** 연락하기 쉽고, 한 시간을 기꺼이 내줄 사람들입니다. 솔직하게 말하고 헷갈릴 때는 헷갈린다고 말해 달라고 하세요.
* **평소에 게임을 잘 하지 않는 사람들.** 이들이 온보딩을 통과할 수 있다면, 거의 누구나 가능합니다.
* **이미 속해 있는 커뮤니티.** 게임 잼 서버, 취미 포럼, 지역 모임, 또는 당신의 게임 테마와 맞는 그룹.

테스터가 Decentraland 초보자라면 들어오는 방법을 도와주세요: 씬의 웹 링크를 공유하세요(`https://decentraland.org/jump/?realm=<your-world-name>.dcl.eth` WORLD의 경우, 또는 `https://decentraland.org/jump/?position=<x>,<y>` LAND의 경우). 이 링크는 게시 후 Creator Hub에 표시됩니다. 클라이언트 설치와 로그인에 대한 짧은 안내도 함께 보내세요. 이 링크는 클라이언트가 설치되기 전에도 브라우저에서 열립니다. 당신의 씬에 도달하기 전에 어디에서 막히는지 기록하되, 그것은 당신의 게임에 대한 피드백과는 별도로 다루세요.

## 피드백을 다음 버전 개선에 활용하세요

각 플레이테스트 후에는 무엇을 만들지 결정하기 전에 배운 내용을 검토하세요.

1. **비슷한 문제를 묶으세요.** 여러 플레이어가 겪은 문제를 찾으세요.
2. **중요한 문제의 우선순위를 정하세요.** 플레이어가 게임을 이해하거나, 진행하거나, 즐기는 것을 막는 것은 무엇이든 고치세요.
3. **개선을 하세요.** 구현이 빠르고 도움이 될 가능성이 높은 변경부터 시작하세요.
4. **다시 테스트하세요.** 새로운 플레이어가 더 나은 경험을 하는지 확인하세요.
5. **변경 사항을 추적하세요.** 무엇을 바꿨고 다음 플레이테스트에서 어떤 일이 일어났는지 짧게 기록해 두세요.
6. **바뀐 내용을 공유하세요.** 플레이테스터에게 무엇을 배웠고, 그들의 피드백 덕분에 무엇을 개선했는지 알려주세요. 원래의 Discord 스레드에 업데이트를 게시해, 그들의 기여가 어떤 영향을 미쳤는지 볼 수 있게 하세요.

플레이어가 **행동한 것** 은 그들이 **말한 것**과 다를 수 있다는 점을 기억하세요. 가능하면 실시간 관찰, Scene Feedback 응답, 게임플레이 데이터를 함께 사용하세요.

## 개선 사항을 플레이테스터와 공유하세요

플레이어는 자신의 피드백이 실제 변화를 만들었다고 볼 수 있을 때 다시 참여할 가능성이 더 높습니다. 변경을 마친 뒤 #playtesters의 플레이테스트 게시물로 돌아가 짧은 업데이트를 공유하세요.

이 간단한 형식을 사용하세요: **당신이 말했고, 우리는 배웠고, 우리는 바꿨습니다.**

### 예시: 플레이테스트 결과

**플레이테스트 업데이트: \[게임 이름]**

게임을 테스트해 주신 모든 분들께 감사드립니다!

**당신이 말한 것:** 첫 번째 라운드를 시작하는 방법이 명확하지 않았습니다.

**우리가 배운 것:** 새로운 플레이어가 시작 버튼을 쉽게 찾지 못했습니다.

**우리가 바꾼 것:** 버튼을 더 크게 만들고 더 명확한 안내를 추가했습니다.

**다시 플레이하기:** \[업데이트된 게임 링크]

업데이트를 게시하기 전에 모든 문제를 고칠 필요는 없습니다. 아직 무엇을 작업 중인지, 다음에 무엇을 테스트하고 싶은지도 설명할 수 있습니다.

## 계속 플레이테스트하세요

플레이테스트는 출시 전에 한 번만 하는 일이 아닙니다.

새 메커니즘을 도입하거나, 온보딩을 바꾸거나, 난이도를 조정하거나, 새 콘텐츠를 추가할 때 테스트하세요. 작은 플레이테스트만으로도 더 많은 플레이어에게 영향을 주기 전에 중요한 문제를 발견하는 데 도움이 됩니다.

**목표는 게임이 작동함을 증명하는 것이 아닙니다. 더 나아지게 만드는 방법을 찾는 것입니다.**

## 자료

* [Scene Feedback 모듈 및 설치 가이드](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)
* [Decentraland Discord](https://discord.gg/decentraland)
* [Decentraland 이벤트](https://decentraland.org/events/)
* [RITE 방법: Rapid Iterative Testing and Evaluation](https://handbook.gitlab.com/handbook/upstream-studios/experience-research/rite/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/designing-the-experience/playtests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
