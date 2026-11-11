> Un jardín cooperativo con estado persistente de las plantas, estimación del desfase del reloj del servidor y animaciones de pétalos reutilizadas.

# Bloom Garden

![Vista previa de Bloom Garden](/docs/_remote/3a9e2127ba7a6f3cdcd435b3.webp)

Una experiencia de jardín cooperativo donde los jugadores trabajan juntos para mantener un jardín vivo y despertar la Gran Floración.

Su Server Multiplayer usa Storage integrado para el estado de las plantas, una clasificación semanal y límites de riego por jugador.

* [Juega en Decentraland](https://decentraland.org/jump?realm=bloomgarden.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · TheLivingGarden

[Ver la fuente upstream](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main)

**Licencia:** [MIT](https://github.com/TheLivingGarden/TheLivingGarden/blob/main/LICENSE). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.


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

### Estimar el desfase entre los relojes del Server y del Client

El asistente de reloj conserva cinco muestras de desfase, rechaza desviaciones grandes y convierte las marcas de tiempo del Server a hora local. El Client de riego le pasa las marcas de tiempo recibidas a través de las definiciones de mensajes de la escena.

**Contexto del proyecto:** Esto ilustra una estimación de temporización ligera; el tiempo de tránsito de la red está incluido en las muestras.

**Referencias de la fuente:**

* [El asistente de reloj](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/shared/clockSync.ts#L3-L55)
* [El Client de riego](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/wateringSystem.ts#L1144-L1146)

### Dejar que los efectos temporales terminen de forma natural

La configuración y los controles de los pétalos crean un pool reutilizable a partir del modelo Petal con nombre de la escena, y luego inician o asientan el efecto. Las fases de actualización escalan los pétalos hacia dentro, los dejan caer y reposar, y los reducen antes de reciclarlos. Estudia esto cuando un efecto deba dejar de generar elementos mientras los objetos existentes completan su animación.

**Contexto del proyecto:** La colocación depende de BLOOM\_CENTER y de las constantes de temporización del módulo.

**Referencias de la fuente:**

* [La configuración y los controles de los pétalos](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L119-L173)
* [Las fases de actualización](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L207-L310)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/bloom-garden.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
