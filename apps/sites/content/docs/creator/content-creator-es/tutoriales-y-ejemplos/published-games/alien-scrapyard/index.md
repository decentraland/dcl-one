> Rondas de construcción cooperativas con acertijos de construcción definidos por datos y cámaras cinemáticas impulsadas por fases.

# Alien Scrapyard

![Vista previa de Alien Scrapyard](/docs/_remote/c94532935e9e0629a121ed5f.webp)

Un juego cooperativo de construcción por tiempo en el que los jugadores trabajan juntos para ensamblar tecnología alienígena y desbloquear nuevas puntuaciones y formas.

* [Juega en Decentraland](https://decentraland.org/jump?realm=alienscrapyard.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · procesista

[Ver la fuente upstream](https://github.com/procesista/Alienscrap)

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

### Representar puzles de construcción como datos

Las definiciones de slots describen la parte requerida, la posición, la escala y la etiqueta de cada pieza. Los helpers de Template recentran y escalan los diseños, exponen una secuencia de puzles y buscan slots individuales. Esto es útil para estudiar cómo añadir nuevas construcciones mediante datos.

**Contexto del proyecto:** Léelo junto con las constantes compartidas de scene-center y part-type.

**Referencias de la fuente:**

* [Definiciones de slots](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L3-L39)
* [Helpers de Template](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L140-L184)

### Vincular una cámara cinemática al estado de la ronda

El sistema cinematográfico activa tomas para las fases de cuenta regresiva y de rendimiento, gestiona a los jugadores que se unen a mitad de ronda y libera la cámara cuando la instantánea está obsoleta. La actualización final añade salidas por cambio de ronda y por tiempo transcurrido antes de mover la cámara a lo largo de su toma.

**Contexto del proyecto:** El ejemplo depende de la instantánea del cliente, las constantes compartidas de tiempo y los helpers de visibilidad del HUD.

**Referencias de la fuente:**

* [El sistema cinematográfico](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L93-L129)
* [La actualización final](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L158-L176)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/alien-scrapyard.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
