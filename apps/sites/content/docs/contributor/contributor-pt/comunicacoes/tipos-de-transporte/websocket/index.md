# WebSocket

O transporte websocket é um dos transportes de mensagens de ilha suportados pelo protocolo de comms da Decentraland. Os clients devem usá-lo quando receberem uma connection string apropriada do serviço do seu realm [Archipelago](/docs/contributor/contributor-pt/comunicacoes/archipelago/) serviço, ou se estiver indicado como um transporte fixo.

No contexto do transporte websocket, uma ilha é chamada de uma *sala*. Ambos os nomes indicam a mesma coisa: um grupo de jogadores próximos que trocam atualizações e chat.

As connection strings começam com o `ws-room:` prefixo, seguido por uma `wss://` URI para a sala específica. Elas se parecem com isto:

```
ws-room:wss://comms.example.com/rooms/<room-id>
```

Na ausência de um protocolo explícito para a URI websocket, `wss://` é assumido.

### Conectando

O transporte usa o protocolo websocket regular sobre HTTPS. Os clients podem abrir uma conexão usando qualquer implementação padrão à sua disposição.

### Pacotes WebSocket

Todas as mensagens do transporte websocket são serializadas usando a [`WsPacket`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L78) estrutura. Elas implementam um conjunto de mensagens de conectividade e autenticação em nível de protocolo, além de um tipo contêiner para mensagens dos clients.


