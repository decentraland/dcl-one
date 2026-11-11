# Simulador

El transporte de Simulator es una implementación completamente arbitraria, sin ningún comportamiento especificado aparte de la interfaz de transporte.

Puede emitir mensajes entrantes y manejar mensajes salientes como se desee, sin requisitos semánticos. La implementación puede elegir no emitir nunca mensajes e ignorar todas las llamadas a métodos, pero el [Transporte sin conexión](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/offline/) es más adecuado para eso.

Las URI de Simulator se ven así:

```
simulator:<sufijo personalizado>
```

Lo que viene después de `simulator:` el prefijo no está especificado y puede ser usado por las implementaciones (o dejarse vacío) como les convenga.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/tipos-de-transporte/simulator.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
