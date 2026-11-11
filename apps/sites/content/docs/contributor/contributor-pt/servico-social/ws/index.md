# WebSocket

O Social Service é um servidor que gere amizades no Decentraland. Utilizando a plataforma Matrix, serve como a fonte autorizada para processar atualizações de Friendships, como pedidos, aceitações, rejeições, cancelamentos e eliminações.

### Casos de uso

O Social Service aborda os seguintes casos de uso relacionados com amizades:

* Solicitar uma amizade
* Aceitar um pedido de amizade
* Rejeitar um pedido de amizade
* Cancelar um pedido de amizade
* Remover um amigo
* Listar todos os meus amigos
* Listar amigos em comum entre mim e outro utilizador

### Contexto

Antes da introdução do Social Service, os dados de amizade eram armazenados e obtidos diretamente a partir do Matrix. Isto originava vários problemas nos fluxos de amizade e informação não fiável. Além disso, implementar novas funcionalidades era desafiante. Como o Matrix lida principalmente com funcionalidades de chat, o Social Service atua como um proxy, aplicando a lógica de negócio necessária antes de interagir com o Matrix.

### Arquitetura

#### Histórico de versões

1. [Primeira versão](https://adr.decentraland.org/adr/ADR-113): Inicialmente, o Social Service era uma REST API que apenas oferecia endpoints GET para listar todos os amigos e as ligações em comum. Não tinha funcionalidades relacionadas com atualizações ou envio de pedidos.
2. [Segunda versão](https://adr.decentraland.org/adr/ADR-189): Esta versão introduziu atualizações no fluxo de amizade. Para responder a estas alterações, foi implementado um servidor WebSocket RPC. Este servidor trata as mensagens essenciais, mantendo ligações abertas com os clientes para fornecer atualizações em tempo útil. As ligações WebSocket também facilitam funcionalidades como [o rastreio de presença](https://github.com/decentraland/adr/blob/420e63926d20166ae832e3de62dd9e3f2370cd49/content/ADR-246-poc-presence.md) no Social Service.

#### URLs

**Domínio .zone**

Servidores do ambiente de desenvolvimento para testes e quando utilizar testnets.

* [Login com Matrix](https://social.decentraland.zone)
* [REST API](https://social.decentraland.zone)
* [API WebSocket](https://rpc-social.decentraland.zone)

**Domínio .org**

Servidores do ambiente de produção e quando utilizar mainnet.

* [Login com Matrix](https://social.decentraland.org)
* [REST API](https://social.decentraland.org)
* [API WebSocket](https://rpc-social.decentraland.org)

### Use o cliente JavaScript

Para usar o cliente JavaScript, consulte <https://github.com/decentraland/social-rpc-client-js#using-the-client>

### Implementações de referência do cliente

* [Cliente Matrix](https://github.com/decentraland/matrix-client)
* [Cliente JS](https://github.com/decentraland/social-rpc-client-js)
* [Cliente de referência Rust](https://github.com/decentraland/social-client)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/servico-social/ws.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
