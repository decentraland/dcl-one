# Fetch firmado

Cuando los clientes de protocolo quieren hacer solicitudes HTTPS autenticadas, pueden aprovechar el estándar *signed fetch* mecanismo.

Un signed fetch es una solicitud que incluye una [cadena de autenticación](/docs/contributor/contributor-es/autenticacion/authchain/)representada mediante encabezados. Los servidores con APIs compatibles con Decentraland pueden validar identidades antes, por ejemplo, [permitiendo solicitudes desde escenas](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/signed-fetch/) o creando [adaptadores](/docs/contributor/contributor-es/comunicaciones/overview/).

## Encabezados

La información que el servidor necesita para validar la cadena de autenticación se transmite en 3+ encabezados:

* `X-Identity-Timestamp`: el `timestamp` incluido en la carga útil firmada (véase abajo).
* `X-Identity-Metadata`: el `metadatos` incluido en la carga útil firmada (véase abajo).
* `X-Identity-AuthChain-<index>`: el [Paso de autenticación serializado en JSON](/docs/contributor/contributor-es/autenticacion/authchain/#constructing) `<index>`, comenzando desde `0`.

La cadena transmitida es validada por el servidor [como se especifica](/docs/contributor/contributor-es/autenticacion/authchain/).

## Cuerpo

El cuerpo de la solicitud no está especificado. Los servicios tienen total flexibilidad para usar los protocolos o formatos que prefieran.

## Carga útil

La cadena de autenticación [carga útil](/docs/contributor/contributor-es/autenticacion/authchain/#constructing) para un signed fetch es una **en minúsculas, separado por dos puntos** cadena que incluye algunos de los elementos de la solicitud:

```
<method>:<path>:<timestamp>:<metadata>
```

El `método` y `ruta` los campos deben coincidir con los de la solicitud, y `timestamp` es el mismo que en el `X-Identity-Timestamp` encabezado.

El último campo, `metadatos`, puede tener contenido arbitrario.

Por ejemplo:

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
GET https://docs.decentraland.org/contributor/contributor-es/autenticacion/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
