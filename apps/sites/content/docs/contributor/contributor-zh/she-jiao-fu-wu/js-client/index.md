# JavaScript 客户端

本文档摘自 [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js#using-the-client)

#### 基础设置和使用

要使用客户端，请在你的 NPM 项目中安装该包：

```bash
npm install -S @dcl/social-rpc-client
```

从已安装的包中导入客户端创建函数：

```typescript
import { createSocialClient } from "@dcl/social-client";
```

通过向客户端提供以下内容来创建一个新的客户端实例：

1. Social Service 的 REST API 的 URL
2. Social Service 的 WebSocket 端点的 URL
3. 用户的地址（与用于签署身份的地址相同）
4. 一个使用用户钱包签名的身份。

```typescript
import { createSocialClient } from "@dcl/social-client";
import { Wallet } from 'ethers'

// 为测试目的生成一个随机钱包，或在生产环境中使用用户的钱包。
const wallet = Wallet.createRandom()
const identity = await createIdentity(wallet, expiration)

const socialClient = await createSocialClient(
  "https://social.decentraland.org",
  "wss://social-service.decentraland.org",
  wallet.address,
  identity
);
```

该 `createSocialClient` 将执行所需操作以连接到 Social Service，并返回已连接的客户端。

使用客户端与 Social Service 交互：

```typescript
import { createSocialClient } from "@dcl/social-client";

const socialClient = await createSocialClient(
  "https://social.decentraland.org",
  "wss://social-service.decentraland.org",
  wallet.address,
  identity
);

const friends = socialClient.getFriends()
for await (const friend of friends) {
  console.log(friend)
}
```

该客户端公开了可通过 [social protobuf](https://github.com/decentraland/protocol/blob/main/public/social.proto) 以及一个断开连接的方法，该方法会断开客户端与 Social Service 的连接。

#### 生成身份

要使用 Social Service 对用户进行身份验证，你需要为他们生成一个身份。为此， `@dcl/crypto` 库提供了 `Authenticator.initializeAuthChain` 方法。使用它为你的用户生成一个身份：

```typescript
  import { Wallet } from 'ethers'
  import { Authenticator } from '@dcl/crypto'
  // 为测试目的生成一个随机钱包，或在生产环境中使用用户的钱包。
  const userWallet = Wallet.createRandom()

  // 为用户生成一个身份。
  const address = await userWallet.getAddress()
  const ephemeralWallet = Wallet.createRandom()
  const payload = {
    address: ephemeralWallet.address,
    privateKey: ephemeralWallet.privateKey,
    publicKey: ephemeralWallet.publicKey
  }
  const identity = await Authenticator.initializeAuthChain(address, payload, expiration, (message: string) => signer.signMessage(message))
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/she-jiao-fu-wu/js-client.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
