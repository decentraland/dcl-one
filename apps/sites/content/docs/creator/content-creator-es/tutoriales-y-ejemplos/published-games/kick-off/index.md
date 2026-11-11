> Una escena de predicciones de la Copa Mundial con tiros de balón cargados, retroalimentación de movimiento local inmediata y paneles de calendario responsivos.

# Kick Off

![Vista previa de Kick Off](/docs/_remote/e381e39b01875ac25f3f33f9.webp)

Un juego de predicción de la Copa del Mundo de temporada.

* [Juega en Decentraland](https://decentraland.org/jump?realm=kickoff.dcl.eth)

## Información del proyecto

**Mantenido por:** Decentraland DAO

[Ver fuente](https://github.com/dcl-regenesislabs/kickoff-2026)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Conecta un tiro cargado con una respuesta visual inmediata

El bucle de carga y liberación combina la entrada del teclado y del móvil, agranda y recolorea una barra de carga, envía un mensaje de tiro e inicia el movimiento local de la pelota. `releaseOptimistic` inicializa ese movimiento.

**Contexto del proyecto:** Estúdialo con los `prodeNet` manejadores de mensajes de la room y la pelota para entender la relación entre la entrada, la respuesta y los mensajes multijugador.

**Referencias de la fuente:**

* [bucle de carga y liberación](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L491-L536)
* [`releaseOptimistic`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L155-L178)

### Escala la UI a partir de un canvas de referencia

`layoutScale` lee `UiCanvasInformation`, compara ambas dimensiones con un espacio de diseño de 1920×1080, limita el resultado y aplica un factor móvil adicional.

**Contexto del proyecto:** Es un ejemplo compacto de un helper de diseño compartido; los límites y el factor móvil están ajustados para los paneles de este proyecto.

**Referencias de la fuente:**

* [`layoutScale`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/schedule/responsive.ts#L1-L21)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/kick-off.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
