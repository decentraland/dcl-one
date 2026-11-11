# Selección de Realm

Un [Catalyst](https://github.com/decentraland/catalyst) es un servidor que agrupa diferentes servicios requeridos por el World de Decentraland. Algunas de las principales responsabilidades del servidor son la gestión del almacenamiento descentralizado para la mayor parte del contenido que necesita el cliente y la orquestación de las comunicaciones entre pares.

Al entrar en un cliente de Decentraland, necesitas conectarte a un Catalyst específico para el soporte de los servicios de backend Content, Lambdas y Comms. Debe usarse uno de los Catalyst de la DAO; podría elegirse aleatoriamente, pero eso podría llevar a que el usuario acceda a un servicio de backend con gran latencia o a un realm con una baja cantidad de usuarios mientras está conectado a otro.

Para mitigar esto, se implementa en Kernel un algoritmo para la selección de realm como se describe en [ADR-86](https://rfc.decentraland.org/adr/ADR-86).

Es un algoritmo configurable que utiliza distintos criterios en una lista ordenada; si una de las condiciones es lo suficientemente fuerte como para tomar una decisión, entonces elige un realm; si no, delega la decisión al criterio siguiente. Si ninguna condición fue lo suficientemente fuerte, entonces se toma una decisión predeterminada (aleatoria).

La configuración predeterminada es:

1. Prioriza los nodos Catalyst con la mayor cantidad de usuarios
2. Prioriza los nodos Catalyst con más usuarios en la ubicación visitada

Otros criterios que se pueden usar son:

* Latencia alta: prioriza los Catalyst con la menor latencia
* Balanceo de carga: es un mecanismo Round Robin para distribuir a todos los pares entre todos los Catalyst.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/catalyst/realm-selection.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
