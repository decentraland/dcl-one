> Rewards 캠페인을 Decentraland 씬 또는 외부 서버에 연결하여 웨어러블과 이모트를 자동으로 지급합니다.

# 통합

캠페인을 생성하고 구성한 뒤 보상을 제공할 수 있을 만큼 충분한 재고가 있는지 확인했다면, 다음 단계는 캠페인을 보상 트리거에 연결하는 것입니다. 이 트리거는 Scene, Quest 또는 외부 서버일 수 있습니다. 이 섹션에서는 Rewards와의 다양한 통합 방법을 설명합니다.

## 씬에서 보상 지급

보상은 Decentraland 씬에 직접 통합할 수 있지만, 이 방식에는 몇 가지 위험이 따릅니다. 로직이 사용자가 접근할 수 있는 씬 코드에 포함되므로, \[EPIC]\([문서 보기](/docs/creator/content-creator-ko/readme/)#rarity).

결정적인 기술 지식을 가진 사용자는 캡차 같은 보안 조치를 우회하고, IP 주소를 변경한 뒤, 사용 가능한 모든 아이템을 민팅할 수 있으며, 이후 이를 마켓플레이스에서 판매할 수 있습니다. 이를 막기 위한 주요 안전장치는 아이템 공급을 충분히 확보하여 모두가 보상을 받을 공정한 기회를 갖도록 하는 것입니다.

### 권장 디스펜서 플래그

이 시나리오에서 악용 위험을 줄이기 위해 다음과 같은 디스펜서 구성이 권장됩니다:

* \[할당 제한]\([문서 보기](/docs/creator/content-creator-ko/readme/)#limit-assignments)
* \[수혜자 서명]\([문서 보기](/docs/creator/content-creator-ko/readme/)#beneficiary-signature)
* \[캡차 보호]\([문서 보기](/docs/creator/content-creator-ko/readme/)#captcha-protection)
* \[Decentraland에 연결됨]\([문서 보기](/docs/creator/content-creator-ko/readme/)#connected-to-decentraland)
* \[Decentraland 내 위치]\([문서 보기](/docs/creator/content-creator-ko/readme/)#position-inside-decentraland) (사용 사례에 해당하는 경우)

### 예시

```tsx
import { getPlayer } from '@dcl/sdk/src/players'
import { signedFetch } from '@decentraland/SignedFetch'
import { getRealm } from '~system/Runtime'

export function main() {
  // 1. 사용자에게 표시할 캡차 챌린지 가져오기
  const request = await fetch(`https://rewards.decentraland.org/api/captcha`, {
    method: 'POST',
  })
  const captcha = await request.json()

  // 2. 플레이어가 완료할 수 있도록 캡차 표시 - studios.decentraland.org/resources의 예시 참고

  // 3. 사용자 데이터 가져오기
  const user = getPlayer()

  // 4. 현재 realm 가져오기
  const realmInfo = await getRealm({})

  // 5. 웨어러블/이모트를 할당하기 위한 요청 전송
  const assignRequest = await signedFetch('https://rewards.decentraland.org/api/rewards', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      campaign_key: '[DISPENSER_KEY]', // 디스펜서 키
      beneficiary: user.userId, // 이더리움 주소
      catalyst: realmInfo.baseUrl, // 캐털리스트 도메인
      captcha_id: captcha.data.id, // "9e6b2d07-b47b-4204-ae87-9c4dea48f9b7"
      captcha_value: '[CAPTCHA_VALUE]', // "123456"
    }),
  })

  const reward = await assignRequest.json()
```

## Decentraland Quest에서 보상 지급

이 이전 통합 방식은 보관된 Quest 문서로 연결됩니다.

다음과 함께 Rewards를 쉽게 통합할 수 있습니다. [Decentraland Quests](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/overview.md). 이는 사용자가 퀘스트를 완료했을 때 보상을 주고 싶을 경우에 이상적입니다.

### 권장 디스펜서 플래그

이 시나리오에서 악용 위험을 줄이기 위해 다음과 같은 디스펜서 구성이 권장됩니다:

* \[할당 제한]\([문서 보기](/docs/creator/content-creator-ko/readme/)#limit-assignments) (사용 사례에 해당하는 경우)

다른 플래그를 사용하면 통합이 실패하게 되므로 사용하지 마세요.


> [!WARNING]
> ⚠️ 디스펜서 키는 비밀로 유지해야 하므로, 어떤 경우에도 사용자에게 노출해서는 안 됩니다.


### 예시

Quest를 Rewards 서비스와 통합하려면 디스펜서 키와 [웹훅을 구성하기만 하면 됩니다](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/rewards.md) 보상을 지급할 수 있습니다.

```js
{
    // ...
    "reward": {
        "hook": {
            "webhookUrl": "https://rewars.decentraland.org/api/rewards",
            "requestBody": {
                "campaign_key": "[DISPENSER_KEY]",
                "beneficiary": "{user_address}"
            }
        },
        // ...
    }
}
```

## 커스텀 서버에서 보상 지급

Rewards는 서버에서 직접 통합할 수 있으며, 이는 아이템을 민팅하기 전에 추가 검사를 수행할 때 이상적입니다. 또 다른 장점은 씬 코드와 달리 서버 코드가 공개되지 않을 수 있어, 사용자가 취약점을 발견하고 악용하기 더 어렵다는 점입니다.

### 권장 디스펜서 플래그

이 시나리오에서 악용 위험을 줄이기 위해 다음과 같은 디스펜서 구성이 권장됩니다:

* \[할당 제한]\([문서 보기](/docs/creator/content-creator-ko/readme/)#limit-assignments) (사용 사례에 해당하는 경우)

다른 플래그를 활성화하면 통합이 복잡해지거나, 사용 사례에 따라 실패할 수도 있습니다. 따라서 특별한 필요가 없다면 사용하는 것은 권장되지 않습니다. 다만 잠재적인 이점을 살펴볼 수는 있습니다.


> [!WARNING]
> ⚠️ 디스펜서 키는 비밀로 유지해야 하므로, 어떤 경우에도 사용자에게 노출해서는 안 됩니다.


### 예시

```tsx
const request = await fetch('https://rewards.decentraland.org/api/rewards', {
	method: 'POST',
	headers: {
		'Content-Type': 'application/json',
	},
	body: JSON.stringify({
		campaign_key: '[DISPENSER_KEY]',
		beneficiary: '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', // 이더리움 주소
	}),
})

const response = await request.json()
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/rewards/integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
