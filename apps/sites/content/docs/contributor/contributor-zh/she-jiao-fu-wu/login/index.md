# 登录

Social Service 的身份验证过程涉及与以下服务交互 [Matrix 服务器](https://matrix.org/)，该服务在幕后用于私聊子系统。用户必须使用自己的地址登录 Matrix，签署一个获取认证令牌的 fetch 操作。然后使用该令牌来授权与 Social Service 的交互。

### HTTP 请求

要获取认证令牌，如 [Matrix 文档所述](https://spec.matrix.org/v1.3/client-server-api/#login)，必须向以下 URL 发送 POST 请求： `https://social.decentraland.org/_matrix/client/r0/login` 如下面示例所示。

要登录 Matrix，你需要使用 AuthChain 创建并签名一条消息。

#### 请求体

```json
{
  "auth_chain": authChain,
  "identifier": {
    "type": "m.id.user",
    "user": address
  },
  "timestamp": timestamp.toString(),
  "type": "m.login.decentraland"
}
```

#### 响应体

```json
{
  "user_id": "@0x123abC:decentraland.org",
  "social_user_id": "0x123abC",
  "access_token": "syt_SomETokEN",
  "device_id": "FRFREGRG",
  "home_server": "decentraland.org",
  "well_known": {
      "m.homeserver": {
          "base_url": "https://synapse.decentraland.org/"
      }
  }
}
```

认证令牌，位于 `access_token` 字段中，是后续与 Social Service 交互所必需的。

### JS 代码示例

```javascript
import fetch from 'cross-fetch'
import { AuthIdentity, Authenticator } from '@dcl/crypto'

export async function getMatrixToken(matrixUrl: string, address: string, identity: AuthIdentity): Promise<string> {
  const timestamp = Date.now()
  const authChain = Authenticator.signPayload(identity, timestamp.toString())

  try {
    const response = await fetch(`${matrixUrl}/_matrix/client/r0/login`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        auth_chain: authChain,
        identifier: {
          type: 'm.id.user',
          user: address
        },
        timestamp: timestamp.toString(),
        type: 'm.login.decentraland'
      })
    })

    if (response.ok) {
      const responseBody = await response.json()
      return responseBody.access_token
    } else {
      throw new Error(`Matrix server responded with a ${response.status} status code`)
    }
  } catch (error) {
    throw new MatrixLoginError(isErrorWithMessage(error) ? error.message : '未知错误')
  }
}
```

来自 [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js/blob/main/src/client.ts#L14)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/she-jiao-fu-wu/login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
