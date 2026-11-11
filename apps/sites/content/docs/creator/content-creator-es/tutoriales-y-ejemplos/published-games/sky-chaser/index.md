> Rondas multijugador de atrapar paquetes con tablas de clasificación persistentes, proyectiles reutilizados y colocación de objetos de recogida sembrada a partir del estado de la ronda.

# Sky Chaser

![Vista previa de Sky Chaser](/docs/_remote/3033913f0028e180a375c5bf.webp)

Un juego multijugador en el que los jugadores compiten por atrapar paquetes que escapan en cada ronda. La escena incluye tablas de clasificación semanales y de todos los tiempos.

* [Juega en Decentraland](https://decentraland.org/jump?realm=skychaser.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · stom66

[Ver la fuente upstream](https://github.com/stom66/dcl-sky-chaser)

**Licencia:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-sky-chaser/blob/main/LICENSE.md). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Características clave

* Servidor multijugador
* Jugabilidad sincronizada mediante un RNG Mulberry32 sincronizado
* API de almacenamiento para tablas de clasificación
* Ajustes de locomoción del jugador y fuerzas físicas para impulsores/trampolines/explosiones
* Sistemas de partículas (estelas de impulso, explosiones, recogibles)

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Reutilizar entidades de proyectiles durante la partida

`fireProjectile` y `getIdleProjectile` reutiliza proyectiles inactivos, crea nuevos cuando se agota el pool y actualiza solo los proyectiles activos.

**Contexto del proyecto:** Sigue el `Projectile` clase y los manejadores de eventos para el movimiento, las respuestas a colisiones y la limpieza al final de la ronda.

**Referencias de la fuente:**

* [`fireProjectile` y `getIdleProjectile`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/projectileManager.ts#L76-L124)

### Inicializar la colocación de pickups a partir del estado de la ronda

`onGameStart` y `getRandomPosition` usa la hora de inicio de la ronda para inicializar el generador aleatorio del proyecto y luego tomar muestras de las distancias y alturas de los pickups.

**Contexto del proyecto:** Lee la configuración de pickups y el almacén de componentes junto con ella: los límites móviles y la actividad posterior de los pickups afectan la secuencia de llamadas aleatorias, por lo que una semilla compartida por sí sola no garantiza disposiciones idénticas en cada client.

**Referencias de la fuente:**

* [`onGameStart` y `getRandomPosition`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/spawners/pickupSpawner.ts#L115-L164)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/sky-chaser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
