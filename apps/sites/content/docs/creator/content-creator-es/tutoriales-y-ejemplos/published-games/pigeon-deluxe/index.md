> Entrega de paquetes cronometrada con áreas de recogida y entrega, un puntero direccional montado en el avatar y puntuación separada de la animación de entrega.

# Pigeon Deluxe

![Vista previa de Pigeon Deluxe](/docs/_remote/5140367a547cfe389db77f7e.webp)

Un juego de entregas cronometrado en el que los jugadores recogen paquetes, los llevan a otra plataforma y ganan tiempo extra por las entregas.

* [Juega en Decentraland](https://decentraland.org/jump?realm=pigeondeluxe.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · PepeGawd

[Ver la fuente upstream](https://github.com/PepeGawd/Pigeon-Deluxe)

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

### Apunta hacia un objetivo mientras está adjunto a un avatar

`initParcelDirectionPointer` y su sistema de actualización adjuntan una flecha encima de la etiqueta de nombre del avatar, calculan la dirección en el espacio global hacia el objetivo activo y convierten esa rotación al espacio local del padre. La rotación avanza usando `dt`, por lo que la flecha gira hacia el objetivo con el tiempo.

**Contexto del proyecto:** El objetivo proviene del sistema de parcels y la flecha usa el `FaceDirection1.glb` recurso.

**Referencias de la fuente:**

* [`initParcelDirectionPointer` y su sistema de actualización](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelDirectionPointer.ts#L26-L96)

### Separar la puntuación de la animación de entrega

`startNewRound` y `handleSpawnAreaEnter` asignan roles de recogida y entrega a las plataformas, hacen seguimiento de si se está llevando un paquete y otorgan puntos y tiempo en cuanto se entra en la plataforma de entrega. La siguiente ronda comienza cuando termina la animación de soltarlo.

**Contexto del proyecto:** Léelo junto con el temporizador de juego de la escena y las funciones auxiliares de adjuntar parcels para ver cómo encajan el estado de la jugabilidad y la retroalimentación visual.

**Referencias de la fuente:**

* [`startNewRound` y `handleSpawnAreaEnter`](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelCollectDeliver.ts#L363-L433)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/pigeon-deluxe.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
