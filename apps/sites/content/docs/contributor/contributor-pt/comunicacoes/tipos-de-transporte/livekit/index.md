# LiveKit

O [LiveKit](https://livekit.io/) transport é o mais versátil e eficiente dos transportes suportados, e também o mais fácil de implementar. Os clientes devem usá-lo quando recebem uma string de conexão apropriada do serviço do seu realm [Archipelago](/docs/contributor/contributor-pt/comunicacoes/archipelago/) ou se estiver indicado como um transporte fixo.


> [!NOTE]
> Você pode ver o transporte do LiveKit em ação e experimentar os seus recursos usando o open-source [Comms Station](https://decentraland.github.io/comms-station/).


Ao contrário do [websocket](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/websocket/) transporte personalizado, ele usa uma biblioteca de terceiros (open source), o que simplifica muito o processo de desenvolvimento ao abstrair toda a lógica de nível de transporte e a serialização.

As URIs de transporte do LiveKit começam com o `livekit:` prefixo, seguido do `wss://` URL do servidor. Elas também incluem um `access_token`pré-autorizado, gerado pelo [Archipelago](/docs/contributor/contributor-pt/comunicacoes/archipelago/) quando o cliente se autentica.

```
livekit:wss://comms.example.com?access_token=eyJhbGciOiJI...
```

### Conectando e Autenticando

A biblioteca cliente do LiveKit encapsula de forma elegante toda a lógica de conexão e autenticação.

Não há nenhuma etapa adicional de autenticação ao usar o transporte do LiveKit. O `access_token` fornecido pelo Archipelago já está verificado e autorizado.

### Trocando Mensagens

Os clientes que dependem da biblioteca LiveKit podem facilmente transmitir, enviar mensagens diretas e publicar feeds de mídia ao vivo.

As cargas úteis são simples comms [`Packets`](/docs/contributor/contributor-pt/comunicacoes/messages/) sem qualquer encapsulamento adicional.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/tipos-de-transporte/livekit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
