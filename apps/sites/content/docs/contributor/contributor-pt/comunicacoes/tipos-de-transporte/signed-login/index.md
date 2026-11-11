# Login assinado

O transporte Signed Login pode ser usado por servidores que querem personalizar a forma como atribuem clientes a ilhas, substituindo ou encapsulando o Archipelago na sua arquitetura.

Em vez de se ligar diretamente a um backend em tempo real, o Signed Login faz uma [signed fetch](/docs/contributor/contributor-pt/autenticacao/signed-fetch/) para obter a string de conexão. Este passo intermédio permite aos servidores empregar a estratégia de atribuição que quiserem.

As URIs de Signed Login têm esta forma:

```
signed-login:https://comms.example.com/auth&param=value
```

A parte depois de `signed-login:` pode ser qualquer URL válida.

### Uso

Um pedido Signed Login é construído usando o [signed fetch](/docs/contributor/contributor-pt/autenticacao/signed-fetch/) mecanismo.

A resposta, se bem-sucedida, terá o status `200` e um corpo JSON com *pelo menos* os seguintes campos:

| Campo          | Tipo     | Valor                                                            |
| -------------- | -------- | ---------------------------------------------------------------- |
| `fixedAdapter` | `string` | A URI de transporte atribuída (por ex. `livekit:` ou `ws-room:`) |

Por exemplo:

```
{ 
  fixedAdapter: "ws-room:wss://comms.example.com/rooms/17"
}
```

Em caso de falha, a resposta pode ter qualquer código de estado apropriado e um corpo JSON com *pelo menos* os seguintes campos:

| Campo     | Tipo     | Valor                                                     |
| --------- | -------- | --------------------------------------------------------- |
| `message` | `string` | (Opcional) uma explicação ou código de erro para a falha. |

As respostas com campos adicionais no corpo JSON são válidas, e o conteúdo do erro `message` fica por especificar. As implementações podem usar esta flexibilidade para integrar `signed-login:` nas suas aplicações como entenderem.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/tipos-de-transporte/signed-login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
