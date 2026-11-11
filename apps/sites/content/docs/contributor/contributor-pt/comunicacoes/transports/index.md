# Transportes

Um *transporte* é uma interface do lado do cliente que pode se conectar a uma URI de comms e trocar mensagens em tempo real com pares ou serviços.


> [!NOTE]
> Você pode ver o protocolo de comms em ação e experimentá-lo usando o Comms Station de código aberto [Comms Station](https://decentraland.github.io/comms-station/).


Os clientes são livres para implementar transportes como acharem adequado (desde que sigam o protocolo esperado pelos outros clientes e serviços), mas o [design recomendado](#recommended) descrito abaixo foi testado na prática e comprovado que se integra bem com diferentes projetos.\`

### Transportes padrão <a href="#types" id="types"></a>

Existem 5 tipos de transporte definidos pelo protocolo:

* [WebSocket](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/websocket/) (`ws-room`) usa um stream padrão de WebSocket.
* [LiveKit](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/livekit/) (`livekit`) usa o framework open-source LiveKit, baseado em WebRTC.
* [SignedLogin](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/signed-login/) (`signed-login`) faz uma requisição HTTPS assinada para obter um transporte atribuído dinamicamente de um servidor.
* [Offline](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/offline/) (`offline`) é um transporte fictício que indica que não há comms no ambiente atual.
* [Simulator](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/simulator/) (`simulator`) é um transporte fictício com comportamento totalmente personalizado, principalmente para desenvolvedores depurarem suas implementações.

As mensagens enviadas e recebidas através de uma interface de transporte são sempre as mesmas (veja [messages](/docs/contributor/contributor-pt/comunicacoes/messages/)). Alguns transportes podem envolvê-las em estruturas de controle para transmissão, mas elas devem ser desempacotadas antes de atravessar a `Transport` interface.

### Design recomendado <a href="#recommended" id="recommended"></a>

Esta é uma `Transport` interface mínima escrita em TypeScript:

```ts
interface Transport {
  // Inicialize o Transport com uma URI:
  constructor(private uriWithConnectionParams: string)

  // Abra uma conexão usando a URI fornecida no construtor:
  connect(): Promise<void>

  // Feche a conexão:
  disconnect(): Promise<void>

  // Envie um payload arbitrário para o service e todos os peers:
  send(packet: Packet): Promise<void>

  // Inscreva-se para mensagens recebidas tanto do service quanto de todos os peers:
  on(event: 'receive', callback: (packet: Packet) => void): void
}
```

Implementações reais normalmente estendem isso, adicionando eventos de conexão/desconexão e de entrar/sair, envio sem broadcast, tipagem mais estrita ou adaptações específicas de linguagem.

#### URIs de transporte

Os valores de `uriWithConnectionParams` estão sempre na forma:

```
<type>:<type connection parameters>
```

O `<type>` corresponde a um dos listados acima, e o formato para `<type connection parameters>` depende do transporte específico. Pode ser uma URL, um token opaco ou quaisquer dados arbitrários.

O [LiveKit](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/livekit/) transporte, por exemplo, usa uma `wss` URL com um `access_token` parâmetro para estabelecer uma conexão autenticada:

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

Para comparação, o [WebSocket](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/websocket/) transporte também usa uma `wss` URL, mas sem um token pré-gerado. Ele requer um fluxo de autenticação depois de conectado.

#### Criando transportes

Como cada `Transport` classe dará suporte a um tipo específico de URI, é uma boa ideia ter um método de fábrica que retorne um transporte válido `Transport` ou falhe imediatamente. Por exemplo:

```ts
function createTransport(uriWithConnectionParams: string) {
  const type = getPrefix(urlWithConnectionParams)

  switch (type) {
    case 'ws-room': return new WsRoomTransport(uriWithConnectionParams)
    case 'livekit': return new LiveKitTransport(uriWithConnectionParams)
    // ... outras implementações suportadas...
  }

  throw new Error(`Tipo de transporte não suportado: ${type}`)
}
```

Os clientes não são obrigados a implementar todos os transportes padrão. Se pretendem usar apenas um subconjunto dos tipos definidos sem a intenção de lidar com todas as URIs, são livres para rejeitar os tipos que não suportam.

Clientes com mais recursos, como um World Explorer, são aconselhados a implementar pelo menos o [websocket](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/websocket/) e [LiveKit](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/livekit/) transportes, que atualmente estão em uso por todos os realms principais.

### Saiba mais

Acesse uma [seção de um tipo específico de transporte](#types) leia sobre os diferentes [tipos de mensagem](/docs/contributor/contributor-pt/comunicacoes/messages/) ou confira o \[\[Comms Demo Station]].


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/transports.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
