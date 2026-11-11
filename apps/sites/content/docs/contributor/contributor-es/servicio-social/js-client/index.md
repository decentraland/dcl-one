# Cliente de JavaScript

Esta documentación se extrae de [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js#using-the-client)

#### Configuración y uso básicos

Para usar el cliente, instala el paquete en tu proyecto NPM:

```bash
npm install -S @dcl/social-rpc-client
```

Importa la función creadora del cliente desde el paquete instalado:

```typescript
import { createSocialClient } from "@dcl/social-client";
```

Crea una nueva instancia del cliente proporcionándole:

1. Una URL para la REST API del Social Service
2. Una URL para el endpoint WebSocket del Social Service
3. La dirección del usuario (la misma que la usada para firmar la identidad)
4. Una identidad, firmada con la Wallet del usuario.

```typescript
import { createSocialClient } from "@dcl/social-client";
import { Wallet } from 'ethers'

// Genera una Wallet aleatoria con fines de prueba o usa la del usuario en entornos de producción.
const wallet = Wallet.createRandom()
const identity = await createIdentity(wallet, expiration)

const socialClient = await createSocialClient(
  "https://social.decentraland.org",
  "wss://social-service.decentraland.org",
  wallet.address,
  identity
);
```

El `createSocialClient` se conectará, realizará las operaciones requeridas para conectarse al Social Service y devolverá el cliente conectado.

Usa el cliente para interactuar con el Social Service:

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

El cliente expone los métodos disponibles a través de [social protobuff](https://github.com/decentraland/protocol/blob/main/public/social.proto) y un método disconnect que desconecta el cliente del Social Service.

#### Generar una identidad

Para autenticar a los usuarios con el Social Service, necesitarás generarles una identidad. Para ello, la `@dcl/crypto` librería proporciona el `Authenticator.initializeAuthChain` método. Úsalo para generar una identidad para tus usuarios:

```typescript
  import { Wallet } from 'ethers'
  import { Authenticator } from '@dcl/crypto'
  // Genera una Wallet aleatoria con fines de prueba o usa la del usuario en entornos de producción.
  const userWallet = Wallet.createRandom()

  // Genera una identidad para el usuario.
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
GET https://docs.decentraland.org/contributor/contributor-es/servicio-social/js-client.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
