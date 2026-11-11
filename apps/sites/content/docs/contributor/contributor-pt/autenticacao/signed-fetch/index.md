# Fetch assinado

Quando clientes de protocolo quiserem fazer requisições HTTPS autenticadas, eles podem aproveitar o padrão *signed fetch* mecanismo.

Um signed fetch é uma requisição que inclui um [cadeia de autenticação](/docs/contributor/contributor-pt/autenticacao/authchain/), representado por meio de cabeçalhos. Servidores com APIs compatíveis com Decentraland podem validar identidades antes de, por exemplo, [permitir requisições de scenes](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/signed-fetch/) ou criar [adaptadores](/docs/contributor/contributor-pt/comunicacoes/overview/).

## Cabeçalhos

As informações de que o servidor precisa para validar a cadeia de autenticação são transmitidas em 3+ cabeçalhos:

* `X-Identity-Timestamp`: o `timestamp` campo incluído no payload assinado (veja abaixo).
* `X-Identity-Metadata`: o `metadados` campo incluído no payload assinado (veja abaixo).
* `X-Identity-AuthChain-<index>`: o [etapa de autenticação serializada em JSON](/docs/contributor/contributor-pt/autenticacao/authchain/#constructing) `<index>`, começando em `0`.

A cadeia transmitida é validada pelo servidor [conforme especificado](/docs/contributor/contributor-pt/autenticacao/authchain/).

## Corpo

O corpo da requisição não é especificado. Os serviços têm total flexibilidade para usar quaisquer protocolos ou formatos de sua preferência.

## Carga útil

A cadeia de autenticação [carga útil](/docs/contributor/contributor-pt/autenticacao/authchain/#constructing) de um signed fetch é uma **em minúsculas, separada por dois-pontos** string que inclui alguns dos elementos da requisição:

```
<method>:<path>:<timestamp>:<metadata>
```

O `método` e `caminho` os campos devem corresponder aos da requisição, e `timestamp` é o mesmo que no `X-Identity-Timestamp` cabeçalho.

O último campo, `metadados`, pode ter conteúdo arbitrário.

Por exemplo:

```
get:/some/path:1682790056:{"some":"custom json"}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/autenticacao/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
