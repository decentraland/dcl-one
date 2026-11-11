> Adivinación social con transiciones explícitas del estado de conversación y un rig de cámara cinematográfica basado en pivote.

# The Fortune Teller

![Vista previa de Fortune Teller](/docs/_remote/2aaa323560ef094922b15d6a.webp)

Una experiencia de rol social donde los jugadores pueden recibir lecturas o hacer lecturas para amigos.

* [Juega en Decentraland](https://decentraland.org/jump?realm=fortunes.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · carlosmu

[Ver la fuente upstream](https://github.com/carlosmu/circus_fortune_teller)

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

### Representar una lectura como transiciones de estado explícitas

La máquina de estados de la fortuna enumera los pasos permitidos desde la selección de categoría y mazo hasta la revelación. `tryTransition` comprueba el siguiente paso, actualiza la sesión y notifica a los listeners de entrada de estado.

**Contexto del proyecto:** Léelo junto con los tipos de sesión y los controladores de acciones para ver cómo se organiza una interacción de varios pasos.

**Referencias de la fuente:**

* [máquina de estados de la fortuna](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/fortuneFsm/machine.ts#L6-L60)

### Construir un rig de cámara alrededor de un objeto de la escena

`ensureHostPivotRig` y `startHostCinematicCamera` asigna una cámara virtual a un pivot debajo de la mesa, elige ángulos de inicio y final desde el lado del jugador y activa la cámara.

**Contexto del proyecto:** El rig depende de la Entity de la mesa, de los valores de configuración y del sistema de actualización en este módulo; esas piezas explican cómo encuadrar una interacción en el contexto de su escena.

**Referencias de la fuente:**

* [`ensureHostPivotRig` y `startHostCinematicCamera`](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/cinematicCamera.ts#L141-L197)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/the-fortune-teller.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
