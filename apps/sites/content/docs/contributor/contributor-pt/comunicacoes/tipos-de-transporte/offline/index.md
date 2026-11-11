# Offline

O transporte Offline é uma implementação fictícia para indicar que não há um servidor de comunicações disponível num Realm. É normalmente usado em ambientes de teste e desenvolvimento.

O transporte nunca emite mensagens de entrada e ignora todas as mensagens de saída. Se quiser uma variante offline com comportamento simulado, veja [Simulator](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/simulator/) em vez disso.

Os URI têm este aspeto:

```
offline:<custom suffix>
```

O que vem depois do `offline:` o prefixo não é especificado e pode ser usado pelas implementações (ou deixado vazio) conforme acharem adequado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/tipos-de-transporte/offline.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