> [!NOTE]
> O [`WsPacket`](#websocket-packets) A estrutura não deve ser confundida com a mensagem \[`Packet`]\[Packet]. É uma camada adicional de encapsulamento específica para o transporte websocket. As mensagens reais de comms estão contidas no [`WsPeerUpdate`](#wspeerupdate) tipo.


| Campo     | Tipo   | Valor                                                                                                                                                                                                                                           |
| --------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `message` | `enum` | <p>Um de <code>WsIdentification</code>, <code>WsChallengeRequired</code>, <code>WsSignedChallenge</code>,<br><code>WsWelcome</code>, <code>WsPeerJoin</code>, <code>WsPeerLeave</code>, <code>WsKicked</code> ou <code>WsPeerUpdate</code>.</p> |

### Autenticação

Antes de começarem a retransmitir mensagens para outros, os clients devem se autenticar assinando uma string de desafio. Isso é necessário mesmo quando a URI do transporte foi obtida de [Archipelago](/docs/contributor/contributor-pt/comunicacoes/archipelago/) após uma rodada anterior de autenticação.

A primeira mensagem que um client envia ao entrar numa ilha é [`WsIdentification`](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64), que contém um endereço público. Será respondida com [`WsChallengeRequired`](#wschallengerequired), e os clients enviam uma [`WsSignedChallenge`](#wssignedchallenge) em resposta.

```
  .----------.                .--------.
  |  Servidor  |                | Cliente |
  '----+-----'                '---+----'
       ⋮                          |
       ⋮                  Conectar |
       o - - - - - - - - - - - - -|
       |                          |
       |                          |
       |         WsIdentification |
       |<-------------------------+
       +------------------------->|
       | WsChallengeRequired      |
       |                          |
       |                          |
       |        WsSignedChallenge |
       |<-------------------------+
       +------------------------->|
       | WsWelcome                |
       |                          |

```

Se o fluxo for concluído com sucesso, o client receberá um [`WsWelcome`](#wswelcome) e poderá começar a enviar mensagens aos pares.

***

### `WsIdentification` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L64) <a href="#wsidentification" id="wsidentification"></a>

| Campo     | Tipo     | Valor                                 |
| --------- | -------- | ------------------------------------- |
| `address` | `string` | O endereço público Ethereum do client |

***

### `WsChallengeRequired` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L48) <a href="#wschallengerequired" id="wschallengerequired"></a>

| Campo               | Tipo     | Valor                                                                                          |
| ------------------- | -------- | ---------------------------------------------------------------------------------------------- |
| `challenge_to_sign` | `string` | A string fornecida pelo servidor para ser assinada como prova de identidade                    |
| `already_connected` | `string` | Uma indicação do servidor aos clients, indicando que conexões anteriores podem ser encerradas. |

***

### `WsSignedChallenge` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L56) <a href="#wssignedchallenge" id="wssignedchallenge"></a>

| Campo             | Tipo     | Valor                                                                                                                                        |
| ----------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `auth_chain_json` | `string` | Um alias serializado [cadeia de autenticação](/docs/contributor/contributor-pt/autenticacao/authchain/) terminando com a assinatura do desafio. |

***

### `WsWelcome` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L11) <a href="#wswelcome" id="wswelcome"></a>

| Campo             | Tipo                  | Valor                                                      |
| ----------------- | --------------------- | ---------------------------------------------------------- |
| `alias`           | `uint32`              | Um ID gerado pelo servidor para a sessão do client         |
| `peer_identities` | `map<uint32, string>` | Os endereços de todos os peers atuais, indexados por alias |

### Conectividade

Três mensagens são definidas para ajudar os clients a acompanhar seus peers e o próprio status.

[`WsPeerJoin`](#wspeerjoin) é recebida quando um outro client se conecta à sala e se autentica com sucesso, enquanto [`WsPeerLeave`](#wspeerleave) é recebida quando os peers se desconectam.

Há também a [`WsKicked`](#wskicked) mensagem, que informa os clients que a sua conexão está prestes a ser encerrada e o motivo disso. Na prática atual, a principal razão pela qual um servidor pode expulsar um client de uma sala é porque ele se conectou simultaneamente a outra sala, quando isso é proibido pela política do servidor.

***

### `WsPeerJoin` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L19) <a href="#wspeerjoin" id="wspeerjoin"></a>

| Campo     | Tipo     | Valor                                                          |
| --------- | -------- | -------------------------------------------------------------- |
| `alias`   | `uint32` | O ID gerado pelo servidor enviado em [`WsWelcome`](#wswelcome) |
| `address` | `string` | O endereço Ethereum do novo peer                               |

***

### `WsPeerLeave` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L27) <a href="#wspeerleave" id="wspeerleave"></a>

| Campo   | Tipo     | Valor                                          |
| ------- | -------- | ---------------------------------------------- |
| `alias` | `uint32` | O ID gerado pelo servidor do peer desconectado |

***

### `WsKicked` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L74) <a href="#wskicked" id="wskicked"></a>

| Campo    | Tipo     | Valor                                                           |
| -------- | -------- | --------------------------------------------------------------- |
| `reason` | `string` | A explicação do servidor sobre por que a conexão será encerrada |

### Mensagens do cliente

Para enviar comms [mensagens](/docs/contributor/contributor-pt/comunicacoes/messages/), os clients os envolvem na [`WsPeerUpdate`](#wspeerupdate) estrutura. Isso diferencia os tipos de mensagem de controle do transporte das mensagens reais enviadas entre peers.

```
.-------------------.
| WsPacket          |
|  .--------------. |
|  | WsPeerUpdate | |
|  | .----------. | |
|  | |  Packet  | | |
|  | '----------' | |
|  '--------------' |
'-------------------'

```

***

### `WsPeerUpdate` [**↗ source**](https://github.com/decentraland/protocol/blob/c48ea0aa00d8173084571552463a6a05a7f49636/proto/decentraland/kernel/comms/rfc5/ws_comms.proto#L34) <a href="#wspeerupdate" id="wspeerupdate"></a>

| Campo           | Tipo     | Valor                                                                                                             |
| --------------- | -------- | ----------------------------------------------------------------------------------------------------------------- |
| `from_alias`    | `uint32` | O ID gerado pelo servidor do remetente                                                                            |
| `body`          | `bytes`  | O conteúdo serializado [message](/docs/contributor/contributor-pt/comunicacoes/messages/) que está sendo encapsulado |
| `não confiável` | `bool`   | Se o remetente priorizou velocidade ou confiabilidade ao entregar esta mensagem                                   |

Os clients devem definir o `from_alias` campo para `0` ao enviar, e o servidor o preencherá com o identificador correto antes de entregá-lo aos peers.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/tipos-de-transporte/websocket.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
