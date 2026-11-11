> Bolos multijugador con física simulada por el servidor, progresión persistente, actualizaciones de estado por lotes y efectos de partículas escalados para móvil.

# Fastlane Bowling

![Vista previa de Fastlane Bowling](/docs/_remote/926145e7ae23468dd849d344.webp)

Un juego de bolos multijugador con físicas realistas proporcionadas por Cannon-ES. Varias partidas se ejecutan en pistas separadas en un espacio compartido.

* [Juega en Decentraland](https://decentraland.org/jump?realm=stom.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · stom66

[Ver la fuente upstream](https://github.com/stom66/dcl-bowling)

**Licencia:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-bowling/blob/main/LICENSE.md). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Características clave

* [**Cannon-ES**](https://github.com/pmndrs/cannon-es) **física** en el Multiplayer Server, con fotogramas clave comprimidos reproducidos por los clientes.
* **Almacenamiento** para la progresión del jugador, las estadísticas, las preferencias, el inventario y las tablas de clasificación semanales/de todos los tiempos.
* Dinámicas **luces** con máscaras de sombra giratorias y focos que siguen la bola.
* **Partículas** estelas y efectos de desbloqueo, con recuentos de partículas reducidos en móviles.
* **Cámaras virtuales** para la animación de entrada, la puntería y la reproducción del lanzamiento.
* **`GltfNodeModifiers`** para desplazar los UV de la textura de neón.
* Controlado por SDK **animaciones glTF** para los topes de pista, las cuentas regresivas y los efectos de puntuación.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Agrupar los cambios de estado sincronizados en eventos del cliente

En `LaneWatcher.onLaneChanged`, un conjunto recopila las pistas cambiadas y programa un único flush. `flush` lee la instantánea de cada pista y emite eventos de estado y de transición de fase. Sigue este flujo para entender cómo la escena conecta los componentes sincronizados con las reacciones del cliente.

**Contexto del proyecto:** Léelo junto con las `LaneStore` definiciones del bus de eventos.

**Referencias de la fuente:**

* [`LaneWatcher.onLaneChanged`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L79-L90)
* [`flush`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L132-L153)

### Escalar los efectos de partículas para móviles

`getParticleRatio` y `scaleConfig` ajustan las tasas de emisión, los límites de partículas y los recuentos de ráfagas usando una proporción móvil, preservando la configuración original. `spawn` aplica esa configuración a un nuevo emisor, con parentado opcional y limpieza temporizada.

**Contexto del proyecto:** Esto es útil al estudiar cómo una configuración de efecto puede servir para distintos dispositivos.

**Referencias de la fuente:**

* [`getParticleRatio` y `scaleConfig`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L176-L218)
* [`spawn`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L91-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/fastlane-bowling.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
