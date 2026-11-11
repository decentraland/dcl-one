# Login

O processo de autenticação para o Social Service envolve interagir com o [servidor Matrix](https://matrix.org/), usado nos bastidores para o subsistema de chat privado. Os utilizadores devem iniciar sessão no Matrix usando o seu endereço, assinando uma operação de fetch para obter um token de autenticação. Esse token é então usado para autorizar interações com o Social Service.

### Pedido HTTP

Para obter o token de autenticação, conforme [a documentação do Matrix refere](https://spec.matrix.org/v1.3/client-server-api/#login), deve ser feito um pedido POST para o seguinte URL: `https://social.decentraland.org/_matrix/client/r0/login` como no exemplo abaixo.

Para iniciar sessão no Matrix, é necessário criar e assinar uma mensagem com AuthChain.

#### Corpo do pedido

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

#### Corpo da resposta

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

O token de autenticação, presente no `access_token` campo, é necessário para interações subsequentes com o Social Service.

### Exemplo de código JS

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
      throw new Error(`O servidor Matrix respondeu com um código de estado ${response.status}`)
    }
  } catch (error) {
    throw new MatrixLoginError(isErrorWithMessage(error) ? error.message : 'Erro desconhecido')
  }
}
```

Código de [social-rpc-client-js](https://github.com/decentraland/social-rpc-client-js/blob/main/src/client.ts#L14)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/servico-social/login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
