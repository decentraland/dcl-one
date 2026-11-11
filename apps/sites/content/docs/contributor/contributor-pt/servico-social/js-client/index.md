# Cliente JavaScript

Esta documentação foi extraída de [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js#using-the-client)

#### Configuração básica e utilização

Para usar o cliente, instale o pacote no seu projeto NPM:

```bash
npm install -S @dcl/social-rpc-client
```

Importe a função criadora do cliente a partir do pacote instalado:

```typescript
import { createSocialClient } from "@dcl/social-client";
```

Crie uma nova instância do cliente fornecendo-lhe:

1. Uma URL para a REST API do Social Service
2. Uma URL para o endpoint WebSocket do Social Service
3. O endereço do usuário (o mesmo usado para assinar a identidade)
4. Uma identidade, assinada com a wallet do usuário.

```typescript
import { createSocialClient } from "@dcl/social-client";
import { Wallet } from 'ethers'

// Gere uma wallet aleatória para fins de teste ou use a do usuário em ambientes de produção.
const wallet = Wallet.createRandom()
const identity = await createIdentity(wallet, expiration)

const socialClient = await createSocialClient(
  "https://social.decentraland.org",
  "wss://social-service.decentraland.org",
  wallet.address,
  identity
);
```

O `createSocialClient` irá executar as operações necessárias para se conectar ao Social Service e retornará o cliente conectado.

Use o cliente para interagir com o Social Service:

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

O cliente expõe os métodos disponíveis através do [social protobuff](https://github.com/decentraland/protocol/blob/main/public/social.proto) e um método disconnect que desconecta o cliente do Social Service.

#### Gerando uma identidade

Para autenticar usuários com o Social Service, você precisará gerar uma identidade para eles. Para isso, a `@dcl/crypto` biblioteca fornece o `Authenticator.initializeAuthChain` método. Use-o para gerar uma identidade para os seus usuários:

```typescript
  import { Wallet } from 'ethers'
  import { Authenticator } from '@dcl/crypto'
  // Gere uma wallet aleatória para fins de teste ou use a do usuário em ambientes de produção.
  const userWallet = Wallet.createRandom()

  // Gere uma identidade para o usuário.
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
GET https://docs.decentraland.org/contributor/contributor-pt/servico-social/js-client.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
