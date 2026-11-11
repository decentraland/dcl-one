> 将 Rewards 活动连接到 Decentraland 场景或外部服务器，以自动发放可穿戴物和表情动作。

# 集成

在创建并配置活动，并确保其有足够库存可用于发放奖励之后，下一步是将该活动连接到奖励触发器。这个触发器可以是场景、任务或外部服务器。本节说明如何进行与 Rewards 的不同集成。

## 从场景发放奖励

奖励可以直接集成到 Decentraland 场景中，但这种方式存在一些风险。由于逻辑嵌入在用户可访问的场景代码中，因此不建议用于铸造稀有度低于 \[EPIC]\([查看文档](/docs/creator/content-creator-zh/readme/)#rarity)。

请记住，有决心且具备足够技术知识的用户可能会绕过验证码之类的安全措施，更改他们的 IP 地址，并铸造所有可用物品，然后将其出售到市场上。对此的主要防护措施是确保有足够的物品供应，这样每个人都有公平的机会获得奖励。

### 推荐的发放器标志

建议使用以下发放器配置，以降低在这种情况下被利用的风险：

* \[限制分配]\([查看文档](/docs/creator/content-creator-zh/readme/)#limit-assignments)
* \[受益人签名]\([查看文档](/docs/creator/content-creator-zh/readme/)#beneficiary-signature)
* \[验证码保护]\([查看文档](/docs/creator/content-creator-zh/readme/)#captcha-protection)
* \[已连接到 Decentraland]\([查看文档](/docs/creator/content-creator-zh/readme/)#connected-to-decentraland)
* \[Decentraland 内的位置]\([查看文档](/docs/creator/content-creator-zh/readme/)#position-inside-decentraland)（如果适用于你的用例）

### 示例

```tsx
import { getPlayer } from '@dcl/sdk/src/players'
import { signedFetch } from '@decentraland/SignedFetch'
import { getRealm } from '~system/Runtime'

export function main() {
  // 1. 获取要展示给用户的验证码挑战
  const request = await fetch(`https://rewards.decentraland.org/api/captcha`, {
    method: 'POST',
  })
  const captcha = await request.json()

  // 2. 向玩家展示验证码以供完成 - 参见 studios.decentraland.org/resources 中的示例

  // 3. 获取用户数据
  const user = getPlayer()

  // 4. 获取当前 realm
  const realmInfo = await getRealm({})

  // 5. 发送请求以分配可穿戴物品/表情
  const assignRequest = await signedFetch('https://rewards.decentraland.org/api/rewards', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      campaign_key: '[DISPENSER_KEY]', // 分配器密钥
      beneficiary: user.userId, // 以太坊地址
      catalyst: realmInfo.baseUrl, // catalyst 域名
      captcha_id: captcha.data.id, // "9e6b2d07-b47b-4204-ae87-9c4dea48f9b7"
      captcha_value: '[CAPTCHA_VALUE]', // "123456"
    }),
  })

  const reward = await assignRequest.json()
```

## 从 Decentraland Quests 发放奖励

此旧版集成链接到已归档的 Quests 文档。

你可以轻松将 Rewards 与 [Decentraland Quests](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/overview.md)集成，如果你希望在用户完成任务时奖励他们，这非常理想。

### 推荐的发放器标志

建议使用以下发放器配置，以降低在这种情况下被利用的风险：

* \[限制分配]\([查看文档](/docs/creator/content-creator-zh/readme/)#limit-assignments)（如果适用于你的用例）

任何其他标志都会使你的集成失败，请避免使用它们。


> [!WARNING]
> ⚠️ 分配器密钥应保持机密，因此你在任何时候都不应向用户暴露它。


### 示例

要将你的 Quest 与 Rewards 服务集成，你只需要一个分配器密钥并且要 [配置一个 webhook](https://github.com/decentraland/documentation/blob/5f8c7f73c67a9a36e85095cb02e72dc6355fa590/content/creator/deprecated/quests/rewards.md) 以发放奖励。

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

## 从自定义服务器发放奖励

你可以直接从你的服务器集成 Rewards，这非常适合在铸造物品之前执行额外检查。另一个优势是，与场景代码不同，你的服务器代码可能不是公开的，这会让用户更难发现并利用漏洞。

### 推荐的发放器标志

建议使用以下发放器配置，以降低在这种情况下被利用的风险：

* \[限制分配]\([查看文档](/docs/creator/content-creator-zh/readme/)#limit-assignments)（如果适用于你的用例）

启用其他任何标志都可能使你的集成变得复杂，或者根据你的用例，可能会导致它失败。因此，除非有明确需求，否则不建议使用它们。不过，你也许可以探索它们的潜在优势。


> [!WARNING]
> ⚠️ 分配器密钥应保持机密，因此你在任何时候都不应向用户暴露它。


### 示例

```tsx
const request = await fetch('https://rewards.decentraland.org/api/rewards', {
	method: 'POST',
	headers: {
		'Content-Type': 'application/json',
	},
	body: JSON.stringify({
		campaign_key: '[DISPENSER_KEY]',
		beneficiary: '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', // 以太坊地址
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
GET https://docs.decentraland.org/creator/content-creator-zh/jiang-li/integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
