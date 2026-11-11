> Una escena multijugador de Festive Trail con avatares personalizados, integración con Marketplace, una rueda de color impulsada por raycast y cámaras de seguimiento.

# Slay the Steps

![Vista previa de Slay the Steps](/docs/_remote/009ced18986123a766b6a4fb.webp)

Una experiencia de servidor multijugador creada por Studio para Decentraland Festive Trail 2025. Incluye avatares personalizados, integración con la Marketplace API y un selector de rueda de colores personalizado. Su código separa los módulos de client, server y compartidos, e incluye los assets fuente junto con la scene desplegable.

* [Juega en Decentraland](https://decentraland.org/jump?realm=slaythesteps.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · stom66

[Ver la fuente upstream](https://github.com/stom66/dcl-slay-the-steps)

**Licencia:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-slay-the-steps/blob/main/LICENSE.md). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Características clave

* Cinemático **cámaras virtuales**
* **AvatarShapes** para mostrar wearables
* **Multijugador** para sincronizar rondas y atuendos de los jugadores
* Selector de color 3D personalizado

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Convierte un impacto del Pointer 3D en una selección de color

`GetLocalMousePosition` lanza raycasts desde la cámara y convierte el impacto en la posición local de la rueda. `SampleColorWheel` mapea su ángulo y radio a tono y saturación, invoca un callback y tiñe el deslizador de valor.

**Contexto del proyecto:** La implementación utiliza los meshes de la rueda del proyecto, el helper de conversión HSV y los nombres de los nodos de material.

**Referencias de la fuente:**

* [`GetLocalMousePosition`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L306-L335)
* [`SampleColorWheel`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L203-L238)

### Sigue un objetivo en movimiento con una cámara virtual

El sistema de posición de la cámara y la cadena de Tween calculan periódicamente una posición relativa al jugador y al objetivo, y luego hacen tween hacia ella. `TrackEntity` crea la cámara con `lookAtEntity` y la activa.

**Contexto del proyecto:** Leer `ResetCamera` y la vinculación de eventos del juego para el final del ciclo de vida del seguimiento.

**Referencias de la fuente:**

* [sistema de posición de la cámara y cadena de Tween](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L46-L96)
* [`TrackEntity`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L99-L137)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/slay-the-steps.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
