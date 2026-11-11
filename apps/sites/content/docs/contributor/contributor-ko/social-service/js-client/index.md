# JavaScript 클라이언트

이 문서는 다음에서 추출되었습니다 [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js#using-the-client)

#### 기본 설정 및 사용법

클라이언트를 사용하려면 NPM 프로젝트에 패키지를 설치하세요:

```bash
npm install -S @dcl/social-rpc-client
```

설치된 패키지에서 클라이언트 생성 함수를 가져오세요:

```typescript
import { createSocialClient } from "@dcl/social-client";
```

다음 항목을 제공하여 새 클라이언트 인스턴스를 생성합니다:

1. Social Service의 REST API URL
2. Social Service의 WebSocket 엔드포인트 URL
3. 사용자의 주소(신원을 서명하는 데 사용한 주소와 동일)
4. 사용자의 지갑으로 서명된 신원.

```typescript
import { createSocialClient } from "@dcl/social-client";
import { Wallet } from 'ethers'

// 테스트 목적으로 임의의 지갑을 생성하거나 프로덕션 환경에서는 사용자의 지갑을 사용하세요.
const wallet = Wallet.createRandom()
const identity = await createIdentity(wallet, expiration)

const socialClient = await createSocialClient(
  "https://social.decentraland.org",
  "wss://social-service.decentraland.org",
  wallet.address,
  identity
);
```

이 `createSocialClient` 는 Social Service에 연결하는 데 필요한 작업을 수행하고 연결된 클라이언트를 반환합니다.

클라이언트를 사용하여 Social Service와 상호작용하세요:

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

클라이언트는 다음을 통해 사용할 수 있는 메서드를 제공합니다: [social 프로토버프](https://github.com/decentraland/protocol/blob/main/public/social.proto) 그리고 클라이언트를 Social Service에서 연결 해제하는 disconnect 메서드도 제공합니다.

#### 신원 생성

사용자를 Social Service에 인증하려면 사용자를 위한 신원을 생성해야 합니다. 이를 위해 `@dcl/crypto` 라이브러리에서 다음을 제공합니다: `Authenticator.initializeAuthChain` 메서드입니다. 이를 사용해 사용자의 신원을 생성하세요:

```typescript
  import { Wallet } from 'ethers'
  import { Authenticator } from '@dcl/crypto'
  // 테스트 목적으로 임의의 지갑을 생성하거나 프로덕션 환경에서는 사용자의 지갑을 사용하세요.
  const userWallet = Wallet.createRandom()

  // 사용자의 신원을 생성합니다.
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
GET https://docs.decentraland.org/contributor/contributor-ko/social-service/js-client.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
