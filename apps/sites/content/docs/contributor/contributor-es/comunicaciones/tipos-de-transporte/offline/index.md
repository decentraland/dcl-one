# Sin conexión

El transporte Offline es una implementación ficticia para señalar que no hay ningún servidor de comunicaciones disponible en un realm. Se usa comúnmente en entornos de pruebas y desarrollo.

El transporte nunca emite mensajes entrantes e ignora todos los mensajes salientes. Si quieres una variante offline con comportamiento simulado, consulta [Simulator](/docs/contributor/contributor-es/comunicaciones/tipos-de-transporte/simulator/) en su lugar.

Los URI tienen este aspecto:

```
offline:<sufijo personalizado>
```

Lo que viene después de `offline:` el prefijo no está especificado y puede ser usado por las implementaciones (o dejarse vacío) como les convenga.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/tipos-de-transporte/offline.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
