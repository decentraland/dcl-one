> Supervivencia zombi con progresión de armas, texto de recompensas reutilizado y guiado local de enemigos alrededor de otros zombis y obstáculos.

# Dead Surge

![Vista previa de Dead Surge](/docs/_remote/fbb6dfc582416446805d17d3.webp)

Juego de supervivencia zombi con combate y progresión de armas.

* [Juega en Decentraland](https://decentraland.org/jump?realm=deadsurge.dcl.eth)

## Información del proyecto

**Mantenido por:** Decentraland DAO

[Ver fuente](https://github.com/dcl-regenesislabs/dead-surge)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Reutilizar entities para el texto flotante de recompensa

`spawnZcRewardTextAtPosition` y su inicializador de pool recorren un conjunto fijo de text entities. El `rewardTextSystem` los hace elevarse, mirar hacia la cámara y desvanecerse antes de ocultarlos para reutilizarlos.

**Contexto del proyecto:** Lee el de la Scene `RewardTextComponent` y las actualizaciones en tiempo de juego con estas funciones para entender su seguimiento del ciclo de vida.

**Referencias de la fuente:**

* [`spawnZcRewardTextAtPosition` y su inicializador de pool](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L592-L640)
* [`rewardTextSystem`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L815-L850)

### Guía a los enemigos entre sí y alrededor de obstáculos simples

`getZombieSeparationDirection` combina la repulsión de los zombis cercanos. `resolveZombieMovePosition` prueba la dirección deseada, una alternativa y los ejes de movimiento individuales contra obstáculos circulares de ladrillo.

**Contexto del proyecto:** Esto ilustra una guía local; el comportamiento depende de las entradas de movimiento de la Scene, las posiciones de los obstáculos y las constantes de radio.

**Referencias de la fuente:**

* [`getZombieSeparationDirection`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L868-L896)
* [`resolveZombieMovePosition`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L898-L939)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/dead-surge.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
