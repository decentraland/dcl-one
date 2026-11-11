> Mecánicas básicas de jugabilidad e interacciones del jugador.

# Jugabilidad e interacción

Mecánicas centrales de jugabilidad e interacciones del jugador.

| Ejemplo                                                                                                                                    | Qué demuestra                                                                           | Mantenido por | Etiquetas                               | Fuente                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- | ------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| [Modificador de entrada](/docs/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction/input-modifier/)                         | Cambiar o restringir la entrada de movimiento del jugador en parte de una scene.        | Decentraland  | `movimiento`, `entrada`, `jugador`      | [Abrir escena](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier)         |
| [Sistema de partículas](/docs/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction/particle-system/)                         | Fuego, humo, chispas, nieve y otros efectos de partículas.                              | Decentraland  | `partículas`, `efectos`, `visuales`     | [Abrir escena](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system)        |
| [Control de bloqueo del puntero](/docs/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction/pointer-lock-control/)           | Capturar la entrada del mouse para controles personalizados.                            | Decentraland  | `entrada`, `mouse`, `cámara`            | [Abrir escena](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control) |
| [Raycasting y capas de Collider](/docs/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction/raycasting-and-collider-layers/) | Apuntar a la geometría de la scene y controlar qué colliders reaccionan a un raycast.   | Decentraland  | `interacción`, `raycast`, `colisión`    | [Abrir escena](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests)   |
| [Áreas de trigger](/docs/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction/trigger-areas/)                                | Detectar jugadores que entran y salen de áreas, incluidos triggers basados en collider. | Decentraland  | `interacción`, `triggers`, `colisión`   | [Abrir escena](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas)        |
| [Zombie Attack](/docs/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction/zombie-attack/)                                   | Una scene de juego completa y jugable con enemigos y puntuación.                        | Decentraland  | `juego`, `scene completa`, `puntuación` | [Abrir escena](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/zombie-attack)                    |

[Explorar todas las escenas de ejemplo](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/gameplay-interaction.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
