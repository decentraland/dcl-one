> Tandas de penaltis multijugador con partidos ejecutados por el servidor, tablas de clasificación persistentes, cámaras impulsadas por fases y animaciones de balón sincronizadas.

# Goal Legends

![Vista previa de Goal Legends](/docs/_remote/aa680097357467a542aed322.webp)

Un juego multijugador de tanda de penaltis. Enfréntate a otro jugador o al GL-Bot: elige izquierda, centro o derecha para marcar o atajar. Cada ronda termina con una repetición al estilo de una transmisión de TV, con la multitud reaccionando en vivo.

* [Juega en Decentraland](https://decentraland.org/jump?realm=goallegends.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · carlosmu

[Ver la fuente upstream](https://github.com/carlosmu/goal-legends-arena)

**Licencia:** [Aviso Apache-2.0](https://github.com/carlosmu/goal-legends-arena/blob/master/LICENCE). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Características clave

* **Servidor multijugador** ejecuta los partidos
* **Almacenamiento** para las tablas de clasificación PvP/PvE diarias/de todos los tiempos, y la bandera del país de cada jugador.
* **Escena** **emotes**
* props de pelota sincronizados con Animator para tiros, zambullidas, goles y atajadas.
* **Cámaras virtuales** para las vistas de gameplay y una cinemática de ganador que todos ven.
* **AvatarShape** NPCs de la multitud que reaccionan a cada gol y atajada.
* **Partículas** para los fuegos artificiales
* Un portal con luces y **`GltfNodeModifiers`** que te lleva a otro World.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Cambia las cámaras con las fases del partido

`initGameplayCamera` selecciona las cámaras de gameplay, de ganador o estándar a partir del client match snapshot. La cámara de ganador orbita un pivote, mientras que los espectadores pueden elegir una cámara libre durante el juego.

**Contexto del proyecto:** Sigue esto junto con `clientSnapshot`, el `GameState` enum, y el pivote de cámara de la escena con nombre.

**Referencias de la fuente:**

* [`initGameplayCamera`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/gameplayCamera.ts#L109-L165)

### Coordina una animación del prop con el estado del juego

`tickAnimationManager` inicia un clip de pelota cuando cambia la fase, elige animaciones de gol/atajada según la dirección del tiro, y oculta brevemente el prop mientras cambia de clip. Esta ruta es específica del pateador bot; los pateadores humanos llevan la pelota en sus emotes.

**Contexto del proyecto:** Depende del modelo de pelota de la escena y de los clips de animación con nombre.

**Referencias de la fuente:**

* [`tickAnimationManager`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/animationManager.ts#L78-L114)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/goal-legends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
