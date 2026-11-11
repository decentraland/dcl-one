> Un juego social de agricultura con etapas cronometradas de crecimiento de cultivos, modificadores de fertilizante y granjas repetidas creadas a partir de una sola jerarquía de Entity.

# Cozy Farm

![Vista previa de Cozy Farm](/docs/_remote/f005f5c4d6dce02d9535e094.webp)

Un juego social de agricultura centrado en cultivar cosechas, expandir una granja y conocer a otros jugadores.

* [Juega en Decentraland](https://decentraland.org/jump?realm=cozyfarm.dcl.eth)

## Información del proyecto

**Mantenido por:** Decentraland DAO

[Ver fuente](https://github.com/dcl-regenesislabs/cozy-farm)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Convertir el tiempo transcurrido en etapas de crecimiento de los cultivos

`growthSystem` deriva el progreso de una marca de tiempo de siembra, ajusta la duración para los cultivos del tutorial y el fertilizante, y cambia los modelos solo cuando una etapa cambia.

**Contexto del proyecto:** Síguelo junto con `PlotState`, `CROP_DATA`, y las funciones auxiliares de modelos en `game/actions` para ver cómo el estado de simulación impulsa la retroalimentación visual.

**Referencias de la fuente:**

* [`growthSystem`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/growthSystem.ts#L62-L142)

### Construye múltiples granjas a partir de una sola jerarquía de Entity

`cloneEntityTree` recrea recursivamente los children de una template, preserva los Transform locales, copia los Component de renderizado seleccionados y registra cada clon por nombre. Esto muestra cómo las áreas repetidas pueden compartir una disposición creada previamente.

**Contexto del proyecto:** El helper espera el mapa de children de la Scene y copia una lista explícita de Component, por lo que los nuevos tipos de Component necesitan un manejo deliberado.

**Referencias de la fuente:**

* [`cloneEntityTree`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/farmInstances.ts#L127-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/cozy-farm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
