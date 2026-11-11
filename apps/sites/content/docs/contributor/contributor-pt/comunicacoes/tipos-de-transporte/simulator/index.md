# Simulador

O transporte do Simulator é uma implementação completamente arbitrária, sem qualquer comportamento especificado além da interface de transporte.

Pode emitir mensagens recebidas e tratar mensagens enviadas como desejar, sem requisitos semânticos. A implementação pode optar por nunca emitir mensagens e ignorar todas as chamadas de método, mas o [Transporte offline](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/offline/) é mais adequado para isso.

Os URIs do Simulator têm este aspeto:

```
simulator:<custom suffix>
```

O que vem depois do `simulator:` o prefixo não é especificado e pode ser usado pelas implementações (ou deixado vazio) conforme acharem adequado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/tipos-de-transporte/simulator.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
