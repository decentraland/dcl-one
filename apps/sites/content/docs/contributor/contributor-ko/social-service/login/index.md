# 로그인

Social Service의 인증 과정은 [Matrix 서버](https://matrix.org/), 비공개 채팅 하위 시스템에서 내부적으로 사용됩니다. 사용자는 자신의 주소를 사용해 Matrix에 로그인해야 하며, 인증 토큰을 얻기 위해 fetch 작업에 서명해야 합니다. 그런 다음 이 토큰은 Social Service와의 상호작용을 승인하는 데 사용됩니다.

### HTTP 요청

인증 토큰을 획득하려면, [Matrix 문서에서 언급하듯](https://spec.matrix.org/v1.3/client-server-api/#login), 다음 URL로 POST 요청을 보내야 합니다: `https://social.decentraland.org/_matrix/client/r0/login` 아래 예시와 같습니다.

Matrix에 로그인하려면 AuthChain으로 메시지를 생성하고 서명해야 합니다.

#### 요청 본문

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

#### 응답 본문

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

해당 필드에 있는 인증 토큰은 `access_token` 이후 Social Service와의 상호작용에 필요합니다.

### JS 코드 예시

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
    throw new MatrixLoginError(isErrorWithMessage(error) ? error.message : 'Unknown error')
  }
}
```

코드 출처: [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js/blob/main/src/client.ts#L14)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/social-service/login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
