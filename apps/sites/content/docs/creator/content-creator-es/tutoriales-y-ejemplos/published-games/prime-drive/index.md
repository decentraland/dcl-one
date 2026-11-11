> Carreras de tres carriles con escenario reciclado, fantasmas de récord basados en la distancia y datos persistentes de la tabla de clasificación.

# Prime Drive

![Vista previa de Prime Drive](/docs/_remote/bba6ea4b5469647296b97b46.webp)

Un juego de carreras de bicicletas de tres carriles en el que los jugadores esquivan obstáculos, recogen monedas y persiguen al fantasma del récord del circuito en un recorrido de 10 km.

* [Juega en Decentraland](https://decentraland.org/jump?realm=primedrive.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · carlosmu

[Ver la fuente upstream](https://github.com/carlosmu/dcl-prime-drive)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Crea una carrera larga con scenery reciclado

`setWorldOffset` y `scrollTrack` mueve la raíz compartida del mundo lateralmente para los cambios de carril y vuelve a colocar las decoraciones de la carretera delante después de que pasen al jugador. La bicicleta permanece en su sitio mientras la escenografía aporta la sensación de movimiento.

**Contexto del proyecto:** Lee los constructores del anillo y los ajustes compartidos `TRACK`/`SPAWN` para entender el espaciado y las distancias de retorno.

**Referencias de la fuente:**

* [`setWorldOffset` y `scrollTrack`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/track.ts#L43-L92)

### Muestra fantasmas de récord en relación con el jugador

`updateGhosts` coloca las bicicletas fantasma según su diferencia de distancia, oculta los fantasmas fuera del rango de visión y los mueve suavemente a carriles alejados del jugador.

**Contexto del proyecto:** Este módulo consume `GhostRunView` datos; muestra el progreso proporcionado y no registra por sí mismo una repetición.

**Referencias de la fuente:**

* [`updateGhosts`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/ghost.ts#L129-L183)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/prime-drive.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
