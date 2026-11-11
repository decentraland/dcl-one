> Plataformeo cronometrado a través de secciones cambiantes de la torre, con precarga de assets y un recorrido de cámara omisible por el nivel.

# Tower of Madness

![Vista previa de Tower of Madness](/docs/_remote/ec24d5b235cc62f71cd1eae3.webp)

Un juego de plataformas cronometrado en el que los jugadores compiten para llegar a la cima.

* [Juega en Decentraland](https://decentraland.org/jump?realm=towerofmadness.dcl.eth)

## Información del proyecto

**Mantenido por:** Decentraland DAO

[Ver fuente](https://github.com/dcl-regenesislabs/towerofmadness)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Precarga las piezas de un nivel cambiante

`setupChunkPreload` deduplica las rutas de los assets de los chunks y crea entities GLTF temporales. Un system vigila sus estados de carga, elimina las entities después de que la carga se complete o falle, y se desregistra cuando no queda ninguna.

**Contexto del proyecto:** Esto ilustra la gestión de un ciclo de vida de precarga; la lista de assets proviene de `shared/chunks` y es específica de la torre.

**Referencias de la fuente:**

* [`setupChunkPreload`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/chunkPreload.ts#L22-L61)

### Crea un recorrido de cámara que se pueda omitir alrededor de geometría cambiante

`playCinematic` ajusta los keyframes de la cámara alrededor de los chunks de inicio y fin actuales, luego selecciona una VirtualCamera. El update system interpola la posición y los objetivos de look-at y admite mantener pulsado un botón para omitir.

**Contexto del proyecto:** Sigue las funciones auxiliares de búsqueda de chunks y `skipCinematic` para ver cómo el recorrido encuentra sus anclas y devuelve el control a la cámara del jugador.

**Referencias de la fuente:**

* [`playCinematic`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L110-L209)
* [system de actualización](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L286-L334)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/tower-of-madness.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
